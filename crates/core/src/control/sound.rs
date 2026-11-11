// SPDX-License-Identifier: GPL-3.0-or-later
//! Output/input devices and volume through `pactl` (PipeWire's Pulse server).
use std::process::Command;

use serde::Deserialize;

use super::CtlResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Output,
    Input,
}

impl Kind {
    fn noun(self) -> &'static str {
        match self {
            Kind::Output => "sink",
            Kind::Input => "source",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub name: String,
    pub description: String,
    /// 0..=1.5 (PipeWire allows boosting past 100%)
    pub volume: f32,
    pub muted: bool,
    pub default: bool,
}

#[derive(Deserialize)]
struct Raw {
    name: String,
    #[serde(default)]
    description: String,
    mute: bool,
    volume: std::collections::HashMap<String, Channel>,
}

#[derive(Deserialize)]
struct Channel {
    value_percent: String,
}

/// Parse `pactl -f json list sinks|sources`; monitors of outputs are not inputs.
pub fn parse(json: &str, default: &str) -> Vec<Device> {
    let raw: Vec<Raw> = serde_json::from_str(json).unwrap_or_default();
    raw.into_iter()
        .filter(|r| !r.name.ends_with(".monitor"))
        .map(|r| {
            let vals: Vec<f32> = r.volume.values().filter_map(|c| c.value_percent.trim_end_matches('%').trim().parse::<f32>().ok()).collect();
            let volume = if vals.is_empty() { 0.0 } else { vals.iter().sum::<f32>() / vals.len() as f32 / 100.0 };
            Device {
                default: r.name == default,
                description: if r.description.is_empty() { r.name.clone() } else { r.description },
                name: r.name,
                volume,
                muted: r.mute,
            }
        })
        .collect()
}

fn pactl(args: &[&str]) -> CtlResult<String> {
    let out = Command::new("pactl").args(args).output().map_err(|e| format!("pactl not available: {e}"))?;
    if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).into_owned()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Blocking.
pub fn list(kind: Kind) -> CtlResult<Vec<Device>> {
    let n = kind.noun();
    let default = pactl(&[&format!("get-default-{n}")]).unwrap_or_default();
    Ok(parse(&pactl(&["-f", "json", "list", &format!("{n}s")])?, default.trim()))
}

/// pactl's own parser miscounts arguments after `--`, so names are checked instead.
fn check(name: &str) -> CtlResult {
    if name.is_empty() || name.starts_with('-') { Err("bad device name".into()) } else { Ok(()) }
}

pub fn set_volume(kind: Kind, name: &str, frac: f32) -> CtlResult {
    check(name)?;
    let pct = format!("{}%", (frac.clamp(0.0, 1.5) * 100.0).round() as u32);
    pactl(&[&format!("set-{}-volume", kind.noun()), name, &pct]).map(drop)
}

pub fn set_mute(kind: Kind, name: &str, mute: bool) -> CtlResult {
    check(name)?;
    pactl(&[&format!("set-{}-mute", kind.noun()), name, if mute { "1" } else { "0" }]).map(drop)
}

pub fn set_default(kind: Kind, name: &str) -> CtlResult {
    check(name)?;
    pactl(&[&format!("set-default-{}", kind.noun()), name]).map(drop)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_pactl_json() {
        let json = r#"[{"name":"alsa_output.analog","description":"Built-in Audio","mute":false,"volume":{"front-left":{"value":24904,"value_percent":"38%"},"front-right":{"value":24904,"value_percent":"40%"}}},
                      {"name":"alsa_output.analog.monitor","description":"Monitor","mute":false,"volume":{"mono":{"value_percent":"100%"}}},
                      {"name":"bluez_output.X","description":"","mute":true,"volume":{"mono":{"value_percent":"150 %"}}}]"#;
        let d = super::parse(json, "bluez_output.X");
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].description.as_str(), d[0].default), ("Built-in Audio", false));
        assert!((d[0].volume - 0.39).abs() < 1e-4);
        assert_eq!((d[1].description.as_str(), d[1].muted, d[1].default, d[1].volume), ("bluez_output.X", true, true, 1.5));
        assert!(super::parse("garbage", "").is_empty());
        assert!(super::set_volume(super::Kind::Output, "-x", 0.5).is_err());
    }
}
