// SPDX-License-Identifier: GPL-3.0-or-later
//! CPU scaling controls (read here, written by ember-helper as root).
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CpuCtl {
    pub governor: Option<String>,
    pub governors: Vec<String>,
    /// intel_pstate status (active/passive) or the cpufreq driver name
    pub driver: Option<String>,
    /// true = turbo/boost off
    pub no_turbo: Option<bool>,
    /// intel_pstate max performance %, if available
    pub max_perf: Option<u8>,
    /// ACPI platform profile (firmware thermal/fan mode), e.g. "performance"
    pub thermal: Option<String>,
    pub thermals: Vec<String>,
}

fn read(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

pub fn read_state() -> CpuCtl {
    let mut c = read_from(Path::new("/sys/devices/system/cpu"));
    (c.thermal, c.thermals) = thermal();
    c
}

/// Current firmware fan/thermal mode and the available ones.
pub fn thermal() -> (Option<String>, Vec<String>) {
    read_thermal(Path::new("/sys/firmware/acpi"))
}

/// Firmware fan/thermal mode from /sys/firmware/acpi/platform_profile{,_choices}.
fn read_thermal(acpi: &Path) -> (Option<String>, Vec<String>) {
    let choices = read(acpi.join("platform_profile_choices")).map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default();
    (read(acpi.join("platform_profile")), choices)
}

/// Friendly label for a platform profile name (Lenovo/ASUS/etc. naming).
pub fn thermal_label(p: &str) -> &str {
    match p {
        "low-power" => "Quiet",
        "cool" => "Cool",
        "quiet" => "Quiet",
        "balanced" => "Balanced",
        "balanced-performance" => "Balanced+",
        "performance" => "Performance",
        "max-power" => "Extreme",
        "custom" => "Custom",
        other => other,
    }
}

fn read_from(root: &Path) -> CpuCtl {
    let pstate = root.join("intel_pstate");
    let cf = root.join("cpu0/cpufreq");
    CpuCtl {
        governor: read(cf.join("scaling_governor")),
        governors: read(cf.join("scaling_available_governors")).map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default(),
        driver: read(cf.join("scaling_driver")),
        // intel_pstate/no_turbo (1 = off), or generic cpufreq/boost (1 = on)
        no_turbo: read(pstate.join("no_turbo")).map(|v| v == "1").or_else(|| read(root.join("cpufreq/boost")).map(|v| v == "0")),
        max_perf: read(pstate.join("max_perf_pct")).and_then(|v| v.parse().ok()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_intel_pstate_and_generic_boost() {
        let root = std::env::temp_dir().join(format!("ember-cpu-{}", std::process::id()));
        fs::create_dir_all(root.join("cpu0/cpufreq")).unwrap();
        fs::create_dir_all(root.join("intel_pstate")).unwrap();
        fs::write(root.join("cpu0/cpufreq/scaling_governor"), "performance\n").unwrap();
        fs::write(root.join("cpu0/cpufreq/scaling_available_governors"), "performance powersave\n").unwrap();
        fs::write(root.join("intel_pstate/no_turbo"), "0\n").unwrap();
        fs::write(root.join("intel_pstate/max_perf_pct"), "100\n").unwrap();
        let c = read_from(&root);
        assert_eq!(c.governor.as_deref(), Some("performance"));
        assert_eq!(c.governors, ["performance", "powersave"]);
        assert_eq!((c.no_turbo, c.max_perf), (Some(false), Some(100)));

        fs::remove_dir_all(root.join("intel_pstate")).unwrap();
        fs::create_dir_all(root.join("cpufreq")).unwrap();
        fs::write(root.join("cpufreq/boost"), "0\n").unwrap();
        let c = read_from(&root);
        assert_eq!((c.no_turbo, c.max_perf), (Some(true), None));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reads_platform_profile() {
        let root = std::env::temp_dir().join(format!("ember-acpi-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        assert_eq!(read_thermal(&root), (None, vec![]));
        fs::write(root.join("platform_profile"), "performance\n").unwrap();
        fs::write(root.join("platform_profile_choices"), "low-power balanced performance max-power custom\n").unwrap();
        let (cur, all) = read_thermal(&root);
        assert_eq!(cur.as_deref(), Some("performance"));
        assert_eq!(all.len(), 5);
        assert_eq!(thermal_label("max-power"), "Extreme");
        fs::remove_dir_all(&root).unwrap();
    }
}
