// SPDX-License-Identifier: GPL-3.0-or-later
//! Bluetooth through BlueZ on the system bus.
use std::collections::HashMap;

use zbus::OwnedVariant as OwnedValue;

use super::CtlResult;

#[derive(Debug, Clone, PartialEq)]
pub struct BtDevice {
    pub path: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
    /// freedesktop icon name BlueZ reports ("audio-headset", "input-mouse"…)
    pub icon: Option<String>,
    pub battery: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BtState {
    /// adapter object path, None = no Bluetooth hardware
    pub adapter: Option<String>,
    pub powered: bool,
    pub devices: Vec<BtDevice>,
}

fn err(e: zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(name, msg, _) => msg.unwrap_or_else(|| name.to_string()),
        e => e.to_string(),
    }
}

fn get<T: TryFrom<OwnedValue>>(props: &HashMap<String, OwnedValue>, key: &str) -> Option<T> {
    props.get(key).and_then(|v| T::try_from(v.try_clone().ok()?).ok())
}

pub async fn read() -> CtlResult<BtState> {
    let conn = zbus::Connection::system().await.map_err(err)?;
    let om = zbus::fdo::ObjectManagerProxy::builder(&conn).destination("org.bluez").path("/").build().await.map_err(err)?;
    let objects = om.get_managed_objects().await.map_err(|e| err(e.into()))?;
    let mut st = BtState::default();
    for (path, ifaces) in objects {
        let ifaces: HashMap<String, HashMap<String, OwnedValue>> = ifaces.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        if let Some(a) = ifaces.get("org.bluez.Adapter1")
            && st.adapter.is_none()
        {
            st.adapter = Some(path.to_string());
            st.powered = get(a, "Powered").unwrap_or(false);
        }
        if let Some(d) = ifaces.get("org.bluez.Device1") {
            let paired: bool = get(d, "Paired").unwrap_or(false);
            if !paired {
                continue; // only devices the user has set up
            }
            st.devices.push(BtDevice {
                path: path.to_string(),
                name: get::<String>(d, "Alias").or_else(|| get(d, "Name")).unwrap_or_else(|| "Unknown device".into()),
                connected: get(d, "Connected").unwrap_or(false),
                paired,
                icon: get(d, "Icon"),
                battery: ifaces.get("org.bluez.Battery1").and_then(|b| get(b, "Percentage")),
            });
        }
    }
    st.devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
    Ok(st)
}

fn check_path(path: &str) -> CtlResult {
    if path.starts_with("/org/bluez/") && path.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'_') {
        Ok(())
    } else {
        Err("bad Bluetooth object path".into())
    }
}

pub async fn set_powered(adapter: &str, on: bool) -> CtlResult {
    check_path(adapter)?;
    let conn = zbus::Connection::system().await.map_err(err)?;
    let p = zbus::Proxy::new(&conn, "org.bluez", adapter, "org.bluez.Adapter1").await.map_err(err)?;
    p.set_property("Powered", on).await.map_err(|e| err(e.into()))
}

/// Connecting can take several seconds (BlueZ waits for the device).
pub async fn set_connected(device: &str, on: bool) -> CtlResult {
    check_path(device)?;
    let conn = zbus::Connection::system().await.map_err(err)?;
    let p = zbus::Proxy::new(&conn, "org.bluez", device, "org.bluez.Device1").await.map_err(err)?;
    p.call_method(if on { "Connect" } else { "Disconnect" }, &()).await.map(drop).map_err(err)
}

#[cfg(test)]
mod tests {
    #[test]
    fn object_paths_validated() {
        assert!(super::check_path("/org/bluez/hci0/dev_3C_B0_ED_C1_DF_CC").is_ok());
        assert!(super::check_path("/org/freedesktop/login1").is_err());
        assert!(super::check_path("/org/bluez/../x").is_err());
    }
}
