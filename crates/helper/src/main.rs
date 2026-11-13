// SPDX-License-Identifier: GPL-3.0-or-later
//! ember-helper: the only part of Ember that runs as root, started through pkexec.
//!
//! Security model: a fixed allowlist of verbs, every argument parsed into a typed
//! value before anything happens, sysfs paths built only from those values (never
//! from user strings), no shell, no environment-driven behavior.
//!
//!   governor <name>              name must be in scaling_available_governors
//!   max-perf <10..=100>          intel_pstate/max_perf_pct
//!   turbo <on|off>               intel_pstate/no_turbo or cpufreq/boost
//!   renice <pid> <-20..=19>
//!   signal <pid> <TERM|KILL|STOP|CONT>
//!   pwm <hwmonN> <1..=9> <0..=255>   only if that pwm node exists
//!   thermal <name>               name must be in platform_profile_choices
//!   fwattr <device> <attr> <n>   integer firmware attribute, n within its min/max
//!   restore                      re-apply saved thermal/fwattr settings (run at boot)
//!
//! Successful thermal/fwattr changes are saved to /etc/ember/firmware.conf, one
//! command per line; `restore` re-parses each line through the same validation.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CPU: &str = "/sys/devices/system/cpu";
const HWMON: &str = "/sys/class/hwmon";
const ACPI: &str = "/sys/firmware/acpi";
const FWATTR: &str = "/sys/class/firmware-attributes";
const SAVED: &str = "/etc/ember/firmware.conf";

#[derive(Debug, PartialEq)]
enum Action {
    Governor(String),
    MaxPerf(u8),
    Turbo(bool),
    Renice(i32, i32),
    Signal(i32, libc::c_int),
    Pwm(u32, u8, u8),
    Thermal(String),
    FwAttr(String, String, u32),
    Restore,
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, String> {
    // reject signs/whitespace tricks the generic parser might accept
    s.parse().map_err(|_| format!("bad {what}: {s:?}"))
}

fn pid(s: &str) -> Result<i32, String> {
    let p: i32 = num(s, "pid")?;
    if p <= 1 {
        return Err("refusing to touch pid 0/1".into());
    }
    Ok(p)
}

fn parse(args: &[String]) -> Result<Action, String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["governor", g] => {
            if g.is_empty() || !g.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                return Err(format!("bad governor: {g:?}"));
            }
            Ok(Action::Governor(g.to_string()))
        }
        ["max-perf", p] => match num::<u8>(p, "percent")? {
            v @ 10..=100 => Ok(Action::MaxPerf(v)),
            v => Err(format!("max-perf {v} out of range 10..=100")),
        },
        ["turbo", "on"] => Ok(Action::Turbo(true)),
        ["turbo", "off"] => Ok(Action::Turbo(false)),
        ["renice", p, n] => match num::<i32>(n, "nice")? {
            v @ -20..=19 => Ok(Action::Renice(pid(p)?, v)),
            v => Err(format!("nice {v} out of range -20..=19")),
        },
        ["signal", p, s] => {
            let sig = match *s {
                "TERM" => libc::SIGTERM,
                "KILL" => libc::SIGKILL,
                "STOP" => libc::SIGSTOP,
                "CONT" => libc::SIGCONT,
                _ => return Err(format!("signal {s:?} not allowed")),
            };
            Ok(Action::Signal(pid(p)?, sig))
        }
        ["pwm", chip, idx, val] => {
            let n = chip.strip_prefix("hwmon").ok_or("chip must be hwmonN")?;
            let n: u32 = num(n, "hwmon index")?;
            let idx = match num::<u8>(idx, "pwm index")? {
                v @ 1..=9 => v,
                v => return Err(format!("pwm index {v} out of range 1..=9")),
            };
            Ok(Action::Pwm(n, idx, num(val, "pwm value")?))
        }
        ["thermal", t] => {
            if t.is_empty() || !t.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
                return Err(format!("bad thermal profile: {t:?}"));
            }
            Ok(Action::Thermal(t.to_string()))
        }
        ["fwattr", dev, attr, n] => {
            let ok = |s: &str, extra: u8| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == extra);
            if !ok(dev, b'-') || !ok(attr, b'_') {
                return Err(format!("bad firmware attribute: {dev:?}/{attr:?}"));
            }
            Ok(Action::FwAttr(dev.to_string(), attr.to_string(), num(n, "value")?))
        }
        ["restore"] => Ok(Action::Restore),
        _ => Err("usage: ember-helper governor|max-perf|turbo|renice|signal|pwm|thermal|fwattr|restore ...".into()),
    }
}

fn write(path: &Path, value: &str) -> Result<(), String> {
    fs::write(path, value).map_err(|e| format!("{}: {e}", path.display()))
}

fn cpufreq_dirs() -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(CPU) else { return vec![] };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n.strip_prefix("cpu").is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(|e| e.path().join("cpufreq"))
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn run(action: Action) -> Result<(), String> {
    match action {
        Action::Governor(g) => {
            let dirs = cpufreq_dirs();
            let avail = fs::read_to_string(dirs.first().ok_or("no cpufreq")?.join("scaling_available_governors")).map_err(|e| e.to_string())?;
            if !avail.split_whitespace().any(|a| a == g) {
                return Err(format!("governor {g} not available (have: {})", avail.trim()));
            }
            dirs.iter().try_for_each(|d| write(&d.join("scaling_governor"), &g))
        }
        Action::MaxPerf(p) => write(&Path::new(CPU).join("intel_pstate/max_perf_pct"), &p.to_string()),
        Action::Turbo(on) => {
            let pstate = Path::new(CPU).join("intel_pstate/no_turbo");
            if pstate.exists() { write(&pstate, if on { "0" } else { "1" }) } else { write(&Path::new(CPU).join("cpufreq/boost"), if on { "1" } else { "0" }) }
        }
        Action::Renice(p, n) => {
            // SAFETY: plain syscall with validated arguments
            if unsafe { libc::setpriority(libc::PRIO_PROCESS, p as libc::id_t, n) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
        Action::Signal(p, s) => {
            // SAFETY: plain syscall with validated arguments
            if unsafe { libc::kill(p, s) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
        Action::Pwm(chip, idx, val) => {
            let node = Path::new(HWMON).join(format!("hwmon{chip}")).join(format!("pwm{idx}"));
            if !node.exists() {
                return Err(format!("{} does not exist", node.display()));
            }
            // manual mode first (1), then the duty value
            let enable = node.with_file_name(format!("pwm{idx}_enable"));
            if enable.exists() {
                write(&enable, "1")?;
            }
            write(&node, &val.to_string())
        }
        Action::Thermal(t) => {
            let avail = fs::read_to_string(Path::new(ACPI).join("platform_profile_choices")).map_err(|_| "no platform_profile on this machine".to_string())?;
            if !avail.split_whitespace().any(|a| a == t) {
                return Err(format!("thermal profile {t} not available (have: {})", avail.trim()));
            }
            write(&Path::new(ACPI).join("platform_profile"), &t)
        }
        Action::FwAttr(dev, attr, n) => {
            let dir = Path::new(FWATTR).join(&dev).join("attributes").join(&attr);
            let get = |f: &str| -> Result<String, String> {
                fs::read_to_string(dir.join(f)).map(|s| s.trim().to_string()).map_err(|_| format!("no firmware attribute {dev}/{attr}"))
            };
            if get("type")? != "integer" {
                return Err(format!("{attr} is not an integer attribute"));
            }
            let lim = |f: &str| get(f).and_then(|v| v.parse::<u32>().map_err(|e| e.to_string()));
            let (min, max) = (lim("min_value")?, lim("max_value")?);
            if !(min..=max).contains(&n) {
                return Err(format!("{attr} {n} out of range {min}..={max}"));
            }
            write(&dir.join("current_value"), &n.to_string())
        }
        Action::Restore => {
            let Ok(conf) = fs::read_to_string(SAVED) else { return Ok(()) };
            // limits only apply in the custom profile (or on machines without profiles)
            let custom = || fs::read_to_string(Path::new(ACPI).join("platform_profile")).map_or(true, |p| p.trim() == "custom");
            let mut errs = vec![];
            for line in conf.lines().filter(|l| !l.trim().is_empty()) {
                let args: Vec<String> = line.split_whitespace().map(String::from).collect();
                let r = match parse(&args) {
                    Ok(Action::FwAttr(..)) if !custom() => continue,
                    Ok(a @ (Action::Thermal(_) | Action::FwAttr(..))) => run(a),
                    Ok(_) => Err("not a saved setting".into()),
                    Err(e) => Err(e),
                };
                if let Err(e) = r {
                    errs.push(format!("{line}: {e}"));
                }
            }
            if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
        }
    }
}

/// Key a saved line replaces: "thermal", or "fwattr <device> <attr>".
fn key(line: &str) -> String {
    let w: Vec<&str> = line.split_whitespace().collect();
    w[..w.len().min(if w.first() == Some(&"fwattr") { 3 } else { 1 })].join(" ")
}

/// Saved config with `line` replacing any line of the same key; thermal stays first
/// so `restore` switches to the custom profile before writing its limits.
fn merge(conf: &str, line: &str) -> String {
    let k = key(line);
    let mut lines: Vec<&str> = conf.lines().filter(|l| !l.trim().is_empty() && key(l) != k).collect();
    lines.push(line);
    lines.sort_by_key(|l| !l.starts_with("thermal"));
    lines.join("\n") + "\n"
}

fn save(args: &[String]) -> Result<(), String> {
    let path = Path::new(SAVED);
    fs::create_dir_all(path.parent().unwrap_or(path)).map_err(|e| e.to_string())?;
    let conf = merge(&fs::read_to_string(path).unwrap_or_default(), &args.join(" "));
    let tmp = path.with_extension("tmp");
    write(&tmp, &conf)?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = parse(&args).and_then(|a| {
        let persist = matches!(a, Action::Thermal(_) | Action::FwAttr(..));
        run(a)?;
        // the change itself worked; a failed save only means it won't survive a reboot
        if persist && let Err(e) = save(&args) {
            eprintln!("ember-helper: applied, but couldn't save for next boot: {e}");
        }
        Ok(())
    });
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ember-helper: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Action, String> {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn accepts_allowlisted_verbs() {
        assert_eq!(p("governor powersave"), Ok(Action::Governor("powersave".into())));
        assert_eq!(p("max-perf 60"), Ok(Action::MaxPerf(60)));
        assert_eq!(p("turbo off"), Ok(Action::Turbo(false)));
        assert_eq!(p("renice 4242 -5"), Ok(Action::Renice(4242, -5)));
        assert_eq!(p("signal 4242 KILL"), Ok(Action::Signal(4242, libc::SIGKILL)));
        assert_eq!(p("pwm hwmon3 2 128"), Ok(Action::Pwm(3, 2, 128)));
        assert_eq!(p("thermal max-power"), Ok(Action::Thermal("max-power".into())));
        assert_eq!(p("fwattr lenovo-wmi-other-0 ppt_pl1_spl 70"), Ok(Action::FwAttr("lenovo-wmi-other-0".into(), "ppt_pl1_spl".into(), 70)));
    }

    #[test]
    fn rejects_everything_else() {
        for bad in [
            "",
            "rm -rf /",
            "governor ../../etc/shadow",
            "governor Performance",
            "max-perf 5",
            "max-perf 101",
            "max-perf -1",
            "turbo maybe",
            "renice 1 5",
            "renice 0 5",
            "renice 4242 -21",
            "renice 4242 20",
            "renice abc 5",
            "signal 4242 HUP",
            "signal 1 KILL",
            "signal -1 KILL",
            "pwm hwmon3 0 10",
            "pwm hwmon3 10 10",
            "pwm hwmon3 1 256",
            "pwm ../hwmon3 1 10",
            "pwm hwmon3/../.. 1 10",
            "governor performance extra",
            "thermal ../../etc/shadow",
            "thermal Performance",
            "thermal",
            "fwattr ../x ppt_pl1_spl 70",
            "fwattr lenovo-wmi-other-0 ../../power 1",
            "fwattr lenovo-wmi-other-0 ppt_pl1_spl -5",
            "fwattr lenovo-wmi-other-0 ppt_pl1_spl",
        ] {
            assert!(p(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn unknown_governor_rejected_before_any_write() {
        // parses (lowercase name) but is refused against scaling_available_governors
        let e = run(Action::Governor("rm".into())).unwrap_err();
        assert!(e.contains("not available") || e.contains("no cpufreq"), "{e}");
    }

    #[test]
    fn merge_replaces_same_key_and_keeps_thermal_first() {
        let c = merge("", "fwattr lenovo-wmi-other-0 ppt_pl1_spl 70");
        let c = merge(&c, "thermal custom");
        let c = merge(&c, "fwattr lenovo-wmi-other-0 ppt_pl1_spl 80");
        let c = merge(&c, "fwattr lenovo-wmi-other-0 gpu_temp 80");
        let c = merge(&c, "thermal performance");
        assert_eq!(c, "thermal performance\nfwattr lenovo-wmi-other-0 ppt_pl1_spl 80\nfwattr lenovo-wmi-other-0 gpu_temp 80\n");
        assert_eq!(p("restore"), Ok(Action::Restore));
    }

    #[test]
    fn unknown_fwattr_rejected_before_any_write() {
        let e = run(Action::FwAttr("nope-0".into(), "nope".into(), 1)).unwrap_err();
        assert!(e.contains("no firmware attribute"), "{e}");
    }
}
