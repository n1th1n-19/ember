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

pub const APPLET_ID: &str = "io.github.n1th1n_19.Ember.Applet";
const WIDGET_DESKTOP: &str = "io.github.n1th1n_19.Ember.Widget.desktop";

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
    Panel(bool),
    EmberApplets(bool),
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

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(".config"))
}

/// Installed binary next to this one (dev build or ~/.local/bin), else /usr/bin.
fn bin(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(name))).filter(|p| p.exists()).unwrap_or_else(|| Path::new("/usr/bin").join(name))
}

fn autostart_file() -> PathBuf {
    config_home().join("autostart").join(WIDGET_DESKTOP)
}

pub fn autostart_enabled() -> bool {
    autostart_file().exists()
}

fn set_autostart(on: bool) -> Result<(), String> {
    let f = autostart_file();
    if !on {
        return match fs::remove_file(&f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    fs::create_dir_all(f.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    let entry = format!(
        "[Desktop Entry]\nName=Ember desktop widget\nComment=Live system readings pinned to the desktop\nType=Application\nExec={}\nIcon=io.github.n1th1n_19.Ember\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        bin("ember-widget").display()
    );
    fs::write(&f, entry).map_err(|e| e.to_string())
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

fn stop_widget() {
    for pid in widget_pids() {
        // SAFETY: plain syscall on our own process
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

// ---------- top panel ----------

fn panel_wings() -> PathBuf {
    config_home().join("cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings")
}

pub fn in_panel() -> bool {
    fs::read_to_string(panel_wings()).is_ok_and(|s| s.contains(&format!("\"{APPLET_ID}\"")))
}

/// Add/remove the applet at the start of the panel's right wing. Edits only that one
/// entry (a backup of the original is kept in ~/.local/state/ember/ the first time).
pub fn set_in_panel(on: bool) -> Result<(), String> {
    let path = panel_wings();
    let text = fs::read_to_string(&path).map_err(|_| "COSMIC top panel config not found.".to_string())?;
    let new = edit_wings(&text, on).ok_or("Unexpected panel config layout; add the applet in COSMIC Settings > Desktop > Panel instead.")?;
    let backup = home().join(".local/state/ember/panel-backup-plugins_wings.ron");
    if !backup.exists() {
        let _ = fs::create_dir_all(backup.parent().unwrap_or(Path::new(".")));
        let _ = fs::write(&backup, &text);
    }
    fs::write(&path, new).map_err(|e| e.to_string())
}

/// Pure edit of a `plugins_wings` RON value: `Some(([left...], [right...]))`.
pub fn edit_wings(text: &str, add: bool) -> Option<String> {
    let entry = format!("\"{APPLET_ID}\"");
    let has = text.contains(&entry);
    if add == has {
        return Some(text.to_string());
    }
    if !add {
        let out: String = text.lines().filter(|l| l.trim().trim_end_matches(',') != entry).map(|l| format!("{l}\n")).collect();
        let out = out.replace(&format!("{entry}, "), "").replace(&format!(", {entry}"), "").replace(&entry, "");
        return Some(out.trim_end().to_string());
    }
    // right wing starts after the first "], [" (multi-line) or "], [" inline
    let i = text.find("], [")?;
    let (head, tail) = text.split_at(i + 4);
    if tail.trim_start().starts_with(']') { Some(format!("{head}\n    {entry},\n{}", tail.trim_start())) } else { Some(format!("{head}\n    {entry},{tail}")) }
}

// ---------- swapping COSMIC's panel applets for Ember's ----------

const E: &str = "io.github.n1th1n_19.Ember.Applet";
/// stock applet → Ember replacement, same spot in the panel
pub const SWAPS: [(&str, &str); 7] = [
    ("com.system76.CosmicAppletTime", "io.github.n1th1n_19.Ember.Applet.Clock"),
    ("com.system76.CosmicAppletTiling", "io.github.n1th1n_19.Ember.Applet.Tiling"),
    ("com.system76.CosmicAppletAudio", "io.github.n1th1n_19.Ember.Applet.Sound"),
    ("com.system76.CosmicAppletBluetooth", "io.github.n1th1n_19.Ember.Applet.Bluetooth"),
    ("com.system76.CosmicAppletNetwork", "io.github.n1th1n_19.Ember.Applet.Network"),
    ("com.system76.CosmicAppletBattery", "io.github.n1th1n_19.Ember.Applet.Power"),
    ("com.system76.CosmicAppletPower", "io.github.n1th1n_19.Ember.Applet.Session"),
];
const DND: &str = "io.github.n1th1n_19.Ember.Applet.Dnd";
const EXTRAS: [&str; 2] = ["io.github.n1th1n_19.Ember.Applet.Media", "io.github.n1th1n_19.Ember.Applet.Awake"];

pub fn ember_applets_on() -> bool {
    fs::read_to_string(panel_wings()).is_ok_and(|s| s.contains(&format!("\"{}\"", SWAPS[0].1)) || s.contains(&format!("\"{DND}\"")))
}

/// Pure edit of `plugins_wings`: swap stock applets for Ember's in place (on) or back
/// (off). DND goes beside COSMIC's notifications applet (which keeps the history);
/// media and keep-awake go after Ember's readings, or start the right wing.
pub fn edit_ember_applets(text: &str, on: bool) -> Option<String> {
    let q = |id: &str| format!("\"{id}\"");
    let mut s = text.to_string();
    if !s.contains("], [") {
        return None;
    }
    let indent = "    ";
    if on {
        for (stock, ember) in SWAPS {
            s = s.replace(&q(stock), &q(ember));
        }
        if !s.contains(&q(DND)) {
            let notif = q("com.system76.CosmicAppletNotifications");
            s = match s.find(&format!("{indent}{notif}")) {
                Some(i) => format!("{}{indent}{},\n{}", &s[..i], q(DND), &s[i..]),
                None => s.replacen("], [\n", &format!("], [\n{indent}{},\n", q(DND)), 1),
            };
        }
        let extras: String = EXTRAS.iter().filter(|e| !s.contains(&q(e))).map(|e| format!("{indent}{},\n", q(e))).collect();
        if !extras.is_empty() {
            let readings = format!("{indent}{},\n", q(E));
            s = match s.find(&readings) {
                Some(i) => format!("{}{extras}{}", &s[..i + readings.len()], &s[i + readings.len()..]),
                None => s.replacen("], [\n", &format!("], [\n{extras}"), 1),
            };
        }
    } else {
        for (stock, ember) in SWAPS {
            s = s.replace(&q(ember), &q(stock));
        }
        for gone in std::iter::once(DND).chain(EXTRAS) {
            s = s.lines().filter(|l| l.trim().trim_end_matches(',') != q(gone)).map(|l| format!("{l}\n")).collect::<String>();
            s = s.trim_end().to_string();
        }
    }
    Some(s)
}

pub fn set_ember_applets(on: bool) -> Result<(), String> {
    let path = panel_wings();
    let text = fs::read_to_string(&path).map_err(|_| "COSMIC top panel config not found.".to_string())?;
    let new = edit_ember_applets(&text, on).ok_or("Unexpected panel config layout.")?;
    let backup = home().join(".local/state/ember/panel-backup-plugins_wings.ron");
    if !backup.exists() {
        let _ = fs::create_dir_all(backup.parent().unwrap_or(Path::new(".")));
        let _ = fs::write(&backup, &text);
    }
    fs::write(&path, new).map_err(|e| e.to_string())
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
                self.status = Some(match set_autostart(on) {
                    Ok(()) => (if on { "Widget will start when you log in." } else { "Widget won't start at login." }.into(), false),
                    Err(e) => (format!("Couldn't change autostart: {e}"), true),
                });
            }
            Msg::EmberApplets(on) => {
                save = false;
                self.status = Some(match set_ember_applets(on) {
                    Ok(()) => (
                        if on { "Ember's applets are in the panel. COSMIC's originals are one click away." } else { "COSMIC's own applets are back." }.into(),
                        false,
                    ),
                    Err(e) => (e, true),
                });
            }
            Msg::Panel(on) => {
                save = false;
                self.status = Some(match set_in_panel(on) {
                    Ok(()) => (if on { "Added to the top panel." } else { "Removed from the top panel." }.into(), false),
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
        let autostart = autostart_enabled();
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

        let in_panel = in_panel();
        let mut akeys = row::with_capacity(8).spacing(8).align_y(Alignment::Center).push(body("Shows"));
        for k in Key::ALL {
            akeys = akeys.push(comic_button(k.short(), Some(msg(Msg::AppletKey(k))), MAGENTA, c.applet().contains(&k)));
        }
        let applet_col = column::with_capacity(4)
            .spacing(12)
            .push(heading("Panel applet"))
            .push(muted("Small ink meters in the top panel; click them for a popup with every reading."))
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body("Readings in top panel"))
                    .push(pick("Yes", Msg::Panel(true), YELLOW, in_panel))
                    .push(pick("No", Msg::Panel(false), YELLOW, !in_panel)),
            )
            .push(
                row::with_capacity(3)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(body("Clock, sound, Bluetooth, network, power, tiling, session"))
                    .push(pick("Ember's", Msg::EmberApplets(true), YELLOW, ember_applets_on()))
                    .push(pick("COSMIC's", Msg::EmberApplets(false), YELLOW, !ember_applets_on())),
            )
            .push(muted("Ember's versions take the same spots, plus Do Not Disturb, media and keep-awake. COSMIC's notification list stays."))
            .push(akeys);

        let mut col = column::with_capacity(5).spacing(22).push(caption("Settings"));
        if let Some((t, e)) = &self.status {
            col = col.push(banner(t, *e));
        }
        page(col.push(card(look)).push(card(widget_col)).push(card(applet_col)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str =
        "Some(([\n    \"com.system76.CosmicAppList\",\n], [\n    \"com.system76.CosmicAppletTime\",\n    \"com.system76.CosmicAppletPower\",\n]))";

    #[test]
    fn panel_wings_edit_roundtrip() {
        let added = edit_wings(REAL, true).unwrap();
        assert!(added.contains("], [\n    \"io.github.n1th1n_19.Ember.Applet\",\n    \"com.system76.CosmicAppletTime\""), "{added}");
        assert_eq!(edit_wings(&added, true).unwrap(), added); // idempotent
        assert_eq!(edit_wings(&added, false).unwrap(), REAL);

        let empty = edit_wings("Some(([], []))", true).unwrap();
        assert_eq!(empty, "Some(([], [\n    \"io.github.n1th1n_19.Ember.Applet\",\n]))");
        assert_eq!(edit_wings("None", true), None);
    }

    #[test]
    fn ember_applets_swap_roundtrip() {
        let stock = "Some(([\n    \"com.system76.CosmicAppList\",\n], [\n    \"io.github.n1th1n_19.Ember.Applet\",\n    \"com.system76.CosmicAppletTime\",\n    \"io.github.marcelogomes90.cosmic-ext-applet-clip-keep\",\n    \"com.system76.CosmicAppletAudio\",\n    \"com.system76.CosmicAppletNotifications\",\n    \"com.system76.CosmicAppletPower\",\n]))";
        let on = edit_ember_applets(stock, true).unwrap();
        let expect = "Some(([\n    \"com.system76.CosmicAppList\",\n], [\n    \"io.github.n1th1n_19.Ember.Applet\",\n    \"io.github.n1th1n_19.Ember.Applet.Media\",\n    \"io.github.n1th1n_19.Ember.Applet.Awake\",\n    \"io.github.n1th1n_19.Ember.Applet.Clock\",\n    \"io.github.marcelogomes90.cosmic-ext-applet-clip-keep\",\n    \"io.github.n1th1n_19.Ember.Applet.Sound\",\n    \"io.github.n1th1n_19.Ember.Applet.Dnd\",\n    \"com.system76.CosmicAppletNotifications\",\n    \"io.github.n1th1n_19.Ember.Applet.Session\",\n]))";
        assert_eq!(on, expect);
        assert_eq!(edit_ember_applets(&on, true).unwrap(), on); // idempotent
        assert_eq!(edit_ember_applets(&on, false).unwrap(), stock);
        assert_eq!(edit_ember_applets("None", true), None);
    }
}
