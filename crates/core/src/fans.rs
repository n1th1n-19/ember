// SPDX-License-Identifier: GPL-3.0-or-later
//! Fans from hwmon. Many laptops expose none — then the list is empty and the UI says so.
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct Fan {
    pub chip: String,
    pub label: String,
    pub rpm: Option<u32>,
    /// Some(path) if a writable-by-root pwm node exists for this fan.
    pub pwm: Option<String>,
}

pub fn read_all() -> Vec<Fan> {
    read_root(Path::new("/sys/class/hwmon"))
}

fn read_root(root: &Path) -> Vec<Fan> {
    let Ok(chips) = fs::read_dir(root) else { return vec![] };
    let mut fans = vec![];
    for chip in chips.flatten().map(|e| e.path()) {
        let read = |f: &str| fs::read_to_string(chip.join(f)).ok().map(|s| s.trim().to_string());
        let chip_name = read("name").unwrap_or_default();
        let Ok(files) = fs::read_dir(&chip) else { continue };
        for f in files.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            let Some(idx) = name.strip_prefix("fan").and_then(|s| s.strip_suffix("_input")) else {
                continue;
            };
            let pwm = format!("pwm{idx}");
            fans.push(Fan {
                chip: chip_name.clone(),
                label: read(&format!("fan{idx}_label")).unwrap_or_else(|| format!("Fan {idx}")),
                rpm: read(&name).and_then(|v| v.parse().ok()),
                pwm: chip.join(&pwm).exists().then(|| chip.join(pwm).display().to_string()),
            });
        }
    }
    fans.sort_by(|a, b| (&a.chip, &a.label).cmp(&(&b.chip, &b.label)));
    fans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fake_hwmon_tree() {
        let dir = std::env::temp_dir().join(format!("ember-fans-{}", std::process::id()));
        let chip = dir.join("hwmon0");
        fs::create_dir_all(&chip).unwrap();
        fs::write(chip.join("name"), "thinkpad\n").unwrap();
        fs::write(chip.join("fan1_input"), "2400\n").unwrap();
        fs::write(chip.join("pwm1"), "128\n").unwrap();
        fs::write(chip.join("fan2_input"), "garbage\n").unwrap();
        fs::write(chip.join("temp1_input"), "45000\n").unwrap();

        let fans = read_root(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(fans.len(), 2);
        assert_eq!((fans[0].label.as_str(), fans[0].rpm), ("Fan 1", Some(2400)));
        assert!(fans[0].pwm.is_some());
        assert_eq!((fans[1].rpm, fans[1].pwm.is_none()), (None, true));
        assert!(read_root(&dir).is_empty()); // missing dir → no fans, no panic
    }
}
