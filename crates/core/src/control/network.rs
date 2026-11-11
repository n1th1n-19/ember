// SPDX-License-Identifier: GPL-3.0-or-later
//! Network status and Wi-Fi through `nmcli` (NetworkManager), airplane mode via rfkill.
use std::process::Command;

use super::CtlResult;

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub device: String,
    pub kind: String,
    pub connection: String,
    pub ipv4: Option<String>,
    /// Mb/s for wired links
    pub speed: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wifi {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    pub in_use: bool,
    /// WPA3-only network (needs SAE instead of a PSK)
    pub wpa3: bool,
    /// a saved connection exists, so it can be joined with one click
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetState {
    pub links: Vec<Link>,
    pub wifi_enabled: bool,
    pub airplane: bool,
    pub networks: Vec<Wifi>,
    /// Wi-Fi interface name (e.g. wlp8s0), if the machine has one
    pub wifi_device: Option<String>,
}

/// Split one `nmcli -t` line on unescaped ':' and unescape `\:` / `\\`.
pub fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut esc = false;
    for c in line.chars() {
        match (esc, c) {
            (true, c) => {
                out.last_mut().unwrap().push(c);
                esc = false;
            }
            (false, '\\') => esc = true,
            (false, ':') => out.push(String::new()),
            (false, c) => out.last_mut().unwrap().push(c),
        }
    }
    out
}

fn nmcli(args: &[&str]) -> CtlResult<String> {
    let out = Command::new("nmcli").args(args).output().map_err(|e| format!("nmcli not available: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("Error: ").to_string())
    }
}

/// Parse `nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list`: strongest entry per SSID.
pub fn parse_wifi(out: &str, known: &[String]) -> Vec<Wifi> {
    let mut nets: Vec<Wifi> = vec![];
    for l in out.lines() {
        let f = fields(l);
        let (Some(ssid), Some(sig)) = (f.get(1), f.get(2)) else { continue };
        if ssid.is_empty() {
            continue; // hidden network
        }
        let w = Wifi {
            ssid: ssid.clone(),
            signal: sig.parse().unwrap_or(0),
            secure: f.get(3).is_some_and(|s| !s.is_empty() && s != "--"),
            wpa3: f.get(3).is_some_and(|s| s.contains("WPA3") && !s.contains("WPA2")),
            in_use: f.first().is_some_and(|s| s.trim() == "*"),
            known: known.contains(ssid),
        };
        match nets.iter_mut().find(|n| n.ssid == w.ssid) {
            Some(n) => {
                n.in_use |= w.in_use;
                n.signal = n.signal.max(w.signal);
            }
            None => nets.push(w),
        }
    }
    nets.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.known.cmp(&a.known)).then(b.signal.cmp(&a.signal)));
    nets
}

/// Blocking (spawns nmcli/rfkill).
pub fn read() -> CtlResult<NetState> {
    let mut st = NetState::default();
    for l in nmcli(&["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "dev"])?.lines() {
        let f = fields(l);
        let (dev, kind, state, conn) =
            (f[0].clone(), f.get(1).cloned().unwrap_or_default(), f.get(2).cloned().unwrap_or_default(), f.get(3).cloned().unwrap_or_default());
        if kind == "wifi" && st.wifi_device.is_none() {
            st.wifi_device = Some(dev.clone());
        }
        // real links only: skip loopback, bridges, VPN tunnels managed elsewhere
        if state == "connected" && (kind == "ethernet" || kind == "wifi") {
            let ipv4 = nmcli(&["-g", "IP4.ADDRESS", "dev", "show", &dev])
                .ok()
                .and_then(|s| s.lines().next().map(|a| a.split('/').next().unwrap_or(a).to_string()))
                .filter(|s| !s.is_empty());
            let speed = std::fs::read_to_string(format!("/sys/class/net/{dev}/speed"))
                .ok()
                .and_then(|s| s.trim().parse::<i64>().ok())
                .filter(|&s| s > 0)
                .map(|s| s as u32);
            st.links.push(Link { device: dev, kind, connection: conn, ipv4, speed });
        }
    }
    st.wifi_enabled = nmcli(&["radio", "wifi"]).map(|s| s.trim() == "enabled").unwrap_or(false);
    st.airplane = Command::new("rfkill")
        .args(["-n", "-o", "SOFT"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .is_some_and(|s| !s.trim().is_empty() && s.lines().all(|l| l.trim() == "blocked"));
    if st.wifi_enabled {
        let known: Vec<String> = nmcli(&["-t", "-f", "NAME,TYPE", "con", "show"])?
            .lines()
            .map(fields)
            .filter(|f| f.get(1).is_some_and(|t| t == "802-11-wireless"))
            .map(|f| f[0].clone())
            .collect();
        st.networks = parse_wifi(&nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list", "--rescan", "no"]).unwrap_or_default(), &known);
    }
    Ok(st)
}

pub fn set_wifi(on: bool) -> CtlResult {
    nmcli(&["radio", "wifi", if on { "on" } else { "off" }]).map(drop)
}

pub fn set_airplane(on: bool) -> CtlResult {
    let out = Command::new("rfkill").args([if on { "block" } else { "unblock" }, "all"]).output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Join a saved network (no password prompt needed).
pub fn connect_known(ssid: &str) -> CtlResult {
    nmcli(&["con", "up", "id", ssid]).map(drop)
}

/// Join a network that has no saved profile. The password travels to NetworkManager over
/// D-Bus (never on a command line, where other users could read it in `ps`). NM saves the
/// profile, so next time it's a one-click known network.
pub async fn connect_new(device: &str, ssid: &str, password: Option<&str>, wpa3: bool) -> CtlResult {
    use std::collections::HashMap;
    use zbus::ObjectPath;
    use zbus::Variant as Value;
    let err = |e: zbus::Error| match e {
        zbus::Error::MethodError(n, m, _) => m.unwrap_or_else(|| n.to_string()),
        e => e.to_string(),
    };
    if password.is_some_and(|p| p.len() < 8 || p.len() > 63) {
        return Err("Wi-Fi passwords are 8 to 63 characters.".into());
    }
    let conn = zbus::Connection::system().await.map_err(err)?;
    let nm =
        zbus::Proxy::new(&conn, "org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager").await.map_err(err)?;
    let dev: zbus::OwnedObjectPath = nm.call("GetDeviceByIpIface", &(device,)).await.map_err(err)?;

    let mut settings: HashMap<&str, HashMap<&str, Value>> = HashMap::new();
    settings.insert("connection", HashMap::from([("id", Value::from(ssid)), ("type", Value::from("802-11-wireless"))]));
    settings.insert("802-11-wireless", HashMap::from([("ssid", Value::from(ssid.as_bytes().to_vec())), ("mode", Value::from("infrastructure"))]));
    if let Some(pw) = password {
        let mgmt = if wpa3 { "sae" } else { "wpa-psk" };
        settings.insert("802-11-wireless-security", HashMap::from([("key-mgmt", Value::from(mgmt)), ("psk", Value::from(pw))]));
    }
    let root = ObjectPath::try_from("/").map_err(|e| e.to_string())?;
    let _: (zbus::OwnedObjectPath, zbus::OwnedObjectPath) = nm.call("AddAndActivateConnection", &(settings, &dev, root)).await.map_err(err)?;
    Ok(())
}

/// Forget a saved network (used when a password turned out wrong).
pub fn forget(ssid: &str) -> CtlResult {
    nmcli(&["con", "delete", "id", ssid]).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nmcli_terse_and_wifi() {
        assert_eq!(fields(r"Wired connection 2:802-3-ethernet"), ["Wired connection 2", "802-3-ethernet"]);
        assert_eq!(fields(r"my\:net:x"), ["my:net", "x"]);
        let out = " :JioFiber-4G:62:WPA2\n*:Home:80:WPA2\n :JioFiber-4G:27:WPA2\n ::15:WPA2\n :Cafe:40:\n";
        let w = parse_wifi(out, &["JioFiber-4G".to_string()]);
        assert_eq!(
            w.iter().map(|n| (n.ssid.as_str(), n.signal, n.in_use, n.known, n.secure)).collect::<Vec<_>>(),
            [("Home", 80, true, false, true), ("JioFiber-4G", 62, false, true, true), ("Cafe", 40, false, false, false)]
        );
    }
}
