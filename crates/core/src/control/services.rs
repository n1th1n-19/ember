// SPDX-License-Identifier: GPL-3.0-or-later
//! systemd units over D-Bus (system bus asks polkit; the user bus needs no auth).
use zbus::OwnedObjectPath;

use super::CtlResult;

#[zbus::proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait Manager {
    #[allow(clippy::type_complexity)]
    fn list_units(&self) -> zbus::Result<Vec<(String, String, String, String, String, String, OwnedObjectPath, u32, String, OwnedObjectPath)>>;
    fn get_unit_file_state(&self, file: &str) -> zbus::Result<String>;
    #[zbus(allow_interactive_auth)]
    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn enable_unit_files(&self, files: &[&str], runtime: bool, force: bool) -> zbus::Result<(bool, Vec<(String, String, String)>)>;
    #[zbus(allow_interactive_auth)]
    fn disable_unit_files(&self, files: &[&str], runtime: bool) -> zbus::Result<Vec<(String, String, String)>>;
    #[zbus(allow_interactive_auth)]
    fn reload(&self) -> zbus::Result<()>;
}

/// Undo systemd's unit-name escaping (`\x2d` → `-`) for display.
pub fn unescape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(i) = rest.find("\\x") {
        out.push_str(&rest[..i]);
        match rest.get(i + 2..i + 4).and_then(|h| u8::from_str_radix(h, 16).ok()) {
            Some(b) => {
                out.push(b as char);
                rest = &rest[i + 4..];
            }
            None => {
                out.push('\\');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn unescapes_unit_names() {
        assert_eq!(super::unescape(r"app-gnome\x2dkeyring\x2dssh@autostart.service"), "app-gnome-keyring-ssh@autostart.service");
        assert_eq!(super::unescape(r"plain.service"), "plain.service");
        assert_eq!(super::unescape(r"bad\xZZ.service"), r"bad\xZZ.service");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub name: String,
    pub description: String,
    pub active: String,
    pub sub: String,
    /// enabled / disabled / static / masked / ...
    pub file_state: Option<String>,
    pub scope: Scope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

async fn manager(scope: Scope) -> zbus::Result<ManagerProxy<'static>> {
    let conn = match scope {
        Scope::System => zbus::Connection::system().await?,
        Scope::User => zbus::Connection::session().await?,
    };
    ManagerProxy::new(&conn).await
}

fn err(e: zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(name, msg, _) => msg.unwrap_or_else(|| name.to_string()),
        e => e.to_string(),
    }
}

/// Loaded `.service` units for one scope, sorted by name.
pub async fn list(scope: Scope) -> CtlResult<Vec<Unit>> {
    let m = manager(scope).await.map_err(err)?;
    let mut units = Vec::new();
    for (name, description, load, active, sub, ..) in m.list_units().await.map_err(err)? {
        if !name.ends_with(".service") || load != "loaded" {
            continue;
        }
        let file_state = m.get_unit_file_state(&name).await.ok();
        units.push(Unit { name, description, active, sub, file_state, scope });
    }
    units.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(units)
}

pub async fn act(scope: Scope, name: &str, action: Action) -> CtlResult {
    if !name.ends_with(".service") || name.contains('/') {
        return Err(format!("{name} is not a service unit"));
    }
    let m = manager(scope).await.map_err(err)?;
    match action {
        Action::Start => m.start_unit(name, "replace").await.map(drop),
        Action::Stop => m.stop_unit(name, "replace").await.map(drop),
        Action::Restart => m.restart_unit(name, "replace").await.map(drop),
        Action::Enable => match m.enable_unit_files(&[name], false, false).await {
            Ok(_) => m.reload().await,
            Err(e) => Err(e),
        },
        Action::Disable => match m.disable_unit_files(&[name], false).await {
            Ok(_) => m.reload().await,
            Err(e) => Err(e),
        },
    }
    .map_err(err)
}
