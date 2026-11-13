// SPDX-License-Identifier: GPL-3.0-or-later
//! Firmware tuning knobs from /sys/class/firmware-attributes (e.g. Lenovo's
//! lenovo-wmi-other: CPU/GPU power and thermal limits used by the "custom" profile).
//! Read here, written by ember-helper as root.
use std::fs;
use std::path::Path;

pub const ROOT: &str = "/sys/class/firmware-attributes";

#[derive(Debug, Clone, PartialEq)]
pub struct FwAttr {
    /// firmware-attributes device dir, e.g. "lenovo-wmi-other-0"
    pub device: String,
    /// attribute dir, e.g. "ppt_pl1_spl"
    pub name: String,
    pub label: String,
    pub value: u32,
    pub default: Option<u32>,
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

impl FwAttr {
    pub fn unit(&self) -> &'static str {
        if self.name.contains("temp") {
            "°C"
        } else if self.name.contains("tau") {
            "s"
        } else {
            "W"
        }
    }
}

fn read(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn num(p: impl AsRef<Path>) -> Option<u32> {
    read(p)?.parse().ok()
}

pub fn read_all() -> Vec<FwAttr> {
    read_from(Path::new(ROOT))
}

/// Re-read attributes paired with a slider value: an attribute whose firmware value
/// hasn't changed keeps the value being edited, otherwise it snaps to the new value.
pub fn refresh(old: &[(FwAttr, u32)]) -> Vec<(FwAttr, u32)> {
    keep_edits(old, read_all())
}

fn keep_edits(old: &[(FwAttr, u32)], new: Vec<FwAttr>) -> Vec<(FwAttr, u32)> {
    new.into_iter()
        .map(|a| {
            let v = old.iter().find(|(o, _)| o.device == a.device && o.name == a.name && o.value == a.value).map_or(a.value, |(_, e)| *e);
            (a, v)
        })
        .collect()
}

/// Integer attributes with a usable min..max range; everything else is skipped.
fn read_from(root: &Path) -> Vec<FwAttr> {
    let mut out = vec![];
    let Ok(devs) = fs::read_dir(root) else { return out };
    for dev in devs.flatten() {
        let Ok(attrs) = fs::read_dir(dev.path().join("attributes")) else { continue };
        for a in attrs.flatten() {
            let p = a.path();
            if read(p.join("type")).as_deref() != Some("integer") {
                continue;
            }
            let (Some(value), Some(min), Some(max)) = (num(p.join("current_value")), num(p.join("min_value")), num(p.join("max_value"))) else { continue };
            if min >= max {
                continue;
            }
            let name = a.file_name().to_string_lossy().into_owned();
            let label = read(p.join("display_name")).map(|l| l.trim_start_matches("Set the ").to_string()).unwrap_or_else(|| name.clone());
            out.push(FwAttr {
                device: dev.file_name().to_string_lossy().into_owned(),
                label: label[..1].to_uppercase() + &label[1..],
                name,
                value,
                default: num(p.join("default_value")),
                min,
                max,
                // firmware reports 0 for "any integer"
                step: num(p.join("scalar_increment")).filter(|&s| s > 0).unwrap_or(1),
            });
        }
    }
    out.sort_by(|a, b| (&a.device, &a.name).cmp(&(&b.device, &b.name)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_integer_attributes_only() {
        let root = std::env::temp_dir().join(format!("ember-fw-{}", std::process::id()));
        let a = root.join("lenovo-wmi-other-0/attributes/ppt_pl1_spl");
        let b = root.join("lenovo-wmi-other-0/attributes/some_enum");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        for (f, v) in [
            ("type", "integer"),
            ("current_value", "60"),
            ("default_value", "60"),
            ("min_value", "50"),
            ("max_value", "95"),
            ("scalar_increment", "0"),
            ("display_name", "Set the CPU sustained power limit"),
        ] {
            fs::write(a.join(f), format!("{v}\n")).unwrap();
        }
        fs::write(b.join("type"), "enumeration\n").unwrap();
        let v = read_from(&root);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].value, v[0].min, v[0].max, v[0].step, v[0].default), (60, 50, 95, 1, Some(60)));
        assert_eq!(v[0].label, "CPU sustained power limit");
        assert_eq!(v[0].unit(), "W");

        // an edit survives a refresh while the firmware value is unchanged, not after it changes
        let edited = vec![(v[0].clone(), 70)];
        assert_eq!(keep_edits(&edited, v.clone())[0].1, 70);
        let mut changed = v.clone();
        changed[0].value = 80;
        assert_eq!(keep_edits(&edited, changed)[0].1, 80);
        fs::remove_dir_all(&root).unwrap();
    }
}
