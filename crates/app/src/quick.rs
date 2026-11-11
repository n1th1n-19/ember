// SPDX-License-Identifier: GPL-3.0-or-later
//! Quick panel: Ember's take on the COSMIC panel applets in one page: clock and
//! calendar, sound, Bluetooth, network, power and brightness, tiling, Do Not Disturb
//! and the session menu.
use cosmic::app::Task;
use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, column, row};
use cosmic::Element;
use ember_core::calendar::{self, MONTHS, WEEKDAYS};
use ember_core::control::bluetooth::{self, BtState};
use ember_core::control::desktop::{self, Brightness, SessionAction};
use ember_core::control::network::{self, NetState};
use ember_core::control::power::{self, PowerState, Profile};
use ember_core::control::sound::{self, Device, Kind};
use ember_core::control::CtlResult;
use ember_ui::ink::{self, CYAN, MAGENTA, RED, YELLOW};

use crate::widgets::{app_icon, banner, body, caption, card, comic_button, heading, muted, page};
use crate::Message;

#[derive(Debug, Clone)]
pub enum Msg {
    Refresh,
    Sound(Kind, CtlResult<Vec<Device>>),
    Net(CtlResult<NetState>),
    Bt(CtlResult<BtState>),
    Bright(Brightness),
    Power(Option<PowerState>),
    // sound
    Volume(Kind, f32),
    ApplyVolume(Kind),
    Mute(Kind, bool),
    DefaultDevice(Kind, String),
    // bluetooth
    BtPower(bool),
    BtConnect(String, bool),
    // network
    Wifi(bool),
    Airplane(bool),
    Join(String, bool, bool),
    Copy(String),
    // power
    Profile(Profile),
    Display(f32),
    Keyboard(f32),
    ApplyDisplay,
    ApplyKeyboard,
    // desktop
    Dnd(bool),
    Autotile(bool),
    Month(i32),
    Session(SessionAction),
    Confirm(SessionAction),
    Cancel,
    Done(String, CtlResult),
}

#[derive(Default)]
pub struct Quick {
    outputs: Vec<Device>,
    inputs: Vec<Device>,
    out_vol: Option<f32>,
    in_vol: Option<f32>,
    net: Option<NetState>,
    bt: Option<BtState>,
    bright: Brightness,
    power: Option<PowerState>,
    month: Option<(i32, u32)>,
    confirm: Option<SessionAction>,
    busy: bool,
    status: Option<(String, bool)>,
}

fn wrap(m: Msg) -> cosmic::Action<Message> {
    cosmic::Action::App(Message::Quick(m))
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) -> Msg + Send + 'static) -> Task<Message> {
    cosmic::task::future(async move {
        match tokio::task::spawn_blocking(f).await {
            Ok(v) => wrap(done(v)),
            Err(e) => wrap(Msg::Done("Task".into(), Err(e.to_string()))),
        }
    })
}

fn act(what: impl Into<String>, f: impl std::future::Future<Output = CtlResult> + Send + 'static) -> Task<Message> {
    let what = what.into();
    cosmic::task::future(async move { wrap(Msg::Done(what, f.await)) })
}

fn act_blocking(what: impl Into<String>, f: impl FnOnce() -> CtlResult + Send + 'static) -> Task<Message> {
    let what = what.into();
    blocking(f, move |r| Msg::Done(what, r))
}

// COSMIC's own settings, read/written like its applets do
fn cosmic_bool(id: &str, key: &str) -> Option<bool> {
    Config::new(id, 1).ok()?.get::<bool>(key).ok()
}
fn set_cosmic_bool(id: &str, key: &str, v: bool) -> CtlResult {
    Config::new(id, 1).map_err(|e| e.to_string())?.set(key, v).map_err(|e| e.to_string())
}
const NOTIFY: &str = "com.system76.CosmicNotifications";
const COMP: &str = "com.system76.CosmicComp";

impl Quick {
    /// Re-read everything (called on open and every few seconds while open).
    pub fn refresh(&self) -> Task<Message> {
        Task::batch([
            blocking(|| sound::list(Kind::Output), |r| Msg::Sound(Kind::Output, r)),
            blocking(|| sound::list(Kind::Input), |r| Msg::Sound(Kind::Input, r)),
            blocking(network::read, Msg::Net),
            cosmic::task::future(async { wrap(Msg::Bt(bluetooth::read().await)) }),
            cosmic::task::future(async { wrap(Msg::Bright(desktop::read_brightness().await)) }),
            cosmic::task::future(async { wrap(Msg::Power(power::read().await)) }),
        ])
    }

    fn default_dev(&self, k: Kind) -> Option<&Device> {
        let list = if k == Kind::Output { &self.outputs } else { &self.inputs };
        list.iter().find(|d| d.default).or(list.first())
    }

    pub fn update(&mut self, msg: Msg) -> Task<Message> {
        match msg {
            Msg::Refresh => return self.refresh(),
            Msg::Sound(k, r) => {
                let list = r.unwrap_or_default();
                if k == Kind::Output { self.outputs = list } else { self.inputs = list }
            }
            Msg::Net(r) => self.net = r.ok(),
            Msg::Bt(r) => self.bt = r.ok(),
            Msg::Bright(b) => self.bright = b,
            Msg::Power(p) => self.power = p,
            Msg::Volume(k, v) => {
                if k == Kind::Output { self.out_vol = Some(v) } else { self.in_vol = Some(v) }
            }
            Msg::ApplyVolume(k) => {
                let v = if k == Kind::Output { self.out_vol.take() } else { self.in_vol.take() };
                if let (Some(v), Some(d)) = (v, self.default_dev(k)) {
                    let name = d.name.clone();
                    return act_blocking("Volume", move || sound::set_volume(k, &name, v));
                }
            }
            Msg::Mute(k, m) => {
                if let Some(d) = self.default_dev(k) {
                    let name = d.name.clone();
                    return act_blocking(if m { "Muted" } else { "Unmuted" }, move || sound::set_mute(k, &name, m));
                }
            }
            Msg::DefaultDevice(k, name) => return act_blocking("Switched device", move || sound::set_default(k, &name)),
            Msg::BtPower(on) => {
                if let Some(a) = self.bt.as_ref().and_then(|b| b.adapter.clone()) {
                    return act(format!("Bluetooth {}", if on { "on" } else { "off" }), async move { bluetooth::set_powered(&a, on).await });
                }
            }
            Msg::BtConnect(path, on) => {
                self.busy = true;
                return act(if on { "Connected" } else { "Disconnected" }, async move { bluetooth::set_connected(&path, on).await });
            }
            Msg::Wifi(on) => return act_blocking(format!("Wi-Fi {}", if on { "on" } else { "off" }), move || network::set_wifi(on)),
            Msg::Airplane(on) => return act_blocking(format!("Airplane mode {}", if on { "on" } else { "off" }), move || network::set_airplane(on)),
            Msg::Join(ssid, known, secure) => {
                if !known && secure {
                    self.status = Some((format!("\"{ssid}\" needs a password. Copy the command and run it in a terminal; it will ask for it."), false));
                    return Task::none();
                }
                self.busy = true;
                let what = format!("Joined {ssid}");
                return act_blocking(what, move || if known { network::connect_known(&ssid) } else { network::connect_open(&ssid) });
            }
            Msg::Copy(cmd) => {
                self.status = Some(("Command copied.".into(), false));
                return cosmic::iced::clipboard::write(cmd);
            }
            Msg::Profile(p) => return act(format!("Power profile: {}", p.name()), async move { power::set_profile(p).await }),
            Msg::Display(v) => self.bright.display = Some(v),
            Msg::Keyboard(v) => self.bright.keyboard = Some(v),
            Msg::ApplyDisplay => {
                if let Some(v) = self.bright.display {
                    return act("Screen brightness", async move { desktop::set_display(v).await });
                }
            }
            Msg::ApplyKeyboard => {
                if let Some(v) = self.bright.keyboard {
                    return act("Keyboard light", async move { desktop::set_keyboard(v).await });
                }
            }
            Msg::Dnd(on) => return act_blocking(format!("Do Not Disturb {}", if on { "on" } else { "off" }), move || set_cosmic_bool(NOTIFY, "do_not_disturb", on)),
            Msg::Autotile(on) => return act_blocking(format!("New workspaces {}", if on { "tiled" } else { "floating" }), move || set_cosmic_bool(COMP, "autotile", on)),
            Msg::Month(by) => {
                let n = calendar::now();
                let (y, m) = self.month.unwrap_or((n.year, n.month));
                self.month = if by == 0 { None } else { Some(calendar::shift_month(y, m, by)) };
            }
            Msg::Session(a) => {
                if a.needs_confirm() {
                    self.confirm = Some(a);
                    return Task::none();
                }
                return act(a.name(), async move { desktop::session(a).await });
            }
            Msg::Confirm(a) => {
                self.confirm = None;
                return act(a.name(), async move { desktop::session(a).await });
            }
            Msg::Cancel => self.confirm = None,
            Msg::Done(what, r) => {
                self.busy = false;
                self.status = Some(match r {
                    Ok(()) => (format!("Done: {what}"), false),
                    Err(e) => (format!("{what} failed: {e}"), true),
                });
                return self.refresh();
            }
        }
        Task::none()
    }

    // ---------- view ----------

    fn clock(&self) -> Element<'_, Message> {
        let n = calendar::now();
        let (y, m) = self.month.unwrap_or((n.year, n.month));
        let (h12, ampm) = (if n.hour.is_multiple_of(12) { 12 } else { n.hour % 12 }, if n.hour < 12 { "AM" } else { "PM" });
        let big = widget::text(format!("{h12}:{:02} {ampm}", n.minute)).font(ink::DISPLAY).size(52).class(cosmic::theme::Text::Color(ink::pal().text));
        let date = body(format!("{}, {} {}, {}", WEEKDAYS[n.weekday as usize], MONTHS[n.month as usize - 1], n.day, n.year));

        let cell = |s: String, today: bool, dim: bool| -> Element<'_, Message> {
            let t = widget::text(s).font(if today { ink::DISPLAY } else { ink::BODY }).size(if today { 18 } else { 14 });
            let c = widget::container(t).width(Length::Fixed(38.0)).height(Length::Fixed(32.0)).align_x(Alignment::Center).align_y(Alignment::Center);
            let c = if today {
                c.class(cosmic::theme::Container::custom(|_| widget::container::Style {
                    text_color: Some(ink::INK),
                    background: Some(cosmic::iced::Background::Color(YELLOW)),
                    border: cosmic::iced::Border { color: ink::INK, width: 2.0, radius: 0.0.into() },
                    ..Default::default()
                }))
            } else if dim {
                c.class(cosmic::theme::Container::custom(|_| widget::container::Style { text_color: Some(cosmic::iced::Color { a: 0.35, ..ink::pal().text }), ..Default::default() }))
            } else {
                c
            };
            c.into()
        };
        let mut grid = column::with_capacity(7).spacing(2);
        let mut head = row::with_capacity(7).spacing(2);
        for d in WEEKDAYS {
            head = head.push(cell(d[..3].to_string(), false, true));
        }
        grid = grid.push(head);
        for week in calendar::month_grid(y, m) {
            if week.iter().all(Option::is_none) {
                continue;
            }
            let mut r = row::with_capacity(7).spacing(2);
            for d in week {
                let today = d == Some(n.day) && (y, m) == (n.year, n.month);
                r = r.push(cell(d.map_or(String::new(), |d| d.to_string()), today, false));
            }
            grid = grid.push(r);
        }
        let nav = row::with_capacity(4)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(widget::container(heading(format!("{} {y}", MONTHS[m as usize - 1]))).width(Length::Fill))
            .push(comic_button("<", Some(Message::Quick(Msg::Month(-1))), CYAN, false))
            .push(comic_button("Today", self.month.map(|_| Message::Quick(Msg::Month(0))), CYAN, false))
            .push(comic_button(">", Some(Message::Quick(Msg::Month(1))), CYAN, false));
        card(column::with_capacity(4).spacing(10).push(big).push(date).push(nav).push(grid))
    }

    fn sound(&self) -> Element<'_, Message> {
        let mut col = column::with_capacity(8).spacing(10).push(heading("Sound"));
        for (k, label, live) in [(Kind::Output, "Volume", self.out_vol), (Kind::Input, "Microphone", self.in_vol)] {
            let Some(d) = self.default_dev(k) else {
                col = col.push(muted(format!("No {} device.", label.to_lowercase())));
                continue;
            };
            let v = live.unwrap_or(d.volume);
            let icon = match (k, d.muted) {
                (Kind::Output, true) => "audio-volume-muted-symbolic",
                (Kind::Output, false) => "audio-volume-high-symbolic",
                (Kind::Input, true) => "microphone-disabled-symbolic",
                (Kind::Input, false) => "audio-input-microphone-symbolic",
            };
            col = col.push(
                row::with_capacity(5)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(app_icon(Some(icon), 20))
                    .push(widget::container(body(label)).width(Length::Fixed(90.0)))
                    .push(widget::slider(0.0..=1.5, v, move |x| Message::Quick(Msg::Volume(k, x))).step(0.01).on_release(Message::Quick(Msg::ApplyVolume(k))).width(Length::Fill))
                    .push(widget::container(body(format!("{:.0}", v * 100.0))).width(Length::Fixed(40.0)))
                    .push(comic_button(if d.muted { "Unmute" } else { "Mute" }, Some(Message::Quick(Msg::Mute(k, !d.muted))), MAGENTA, d.muted)),
            );
            let list = if k == Kind::Output { &self.outputs } else { &self.inputs };
            if list.len() > 1 {
                let mut r = row::with_capacity(list.len()).spacing(8);
                for dev in list {
                    r = r.push(comic_button(&dev.description, (!dev.default).then(|| Message::Quick(Msg::DefaultDevice(k, dev.name.clone()))), CYAN, dev.default));
                }
                col = col.push(r.wrap());
            }
        }
        card(col)
    }

    fn bluetooth(&self) -> Element<'_, Message> {
        let mut col = column::with_capacity(8).spacing(10);
        let Some(bt) = &self.bt else { return card(col.push(heading("Bluetooth")).push(muted("Reading Bluetooth…"))) };
        if bt.adapter.is_none() {
            return card(col.push(heading("Bluetooth")).push(muted("No Bluetooth adapter.")));
        }
        col = col.push(
            row::with_capacity(3)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(widget::container(heading("Bluetooth")).width(Length::Fill))
                .push(comic_button("On", (!bt.powered).then_some(Message::Quick(Msg::BtPower(true))), YELLOW, bt.powered))
                .push(comic_button("Off", bt.powered.then_some(Message::Quick(Msg::BtPower(false))), YELLOW, !bt.powered)),
        );
        if bt.powered {
            for d in &bt.devices {
                let status = match (d.connected, d.battery) {
                    (true, Some(b)) => format!("Connected · {b}%"),
                    (true, None) => "Connected".into(),
                    (false, _) => String::new(),
                };
                col = col.push(
                    row::with_capacity(4)
                        .spacing(10)
                        .align_y(Alignment::Center)
                        .push(app_icon(Some(&format!("{}-symbolic", d.icon.as_deref().unwrap_or("bluetooth"))), 20))
                        .push(widget::container(body(d.name.clone())).width(Length::Fill))
                        .push(muted(status))
                        .push(comic_button(if d.connected { "Disconnect" } else { "Connect" }, (!self.busy).then(|| Message::Quick(Msg::BtConnect(d.path.clone(), !d.connected))), CYAN, d.connected)),
                );
            }
            if bt.devices.is_empty() {
                col = col.push(muted("No paired devices. Pair new ones in COSMIC Settings > Bluetooth."));
            }
        }
        card(col)
    }

    fn network(&self) -> Element<'_, Message> {
        let mut col = column::with_capacity(10).spacing(10).push(heading("Network"));
        let Some(n) = &self.net else { return card(col.push(muted("Reading network…"))) };
        for l in &n.links {
            let icon = if l.kind == "wifi" { "network-wireless-symbolic" } else { "network-wired-symbolic" };
            let detail = [l.ipv4.clone().map(|ip| format!("IPv4 {ip}")), l.speed.map(|s| format!("{s} Mbps"))].into_iter().flatten().collect::<Vec<_>>().join("  ·  ");
            col = col.push(
                row::with_capacity(3)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(app_icon(Some(icon), 20))
                    .push(column::with_capacity(2).push(body(format!("{} · connected", l.connection))).push(muted(detail))),
            );
        }
        if n.links.is_empty() {
            col = col.push(muted("Not connected."));
        }
        let toggle = |label: &str, on: bool, m: fn(bool) -> Msg| {
            row::with_capacity(3)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(widget::container(body(label.to_string())).width(Length::Fill))
                .push(comic_button("On", (!on).then(|| Message::Quick(m(true))), YELLOW, on))
                .push(comic_button("Off", on.then(|| Message::Quick(m(false))), YELLOW, !on))
        };
        col = col.push(toggle("Airplane mode", n.airplane, Msg::Airplane)).push(toggle("Wi-Fi", n.wifi_enabled, Msg::Wifi));
        if n.wifi_enabled && !n.airplane {
            for w in n.networks.iter().take(8) {
                let bars = match w.signal {
                    s if s >= 75 => "network-wireless-signal-excellent-symbolic",
                    s if s >= 50 => "network-wireless-signal-good-symbolic",
                    s if s >= 25 => "network-wireless-signal-ok-symbolic",
                    _ => "network-wireless-signal-weak-symbolic",
                };
                let mut r = row::with_capacity(5)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(app_icon(Some(bars), 18))
                    .push(widget::container(body(w.ssid.clone())).width(Length::Fill))
                    .push(muted(format!("{}%{}", w.signal, if w.secure { " · locked" } else { "" })));
                if w.in_use {
                    r = r.push(muted("connected"));
                } else if !w.known && w.secure {
                    r = r.push(comic_button("Copy join command", Some(Message::Quick(Msg::Copy(network::join_command(&w.ssid)))), YELLOW, false));
                } else {
                    r = r.push(comic_button("Join", (!self.busy).then(|| Message::Quick(Msg::Join(w.ssid.clone(), w.known, w.secure))), CYAN, false));
                }
                col = col.push(r);
            }
        }
        card(col)
    }

    fn power(&self) -> Element<'_, Message> {
        let mut col = column::with_capacity(6).spacing(10).push(heading("Power & battery"));
        if let Some(p) = &self.power {
            let mut r = row::with_capacity(3).spacing(10);
            for pr in Profile::ALL {
                r = r.push(comic_button(pr.name(), (p.profile != Some(pr)).then_some(Message::Quick(Msg::Profile(pr))), YELLOW, p.profile == Some(pr)));
            }
            col = col.push(r);
        }
        let slider = |icon: &'static str, label: &'static str, v: f32, on: fn(f32) -> Msg, done: Msg| {
            row::with_capacity(4)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(app_icon(Some(icon), 20))
                .push(widget::container(body(label)).width(Length::Fixed(90.0)))
                .push(widget::slider(0.0..=1.0, v, move |x| Message::Quick(on(x))).step(0.01).on_release(Message::Quick(done)).width(Length::Fill))
                .push(widget::container(body(format!("{:.0}%", v * 100.0))).width(Length::Fixed(48.0)))
        };
        if let Some(v) = self.bright.display {
            col = col.push(slider("display-brightness-symbolic", "Screen", v, Msg::Display, Msg::ApplyDisplay));
        }
        if let Some(v) = self.bright.keyboard {
            col = col.push(slider("keyboard-brightness-symbolic", "Keyboard", v, Msg::Keyboard, Msg::ApplyKeyboard));
        }
        card(col)
    }

    fn desktop(&self) -> Element<'_, Message> {
        let dnd = cosmic_bool(NOTIFY, "do_not_disturb").unwrap_or(false);
        let tile = cosmic_bool(COMP, "autotile").unwrap_or(false);
        let pair = |label: &str, a: &str, b: &str, on: bool, m: fn(bool) -> Msg| {
            row::with_capacity(3)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(widget::container(body(label.to_string())).width(Length::Fill))
                .push(comic_button(a, (!on).then(|| Message::Quick(m(true))), CYAN, on))
                .push(comic_button(b, on.then(|| Message::Quick(m(false))), CYAN, !on))
        };
        card(
            column::with_capacity(4)
                .spacing(10)
                .push(heading("Desktop"))
                .push(pair("Do Not Disturb", "On", "Off", dnd, Msg::Dnd))
                .push(pair("New workspaces", "Tiled", "Floating", tile, Msg::Autotile))
                .push(muted("Tile or float the current workspace with Super + Y.")),
        )
    }

    fn session(&self) -> Element<'_, Message> {
        let mut r = row::with_capacity(5).spacing(10);
        for (a, accent) in [(SessionAction::Lock, CYAN), (SessionAction::LogOut, YELLOW), (SessionAction::Suspend, CYAN), (SessionAction::Restart, YELLOW), (SessionAction::ShutDown, RED)] {
            r = r.push(comic_button(a.name(), Some(Message::Quick(Msg::Session(a))), accent, false));
        }
        let mut col = column::with_capacity(3).spacing(10).push(heading("Session")).push(r.wrap());
        if let Some(a) = self.confirm {
            col = col.push(
                row::with_capacity(3)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(body(format!("{}? Unsaved work in other apps may be lost.", a.name())))
                    .push(comic_button(&format!("Yes, {}", a.name().to_lowercase()), Some(Message::Quick(Msg::Confirm(a))), RED, true))
                    .push(comic_button("Cancel", Some(Message::Quick(Msg::Cancel)), CYAN, false)),
            );
        }
        card(col)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut col = column::with_capacity(4).spacing(22).push(caption("Quick Panel"));
        if let Some((t, e)) = &self.status {
            col = col.push(banner(t, *e));
        }
        let left = column::with_capacity(3).spacing(22).width(Length::FillPortion(1)).push(self.clock()).push(self.desktop()).push(self.session());
        let right = column::with_capacity(4).spacing(22).width(Length::FillPortion(1)).push(self.sound()).push(self.bluetooth()).push(self.network()).push(self.power());
        page(col.push(row::with_capacity(2).spacing(22).push(left).push(right)))
    }
}
