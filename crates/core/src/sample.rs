// SPDX-License-Identifier: GPL-3.0-or-later
use std::time::Instant;

use sysinfo::{Components, Disks, Networks, System};

use crate::{battery, fans, gpu};

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub cpu_percent: Option<f32>,
    pub per_core: Vec<f32>,
    pub cpu_mhz: Option<u64>,
    /// hardware max clock (cpufreq cpuinfo_max_freq), for gauges
    pub cpu_max_mhz: Option<u64>,
    pub cpu_brand: String,
    pub load: Option<[f64; 3]>,
    pub uptime_s: Option<u64>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub disks: Vec<DiskStat>,
    /// bytes/s, summed over non-loopback interfaces
    pub net_rx: Option<f64>,
    pub net_tx: Option<f64>,
    pub gpus: Vec<GpuStat>,
    pub temps: Vec<Temp>,
    pub fans: Vec<fans::Fan>,
    pub batteries: Vec<battery::Battery>,
}

impl Snapshot {
    pub fn mem_percent(&self) -> Option<f32> {
        (self.mem_total > 0).then(|| self.mem_used as f32 / self.mem_total as f32 * 100.0)
    }
    /// Hottest CPU-ish sensor, falling back to hottest sensor overall.
    pub fn cpu_temp(&self) -> Option<f32> {
        let max = |it: &mut dyn Iterator<Item = &Temp>| it.map(|t| t.celsius).reduce(f32::max);
        let cpuish = |t: &&Temp| {
            let l = t.label.to_lowercase();
            l.contains("package") || l.contains("tctl") || l.contains("coretemp") || l.contains("cpu")
        };
        max(&mut self.temps.iter().filter(cpuish)).or_else(|| max(&mut self.temps.iter()))
    }
    /// Built-in GPU (Intel/AMD with a driver), the one driving the screen normally.
    pub fn igpu(&self) -> Option<&GpuStat> {
        self.gpus.iter().find(|g| g.vendor != "NVIDIA" && g.driver.is_some())
    }
    /// Discrete NVIDIA GPU, on or off.
    pub fn dgpu(&self) -> Option<&GpuStat> {
        self.gpus.iter().find(|g| g.vendor == "NVIDIA")
    }
}

#[derive(Debug, Clone)]
pub struct DiskStat {
    pub name: String,
    pub mount: String,
    pub used: u64,
    pub total: u64,
    pub read_bps: Option<f64>,
    pub write_bps: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct GpuStat {
    pub name: String,
    pub vendor: &'static str,
    /// None = no kernel driver bound (dGPU switched off)
    pub driver: Option<String>,
    pub cur_mhz: Option<u32>,
    pub max_mhz: Option<u32>,
    pub usage: Option<f32>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub temp_c: Option<f32>,
    pub power_w: Option<f32>,
    /// driver loaded but the card is in runtime sleep (D3); not woken to read it
    pub asleep: bool,
}

#[derive(Debug, Clone)]
pub struct Temp {
    pub label: String,
    pub celsius: f32,
    pub critical: Option<f32>,
}

/// Owns the sysinfo handles; call `sample()` on an interval (≥ 500ms for meaningful CPU %).
pub struct Sampler {
    sys: System,
    nets: Option<Networks>,
    disks: Option<Disks>,
    comps: Option<Components>,
    last: Instant,
}

impl Sampler {
    pub fn new() -> Self {
        let mut sys = System::new().expect("sysinfo unsupported on this OS");
        sys.refresh_cpu_usage(); // prime: first CPU % needs a previous reading
        Self {
            sys,
            nets: Networks::new_with_refreshed_list().ok(),
            disks: Disks::new_with_refreshed_list().ok(),
            comps: Components::new_with_refreshed_list().ok(),
            last: Instant::now(),
        }
    }

    pub fn sample(&mut self) -> Snapshot {
        let dt = self.last.elapsed().as_secs_f64().max(1e-3);
        self.last = Instant::now();
        let rate = |bytes: u64| Some(bytes as f64 / dt);

        let s = &mut self.sys;
        s.refresh_cpu_usage();
        s.refresh_cpu_frequency();
        s.refresh_memory();
        let cpus = s.cpus();

        let (net_rx, net_tx) = match self.nets.as_mut() {
            Some(n) => {
                n.refresh(true);
                let real = n.iter().filter(|(name, _)| name.to_str() != Some("lo"));
                let (rx, tx) = real.fold((0, 0), |(r, t), (_, d)| (r + d.received(), t + d.transmitted()));
                (rate(rx), rate(tx))
            }
            None => (None, None),
        };

        let disks = self.disks.as_mut().map_or_else(Vec::new, |d| {
            d.refresh(true);
            d.iter()
                .filter(|d| d.total_space() > 0 && d.name() != "overlay")
                .map(|d| {
                    let u = d.usage();
                    DiskStat {
                        name: d.name().to_string_lossy().into_owned(),
                        mount: d.mount_point().display().to_string(),
                        used: d.total_space().saturating_sub(d.available_space()),
                        total: d.total_space(),
                        read_bps: rate(u.read_bytes),
                        write_bps: rate(u.written_bytes),
                    }
                })
                .collect()
        });

        let temps = self.comps.as_mut().map_or_else(Vec::new, |c| {
            c.refresh(true);
            c.iter()
                .filter_map(|c| Some(Temp { label: c.label().to_string(), celsius: c.temperature().filter(|t| t.is_finite())?, critical: c.critical() }))
                .collect()
        });

        // PCI list is the source of truth for *which* GPUs exist; NVML adds NVIDIA readings,
        // but only for cards already awake.
        let pci = gpu::read_pci();
        let nv_slots: Vec<&str> =
            pci.iter().filter(|p| p.vendor == "NVIDIA" && p.driver.as_deref() == Some("nvidia") && gpu::awake(&p.slot)).map(|p| p.slot.as_str()).collect();
        let nv = if nv_slots.is_empty() { vec![] } else { gpu::nvml_read(&nv_slots) };
        let gpus = pci
            .iter()
            .map(|p| {
                let n = nv_slots.iter().position(|s| *s == p.slot).and_then(|i| nv[i].clone()).unwrap_or_default();
                GpuStat {
                    name: n.name.unwrap_or_else(|| format!("{} GPU", p.vendor)),
                    vendor: p.vendor,
                    driver: p.driver.clone(),
                    cur_mhz: n.cur_mhz.or(p.cur_mhz),
                    max_mhz: n.max_mhz.or(p.max_mhz),
                    usage: n.usage,
                    vram_used: n.vram_used,
                    vram_total: n.vram_total,
                    temp_c: n.temp_c,
                    power_w: n.power_w,
                    asleep: p.driver.is_some() && !gpu::awake(&p.slot),
                }
            })
            .collect();

        Snapshot {
            cpu_percent: (!cpus.is_empty()).then(|| s.global_cpu_usage()),
            per_core: cpus.iter().map(|c| c.usage()).collect(),
            cpu_mhz: cpus.iter().map(|c| c.frequency()).max().filter(|&f| f > 0),
            cpu_max_mhz: std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|khz| khz / 1000),
            cpu_brand: cpus.first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
            load: System::load_average().ok().map(|l| [l.one, l.five, l.fifteen]),
            uptime_s: System::uptime().ok(),
            mem_used: s.used_memory(),
            mem_total: s.total_memory(),
            swap_used: s.used_swap(),
            swap_total: s.total_swap(),
            disks,
            net_rx,
            net_tx,
            gpus,
            temps,
            fans: fans::read_all(),
            batteries: battery::read_all(),
        }
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}
