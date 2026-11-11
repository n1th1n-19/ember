// SPDX-License-Identifier: GPL-3.0-or-later
//! system76-power over D-Bus: power profile, graphics mode, battery charge limits.
//! All optional: on machines without the daemon, `read()` returns None and the UI hides it.
use super::CtlResult;

#[zbus::proxy(
    interface = "com.system76.PowerDaemon",
    default_service = "com.system76.PowerDaemon",
    default_path = "/com/system76/PowerDaemon"
)]
trait PowerDaemon {
    fn get_profile(&self) -> zbus::Result<String>;
    #[zbus(allow_interactive_auth)]
    fn battery(&self) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn balanced(&self) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn performance(&self) -> zbus::Result<()>;
    fn get_graphics(&self) -> zbus::Result<String>;
    fn get_switchable(&self) -> zbus::Result<bool>;
    #[zbus(allow_interactive_auth)]
    fn set_graphics(&self, vendor: &str) -> zbus::Result<()>;
    fn get_charge_thresholds(&self) -> zbus::Result<(u8, u8)>;
    #[zbus(allow_interactive_auth)]
    fn set_charge_thresholds(&self, thresholds: &(u8, u8)) -> zbus::Result<()>;
    fn get_charge_profiles(&self) -> zbus::Result<Vec<(String, String, String, u8, u8)>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Battery,
    Balanced,
    Performance,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::Battery, Profile::Balanced, Profile::Performance];
    pub fn name(self) -> &'static str {
        match self {
            Profile::Battery => "Battery",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Graphics modes system76-power accepts.
pub const GRAPHICS: [&str; 4] = ["integrated", "hybrid", "nvidia", "compute"];

#[derive(Debug, Clone, PartialEq)]
pub struct ChargeProfile {
    pub id: String,
    pub title: String,
    pub description: String,
    pub start: u8,
    pub end: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerState {
    pub profile: Option<Profile>,
    pub graphics: Option<String>,
    pub switchable: bool,
    /// (start, end) charge thresholds, None if the hardware doesn't support them
    pub thresholds: Option<(u8, u8)>,
    pub charge_profiles: Vec<ChargeProfile>,
}

async fn proxy() -> zbus::Result<PowerDaemonProxy<'static>> {
    PowerDaemonProxy::new(&zbus::Connection::system().await?).await
}

fn err(e: zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(name, msg, _) => msg.unwrap_or_else(|| name.to_string()),
        e => e.to_string(),
    }
}

/// None when system76-power isn't running.
pub async fn read() -> Option<PowerState> {
    let p = proxy().await.ok()?;
    let profile = p.get_profile().await.ok()?;
    Some(PowerState {
        profile: Profile::parse(&profile),
        graphics: p.get_graphics().await.ok(),
        switchable: p.get_switchable().await.unwrap_or(false),
        thresholds: p.get_charge_thresholds().await.ok(),
        charge_profiles: p
            .get_charge_profiles()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(id, title, description, start, end)| ChargeProfile { id, title, description, start, end })
            .collect(),
    })
}

pub async fn set_profile(profile: Profile) -> CtlResult {
    let p = proxy().await.map_err(err)?;
    match profile {
        Profile::Battery => p.battery().await,
        Profile::Balanced => p.balanced().await,
        Profile::Performance => p.performance().await,
    }
    .map_err(err)
}

/// Takes effect after a reboot.
pub async fn set_graphics(mode: &str) -> CtlResult {
    if !GRAPHICS.contains(&mode) {
        return Err(format!("unknown graphics mode {mode}"));
    }
    proxy().await.map_err(err)?.set_graphics(mode).await.map_err(err)
}

pub async fn set_thresholds(start: u8, end: u8) -> CtlResult {
    if !(start < end && end <= 100) {
        return Err("charge start must be below the end, and the end at most 100%".into());
    }
    proxy().await.map_err(err)?.set_charge_thresholds(&(start, end)).await.map_err(err)
}
