// SPDX-License-Identifier: GPL-3.0-or-later
//! Desktop integration shared by every Ember binary, so each package can set itself up
//! on its own: placing applets in the COSMIC top panel (any subset, swapping COSMIC's
//! stock applet where there is one) and the desktop widget's login autostart.
use std::fs;
use std::path::{Path, PathBuf};

/// One Ember panel applet: CLI name, applet ID, and the COSMIC applet it replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applet {
    pub name: &'static str,
    pub id: &'static str,
    pub stock: Option<&'static str>,
}

const fn a(name: &'static str, id: &'static str, stock: Option<&'static str>) -> Applet {
    Applet { name, id, stock }
}

/// Canonical order in the panel's right wing.
pub const APPLETS: [Applet; 11] = [
    a("readings", "io.github.n1th1n_19.Ember.Applet", None),
    a("media", "io.github.n1th1n_19.Ember.Applet.Media", None),
    a("awake", "io.github.n1th1n_19.Ember.Applet.Awake", None),
    a("clock", "io.github.n1th1n_19.Ember.Applet.Clock", Some("com.system76.CosmicAppletTime")),
    a("tiling", "io.github.n1th1n_19.Ember.Applet.Tiling", Some("com.system76.CosmicAppletTiling")),
    a("sound", "io.github.n1th1n_19.Ember.Applet.Sound", Some("com.system76.CosmicAppletAudio")),
    a("bluetooth", "io.github.n1th1n_19.Ember.Applet.Bluetooth", Some("com.system76.CosmicAppletBluetooth")),
    a("network", "io.github.n1th1n_19.Ember.Applet.Network", Some("com.system76.CosmicAppletNetwork")),
    a("power", "io.github.n1th1n_19.Ember.Applet.Power", Some("com.system76.CosmicAppletBattery")),
    // beside COSMIC's notifications applet, which keeps the notification history
    a("dnd", "io.github.n1th1n_19.Ember.Applet.Dnd", None),
    a("session", "io.github.n1th1n_19.Ember.Applet.Session", Some("com.system76.CosmicAppletPower")),
];

const NOTIFICATIONS: &str = "com.system76.CosmicAppletNotifications";
const INDENT: &str = "    ";

pub fn by_name(name: &str) -> Option<Applet> {
    APPLETS.iter().copied().find(|a| a.name == name)
}

/// Parse "sound,clock" or "all" into applets, keeping the canonical order.
pub fn parse_list(s: &str) -> Result<Vec<Applet>, String> {
    if s.trim() == "all" || s.trim().is_empty() {
        return Ok(APPLETS.to_vec());
    }
    let mut out = vec![];
    for n in s.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        out.push(by_name(n).ok_or_else(|| format!("unknown applet {n:?} (known: {})", APPLETS.map(|a| a.name).join(", ")))?);
    }
    out.sort_by_key(|x| APPLETS.iter().position(|a| a == x));
    out.dedup();
    Ok(out)
}

fn q(id: &str) -> String {
    format!("\"{id}\"")
}

fn line(id: &str) -> String {
    format!("{INDENT}{},\n", q(id))
}

/// Pure edit of the panel's `plugins_wings` RON (`Some(([left], [right]))`, as COSMIC
/// writes it: one entry per line). `on` adds the applets (replacing COSMIC's stock one in
/// place when present); off removes them (putting the stock one back). None = layout not
/// recognised, nothing should be written.
pub fn edit_wings(text: &str, applets: &[Applet], on: bool) -> Option<String> {
    let mut s = text.to_string();
    let right = s.find("], [")? + 4;
    for ap in applets {
        let me = q(ap.id);
        if on {
            if s.contains(&me) {
                continue;
            }
            if let Some(stock) = ap.stock.filter(|st| s.contains(&q(st))) {
                s = s.replacen(&q(stock), &me, 1);
                continue;
            }
            // insert after the nearest earlier Ember applet already present, else at the
            // start of the right wing (dnd goes just before COSMIC's notifications applet)
            let before_notif = (ap.name == "dnd").then(|| s.find(&format!("{INDENT}{}", q(NOTIFICATIONS)))).flatten();
            let at = before_notif.unwrap_or_else(|| {
                let idx = APPLETS.iter().position(|x| x == ap).unwrap_or(0);
                APPLETS[..idx].iter().rev().find_map(|prev| s.find(&line(prev.id)).map(|i| i + line(prev.id).len())).unwrap_or_else(|| {
                    // right wing may be "[\n" (entries) or "[]" (empty)
                    let r = s.find("], [").map(|i| i + 4).unwrap_or(right);
                    if s[r..].starts_with('\n') { r + 1 } else { r }
                })
            });
            if s[..at].ends_with('[') {
                // empty inline wing "[]": open it up
                s.insert_str(at, &format!("\n{}", line(ap.id)));
            } else {
                s.insert_str(at, &line(ap.id));
            }
        } else {
            match ap.stock {
                Some(stock) if !s.contains(&q(stock)) => s = s.replacen(&me, &q(stock), 1),
                _ => {
                    s = s.lines().filter(|l| l.trim().trim_end_matches(',') != me).map(|l| format!("{l}\n")).collect::<String>();
                    s = s.trim_end().to_string();
                }
            }
        }
    }
    Some(s)
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(".config"))
}

pub fn panel_wings() -> PathBuf {
    config_home().join("cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings")
}

/// Which of `applets` are currently in the top panel.
pub fn in_panel(applets: &[Applet]) -> Vec<Applet> {
    let text = fs::read_to_string(panel_wings()).unwrap_or_default();
    applets.iter().copied().filter(|a| text.contains(&q(a.id))).collect()
}

/// Add or remove `applets` in the top panel. The first time, the original panel layout
/// is backed up to ~/.local/state/ember/.
pub fn set_panel(applets: &[Applet], on: bool) -> Result<(), String> {
    let path = panel_wings();
    let text = fs::read_to_string(&path).map_err(|_| "COSMIC top panel config not found (is this COSMIC?).".to_string())?;
    let new = edit_wings(&text, applets, on).ok_or("Unexpected panel config layout; use COSMIC Settings > Desktop > Panel instead.")?;
    let backup = home().join(".local/state/ember/panel-backup-plugins_wings.ron");
    if !backup.exists() {
        let _ = fs::create_dir_all(backup.parent().unwrap_or(Path::new(".")));
        let _ = fs::write(&backup, &text);
    }
    if new != text { fs::write(&path, new).map_err(|e| e.to_string()) } else { Ok(()) }
}

// ---------- desktop widget autostart ----------

pub const WIDGET_DESKTOP: &str = "io.github.n1th1n_19.Ember.Widget.desktop";

pub fn autostart_file() -> PathBuf {
    config_home().join("autostart").join(WIDGET_DESKTOP)
}

pub fn autostart_enabled() -> bool {
    autostart_file().exists()
}

/// `widget` = path of the ember-widget binary to start at login.
pub fn set_autostart(on: bool, widget: &Path) -> Result<(), String> {
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
        widget.display()
    );
    fs::write(&f, entry).map_err(|e| e.to_string())
}

/// Installed binary next to the running one (dev build or ~/.local/bin), else /usr/bin.
pub fn sibling_bin(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(name))).filter(|p| p.exists()).unwrap_or_else(|| Path::new("/usr/bin").join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wings(right: &[&str]) -> String {
        let r: String = right.iter().map(|id| format!("    \"{id}\",\n")).collect();
        format!("Some(([\n    \"com.system76.CosmicAppList\",\n], [\n{r}]))")
    }

    const STOCK: [&str; 5] = [
        "com.system76.CosmicAppletTime",
        "io.github.marcelogomes90.cosmic-ext-applet-clip-keep",
        "com.system76.CosmicAppletAudio",
        NOTIFICATIONS,
        "com.system76.CosmicAppletPower",
    ];

    #[test]
    fn all_applets_roundtrip() {
        let stock = wings(&STOCK);
        let on = edit_wings(&stock, &APPLETS, true).unwrap();
        let expect = wings(&[
            "io.github.n1th1n_19.Ember.Applet",
            "io.github.n1th1n_19.Ember.Applet.Media",
            "io.github.n1th1n_19.Ember.Applet.Awake",
            "io.github.n1th1n_19.Ember.Applet.Clock",
            "io.github.n1th1n_19.Ember.Applet.Tiling",
            "io.github.marcelogomes90.cosmic-ext-applet-clip-keep",
            "io.github.n1th1n_19.Ember.Applet.Sound",
            "io.github.n1th1n_19.Ember.Applet.Bluetooth",
            "io.github.n1th1n_19.Ember.Applet.Network",
            "io.github.n1th1n_19.Ember.Applet.Power",
            "io.github.n1th1n_19.Ember.Applet.Dnd",
            NOTIFICATIONS,
            "io.github.n1th1n_19.Ember.Applet.Session",
        ]);
        assert_eq!(on, expect);
        assert_eq!(edit_wings(&on, &APPLETS, true).unwrap(), on);
        // stock-less extras are removed; ones with no stock present (tiling, bluetooth…) were
        // inserted, so removing them deletes the line rather than inventing a stock applet
        let off = edit_wings(&on, &APPLETS, false).unwrap();
        assert!(!off.replace("com.system76.CosmicAppletTiling", "").contains("Ember"));
    }

    #[test]
    fn single_applets_swap_and_restore_exactly() {
        let stock = wings(&STOCK);
        for name in ["sound", "clock", "session"] {
            let ap = [by_name(name).unwrap()];
            let on = edit_wings(&stock, &ap, true).unwrap();
            assert!(on.contains(ap[0].id) && !on.contains(ap[0].stock.unwrap()), "{name}");
            assert_eq!(edit_wings(&on, &ap, false).unwrap(), stock, "{name}");
        }
        let dnd = edit_wings(&stock, &[by_name("dnd").unwrap()], true).unwrap();
        assert!(dnd.contains("\"io.github.n1th1n_19.Ember.Applet.Dnd\",\n    \"com.system76.CosmicAppletNotifications\""));
        assert_eq!(edit_wings(&dnd, &[by_name("dnd").unwrap()], false).unwrap(), stock);
    }

    #[test]
    fn empty_wing_and_bad_input() {
        let empty = "Some(([], []))";
        let on = edit_wings(empty, &[by_name("readings").unwrap()], true).unwrap();
        assert_eq!(on, "Some(([], [\n    \"io.github.n1th1n_19.Ember.Applet\",\n]))");
        assert_eq!(edit_wings("None", &APPLETS, true), None);
        assert!(parse_list("sound, clock").unwrap() == [by_name("clock").unwrap(), by_name("sound").unwrap()]);
        assert_eq!(parse_list("all").unwrap().len(), 11);
        assert!(parse_list("sound,nope").is_err());
    }
}
