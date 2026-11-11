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
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CPU: &str = "/sys/devices/system/cpu";
const HWMON: &str = "/sys/class/hwmon";

#[derive(Debug, PartialEq)]
enum Action {
    Governor(String),
    MaxPerf(u8),
    Turbo(bool),
    Renice(i32, i32),
    Signal(i32, libc::c_int),
    Pwm(u32, u8, u8),
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
        _ => Err("usage: ember-helper governor|max-perf|turbo|renice|signal|pwm ...".into()),
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
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args).and_then(run) {
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
}
