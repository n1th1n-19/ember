// SPDX-License-Identifier: GPL-3.0-or-later
//! Processes, Services and Control pages: real widgets, async actions, status banner.
use cosmic::Element;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, column, row};
use ember_core::control::firmware::{self, FwAttr};
use ember_core::control::power::{self, PowerState, Profile};
use ember_core::control::procs::{self, ProcRow, Procs, Signal};
use ember_core::control::services::{self, Action, Scope, Unit};
use ember_core::control::{CtlResult, cpu};
use ember_core::fans::Fan;
use ember_ui::ink::{CYAN, MAGENTA, RED, YELLOW, bytes};

use crate::Message;
use ember_ui::widgets::{app_icon, banner, body, caption, card, comic_button, heading, muted, page, row_button};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Cpu,
    Memory,
    Name,
}

#[derive(Debug, Clone)]
pub enum Msg {
    // processes
    ProcSearch(String),
    ProcSort(Sort),
    ProcSelect(u32),
    ProcSignal(u32, Signal),
    ProcRenice(u32, i32),
    // services
    SvcScope(Scope),
    SvcSearch(String),
    SvcLoaded(Scope, CtlResult<Vec<Unit>>),
    SvcSelect(String),
    SvcAct(Scope, String, Action),
    // control
    PowerLoaded(Option<PowerState>),
    SetProfile(Profile),
    SetGraphics(String),
    SetCharge(u8, u8),
    SetGovernor(String),
    SetTurbo(bool),
    MaxPerf(u8),
    ApplyMaxPerf,
    SetPwm(String, u8),
    SetThermal(String),
    FwEdit(usize, u32),
    FwApply(usize),
    // shared
    Done(String, CtlResult),
}

pub struct Controls {
    procs: Procs,
    /// program name → launcher icon, loaded on first visit to Processes
    icons: Option<std::collections::HashMap<String, String>>,
    rows: Vec<ProcRow>,
    proc_search: String,
    sort: Sort,
    selected_pid: Option<u32>,
    scope: Scope,
    units: Vec<Unit>,
    units_loading: bool,
    svc_search: String,
    selected_unit: Option<String>,
    power: Option<PowerState>,
    power_loaded: bool,
    cpu: cpu::CpuCtl,
    max_perf: u8,
    graphics_pending: Option<String>,
    /// firmware power/thermal limits, plus the slider value being edited for each
    fw: Vec<(FwAttr, u32)>,
    status: Option<(String, bool)>,
}

impl Default for Controls {
    fn default() -> Self {
        let cpu = cpu::read_state();
        Self {
            procs: Procs::new(),
            icons: None,
            rows: vec![],
            proc_search: String::new(),
            sort: Sort::Cpu,
            selected_pid: None,
            scope: Scope::User,
            units: vec![],
            units_loading: false,
            svc_search: String::new(),
            selected_unit: None,
            power: None,
            power_loaded: false,
            max_perf: cpu.max_perf.unwrap_or(100),
            cpu,
            graphics_pending: None,
            fw: firmware::refresh(&[]),
            status: None,
        }
    }
}

fn wrap(m: Msg) -> cosmic::Action<Message> {
    cosmic::Action::App(Message::Ctl(m))
}

/// Run a blocking control (may show a polkit prompt) off the UI thread.
fn blocking(what: impl Into<String>, f: impl FnOnce() -> CtlResult + Send + 'static) -> Task<Message> {
    let what = what.into();
    cosmic::task::future(async move {
        let r = tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| Err(e.to_string()));
        wrap(Msg::Done(what, r))
    })
}

fn helper(what: impl Into<String>, args: Vec<String>) -> Task<Message> {
    blocking(what, move || ember_core::control::helper::run(&args))
}

fn load_units(scope: Scope) -> Task<Message> {
    cosmic::task::future(async move { wrap(Msg::SvcLoaded(scope, services::list(scope).await)) })
}

fn load_power() -> Task<Message> {
    cosmic::task::future(async { wrap(Msg::PowerLoaded(power::read().await)) })
}

impl Controls {
    /// Called once per second by the app while one of these pages is open.
    pub fn tick(&mut self, page: crate::Page) -> Task<Message> {
        match page {
            crate::Page::Processes => {
                if self.icons.is_none() {
                    self.icons = Some(ember_core::apps::launcher_icons());
                }
                self.rows = self.procs.list();
                Task::none()
            }
            crate::Page::Services if self.units.is_empty() && !self.units_loading => {
                self.units_loading = true;
                load_units(self.scope)
            }
            crate::Page::Control if !self.power_loaded => {
                self.power_loaded = true;
                load_power()
            }
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, msg: Msg) -> Task<Message> {
        match msg {
            Msg::ProcSearch(s) => self.proc_search = s,
            Msg::ProcSort(s) => self.sort = s,
            Msg::ProcSelect(pid) => self.selected_pid = (self.selected_pid != Some(pid)).then_some(pid),
            Msg::ProcSignal(pid, sig) => return blocking(format!("{} sent to {pid}", sig.name()), move || procs::signal(pid, sig)),
            Msg::ProcRenice(pid, n) => return blocking(format!("{pid} reniced to {n}"), move || procs::renice(pid, n)),
            Msg::SvcScope(s) => {
                self.scope = s;
                self.units.clear();
                self.selected_unit = None;
                self.units_loading = true;
                return load_units(s);
            }
            Msg::SvcSearch(s) => self.svc_search = s,
            Msg::SvcLoaded(scope, r) => {
                self.units_loading = false;
                if scope == self.scope {
                    match r {
                        Ok(u) => self.units = u,
                        Err(e) => self.status = Some((format!("Couldn't list services: {e}"), true)),
                    }
                }
            }
            Msg::SvcSelect(n) => self.selected_unit = (self.selected_unit.as_ref() != Some(&n)).then_some(n),
            Msg::SvcAct(scope, name, action) => {
                let what = format!("{name}: {action:?}").to_lowercase();
                return cosmic::task::future(async move {
                    let r = services::act(scope, &name, action).await;
                    wrap(Msg::Done(what, r))
                });
            }
            Msg::PowerLoaded(p) => self.power = p,
            Msg::SetProfile(p) => {
                return cosmic::task::future(async move { wrap(Msg::Done(format!("Power profile: {}", p.name()), power::set_profile(p).await)) });
            }
            Msg::SetGraphics(m) => {
                self.graphics_pending = Some(m.clone());
                return cosmic::task::future(async move {
                    let r = power::set_graphics(&m).await;
                    wrap(Msg::Done(format!("Graphics set to {m}; reboot to apply"), r))
                });
            }
            Msg::SetCharge(a, b) => {
                return cosmic::task::future(async move { wrap(Msg::Done(format!("Charge limit {a}–{b}%"), power::set_thresholds(a, b).await)) });
            }
            Msg::SetGovernor(g) => return helper(format!("Governor: {g}"), vec!["governor".into(), g]),
            Msg::SetTurbo(on) => return helper(format!("Turbo {}", if on { "on" } else { "off" }), vec!["turbo".into(), if on { "on" } else { "off" }.into()]),
            Msg::MaxPerf(v) => self.max_perf = v,
            Msg::ApplyMaxPerf => return helper(format!("Max performance {}%", self.max_perf), vec!["max-perf".into(), self.max_perf.to_string()]),
            Msg::SetPwm(path, v) => {
                // path looks like /sys/class/hwmon/hwmonN/pwmK
                let parts: Vec<&str> = path.rsplit('/').take(2).collect();
                if let [pwm, chip] = parts[..] {
                    let idx = pwm.trim_start_matches("pwm").to_string();
                    return helper(format!("Fan {chip}/{pwm} set to {v}"), vec!["pwm".into(), chip.into(), idx, v.to_string()]);
                }
            }
            Msg::SetThermal(t) => return helper(format!("Fan mode: {}", cpu::thermal_label(&t)), vec!["thermal".into(), t]),
            Msg::FwEdit(i, v) => {
                if let Some(e) = self.fw.get_mut(i) {
                    e.1 = v;
                }
            }
            Msg::FwApply(i) => {
                if let Some((a, v)) = self.fw.get(i) {
                    return helper(format!("{} set to {v} {}", a.label, a.unit()), vec!["fwattr".into(), a.device.clone(), a.name.clone(), v.to_string()]);
                }
            }
            Msg::Done(what, r) => {
                self.status = Some(match r {
                    Ok(()) => (format!("Done: {what}"), false),
                    Err(e) => (format!("{what} failed: {e}"), true),
                });
                // re-read state after any change
                self.cpu = cpu::read_state();
                self.fw = firmware::refresh(&self.fw);
                self.max_perf = self.cpu.max_perf.unwrap_or(self.max_perf);
                self.rows = self.procs.list();
                let mut tasks = vec![load_power()];
                if !self.units.is_empty() {
                    tasks.push(load_units(self.scope));
                }
                return Task::batch(tasks);
            }
        }
        Task::none()
    }

    fn status_line(&self) -> Option<Element<'_, Message>> {
        self.status.as_ref().map(|(t, e)| banner(t, *e))
    }

    // ---------- processes ----------

    pub fn view_processes(&self) -> Element<'_, Message> {
        let q = self.proc_search.to_lowercase();
        let mut rows: Vec<&ProcRow> = self
            .rows
            .iter()
            .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q) || r.cmd.to_lowercase().contains(&q) || r.pid.to_string() == q)
            .collect();
        match self.sort {
            Sort::Cpu => rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            Sort::Memory => rows.sort_by_key(|r| std::cmp::Reverse(r.mem)),
            Sort::Name => rows.sort_by_key(|a| a.name.to_lowercase()),
        }
        let total = rows.len();
        let sort_btn = |label, s| comic_button(label, Some(Message::Ctl(Msg::ProcSort(s))), CYAN, self.sort == s);
        let tools = row::with_capacity(8)
            .spacing(12)
            .align_y(Alignment::Center)
            .push(
                widget::text_input::search_input("Search name, command or PID", &self.proc_search)
                    .on_input(|s| Message::Ctl(Msg::ProcSearch(s)))
                    .width(Length::Fixed(320.0)),
            )
            .push(sort_btn("CPU", Sort::Cpu))
            .push(sort_btn("Memory", Sort::Memory))
            .push(sort_btn("Name", Sort::Name))
            .push(muted(format!("{total} processes")));

        let header = row::with_capacity(8)
            .spacing(10)
            .push(widget::Space::new().width(Length::Fixed(20.0)))
            .push(widget::container(heading("Name")).width(Length::FillPortion(4)))
            .push(widget::container(heading("PID")).width(Length::FillPortion(1)))
            .push(widget::container(heading("User")).width(Length::FillPortion(2)))
            .push(widget::container(heading("CPU")).width(Length::FillPortion(1)))
            .push(widget::container(heading("Mem")).width(Length::FillPortion(1)))
            .push(widget::container(heading("Nice")).width(Length::FillPortion(1)));
        let mut list = column::with_capacity(8).spacing(2).push(header);
        // ponytail: first 150 rows only; search narrows the rest
        for r in rows.iter().take(150) {
            let selected = self.selected_pid == Some(r.pid);
            let icon = self.icons.as_ref().and_then(|m| m.get(&r.name.to_lowercase())).map(String::as_str);
            let line = row::with_capacity(8)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(app_icon(icon, 20))
                .push(widget::container(body(r.name.clone())).width(Length::FillPortion(4)))
                .push(widget::container(body(r.pid.to_string())).width(Length::FillPortion(1)))
                .push(widget::container(body(r.user.clone())).width(Length::FillPortion(2)))
                .push(widget::container(body(format!("{:.1}%", r.cpu))).width(Length::FillPortion(1)))
                .push(widget::container(body(bytes(r.mem as f64))).width(Length::FillPortion(1)))
                .push(widget::container(body(r.nice.map_or("--".into(), |n| n.to_string()))).width(Length::FillPortion(1)));
            list = list.push(row_button(line, Message::Ctl(Msg::ProcSelect(r.pid)), selected));
            if selected {
                list = list.push(self.proc_actions(r));
            }
        }

        let mut col = column::with_capacity(8).spacing(18).push(caption("Process Room"));
        if let Some(s) = self.status_line() {
            col = col.push(s);
        }
        page(col.push(card(column::with_capacity(8).spacing(14).push(tools).push(list))))
    }

    fn proc_actions<'a>(&self, r: &ProcRow) -> Element<'a, Message> {
        let pid = r.pid;
        let nice = r.nice.unwrap_or(0);
        let sig = |label, s, accent| comic_button(label, Some(Message::Ctl(Msg::ProcSignal(pid, s))), accent, true);
        let actions = row::with_capacity(8)
            .spacing(10)
            .align_y(Alignment::Center)
            .push(sig("End", Signal::Term, YELLOW))
            .push(sig("Kill", Signal::Kill, RED))
            .push(sig("Pause", Signal::Stop, CYAN))
            .push(sig("Resume", Signal::Cont, CYAN))
            .push(muted("Priority"))
            .push(comic_button("Lower", (nice < 19).then(|| Message::Ctl(Msg::ProcRenice(pid, (nice + 5).min(19)))), MAGENTA, false))
            .push(comic_button("Raise", (nice > -20).then(|| Message::Ctl(Msg::ProcRenice(pid, (nice - 5).max(-20)))), MAGENTA, false));
        let cmd = if r.cmd.is_empty() { r.name.clone() } else { r.cmd.chars().take(160).collect() };
        widget::container(column::with_capacity(8).spacing(8).push(muted(format!("{cmd}  ·  {}", r.status))).push(actions)).padding([8, 12, 14, 12]).into()
    }

    // ---------- services ----------

    pub fn view_services(&self) -> Element<'_, Message> {
        let q = self.svc_search.to_lowercase();
        let units: Vec<&Unit> =
            self.units.iter().filter(|u| q.is_empty() || u.name.to_lowercase().contains(&q) || u.description.to_lowercase().contains(&q)).collect();
        let scope_btn = |label, s| comic_button(label, Some(Message::Ctl(Msg::SvcScope(s))), CYAN, self.scope == s);
        let tools = row::with_capacity(8)
            .spacing(12)
            .align_y(Alignment::Center)
            .push(scope_btn("My services", Scope::User))
            .push(scope_btn("System", Scope::System))
            .push(
                widget::text_input::search_input("Search services", &self.svc_search).on_input(|s| Message::Ctl(Msg::SvcSearch(s))).width(Length::Fixed(320.0)),
            )
            .push(muted(if self.units_loading { "Loading…".to_string() } else { format!("{} services", units.len()) }));

        let mut list = column::with_capacity(8).spacing(2);
        for u in units {
            let selected = self.selected_unit.as_ref() == Some(&u.name);
            let running = u.active == "active";
            let line = row::with_capacity(8)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(widget::container(body(services::unescape(u.name.trim_end_matches(".service")))).width(Length::FillPortion(3)).clip(true))
                .push(widget::container(muted(u.description.clone())).width(Length::FillPortion(4)))
                .push(widget::container(body(format!("{} ({})", u.active, u.sub))).width(Length::FillPortion(2)))
                .push(widget::container(body(u.file_state.clone().unwrap_or_else(|| "--".into()))).width(Length::FillPortion(1)));
            list = list.push(row_button(line, Message::Ctl(Msg::SvcSelect(u.name.clone())), selected));
            if selected {
                let act = |label, a, accent, on: bool| comic_button(label, Some(Message::Ctl(Msg::SvcAct(u.scope, u.name.clone(), a))), accent, on);
                let enabled = u.file_state.as_deref() == Some("enabled");
                let can_toggle = matches!(u.file_state.as_deref(), Some("enabled") | Some("disabled"));
                let mut actions = row::with_capacity(8).spacing(10).align_y(Alignment::Center);
                actions = if running {
                    actions.push(act("Stop", Action::Stop, RED, true)).push(act("Restart", Action::Restart, YELLOW, true))
                } else {
                    actions.push(act("Start", Action::Start, YELLOW, true))
                };
                if can_toggle {
                    actions = if enabled {
                        actions.push(act("Disable at boot", Action::Disable, MAGENTA, false))
                    } else {
                        actions.push(act("Enable at boot", Action::Enable, CYAN, false))
                    };
                }
                if u.scope == Scope::System {
                    actions = actions.push(muted("System changes ask for your password."));
                }
                list = list.push(widget::container(actions).padding([6, 12, 14, 12]));
            }
        }

        let mut col = column::with_capacity(8).spacing(18).push(caption("Service Desk"));
        if let Some(s) = self.status_line() {
            col = col.push(s);
        }
        page(col.push(card(column::with_capacity(8).spacing(14).push(tools).push(list))))
    }

    // ---------- control ----------

    pub fn view_control<'a>(&'a self, fans: &'a [Fan]) -> Element<'a, Message> {
        let mut col = column::with_capacity(8).spacing(22).push(caption("Control Room"));
        if let Some(s) = self.status_line() {
            col = col.push(s);
        }

        // power profile + graphics + charge (system76-power)
        match &self.power {
            None if !self.power_loaded => col = col.push(card(muted("Reading power settings…"))),
            None => {
                col = col.push(card(
                    column::with_capacity(8)
                        .spacing(8)
                        .push(heading("Power"))
                        .push(muted("system76-power isn't running, so power profile and graphics switching aren't available.")),
                ))
            }
            Some(p) => {
                let mut prof = row::with_capacity(8).spacing(12);
                for pr in Profile::ALL {
                    prof = prof.push(comic_button(pr.name(), Some(Message::Ctl(Msg::SetProfile(pr))), YELLOW, p.profile == Some(pr)));
                }
                col = col.push(card(
                    column::with_capacity(8)
                        .spacing(12)
                        .push(heading("Power profile"))
                        .push(muted("Battery saves power, Performance unlocks full clocks and fans."))
                        .push(prof),
                ));

                if p.switchable {
                    let mut g = row::with_capacity(8).spacing(12);
                    for m in power::GRAPHICS {
                        let current = p.graphics.as_deref() == Some(m);
                        g = g.push(comic_button(m, (!current).then(|| Message::Ctl(Msg::SetGraphics(m.to_string()))), CYAN, current));
                    }
                    let note = match &self.graphics_pending {
                        Some(m) => format!("Switching to {m} after the next reboot."),
                        None => "Integrated saves battery (NVIDIA off). Hybrid runs apps on Intel and lets you pick NVIDIA per app. NVIDIA uses the dGPU for everything. Needs a reboot.".into(),
                    };
                    col = col.push(card(column::with_capacity(8).spacing(12).push(heading("Graphics mode")).push(muted(note)).push(g)));
                }

                let mut charge = column::with_capacity(8).spacing(12).push(heading("Battery charge limit"));
                match p.thresholds {
                    Some((a, b)) => {
                        let mut r = row::with_capacity(8).spacing(12);
                        for cp in &p.charge_profiles {
                            r = r.push(comic_button(&cp.title, Some(Message::Ctl(Msg::SetCharge(cp.start, cp.end))), MAGENTA, (a, b) == (cp.start, cp.end)));
                        }
                        charge = charge.push(muted(format!("Charging now stops at {b}% and restarts below {a}%."))).push(r);
                    }
                    None => charge = charge.push(muted("This laptop's firmware doesn't let the OS limit charging.")),
                }
                col = col.push(card(charge));
            }
        }

        // CPU scaling (helper)
        let c = &self.cpu;
        let mut cpu_col = column::with_capacity(8)
            .spacing(12)
            .push(heading("CPU"))
            .push(muted(format!("Driver {} · changes ask for your password", c.driver.clone().unwrap_or_else(|| "--".into()))));
        if !c.governors.is_empty() {
            let mut g = row::with_capacity(8).spacing(12).align_y(Alignment::Center).push(body("Governor"));
            for gov in &c.governors {
                let current = c.governor.as_ref() == Some(gov);
                g = g.push(comic_button(gov, (!current).then(|| Message::Ctl(Msg::SetGovernor(gov.clone()))), YELLOW, current));
            }
            cpu_col = cpu_col.push(g);
        }
        if let Some(off) = c.no_turbo {
            cpu_col = cpu_col.push(
                row::with_capacity(8)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body("Turbo boost"))
                    .push(comic_button("On", off.then_some(Message::Ctl(Msg::SetTurbo(true))), YELLOW, !off))
                    .push(comic_button("Off", (!off).then_some(Message::Ctl(Msg::SetTurbo(false))), CYAN, off)),
            );
        }
        if let Some(cur) = c.max_perf {
            cpu_col = cpu_col.push(
                row::with_capacity(8)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body(format!("Max performance {}%", self.max_perf)))
                    .push(widget::slider(10..=100, self.max_perf, |v| Message::Ctl(Msg::MaxPerf(v))).width(Length::Fixed(260.0)))
                    .push(comic_button("Apply", (self.max_perf != cur).then_some(Message::Ctl(Msg::ApplyMaxPerf)), YELLOW, true)),
            );
        }
        col = col.push(card(cpu_col));

        // fans (only writable pwm nodes)
        let mut fan_col = column::with_capacity(8).spacing(12).push(heading("Fans"));
        if !c.thermals.is_empty() {
            let mut r = row::with_capacity(8).spacing(12).align_y(Alignment::Center).push(body("Fan mode"));
            for t in &c.thermals {
                let current = c.thermal.as_ref() == Some(t);
                r = r.push(comic_button(cpu::thermal_label(t), (!current).then(|| Message::Ctl(Msg::SetThermal(t.clone()))), CYAN, current));
            }
            fan_col =
                fan_col.push(muted("Firmware thermal mode: picks the fan curve and power limits. Changing the power profile above may reset it.")).push(r);
        }
        if !self.fw.is_empty() {
            col = col.push(self.custom_card());
        }
        let writable: Vec<&Fan> = fans.iter().filter(|f| f.pwm.is_some()).collect();
        if writable.is_empty() && c.thermals.is_empty() {
            fan_col = fan_col.push(muted("No controllable fans: the firmware keeps fan speed to itself."));
        } else {
            for f in writable {
                let path = f.pwm.clone().unwrap_or_default();
                let mut r = row::with_capacity(8).spacing(12).align_y(Alignment::Center).push(body(format!("{} {}", f.chip, f.label)));
                for (label, v) in [("Quiet", 80u8), ("Normal", 160), ("Max", 255)] {
                    r = r.push(comic_button(label, Some(Message::Ctl(Msg::SetPwm(path.clone(), v))), CYAN, false));
                }
                fan_col = fan_col.push(r);
            }
        }
        page(col.push(card(fan_col)))
    }

    /// Custom-mode power and thermal limits (firmware-attributes).
    fn custom_card(&self) -> Element<'_, Message> {
        let custom = self.cpu.thermal.as_deref() == Some("custom");
        let has_custom = self.cpu.thermals.iter().any(|t| t == "custom");
        let note = if custom || !has_custom {
            "Power and temperature limits the firmware uses in Custom mode. Higher limits mean more heat and louder fans."
        } else {
            "Set Fan mode to Custom to change these. Higher limits mean more heat and louder fans."
        };
        let enabled = custom || !has_custom;
        let mut c = column::with_capacity(16).spacing(12).push(heading("Custom mode")).push(muted(note));
        for (i, (a, v)) in self.fw.iter().enumerate() {
            let u = a.unit();
            let reset = a.default.filter(|d| d != v).map(|d| Message::Ctl(Msg::FwEdit(i, d)));
            c = c.push(
                row::with_capacity(8)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(widget::container(body(a.label.clone())).width(Length::Fixed(300.0)))
                    .push(widget::slider(a.min..=a.max, *v, move |n| Message::Ctl(Msg::FwEdit(i, n))).step(a.step).width(Length::Fixed(240.0)))
                    .push(widget::container(body(format!("{v} {u}"))).width(Length::Fixed(70.0)))
                    .push(comic_button("Apply", (enabled && *v != a.value).then_some(Message::Ctl(Msg::FwApply(i))), YELLOW, true))
                    .push(comic_button("Default", reset, CYAN, false)),
            );
        }
        card(c)
    }
}
