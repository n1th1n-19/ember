// SPDX-License-Identifier: GPL-3.0-or-later
//! Compact readings shared by the dock applet and the desktop widget.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::Frame;
use cosmic::iced::{Color, Point, Rectangle, Size};
use ember_core::Snapshot;

use crate::ink::{self, CYAN, DISPLAY, GREY, Level, MAGENTA, PAPER, RED, YELLOW};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Cpu,
    Ram,
    Temp,
    Gpu,
    Battery,
    Net,
    Disk,
}

impl Key {
    pub const ALL: [Key; 7] = [Key::Cpu, Key::Ram, Key::Temp, Key::Gpu, Key::Battery, Key::Net, Key::Disk];
    pub fn short(self) -> &'static str {
        match self {
            Key::Cpu => "CPU",
            Key::Ram => "RAM",
            Key::Temp => "TEMP",
            Key::Gpu => "GPU",
            Key::Battery => "BAT",
            Key::Net => "NET",
            Key::Disk => "DISK",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Key::Cpu => "cpu",
            Key::Ram => "ram",
            Key::Temp => "temp",
            Key::Gpu => "gpu",
            Key::Battery => "battery",
            Key::Net => "net",
            Key::Disk => "disk",
        }
    }
    pub fn parse(s: &str) -> Option<Key> {
        Key::ALL.into_iter().find(|k| k.id() == s.trim())
    }
}

#[derive(Debug, Clone)]
pub struct Reading {
    pub key: Key,
    pub value: String,
    /// 0..1 for the bar; None = no reading
    pub frac: Option<f32>,
    pub color: Color,
}

fn pct_color(p: f32) -> Color {
    Level::from_percent(p).color()
}

pub fn read(s: &Snapshot, key: Key) -> Reading {
    let (value, frac, color) = match key {
        Key::Cpu => (ink::opt(s.cpu_percent, |v| format!("{v:.0}%")), s.cpu_percent.map(|v| v / 100.0), s.cpu_percent.map_or(GREY, pct_color)),
        Key::Ram => (ink::opt(s.mem_percent(), |v| format!("{v:.0}%")), s.mem_percent().map(|v| v / 100.0), MAGENTA),
        Key::Temp => {
            (ink::opt(s.cpu_temp(), |c| format!("{c:.0}°")), s.cpu_temp().map(|c| c / 100.0), s.cpu_temp().map_or(GREY, |c| Level::from_celsius(c).color()))
        }
        Key::Gpu => {
            // busy NVIDIA card first (usage %), else the built-in GPU's clock
            match s.dgpu().filter(|g| g.usage.is_some()).or(s.igpu()) {
                Some(g) if g.usage.is_some() => (ink::opt(g.usage, |u| format!("{u:.0}%")), g.usage.map(|u| u / 100.0), CYAN),
                Some(g) => (ink::opt(g.cur_mhz, |m| format!("{m}")), g.cur_mhz.zip(g.max_mhz).map(|(c, m)| c as f32 / m.max(1) as f32), CYAN),
                None => ("OFF".into(), None, GREY),
            }
        }
        Key::Battery => {
            let p = s.batteries.first().and_then(|b| b.percent);
            (ink::opt(p, |v| format!("{v:.0}%")), p.map(|v| v / 100.0), if p.is_some_and(|v| v < 15.0) { RED } else { YELLOW })
        }
        Key::Net => {
            let t = s.net_rx.unwrap_or(0.0) + s.net_tx.unwrap_or(0.0);
            let v = if s.net_rx.is_none() { "--".into() } else { short_rate(t) };
            // log scale: 1 KB/s .. 100 MB/s
            let frac = (s.net_rx.is_some()).then(|| ((t.max(1.0).log10() - 3.0) / 5.0).clamp(0.0, 1.0) as f32);
            (v, frac, CYAN)
        }
        Key::Disk => {
            let d = s.disks.iter().max_by_key(|d| d.total);
            let frac = d.map(|d| d.used as f32 / d.total.max(1) as f32);
            (ink::opt(frac, |f| format!("{:.0}%", f * 100.0)), frac, frac.map_or(GREY, |f| pct_color(f * 100.0)))
        }
    };
    Reading { key, value, frac, color }
}

/// "1.2M", "340K": fits a dock tile.
pub fn short_rate(bps: f64) -> String {
    match bps {
        b if b >= 1e9 => format!("{:.1}G", b / 1e9),
        b if b >= 1e6 => format!("{:.1}M", b / 1e6),
        b if b >= 1e3 => format!("{:.0}K", b / 1e3),
        b => format!("{b:.0}B"),
    }
}

/// Dock/panel tile: one small ink meter per reading. Values are printed when the tile
/// is big enough to read them.
pub fn meters(f: &mut Frame, r: Rectangle, readings: &[Reading], horizontal: bool) {
    let n = readings.len().max(1) as f32;
    let gap = 3.0;
    for (i, rd) in readings.iter().enumerate() {
        let cell = if horizontal {
            let w = (r.width - gap * (n - 1.0)) / n;
            Rectangle::new(Point::new(r.x + i as f32 * (w + gap), r.y), Size::new(w, r.height))
        } else {
            let h = (r.height - gap * (n - 1.0)) / n;
            Rectangle::new(Point::new(r.x, r.y + i as f32 * (h + gap)), Size::new(r.width, h))
        };
        let label_h = if cell.height >= 30.0 { (cell.height * 0.32).min(14.0) } else { 0.0 };
        let bar = Rectangle::new(cell.position(), Size::new(cell.width, cell.height - label_h));
        f.fill(&ink::rect(bar), PAPER);
        if let Some(fr) = rd.frac {
            let h = bar.height * fr.clamp(0.0, 1.0);
            f.fill(&ink::rect(Rectangle::new(Point::new(bar.x, bar.y + bar.height - h), Size::new(bar.width, h))), rd.color);
        } else {
            ink::hatch(f, bar, 5.0, 1.0, GREY);
        }
        f.stroke(&ink::rect(bar), ink::stroke(2.0));
        if label_h > 0.0 {
            let size = label_h.min(cell.width / (rd.value.chars().count().max(2) as f32 * 0.5));
            ink::text(
                f,
                rd.value.clone(),
                Point::new(cell.center_x(), cell.y + cell.height - label_h / 2.0),
                size,
                DISPLAY,
                ink::pal().text,
                Alignment::Center,
                Vertical::Center,
            );
        }
    }
}

/// Comic card listing readings as label · bar · value rows (popup and desktop widget).
pub fn card(f: &mut Frame, r: Rectangle, title: &str, readings: &[Reading]) {
    let p = ink::pal();
    ink::panel(f, r, p.panel);
    let inner = ink::inset(r, 16.0);
    let top = inner.y + 26.0;
    let n = readings.len().max(1) as f32;
    let row_h = ((inner.y + inner.height - top) / n).min(46.0);
    for (i, rd) in readings.iter().enumerate() {
        let y = top + i as f32 * row_h;
        let mid = y + row_h / 2.0;
        let label_w = inner.width * 0.24;
        let value_w = inner.width * 0.26;
        ink::text(f, rd.key.short(), Point::new(inner.x, mid), (row_h * 0.42).min(20.0), DISPLAY, p.text, Alignment::Left, Vertical::Center);
        let bar = Rectangle::new(Point::new(inner.x + label_w, mid - row_h * 0.2), Size::new(inner.width - label_w - value_w - 8.0, row_h * 0.4));
        crate::chart::hbar(f, bar, rd.frac.unwrap_or(0.0), rd.color);
        ink::text(f, rd.value.clone(), Point::new(inner.x + inner.width, mid), (row_h * 0.46).min(22.0), DISPLAY, p.text, Alignment::Right, Vertical::Center);
    }
    ink::caption(f, Point::new(r.x - 6.0, r.y - 14.0), title, 20.0, YELLOW);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_rates() {
        for k in Key::ALL {
            assert_eq!(Key::parse(k.id()), Some(k));
        }
        assert_eq!(Key::parse("nope"), None);
        assert_eq!(short_rate(340_000.0), "340K");
        assert_eq!(short_rate(1_260_000.0), "1.3M");
        assert_eq!(short_rate(12.0), "12B");
    }

    #[test]
    fn missing_values_show_dashes() {
        let s = Snapshot::default();
        for k in Key::ALL {
            let r = read(&s, k);
            assert!(r.value == "--" || r.value == "OFF", "{k:?} -> {}", r.value);
            assert!(r.frac.is_none());
        }
    }
}
