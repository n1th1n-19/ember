// SPDX-License-Identifier: GPL-3.0-or-later
//! Brightness (cosmic-settings-daemon, logind for the keyboard light) and session
//! actions (lock, log out, suspend, restart, shut down).
use super::CtlResult;

fn err(e: zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(name, msg, _) => msg.unwrap_or_else(|| name.to_string()),
        e => e.to_string(),
    }
}

#[zbus::proxy(interface = "com.system76.CosmicSettingsDaemon", default_service = "com.system76.CosmicSettingsDaemon", default_path = "/com/system76/CosmicSettingsDaemon")]
trait SettingsDaemon {
    #[zbus(property)]
    fn display_brightness(&self) -> zbus::Result<i32>;
    #[zbus(property)]
    fn set_display_brightness(&self, v: i32) -> zbus::Result<()>;
    #[zbus(property)]
    fn max_display_brightness(&self) -> zbus::Result<i32>;
}

#[zbus::proxy(interface = "org.freedesktop.login1.Session", default_service = "org.freedesktop.login1", default_path = "/org/freedesktop/login1/session/auto")]
trait Session {
    fn set_brightness(&self, subsystem: &str, name: &str, brightness: u32) -> zbus::Result<()>;
    fn lock(&self) -> zbus::Result<()>;
}

#[zbus::proxy(interface = "org.freedesktop.login1.Manager", default_service = "org.freedesktop.login1", default_path = "/org/freedesktop/login1")]
trait Login {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<zbus::OwnedFd>;
    #[zbus(allow_interactive_auth)]
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Brightness {
    /// 0..=1, None when there's no controllable backlight
    pub display: Option<f32>,
    pub keyboard: Option<f32>,
}

const KBD: &str = "/sys/class/leds/platform::kbd_backlight";

fn kbd() -> Option<(u32, u32)> {
    let r = |f: &str| std::fs::read_to_string(format!("{KBD}/{f}")).ok()?.trim().parse::<u32>().ok();
    Some((r("brightness")?, r("max_brightness")?.max(1)))
}

pub async fn read_brightness() -> Brightness {
    let display = async {
        let p = SettingsDaemonProxy::new(&zbus::Connection::session().await.ok()?).await.ok()?;
        let (v, max) = (p.display_brightness().await.ok()?, p.max_display_brightness().await.ok()?);
        (v >= 0 && max > 0).then(|| v as f32 / max as f32)
    }
    .await;
    Brightness { display, keyboard: kbd().map(|(v, m)| v as f32 / m as f32) }
}

pub async fn set_display(frac: f32) -> CtlResult {
    let p = SettingsDaemonProxy::new(&zbus::Connection::session().await.map_err(err)?).await.map_err(err)?;
    let max = p.max_display_brightness().await.map_err(err)?;
    // never fully black: 5% floor so the screen stays usable
    let v = ((frac.clamp(0.05, 1.0)) * max as f32).round() as i32;
    p.set_display_brightness(v).await.map_err(err)
}

pub async fn set_keyboard(frac: f32) -> CtlResult {
    let (_, max) = kbd().ok_or("No keyboard backlight.")?;
    let v = (frac.clamp(0.0, 1.0) * max as f32).round() as u32;
    let s = SessionProxy::new(&zbus::Connection::system().await.map_err(err)?).await.map_err(err)?;
    s.set_brightness("leds", "platform::kbd_backlight", v).await.map_err(err)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    Lock,
    LogOut,
    Suspend,
    Restart,
    ShutDown,
}

impl SessionAction {
    pub fn name(self) -> &'static str {
        match self {
            SessionAction::Lock => "Lock",
            SessionAction::LogOut => "Log out",
            SessionAction::Suspend => "Suspend",
            SessionAction::Restart => "Restart",
            SessionAction::ShutDown => "Shut down",
        }
    }
    /// Ends the session or turns the machine off: needs a second click.
    pub fn needs_confirm(self) -> bool {
        matches!(self, SessionAction::LogOut | SessionAction::Restart | SessionAction::ShutDown)
    }
}

pub async fn session(action: SessionAction) -> CtlResult {
    match action {
        SessionAction::Lock => SessionProxy::new(&zbus::Connection::system().await.map_err(err)?).await.map_err(err)?.lock().await.map_err(err),
        SessionAction::LogOut => {
            let conn = zbus::Connection::session().await.map_err(err)?;
            let p = zbus::Proxy::new(&conn, "com.system76.CosmicSession", "/com/system76/CosmicSession", "com.system76.CosmicSession").await.map_err(err)?;
            p.call_method("Exit", &()).await.map(drop).map_err(err)
        }
        a => {
            let l = LoginProxy::new(&zbus::Connection::system().await.map_err(err)?).await.map_err(err)?;
            match a {
                SessionAction::Suspend => l.suspend(true).await,
                SessionAction::Restart => l.reboot(true).await,
                _ => l.power_off(true).await,
            }
            .map_err(err)
        }
    }
}

/// Keep the machine awake: blocks idle (screen blanking/lock) and sleep for as long as
/// the returned fd is open. Drop it to allow sleep again.
pub async fn inhibit_sleep() -> CtlResult<zbus::OwnedFd> {
    let l = LoginProxy::new(&zbus::Connection::system().await.map_err(err)?).await.map_err(err)?;
    l.inhibit("idle:sleep", "Ember", "Keep awake is on", "block").await.map_err(err)
}

/// Held keep-awake inhibitor (re-exported so UI crates needn't depend on zbus).
pub type Inhibitor = zbus::OwnedFd;
