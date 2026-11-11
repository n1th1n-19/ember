// SPDX-License-Identifier: GPL-3.0-or-later
//! Every display controller on PCI, even with no driver loaded (e.g. dGPU off in
//! system76-power "integrated" mode). sysinfo/NVML fills usage+VRAM when available;
//! sysfs gives current/max clock for i915/amdgpu.
// ponytail: Intel busy % needs perf PMU (root) → `--`; add via helper if anyone asks.
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct PciGpu {
    pub slot: String,
    pub vendor: &'static str,
    pub driver: Option<String>,
    pub cur_mhz: Option<u32>,
    pub max_mhz: Option<u32>,
}

pub fn vendor_name(id: &str) -> &'static str {
    match id.trim() {
        "0x8086" => "Intel",
        "0x10de" => "NVIDIA",
        "0x1002" => "AMD",
        _ => "GPU",
    }
}

pub fn read_pci() -> Vec<PciGpu> {
    read_pci_root(Path::new("/sys/bus/pci/devices"))
}

fn read_pci_root(root: &Path) -> Vec<PciGpu> {
    let Ok(devs) = fs::read_dir(root) else { return vec![] };
    let mut out: Vec<PciGpu> = devs
        .flatten()
        .filter_map(|e| {
            let d = e.path();
            let read = |f: &str| fs::read_to_string(d.join(f)).ok();
            if !read("class")?.starts_with("0x03") {
                return None;
            }
            let driver = fs::read_link(d.join("driver")).ok().and_then(|p| Some(p.file_name()?.to_string_lossy().into_owned()));
            let card = fs::read_dir(d.join("drm"))
                .ok()
                .and_then(|it| it.flatten().map(|c| c.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("card"))));
            let mhz = |f: &str| fs::read_to_string(card.as_ref()?.join(f)).ok()?.trim().parse().ok();
            Some(PciGpu {
                slot: e.file_name().to_string_lossy().into_owned(),
                vendor: vendor_name(&read("vendor")?),
                driver,
                cur_mhz: mhz("gt_cur_freq_mhz"),
                max_mhz: mhz("gt_max_freq_mhz"),
            })
        })
        .collect();
    out.sort_by(|a, b| a.slot.cmp(&b.slot));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_gpus_with_and_without_driver() {
        let root = std::env::temp_dir().join(format!("ember-pci-{}", std::process::id()));
        let igpu = root.join("0000:00:02.0");
        let dgpu = root.join("0000:01:00.0");
        let nic = root.join("0000:02:00.0");
        fs::create_dir_all(igpu.join("drm/card0")).unwrap();
        fs::create_dir_all(&dgpu).unwrap();
        fs::create_dir_all(&nic).unwrap();
        fs::write(igpu.join("class"), "0x030000\n").unwrap();
        fs::write(igpu.join("vendor"), "0x8086\n").unwrap();
        fs::write(igpu.join("drm/card0/gt_cur_freq_mhz"), "1183\n").unwrap();
        fs::write(igpu.join("drm/card0/gt_max_freq_mhz"), "1300\n").unwrap();
        fs::write(dgpu.join("class"), "0x030200\n").unwrap();
        fs::write(dgpu.join("vendor"), "0x10de\n").unwrap();
        fs::write(nic.join("class"), "0x020000\n").unwrap();
        fs::write(nic.join("vendor"), "0x8086\n").unwrap();

        let g = read_pci_root(&root);
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].vendor, g[0].cur_mhz, g[0].max_mhz), ("Intel", Some(1183), Some(1300)));
        assert_eq!((g[1].vendor, g[1].driver.as_deref(), g[1].cur_mhz), ("NVIDIA", None, None));
    }
}
