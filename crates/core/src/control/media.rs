// SPDX-License-Identifier: GPL-3.0-or-later
//! Now playing + transport controls for any MPRIS player (browsers, Spotify, mpv…).
use std::collections::HashMap;

use zbus::OwnedVariant as OwnedValue;

use super::CtlResult;

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub bus: String,
    pub app: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    PlayPause,
    Next,
    Previous,
}

fn err(e: zbus::Error) -> String {
    e.to_string()
}

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";

fn text(meta: &HashMap<String, OwnedValue>, key: &str) -> String {
    let get = || meta.get(key).and_then(|v| v.try_clone().ok());
    // titles are strings, artists are string arrays
    get().and_then(|v| String::try_from(v).ok()).or_else(|| get().and_then(|v| Vec::<String>::try_from(v).ok()).map(|a| a.join(", "))).unwrap_or_default()
}

/// The player to show: a playing one first, else the first one found.
pub async fn read() -> CtlResult<Option<Player>> {
    let conn = zbus::Connection::session().await.map_err(err)?;
    let dbus = zbus::fdo::DBusProxy::new(&conn).await.map_err(err)?;
    let names = dbus.list_names().await.map_err(|e| err(e.into()))?;
    let mut players = vec![];
    for name in names.iter().map(|n| n.to_string()).filter(|n| n.starts_with(PREFIX)) {
        let Ok(p) = zbus::Proxy::new(&conn, name.as_str(), PATH, "org.mpris.MediaPlayer2.Player").await else { continue };
        let status: String = p.get_property("PlaybackStatus").await.unwrap_or_default();
        let meta: HashMap<String, OwnedValue> = p.get_property("Metadata").await.unwrap_or_default();
        let app = match zbus::Proxy::new(&conn, name.as_str(), PATH, "org.mpris.MediaPlayer2").await {
            Ok(r) => r.get_property::<String>("Identity").await.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let app = if app.is_empty() { name.trim_start_matches(PREFIX).split('.').next().unwrap_or("Player").to_string() } else { app };
        players.push(Player { bus: name, app, title: text(&meta, "xesam:title"), artist: text(&meta, "xesam:artist"), playing: status == "Playing" });
    }
    players.sort_by_key(|p| !p.playing);
    Ok(players.into_iter().next())
}

pub async fn control(bus: &str, cmd: Cmd) -> CtlResult {
    if !bus.starts_with(PREFIX) || bus.contains('/') {
        return Err("not an MPRIS player".into());
    }
    let conn = zbus::Connection::session().await.map_err(err)?;
    let p = zbus::Proxy::new(&conn, bus, PATH, "org.mpris.MediaPlayer2.Player").await.map_err(err)?;
    let m = match cmd {
        Cmd::PlayPause => "PlayPause",
        Cmd::Next => "Next",
        Cmd::Previous => "Previous",
    };
    p.call_method(m, &()).await.map(drop).map_err(err)
}
