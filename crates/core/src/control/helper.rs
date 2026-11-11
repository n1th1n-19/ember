// SPDX-License-Identifier: GPL-3.0-or-later
//! Run `ember-helper` as root through pkexec (polkit shows the password prompt).
use std::path::PathBuf;
use std::process::Command;

use super::CtlResult;

pub const INSTALLED: &str = "/usr/libexec/ember/ember-helper";

/// Installed helper, or the one next to the running binary (dev builds).
pub fn path() -> PathBuf {
    let installed = PathBuf::from(INSTALLED);
    if installed.exists() {
        return installed;
    }
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("ember-helper"))).unwrap_or(installed)
}

/// Blocking: waits for the polkit prompt. Call from a background task.
pub fn run(args: &[String]) -> CtlResult {
    let out = Command::new("pkexec").arg(path()).args(args).output().map_err(|e| format!("couldn't start pkexec: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(()),
        // pkexec: 126 = dialog dismissed / not authorized, 127 = auth failed
        Some(126) | Some(127) => Err("Not authorized (password prompt was cancelled or failed).".into()),
        _ => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
    }
}
