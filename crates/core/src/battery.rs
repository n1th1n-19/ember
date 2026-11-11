// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub name: String,
    pub percent: Option<f32>,
    pub status: Option<String>,
    /// energy_full / energy_full_design, 0..=1
    pub health: Option<f32>,
    /// Watts, positive = draw/charge rate as reported
    pub power_w: Option<f32>,
    pub cycles: Option<u32>,
}

/// Parse a power_supply `uevent` file. Returns None if it isn't a battery.
pub fn parse_uevent(text: &str) -> Option<Battery> {
    let kv: HashMap<&str, &str> = text.lines().filter_map(|l| l.strip_prefix("POWER_SUPPLY_")?.split_once('=')).collect();
    if kv.get("TYPE") != Some(&"Battery") || kv.get("PRESENT") == Some(&"0") {
        return None;
    }
    let num = |k: &str| kv.get(k).and_then(|v| v.trim().parse::<f64>().ok());
    let ratio = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) if b > 0.0 => Some((a / b) as f32),
        _ => None,
    };
    let health = ratio(num("ENERGY_FULL"), num("ENERGY_FULL_DESIGN")).or_else(|| ratio(num("CHARGE_FULL"), num("CHARGE_FULL_DESIGN")));
    // µW, or µA × µV
    let power_uw = num("POWER_NOW").or_else(|| Some(num("CURRENT_NOW")? * num("VOLTAGE_NOW")? / 1e6));
    Some(Battery {
        name: kv.get("NAME").unwrap_or(&"battery").to_string(),
        percent: num("CAPACITY").map(|v| v as f32),
        status: kv.get("STATUS").map(|s| s.to_string()),
        health,
        power_w: power_uw.map(|uw| (uw / 1e6).abs() as f32),
        cycles: num("CYCLE_COUNT").map(|v| v as u32),
    })
}

/// All batteries under /sys/class/power_supply (BAT0, BAT1, CMB0…).
pub fn read_all() -> Vec<Battery> {
    read_dir(Path::new("/sys/class/power_supply"))
}

fn read_dir(root: &Path) -> Vec<Battery> {
    let Ok(entries) = fs::read_dir(root) else { return vec![] };
    entries.flatten().filter_map(|e| parse_uevent(&fs::read_to_string(e.path().join("uevent")).ok()?)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAT1: &str = "POWER_SUPPLY_NAME=BAT1\nPOWER_SUPPLY_TYPE=Battery\nPOWER_SUPPLY_STATUS=Not charging\nPOWER_SUPPLY_PRESENT=1\nPOWER_SUPPLY_CYCLE_COUNT=76\nPOWER_SUPPLY_VOLTAGE_NOW=16523000\nPOWER_SUPPLY_POWER_NOW=8500000\nPOWER_SUPPLY_ENERGY_FULL_DESIGN=60000000\nPOWER_SUPPLY_ENERGY_FULL=57220000\nPOWER_SUPPLY_CAPACITY=78\n";

    #[test]
    fn energy_battery() {
        let b = parse_uevent(BAT1).unwrap();
        assert_eq!(b.name, "BAT1");
        assert_eq!(b.percent, Some(78.0));
        assert_eq!(b.cycles, Some(76));
        assert!((b.health.unwrap() - 0.9537).abs() < 1e-3);
        assert!((b.power_w.unwrap() - 8.5).abs() < 1e-3);
    }

    #[test]
    fn charge_battery_and_current_power() {
        let t = "POWER_SUPPLY_TYPE=Battery\nPOWER_SUPPLY_CHARGE_FULL=4000000\nPOWER_SUPPLY_CHARGE_FULL_DESIGN=5000000\nPOWER_SUPPLY_CURRENT_NOW=-1000000\nPOWER_SUPPLY_VOLTAGE_NOW=12000000\n";
        let b = parse_uevent(t).unwrap();
        assert_eq!(b.health, Some(0.8));
        assert_eq!(b.power_w, Some(12.0));
        assert_eq!(b.percent, None);
    }

    #[test]
    fn non_battery_or_absent_skipped() {
        assert!(parse_uevent("POWER_SUPPLY_TYPE=Mains\nPOWER_SUPPLY_ONLINE=1").is_none());
        assert!(parse_uevent("POWER_SUPPLY_TYPE=Battery\nPOWER_SUPPLY_PRESENT=0").is_none());
    }
}
