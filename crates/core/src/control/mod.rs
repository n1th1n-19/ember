// SPDX-License-Identifier: GPL-3.0-or-later
//! System controls. Everything that needs root goes through `ember-helper` via pkexec;
//! D-Bus services (system76-power, systemd) do their own polkit checks.
pub mod bluetooth;
pub mod cpu;
pub mod desktop;
pub mod helper;
pub mod network;
pub mod power;
pub mod procs;
pub mod services;
pub mod sound;

/// Control errors are shown to the user as-is, so they're plain sentences.
pub type CtlResult<T = ()> = Result<T, String>;
