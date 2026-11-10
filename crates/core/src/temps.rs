// SPDX-License-Identifier: GPL-3.0-or-later
//! Group raw sensors ("coretemp Core 8", "nvme Composite …") into readable parts.
use crate::sample::Temp;

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: &'static str,
    /// hottest sensor in the group
    pub celsius: f32,
    /// lowest critical value in the group, if any sensor reports one
    pub critical: Option<f32>,
}

pub fn part_of(label: &str) -> &'static str {
    let chip = label.split_whitespace().next().unwrap_or("").to_lowercase();
    match chip.as_str() {
        "coretemp" | "k10temp" | "zenpower" | "cpu_thermal" => "CPU",
        "nvme" | "drivetemp" => "SSD",
        "spd5118" | "jc42" => "RAM",
        "amdgpu" | "nouveau" | "nvidia" | "radeon" => "GPU",
        "iwlwifi_1" | "iwlwifi" | "ath11k_hwmon" => "Wi-Fi",
        _ => "Board",
    }
}

pub fn group(temps: &[Temp]) -> Vec<Group> {
    const ORDER: [&str; 6] = ["CPU", "GPU", "SSD", "RAM", "Wi-Fi", "Board"];
    ORDER
        .iter()
        .filter_map(|&name| {
            let mut it = temps.iter().filter(|t| part_of(&t.label) == name).peekable();
            it.peek()?;
            let (mut hot, mut crit) = (f32::MIN, None::<f32>);
            for t in it {
                hot = hot.max(t.celsius);
                crit = match (crit, t.critical) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
            Some(Group { name, celsius: hot, critical: crit })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(label: &str, c: f32, crit: Option<f32>) -> Temp {
        Temp { label: label.into(), celsius: c, critical: crit }
    }

    #[test]
    fn groups_real_laptop_labels() {
        let g = group(&[
            t("coretemp Core 8", 47.0, Some(100.0)),
            t("coretemp Package id 0", 51.0, Some(100.0)),
            t("nvme Composite SAMSUNG", 49.85, Some(84.85)),
            t("nvme Sensor 1 SAMSUNG", 51.85, None),
            t("spd5118 temp1", 50.75, Some(85.0)),
            t("acpitz temp1", 27.8, None),
        ]);
        let names: Vec<_> = g.iter().map(|g| g.name).collect();
        assert_eq!(names, ["CPU", "SSD", "RAM", "Board"]);
        assert_eq!((g[0].celsius, g[0].critical), (51.0, Some(100.0)));
        assert_eq!((g[1].celsius, g[1].critical), (51.85, Some(84.85)));
        assert_eq!(g[3].critical, None);
    }
}
