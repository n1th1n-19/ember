// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings page. Changes are written to cosmic-config; the applet and the desktop
//! widget watch it and update live. Also manages the widget's process and login
//! autostart, and the applet's place in the top panel.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cosmic::Element;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, column, row};
use ember_ui::config::{Corner, EmberConfig};
use ember_ui::ink::{CYAN, MAGENTA, RED, YELLOW};
use ember_ui::mini::Key;

use crate::Message;
use ember_ui::widgets::{banner, body, caption, card, comic_button, heading, muted, page};

use ember_ui::setup::{self, APPLETS};

#[derive(Debug, Clone)]
pub enum Msg {
    Dark(bool),
    Motion(bool),
    Corner(Corner),
    X(i32),
    Y(i32),
    SaveOffsets,
    OnTop(bool),
    WidgetKey(Key),
    AppletKey(Key),
    StartWidget,
    StopWidget,
    Autostart(bool),
    /// one applet in/out of the top panel (by name)
    PanelApplet(&'static str, bool),
    /// every Ember applet in, or all out (COSMIC's back)
    PanelAll(bool),
}

pub struct Settings {
    pub cfg: EmberConfig,
    status: Option<(String, bool)>,
}

impl Settings {
    pub fn load() -> Self {
        Self { cfg: EmberConfig::load(), status: None }
    }
}

// ---------- files & processes ----------

/// Installed binary next to this one (dev build or ~/.local/bin), else /usr/bin.
fn bin(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(name))).filter(|p| p.exists()).unwrap_or_else(|| Path::new("/usr/bin").join(name))
}

/// PIDs of our own running ember-widget processes.
fn widget_pids() -> Vec<i32> {
    let me = unsafe { libc::getuid() };
    let Ok(rd) = fs::read_dir("/proc") else { return vec![] };
    rd.flatten()
        .filter_map(|e| {
            let pid: i32 = e.file_name().to_str()?.parse().ok()?;
            let comm = fs::read_to_string(e.path().join("comm")).ok()?;
            let uid = std::os::unix::fs::MetadataExt::uid(&e.metadata().ok()?);
            (comm.trim() == "ember-widget" && uid == me).then_some(pid)
        })
        .collect()
}

pub fn widget_running() -> bool {
    !widget_pids().is_empty()
}

fn start_widget() -> Result<(), String> {
    if widget_running() {
        return Ok(());
    }
    Command::new(bin("ember-widget"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("couldn't start ember-widget: {e}"))
}

pub fn stop_widget() {
    for pid in widget_pids() {
        // SAFETY: plain syscall on our own process
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

// ---------- update & view ----------

impl Settings {
    pub fn update(&mut self, msg: Msg) -> Task<Message> {
        let mut save = true;
        match msg {
            Msg::Dark(d) => {
                self.cfg.dark = d;
                ember_ui::ink::set_dark(d);
            }
            Msg::Motion(m) => self.cfg.motion = m,
            Msg::Corner(c) => self.cfg.widget_corner = c,
            // sliders update locally while dragging, save on release
            Msg::X(x) => {
                self.cfg.widget_x = x;
                save = false;
            }
            Msg::Y(y) => {
                self.cfg.widget_y = y;
                save = false;
            }
            Msg::SaveOffsets => {}
            Msg::OnTop(t) => self.cfg.widget_on_top = t,
            Msg::WidgetKey(k) => EmberConfig::toggle(&mut self.cfg.widget_keys, k),
            Msg::AppletKey(k) => EmberConfig::toggle(&mut self.cfg.applet_keys, k),
            Msg::StartWidget => {
                save = false;
                self.status = Some(match start_widget() {
                    Ok(()) => ("Widget started. It sits under your windows: show the desktop to see it.".into(), false),
                    Err(e) => (e, true),
                });
            }
            Msg::StopWidget => {
                save = false;
                stop_widget();
                self.status = Some(("Widget stopped.".into(), false));
            }
            Msg::Autostart(on) => {
                save = false;
                self.status = Some(match setup::set_autostart(on, &bin("ember-widget")) {
                    Ok(()) => (if on { "Widget will start when you log in." } else { "Widget won't start at login." }.into(), false),
                    Err(e) => (format!("Couldn't change autostart: {e}"), true),
                });
            }
            Msg::PanelApplet(name, on) => {
                save = false;
                let ap = setup::by_name(name).into_iter().collect::<Vec<_>>();
                self.status = Some(match setup::set_panel(&ap, on) {
                    Ok(()) => (format!("{} {} the top panel.", name, if on { "added to" } else { "removed from" }), false),
                    Err(e) => (e, true),
                });
            }
            Msg::PanelAll(on) => {
                save = false;
                self.status = Some(match setup::set_panel(&APPLETS, on) {
                    Ok(()) => (if on { "All Ember applets are in the panel." } else { "COSMIC's own applets are back." }.into(), false),
                    Err(e) => (e, true),
                });
            }
        }
        if save {
            self.cfg.save();
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let c = &self.cfg;
        let msg = |m: Msg| Message::Settings(m);
        let pick = |label: &str, m: Msg, accent, on: bool| comic_button(label, (!on).then(|| msg(m)), accent, on);

        let look = column::with_capacity(4)
            .spacing(12)
            .push(heading("Look"))
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body("Theme"))
                    .push(pick("Dark", Msg::Dark(true), YELLOW, c.dark))
                    .push(pick("Light", Msg::Dark(false), YELLOW, !c.dark)),
            )
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body("Animation"))
                    .push(pick("On", Msg::Motion(true), YELLOW, c.motion))
                    .push(pick("Off", Msg::Motion(false), YELLOW, !c.motion)),
            )
            .push(muted("Off freezes every loop; the art still shows the current state."));

        let running = widget_running();
        let autostart = setup::autostart_enabled();
        let mut corners = row::with_capacity(5).spacing(10).align_y(Alignment::Center).push(body("Corner"));
        for k in Corner::ALL {
            corners = corners.push(pick(k.name(), Msg::Corner(k), CYAN, c.widget_corner == k));
        }
        let mut wkeys = row::with_capacity(8).spacing(8).align_y(Alignment::Center).push(body("Shows"));
        for k in Key::ALL {
            wkeys = wkeys.push(comic_button(k.short(), Some(msg(Msg::WidgetKey(k))), MAGENTA, c.widget().contains(&k)));
        }
        let side = if c.widget_corner.left() { "left" } else { "right" };
        let edge = if c.widget_corner.top() { "top" } else { "bottom" };
        let widget_col = column::with_capacity(8)
            .spacing(12)
            .push(heading("Desktop widget"))
            .push(muted("A readings card on your desktop, under your windows. Changes apply live."))
            .push(
                row::with_capacity(4)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body(if running { "Running" } else { "Not running" }))
                    .push(if running {
                        comic_button("Stop", Some(msg(Msg::StopWidget)), RED, true)
                    } else {
                        comic_button("Start", Some(msg(Msg::StartWidget)), YELLOW, true)
                    })
                    .push(body("Start at login"))
                    .push(pick("On", Msg::Autostart(true), YELLOW, autostart))
                    .push(pick("Off", Msg::Autostart(false), YELLOW, !autostart)),
            )
            .push(corners)
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(widget::container(body(format!("From {side} edge: {} px", c.widget_x))).width(Length::Fixed(210.0)))
                    .push(widget::slider(0..=1600, c.widget_x, |v| Message::Settings(Msg::X(v))).on_release(msg(Msg::SaveOffsets)).width(Length::Fixed(360.0))),
            )
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(widget::container(body(format!("From {edge} edge: {} px", c.widget_y))).width(Length::Fixed(210.0)))
                    .push(widget::slider(0..=1000, c.widget_y, |v| Message::Settings(Msg::Y(v))).on_release(msg(Msg::SaveOffsets)).width(Length::Fixed(360.0))),
            )
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body("Float above windows"))
                    .push(pick("No", Msg::OnTop(false), CYAN, !c.widget_on_top))
                    .push(pick("Yes", Msg::OnTop(true), CYAN, c.widget_on_top)),
            )
            .push(wkeys);

        let present = setup::in_panel(&APPLETS);
        let mut chips = row::with_capacity(APPLETS.len()).spacing(8);
        for ap in APPLETS {
            let on = present.contains(&ap);
            chips = chips.push(comic_button(ap.name, Some(msg(Msg::PanelApplet(ap.name, !on))), YELLOW, on));
        }
        let mut akeys = row::with_capacity(8).spacing(8).align_y(Alignment::Center).push(body("Readings shows"));
        for k in Key::ALL {
            akeys = akeys.push(comic_button(k.short(), Some(msg(Msg::AppletKey(k))), MAGENTA, c.applet().contains(&k)));
        }
        let applet_col = column::with_capacity(6)
            .spacing(12)
            .push(heading("Panel applets"))
            .push(muted("Highlighted ones are in your top panel; click to add or remove. Ember's clock, sound, Bluetooth, network, power, tiling and session take the place of COSMIC's; removing one puts COSMIC's back."))
            .push(chips.wrap())
            .push(
                row::with_capacity(2)
                    .spacing(12)
                    .push(comic_button("All Ember's", Some(msg(Msg::PanelAll(true))), CYAN, false))
                    .push(comic_button("All COSMIC's", Some(msg(Msg::PanelAll(false))), CYAN, false)),
            )
            .push(akeys);

        let mut col = column::with_capacity(5).spacing(22).push(caption("Settings"));
        if let Some((t, e)) = &self.status {
            col = col.push(banner(t, *e));
        }
        page(col.push(card(look)).push(card(widget_col)).push(card(applet_col)))
    }
}
