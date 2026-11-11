// SPDX-License-Identifier: GPL-3.0-or-later
//! Process list + signals/renice. Own processes are handled directly; anything we're
//! not allowed to touch falls back to ember-helper (root, via pkexec).
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind, Users};

use super::{helper, CtlResult};

#[derive(Debug, Clone, PartialEq)]
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub cpu: f32,
    pub mem: u64,
    pub status: String,
    pub nice: Option<i32>,
    pub cmd: String,
}

pub struct Procs {
    sys: System,
    users: Users,
}

impl Default for Procs {
    fn default() -> Self {
        Self::new()
    }
}

impl Procs {
    pub fn new() -> Self {
        let kind = ProcessRefreshKind::nothing().with_cpu().with_memory().with_user(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet);
        let sys = System::new_with_specifics(RefreshKind::nothing().with_processes(kind)).expect("sysinfo unsupported on this OS");
        Self { sys, users: Users::new_with_refreshed_list().unwrap_or_else(|_| Users::new().expect("users")) }
    }

    /// Refresh and list. Call ≥ 1 s apart for meaningful CPU %.
    pub fn list(&mut self) -> Vec<ProcRow> {
        let kind = ProcessRefreshKind::nothing().with_cpu().with_memory().with_user(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet);
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get()) as f32;
        let mut rows: Vec<ProcRow> = self
            .sys
            .processes()
            .iter()
            // skip threads; they show up under their process
            .filter(|(_, p)| p.thread_kind().is_none())
            .map(|(pid, p)| {
                let pid = pid.as_u32();
                // comm is cut at 15 chars by the kernel; take the full name from argv[0]
                let comm = p.name().to_string_lossy().into_owned();
                let name = p
                    .cmd()
                    .first()
                    .and_then(|a| std::path::Path::new(a).file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|n| comm.len() >= 15 && n.starts_with(&comm))
                    .unwrap_or(comm);
                ProcRow {
                    pid,
                    name,
                    user: p.user_id().and_then(|u| self.users.get_user_by_id(u)).map_or_else(|| "?".into(), |u| u.name().to_string()),
                    // sysinfo reports per-core %, show share of the whole machine
                    cpu: p.cpu_usage() / cpus,
                    mem: p.memory(),
                    status: p.status().to_string(),
                    nice: nice_of(pid),
                    cmd: p.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" "),
                }
            })
            .collect();
        rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        rows
    }
}

/// Nice value: field 19 of /proc/<pid>/stat (after the ")" that ends the name).
pub fn parse_nice(stat: &str) -> Option<i32> {
    stat.rsplit_once(')')?.1.split_whitespace().nth(16)?.parse().ok()
}

fn nice_of(pid: u32) -> Option<i32> {
    parse_nice(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Term,
    Kill,
    Stop,
    Cont,
}

impl Signal {
    pub fn name(self) -> &'static str {
        match self {
            Signal::Term => "TERM",
            Signal::Kill => "KILL",
            Signal::Stop => "STOP",
            Signal::Cont => "CONT",
        }
    }
    fn raw(self) -> libc::c_int {
        match self {
            Signal::Term => libc::SIGTERM,
            Signal::Kill => libc::SIGKILL,
            Signal::Stop => libc::SIGSTOP,
            Signal::Cont => libc::SIGCONT,
        }
    }
}

fn refuse(pid: u32) -> CtlResult {
    if pid <= 1 || pid == std::process::id() {
        return Err("Ember won't signal init or itself.".into());
    }
    Ok(())
}

/// Blocking (may show a polkit prompt): run from a background task.
pub fn signal(pid: u32, sig: Signal) -> CtlResult {
    refuse(pid)?;
    // SAFETY: plain syscall with validated arguments
    if unsafe { libc::kill(pid as libc::pid_t, sig.raw()) } == 0 {
        return Ok(());
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) => helper::run(&["signal".into(), pid.to_string(), sig.name().into()]),
        Some(libc::ESRCH) => Err("That process already exited.".into()),
        _ => Err(std::io::Error::last_os_error().to_string()),
    }
}

/// Blocking (may show a polkit prompt): raising priority needs root.
pub fn renice(pid: u32, nice: i32) -> CtlResult {
    refuse(pid)?;
    if !(-20..=19).contains(&nice) {
        return Err("Nice must be between -20 and 19.".into());
    }
    // SAFETY: plain syscall with validated arguments
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) } == 0 {
        return Ok(());
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => helper::run(&["renice".into(), pid.to_string(), nice.to_string()]),
        Some(libc::ESRCH) => Err("That process already exited.".into()),
        _ => Err(std::io::Error::last_os_error().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_from_stat_with_spaces_in_name() {
        let stat = "1234 (Web Content (x)) S 1 1234 1234 0 -1 4194560 100 0 0 0 5 3 0 0 20 -5 12 0 100 1000 50";
        assert_eq!(parse_nice(stat), Some(-5));
        assert_eq!(parse_nice("garbage"), None);
    }

    #[test]
    fn refuses_init_and_self() {
        assert!(signal(1, Signal::Kill).is_err());
        assert!(signal(std::process::id(), Signal::Term).is_err());
        assert!(renice(1, 5).is_err());
    }

    #[test]
    fn signals_own_child() {
        let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        renice(child.id(), 10).unwrap();
        assert_eq!(nice_of(child.id()), Some(10));
        signal(child.id(), Signal::Term).unwrap();
        assert!(!child.wait().unwrap().success());
    }
}
