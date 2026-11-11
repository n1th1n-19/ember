// SPDX-License-Identifier: GPL-3.0-or-later
//! Installed apps from every source: APT, Flatpak, Snap, AppImage, hand-installed
//! launchers, and cargo / npm / bun globals.
//!
//! An "app" is a launcher (`.desktop` entry) or a package-manager global. Plain APT
//! libraries are left out (2,000+ rows nobody launches). Each app knows how to be
//! uninstalled by a button, or, when that isn't safe or possible, the exact command
//! to run instead.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::control::CtlResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Apt,
    Flatpak,
    Snap,
    AppImage,
    Manual,
    Cargo,
    Npm,
    Bun,
}

impl Source {
    pub const ALL: [Source; 8] = [Source::Apt, Source::Flatpak, Source::Snap, Source::AppImage, Source::Manual, Source::Cargo, Source::Npm, Source::Bun];
    pub fn name(self) -> &'static str {
        match self {
            Source::Apt => "APT",
            Source::Flatpak => "Flatpak",
            Source::Snap => "Snap",
            Source::AppImage => "AppImage",
            Source::Manual => "Manual",
            Source::Cargo => "Cargo",
            Source::Npm => "npm",
            Source::Bun => "Bun",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub name: String,
    pub source: Source,
    /// package / app id / file path, depending on source
    pub id: String,
    pub version: Option<String>,
    pub size: Option<u64>,
    /// launcher file, if any
    pub desktop: Option<PathBuf>,
    /// executable (AppImage path)
    pub path: Option<PathBuf>,
    /// Flatpak: installed per-user (vs system-wide)
    pub user: bool,
    /// launcher hidden (NoDisplay) or snap disabled
    pub disabled: bool,
    /// icon theme name or absolute file path (launcher `Icon=`)
    pub icon: Option<String>,
}

// ---------- desktop entries ----------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Desktop {
    pub name: String,
    pub exec: String,
    pub no_display: bool,
    pub app_type: String,
    pub icon: String,
    pub wm_class: String,
}

/// Minimal parser: keys of the `[Desktop Entry]` group only, unlocalized.
pub fn parse_desktop(text: &str) -> Desktop {
    let mut d = Desktop::default();
    let mut in_main = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "Name" => d.name = v.trim().to_string(),
            "Exec" => d.exec = v.trim().to_string(),
            "NoDisplay" => d.no_display = v.trim() == "true",
            "Hidden" => d.no_display |= v.trim() == "true",
            "Type" => d.app_type = v.trim().to_string(),
            "Icon" => d.icon = v.trim().to_string(),
            "StartupWMClass" => d.wm_class = v.trim().to_string(),
            _ => {}
        }
    }
    d
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn user_apps_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).or_else(|| Some(home()?.join(".local/share")))?;
    Some(data.join("applications"))
}

/// Launcher dirs, highest precedence first.
fn launcher_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(u) = user_apps_dir() {
        v.push(u);
    }
    if let Some(h) = home() {
        v.push(h.join(".local/share/flatpak/exports/share/applications"));
    }
    v.push("/var/lib/flatpak/exports/share/applications".into());
    v.push("/var/lib/snapd/desktop/applications".into());
    v.push("/usr/local/share/applications".into());
    v.push("/usr/share/applications".into());
    v
}

fn read_launchers() -> Vec<(PathBuf, Desktop)> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for dir in launcher_dirs() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "desktop")).collect();
        files.sort();
        for p in files {
            let file_id = p.file_name().unwrap_or_default().to_os_string();
            // first dir wins (same rule as the desktop), so an override hides the original
            if !seen.insert(file_id) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&p) else { continue };
            let d = parse_desktop(&text);
            if d.app_type == "Application" && !d.name.is_empty() {
                out.push((p, d));
            }
        }
    }
    out
}

/// Lowercase program name → launcher icon, for decorating process lists. Keys are the
/// Exec binary's file name, StartupWMClass and the launcher file id.
pub fn launcher_icons() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (path, d) in read_launchers() {
        if d.icon.is_empty() {
            continue;
        }
        let exec = d.exec.split_whitespace().find(|t| !t.contains('=') && *t != "env").unwrap_or("").trim_matches('"');
        let mut keys = vec![Path::new(exec).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()];
        keys.push(d.wm_class.to_lowercase());
        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy().to_lowercase();
            // "org.gnome.Nautilus" → also "nautilus"
            keys.push(stem.rsplit('.').next().unwrap_or(&stem).to_string());
            keys.push(stem);
        }
        for k in keys.into_iter().filter(|k| k.len() > 1 && k != "flatpak" && k != "snap" && k != "sh" && k != "bash") {
            map.entry(k).or_insert_with(|| d.icon.clone());
        }
    }
    map
}

// ---------- parsers for package-manager output ----------

/// `dpkg -S path...` stdout: "pkg1, pkg2: /path" per line → path → first package.
pub fn parse_dpkg_s(out: &str) -> HashMap<String, String> {
    out.lines()
        .filter_map(|l| {
            let (pkgs, path) = l.split_once(": ")?;
            // "diversion by" lines and the like don't have a plain package list
            let pkg = pkgs.split(", ").next()?.split(':').next()?.trim();
            (!pkg.contains(' ')).then(|| (path.trim().to_string(), pkg.to_string()))
        })
        .collect()
}

/// `dpkg-query -W -f '${Package}\t${Version}\t${Installed-Size}\n'`
pub fn parse_dpkg_query(out: &str) -> HashMap<String, (String, Option<u64>)> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let (pkg, ver, size) = (it.next()?, it.next()?, it.next());
            Some((pkg.to_string(), (ver.to_string(), size.and_then(|s| s.trim().parse::<u64>().ok()).map(|k| k * 1024))))
        })
        .collect()
}

/// `flatpak list --app --columns=application,name,version,installation,size`
/// Size column is human ("24.3 MB"); converted to bytes.
pub fn parse_flatpak(out: &str) -> Vec<App> {
    out.lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            let (id, name) = (*c.first()?, *c.get(1)?);
            if id.is_empty() {
                return None;
            }
            Some(App {
                name: if name.is_empty() { id.to_string() } else { name.to_string() },
                source: Source::Flatpak,
                id: id.to_string(),
                version: c.get(2).filter(|v| !v.is_empty()).map(|v| v.to_string()),
                size: c.get(4).and_then(|s| parse_human_size(s)),
                desktop: None,
                path: None,
                user: c.get(3) == Some(&"user"),
                disabled: false,
                icon: None,
            })
        })
        .collect()
}

pub fn parse_human_size(s: &str) -> Option<u64> {
    let (n, unit) = s.trim().split_once(|c: char| c.is_whitespace() || c == '\u{a0}')?;
    let n: f64 = n.replace(',', ".").parse().ok()?;
    let k = match unit.trim() {
        "bytes" | "B" => 1.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    Some((n * k) as u64)
}

/// `snap list` table (active revisions only; a disabled snap carries "disabled" in Notes). Skips bases/snapd/core (system plumbing, not apps).
pub fn parse_snap(out: &str) -> Vec<App> {
    out.lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split_whitespace().collect();
            let (name, version) = (*c.first()?, *c.get(1)?);
            let notes = c.get(5).copied().unwrap_or("");
            if notes.contains("base")
                || notes.contains("core")
                || notes.contains("snapd")
                || name == "snapd"
                || name.starts_with("gtk-common-themes")
                || name.starts_with("gnome-") && name.contains("-2")
            {
                return None;
            }
            Some(App {
                name: name.to_string(),
                source: Source::Snap,
                id: name.to_string(),
                version: Some(version.to_string()),
                size: None,
                desktop: None,
                path: None,
                user: false,
                disabled: notes.contains("disabled"),
                icon: None,
            })
        })
        .collect()
}

/// `cargo install --list`: "name v1.2.3:" lines (indented lines are binaries).
pub fn parse_cargo(out: &str) -> Vec<App> {
    out.lines()
        .filter(|l| !l.starts_with(char::is_whitespace))
        .filter_map(|l| {
            let (name, ver) = l.trim_end_matches(':').split_once(' ')?;
            Some(global(Source::Cargo, name, Some(ver.trim_start_matches('v').split_whitespace().next()?.to_string())))
        })
        .collect()
}

/// `npm ls -g --depth 0 --parseable`: one path per line, first line is the prefix.
pub fn parse_npm(out: &str) -> Vec<App> {
    out.lines()
        .filter_map(|l| {
            let rest = l.split_once("/node_modules/")?.1;
            (rest != "npm" && rest != "corepack").then(|| global(Source::Npm, rest, None))
        })
        .collect()
}

/// `bun pm ls -g`: tree lines like "├── name@1.2.3".
pub fn parse_bun(out: &str) -> Vec<App> {
    out.lines()
        .filter_map(|l| {
            let item = l.trim_start_matches(|c: char| !c.is_alphanumeric() && c != '@').trim();
            let at = item.rfind('@').filter(|&i| i > 0)?;
            Some(global(Source::Bun, &item[..at], Some(item[at + 1..].to_string())))
        })
        .collect()
}

fn global(source: Source, name: &str, version: Option<String>) -> App {
    App {
        name: name.to_string(),
        source,
        id: name.to_string(),
        version,
        size: None,
        desktop: None,
        path: None,
        user: true,
        disabled: false,
        icon: Some("utilities-terminal".into()),
    }
}

/// ELF with the AppImage magic at offset 8 ("AI" + type 1 or 2).
pub fn is_appimage(head: &[u8]) -> bool {
    head.len() >= 11 && head.starts_with(b"\x7fELF") && &head[8..10] == b"AI" && matches!(head[10], 1 | 2)
}

fn file_is_appimage(p: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 11];
    fs::File::open(p).and_then(|mut f| f.read_exact(&mut head)).is_ok() && is_appimage(&head)
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---------- listing ----------

/// Everything installed. Slow-ish (spawns package managers): call off the UI thread.
pub fn list() -> Vec<App> {
    let mut apps = vec![];
    let launchers = read_launchers();

    // which system launchers belong to an APT package
    let sys: Vec<String> = launchers.iter().map(|(p, _)| p).filter(|p| p.starts_with("/usr/")).map(|p| p.display().to_string()).collect();
    let owners = if sys.is_empty() { HashMap::new() } else { parse_dpkg_s(&run_lossy("dpkg", &["-S"], &sys)) };
    let pkgs = run("dpkg-query", &["-W", "-f", "${Package}\t${Version}\t${Installed-Size}\n"]).map(|o| parse_dpkg_query(&o)).unwrap_or_default();

    let flatpaks: HashMap<String, App> = run("flatpak", &["list", "--app", "--columns=application,name,version,installation,size"])
        .map(|o| parse_flatpak(&o))
        .unwrap_or_default()
        .into_iter()
        .map(|a| (a.id.clone(), a))
        .collect();
    let mut flat_seen = HashSet::new();
    let snaps: HashMap<String, App> = run("snap", &["list"]).map(|o| parse_snap(&o)).unwrap_or_default().into_iter().map(|a| (a.id.clone(), a)).collect();
    let mut snap_seen = HashSet::new();
    let mut apt_seen = HashSet::new();

    for (path, d) in &launchers {
        let file_id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ps = path.display().to_string();
        let exec_bin = d.exec.split_whitespace().next().unwrap_or("").trim_matches('"').to_string();
        let icon = (!d.icon.is_empty()).then(|| d.icon.clone());
        let mut app = App {
            name: d.name.clone(),
            source: Source::Manual,
            id: ps.clone(),
            version: None,
            size: None,
            desktop: Some(path.clone()),
            path: None,
            user: true,
            disabled: d.no_display,
            icon: icon.clone(),
        };

        if ps.contains("/flatpak/exports/") {
            let Some(f) = flatpaks.get(&file_id).or_else(|| flatpaks.values().find(|f| file_id.starts_with(&f.id))) else { continue };
            if !flat_seen.insert(f.id.clone()) {
                continue;
            }
            app = App { desktop: Some(path.clone()), disabled: d.no_display, name: d.name.clone(), icon, ..f.clone() };
        } else if ps.starts_with("/var/lib/snapd/") {
            let name = file_id.split('_').next().unwrap_or(&file_id).to_string();
            let Some(s) = snaps.get(&name) else { continue };
            if !snap_seen.insert(name) {
                continue;
            }
            app = App { desktop: Some(path.clone()), name: d.name.clone(), icon, ..s.clone() };
        } else if let Some(pkg) = owners.get(&ps) {
            // one row per package even if it ships several launchers
            if !apt_seen.insert(pkg.clone()) || d.no_display {
                continue;
            }
            let (ver, size) = pkgs.get(pkg).cloned().unwrap_or_default();
            app = App { source: Source::Apt, id: pkg.clone(), version: Some(ver).filter(|v| !v.is_empty()), size, ..app };
        } else if exec_bin.to_lowercase().ends_with(".appimage") {
            let p = PathBuf::from(&exec_bin);
            app = App { source: Source::AppImage, id: exec_bin.clone(), size: fs::metadata(&p).ok().map(|m| m.len()), path: Some(p), ..app };
        } else if ps.starts_with("/usr/") && d.no_display {
            continue; // unowned hidden system helpers: noise
        }
        apps.push(app);
    }

    // packages with no launcher of their own
    apps.extend(flatpaks.into_values().filter(|f| !flat_seen.contains(&f.id)));
    apps.extend(snaps.into_values().filter(|s| !snap_seen.contains(&s.id)));

    // AppImages lying around without a launcher
    let known: HashSet<PathBuf> = apps.iter().filter_map(|a| a.path.clone()).collect();
    if let Some(h) = home() {
        for dir in ["Applications", "Downloads", "Desktop", ".local/bin"] {
            let Ok(rd) = fs::read_dir(h.join(dir)) else { continue };
            for p in rd.flatten().map(|e| e.path()) {
                if p.is_file() && !known.contains(&p) && file_is_appimage(&p) {
                    let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    apps.push(App {
                        name,
                        source: Source::AppImage,
                        id: p.display().to_string(),
                        version: None,
                        size: fs::metadata(&p).ok().map(|m| m.len()),
                        desktop: None,
                        path: Some(p),
                        user: true,
                        disabled: false,
                        icon: None,
                    });
                }
            }
        }
    }

    apps.extend(run("cargo", &["install", "--list"]).map(|o| parse_cargo(&o)).unwrap_or_default());
    apps.extend(run("npm", &["ls", "-g", "--depth", "0", "--parseable"]).map(|o| parse_npm(&o)).unwrap_or_default());
    apps.extend(run("bun", &["pm", "ls", "-g"]).map(|o| parse_bun(&o)).unwrap_or_default());

    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Like `run` but keeps stdout even on a non-zero exit (dpkg -S fails if any path is unowned).
fn run_lossy(cmd: &str, args: &[&str], more: &[String]) -> String {
    Command::new(cmd).args(args).args(more).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

// ---------- safety ----------

/// Package names APT accepts (Debian policy): no option injection, no paths.
pub fn valid_pkg(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphanumeric() && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"+-.".contains(c))
}

/// Never removed by a button: the desktop, boot, package tooling, auth, Ember itself.
pub fn is_protected(pkg: &str) -> bool {
    const EXACT: [&str; 14] = [
        "apt",
        "dpkg",
        "sudo",
        "systemd",
        "polkitd",
        "pkexec",
        "libc6",
        "bash",
        "coreutils",
        "network-manager",
        "pop-desktop",
        "ubuntu-desktop",
        "ember",
        "init",
    ];
    const PREFIX: [&str; 7] = ["cosmic-", "linux-", "systemd-", "libsystemd", "grub", "pop-", "xdg-desktop-portal"];
    EXACT.contains(&pkg) || PREFIX.iter().any(|p| pkg.starts_with(p))
}

/// Packages `apt-get -s remove` says would go ("Remv name [ver]").
pub fn parse_apt_sim(out: &str) -> Vec<String> {
    out.lines().filter_map(|l| l.strip_prefix("Remv ")).filter_map(|r| r.split_whitespace().next()).map(String::from).collect()
}

/// Essential or required-priority packages among `pkgs` (from dpkg-query).
fn essential(pkgs: &[String]) -> Vec<String> {
    if pkgs.is_empty() {
        return vec![];
    }
    let mut args = vec!["-W".to_string(), "-f".to_string(), "${Package}\t${Essential}\t${Priority}\n".to_string()];
    args.extend(pkgs.iter().cloned());
    let out = Command::new("dpkg-query").args(&args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.get(1) == Some(&"yes") || c.get(2) == Some(&"required")).then(|| c[0].to_string())
        })
        .collect()
}

/// What an uninstall would do, shown in the confirm step.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// every package that would be removed (APT may take dependants with it)
    pub removes: Vec<String>,
    /// why the button is blocked; when set, only the command is offered
    pub blocked: Option<String>,
    /// shell command equivalent (always shown, copyable)
    pub command: String,
    /// true if Ember can run it with a button
    pub button: bool,
}

fn quote(s: &str) -> String {
    if s.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._-+@:".contains(&b)) { s.to_string() } else { format!("'{}'", s.replace('\'', r"'\''")) }
}

/// Blocking: may run `apt-get -s`. Call off the UI thread.
pub fn plan(app: &App) -> Plan {
    let q = quote(&app.id);
    match app.source {
        Source::Apt => {
            let command = format!("sudo apt remove {q}");
            if !valid_pkg(&app.id) {
                return Plan { removes: vec![], blocked: Some("Unusual package name.".into()), command, button: false };
            }
            let sim = Command::new("apt-get")
                .args(["-s", "remove", "--", &app.id])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
            let removes = parse_apt_sim(&sim);
            let mut bad: Vec<String> = removes.iter().filter(|p| is_protected(p)).cloned().collect();
            bad.extend(essential(&removes));
            bad.sort();
            bad.dedup();
            let blocked = if is_protected(&app.id) || !bad.is_empty() {
                Some(format!(
                    "Part of the system ({}). Ember won't remove it with a button; the command is here if you really mean it.",
                    if bad.is_empty() { app.id.clone() } else { bad.join(", ") }
                ))
            } else if removes.is_empty() {
                Some("APT says nothing would be removed (already gone or held).".into())
            } else {
                None
            };
            let button = blocked.is_none();
            Plan { removes, blocked, command, button }
        }
        Source::Flatpak => {
            let scope = if app.user { "--user" } else { "--system" };
            Plan { removes: vec![app.id.clone()], blocked: None, command: format!("flatpak uninstall {scope} {q}"), button: true }
        }
        Source::Snap => Plan { removes: vec![app.id.clone()], blocked: None, command: format!("snap remove {q}"), button: true },
        Source::AppImage => {
            let mut files: Vec<String> = app.path.iter().chain(app.desktop.iter()).map(|p| p.display().to_string()).collect();
            files.dedup();
            let command = format!("gio trash {}", files.iter().map(|f| quote(f)).collect::<Vec<_>>().join(" "));
            Plan { removes: files, blocked: None, command, button: app.path.is_some() }
        }
        Source::Manual => {
            let d = app.desktop.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let user_owned = user_apps_dir().is_some_and(|u| d.starts_with(&u.display().to_string()));
            let note = if user_owned {
                "Ember can't tell which files belong to this app. The command removes only its launcher; delete the program's folder yourself."
            } else {
                "This launcher isn't owned by any package manager. Remove it by hand if you're sure."
            };
            let command = if user_owned { format!("gio trash {}", quote(&d)) } else { format!("sudo rm {}", quote(&d)) };
            Plan { removes: vec![d], blocked: Some(note.into()), command, button: false }
        }
        Source::Cargo => Plan { removes: vec![app.id.clone()], blocked: None, command: format!("cargo uninstall {q}"), button: true },
        Source::Npm => Plan { removes: vec![app.id.clone()], blocked: None, command: format!("npm rm -g {q}"), button: true },
        Source::Bun => Plan { removes: vec![app.id.clone()], blocked: None, command: format!("bun remove -g {q}"), button: true },
    }
}

fn exec(cmd: &str, args: &[&str]) -> CtlResult {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("couldn't start {cmd}: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) if cmd == "pkexec" => Err("Not authorized (password prompt was cancelled or failed).".into()),
        _ => {
            let err = String::from_utf8_lossy(&out.stderr);
            let err = err.trim();
            Err(if err.is_empty() { format!("{cmd} exited with {}", out.status) } else { err.lines().last().unwrap_or(err).to_string() })
        }
    }
}

/// Blocking (package managers, polkit prompts). Re-checks the plan so a stale UI can't
/// remove something that became protected.
pub fn uninstall(app: &App) -> CtlResult {
    let plan = plan(app);
    if !plan.button {
        return Err(plan.blocked.unwrap_or_else(|| "Use the command shown.".into()));
    }
    let id = app.id.as_str();
    match app.source {
        Source::Apt => exec("pkexec", &["apt-get", "remove", "-y", "--", id]),
        Source::Flatpak => exec("flatpak", &["uninstall", if app.user { "--user" } else { "--system" }, "-y", "--noninteractive", "--", id]),
        Source::Snap => exec("snap", &["remove", "--", id]),
        Source::AppImage => {
            let files: Vec<String> = app.path.iter().chain(app.desktop.iter()).map(|p| p.display().to_string()).collect();
            let mut args = vec!["trash", "--"];
            args.extend(files.iter().map(String::as_str));
            exec("gio", &args)
        }
        Source::Cargo => exec("cargo", &["uninstall", "--", id]),
        Source::Npm => exec("npm", &["rm", "-g", "--", id]),
        Source::Bun => exec("bun", &["remove", "-g", id]),
        Source::Manual => unreachable!("plan never offers a button"),
    }
}

/// Delete an app's leftover user data (Flatpak sandbox dir only; others are app-specific).
pub fn delete_data(app: &App) -> CtlResult {
    if app.source != Source::Flatpak || !app.id.contains('.') || app.id.contains('/') {
        return Err("Only Flatpak apps keep their data in one known folder.".into());
    }
    let dir = home().ok_or("no HOME")?.join(".var/app").join(&app.id);
    if dir.exists() { exec("gio", &["trash", "--", &dir.display().to_string()]) } else { Ok(()) }
}

// ---------- disable / enable ----------

const MARK: &str = "X-Ember-Hidden=true";

/// Hide from the launcher (reversible). Snaps are truly disabled instead.
pub fn set_disabled(app: &App, off: bool) -> CtlResult {
    if app.source == Source::Snap {
        return exec("snap", &[if off { "disable" } else { "enable" }, "--", &app.id]);
    }
    let src = app.desktop.as_ref().ok_or("This app has no launcher to hide.")?;
    let dir = user_apps_dir().ok_or("no user applications folder")?;
    set_hidden_in(src, &dir, off)
}

/// Core of disable/enable, testable with any folder:
/// - launcher already in the user folder: add/remove `NoDisplay=true` + our marker in place
/// - system launcher: write/delete a marked copy in the user folder (it takes precedence)
pub fn set_hidden_in(src: &Path, user_dir: &Path, hide: bool) -> CtlResult {
    let name = src.file_name().ok_or("bad launcher path")?;
    let target = user_dir.join(name);
    let in_place = src.parent() == Some(user_dir);
    let text = fs::read_to_string(if target.exists() { &target } else { src }).map_err(|e| e.to_string())?;
    let ours = text.lines().any(|l| l.trim() == MARK);

    if !hide {
        if !ours {
            return Ok(()); // hidden by someone else, or not hidden: leave it alone
        }
        if !in_place && target.exists() {
            return fs::remove_file(&target).map_err(|e| e.to_string());
        }
        let cleaned: String = text.lines().filter(|l| l.trim() != MARK && l.trim() != "NoDisplay=true").map(|l| format!("{l}\n")).collect();
        return fs::write(&target, cleaned).map_err(|e| e.to_string());
    }
    if ours {
        return Ok(());
    }
    // insert after the [Desktop Entry] header so it applies to the right group
    let mut out = String::new();
    let mut done = false;
    for l in text.lines() {
        out.push_str(l);
        out.push('\n');
        if !done && l.trim() == "[Desktop Entry]" {
            out.push_str("NoDisplay=true\n");
            out.push_str(MARK);
            out.push('\n');
            done = true;
        }
    }
    if !done {
        return Err("Launcher has no [Desktop Entry] section.".into());
    }
    fs::create_dir_all(user_dir).map_err(|e| e.to_string())?;
    fs::write(&target, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_main_group_only() {
        let d = parse_desktop(
            "[Desktop Entry]\nName=GIMP\nName[de]=GIMP de\nExec=gimp %U\nType=Application\n[Desktop Action new]\nName=New Window\nNoDisplay=true\n",
        );
        assert_eq!((d.name.as_str(), d.exec.as_str(), d.no_display, d.app_type.as_str()), ("GIMP", "gimp %U", false, "Application"));
    }

    #[test]
    fn package_manager_outputs() {
        let owners = parse_dpkg_s(
            "firefox: /usr/share/applications/firefox.desktop\nlibreoffice-common, libreoffice-calc: /usr/share/applications/calc.desktop\ndiversion by foo from: /x\n",
        );
        assert_eq!(owners["/usr/share/applications/firefox.desktop"], "firefox");
        assert_eq!(owners["/usr/share/applications/calc.desktop"], "libreoffice-common");
        assert!(!owners.contains_key("/x"));

        let q = parse_dpkg_query("vlc\t3.0.20-3\t1234\nfoo\t1.0\t\n");
        assert_eq!(q["vlc"], ("3.0.20-3".to_string(), Some(1234 * 1024)));
        assert_eq!(q["foo"].1, None);

        let f = parse_flatpak("com.obsproject.Studio\tOBS Studio\t32.2.2\tuser\t528.5 MB\norg.gimp.GIMP\tGIMP\t3.0\tsystem\t1.2 GB\n");
        assert_eq!((f[0].name.as_str(), f[0].user, f[0].size), ("OBS Studio", true, Some(528_500_000)));
        assert_eq!((f[1].user, f[1].size), (false, Some(1_200_000_000)));

        let s = parse_snap(
            "Name  Version  Rev  Tracking  Publisher  Notes\nblender  5.3.0  7998  latest/stable  blenderfoundation**  classic\ncore22  20240111  1122  latest/stable  canonical**  base\nsnapd  2.61  21  latest/stable  canonical**  snapd\nold-app  1.0  3  latest/stable  someone  disabled\n",
        );
        assert_eq!(s.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["blender", "old-app"]);
        assert!(s[1].disabled);

        let c = parse_cargo("cargo-expand v1.0.127:\n    cargo-expand\nflutter_rust_bridge_codegen v2.12.0:\n    flutter_rust_bridge_codegen\n");
        assert_eq!(
            c.iter().map(|a| (a.id.as_str(), a.version.as_deref())).collect::<Vec<_>>(),
            [("cargo-expand", Some("1.0.127")), ("flutter_rust_bridge_codegen", Some("2.12.0"))]
        );

        let n = parse_npm(
            "/home/u/.nvm/lib\n/home/u/.nvm/lib/node_modules/@usebruno/cli\n/home/u/.nvm/lib/node_modules/npm\n/home/u/.nvm/lib/node_modules/typescript\n",
        );
        assert_eq!(n.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["@usebruno/cli", "typescript"]);

        let b = parse_bun("/home/u/.bun/install/global node_modules (3)\n├── @anthropic-ai/sdk@0.50.0\n└── prettier@3.3.3\n");
        assert_eq!(
            b.iter().map(|a| (a.id.as_str(), a.version.as_deref())).collect::<Vec<_>>(),
            [("@anthropic-ai/sdk", Some("0.50.0")), ("prettier", Some("3.3.3"))]
        );
    }

    #[test]
    fn appimage_magic() {
        assert!(is_appimage(b"\x7fELF\x02\x01\x01\0AI\x02\0"));
        assert!(!is_appimage(b"\x7fELF\x02\x01\x01\0\0\0\0\0"));
        assert!(!is_appimage(b"#!/bin/sh\necho"));
    }

    #[test]
    fn safety_rules() {
        for p in ["systemd", "systemd-resolved", "cosmic-comp", "linux-image-6.8", "pop-desktop", "sudo", "apt", "ember"] {
            assert!(is_protected(p), "{p}");
        }
        for p in ["vlc", "gimp", "cowsay"] {
            assert!(!is_protected(p), "{p}");
        }
        assert!(valid_pkg("libreoffice-calc") && valid_pkg("g++") && valid_pkg("python3.12"));
        for bad in ["-y", "--purge", "../x", "Foo", "a", "vlc;rm", "a b"] {
            assert!(!valid_pkg(bad), "{bad}");
        }
        assert_eq!(parse_apt_sim("NOTE: x\nRemv cowsay [3.03]\nRemv cowsay-off [3.03]\n"), ["cowsay", "cowsay-off"]);
        assert_eq!(quote("/a b/it's"), r"'/a b/it'\''s'");
        assert_eq!(quote("org.gimp.GIMP"), "org.gimp.GIMP");
        let manual = App {
            name: "X".into(),
            source: Source::Manual,
            id: "x".into(),
            version: None,
            size: None,
            desktop: Some("/usr/share/applications/x.desktop".into()),
            path: None,
            user: false,
            disabled: false,
            icon: None,
        };
        assert!(!plan(&manual).button);
        assert!(uninstall(&manual).is_err());
    }

    #[test]
    fn hide_and_restore_launchers() {
        let root = std::env::temp_dir().join(format!("ember-apps-{}", std::process::id()));
        let (sys, user) = (root.join("sys"), root.join("user"));
        fs::create_dir_all(&sys).unwrap();
        fs::create_dir_all(&user).unwrap();
        let entry = "[Desktop Entry]\nName=Thing\nExec=thing\nType=Application\n[Desktop Action x]\nName=X\n";

        // system launcher → marked override copy, removed on enable
        fs::write(sys.join("thing.desktop"), entry).unwrap();
        set_hidden_in(&sys.join("thing.desktop"), &user, true).unwrap();
        let copy = fs::read_to_string(user.join("thing.desktop")).unwrap();
        assert!(parse_desktop(&copy).no_display && copy.contains(MARK));
        set_hidden_in(&sys.join("thing.desktop"), &user, true).unwrap(); // idempotent
        set_hidden_in(&sys.join("thing.desktop"), &user, false).unwrap();
        assert!(!user.join("thing.desktop").exists());
        assert_eq!(fs::read_to_string(sys.join("thing.desktop")).unwrap(), entry);

        // user launcher → edited in place and restored exactly
        fs::write(user.join("mine.desktop"), entry).unwrap();
        set_hidden_in(&user.join("mine.desktop"), &user, true).unwrap();
        assert!(parse_desktop(&fs::read_to_string(user.join("mine.desktop")).unwrap()).no_display);
        set_hidden_in(&user.join("mine.desktop"), &user, false).unwrap();
        assert_eq!(fs::read_to_string(user.join("mine.desktop")).unwrap(), entry);

        // hidden by someone else: enable leaves it alone
        let foreign = "[Desktop Entry]\nName=F\nType=Application\nNoDisplay=true\n";
        fs::write(user.join("f.desktop"), foreign).unwrap();
        set_hidden_in(&user.join("f.desktop"), &user, false).unwrap();
        assert_eq!(fs::read_to_string(user.join("f.desktop")).unwrap(), foreign);
        fs::remove_dir_all(&root).unwrap();
    }
}
