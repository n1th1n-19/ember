// SPDX-License-Identifier: GPL-3.0-or-later
//! Every page is one canvas: comic panel frame, art on the left, big number + charts on the right,
//! ink details strip at the bottom. Layout follows DESIGN.md.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{self, Frame, Geometry};
use cosmic::iced::{mouse, Color, Point, Rectangle, Size};
use cosmic::{Renderer, Theme};
use ember_ui::{
    art, chart,
    ink::{self, bytes, opt, Level, BODY, CYAN, DISPLAY, GREY, INK, MAGENTA, PAPER, RED, WHITE, YELLOW},
};

use crate::{App, Message, Page, BURST_POSES};

pub struct View<'a> {
    pub app: &'a App,
    pub page: Page,
}

impl canvas::Program<Message, Theme, Renderer> for View<'_> {
    type State = ();

    fn update(&self, _: &mut (), event: &canvas::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        if self.page != Page::Overview {
            return None;
        }
        let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event else { return None };
        let p = cursor.position_in(bounds)?;
        let local = Rectangle::new(Point::ORIGIN, bounds.size());
        overview_cells(local).into_iter().find(|(_, r)| r.contains(p)).map(|(page, _)| canvas::Action::publish(Message::Open(page)).and_capture())
    }

    fn mouse_interaction(&self, _: &(), bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let over = self.page == Page::Overview
            && cursor.position_in(bounds).is_some_and(|p| overview_cells(Rectangle::new(Point::ORIGIN, bounds.size())).iter().any(|(_, r)| r.contains(p)));
        if over { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let b = Rectangle::new(Point::ORIGIN, bounds.size());
        f.fill(&ink::rect(b), ink::pal().page);
        let a = self.app;
        match self.page {
            Page::Overview => overview(&mut f, b, a),
            Page::Cpu => cpu(&mut f, b, a),
            Page::Memory => memory(&mut f, b, a),
            Page::Disks => disks(&mut f, b, a),
            Page::Network => network(&mut f, b, a),
            Page::Temps => temps(&mut f, b, a),
            Page::Battery => battery(&mut f, b, a),
            Page::Gpu => gpu(&mut f, b, a),
            Page::Fans => fans(&mut f, b, a),
            // widget pages are drawn by controls.rs
            Page::Processes | Page::Services | Page::Control | Page::Apps | Page::Settings => {}
        }
        vec![f.into_geometry()]
    }
}

// ---------- shared layout ----------

struct Detail<'a> {
    title: &'a str,
    ground: Color,
    label: &'a str,
    big: String,
    sub: String,
    details: Vec<(&'a str, String)>,
    /// render detail values in the body font (driver names like "i915" read wrong in Bangers)
    body_values: bool,
    burst: Option<(&'a str, u64)>,
}

/// Draws the page frame; returns (art box, chart area).
fn frame(f: &mut Frame, b: Rectangle, d: &Detail, pose: u64) -> (Rectangle, Rectangle) {
    let pad = 28.0;
    let p = Rectangle::new(Point::new(b.x + pad, b.y + pad + 12.0), Size::new(b.width - 2.0 * pad - 6.0, b.height - 2.0 * pad - 18.0));
    let pal = ink::pal();
    ink::panel(f, p, pal.panel);
    let inner = ink::inset(p, 20.0);

    // details strip: ink ground, paper text
    let strip_h = 66.0;
    let strip = Rectangle::new(Point::new(inner.x, inner.y + inner.height - strip_h), Size::new(inner.width, strip_h));
    f.fill(&ink::rect(strip), INK);
    let n = d.details.len().max(1) as f32;
    let col = (strip.width - 16.0) / n;
    for (i, (k, v)) in d.details.iter().enumerate() {
        let x = strip.x + 16.0 + i as f32 * col;
        // shrink long values so columns never collide
        let (font, em) = if d.body_values { (BODY, 0.56) } else { (DISPLAY, 0.46) };
        let size = (if d.body_values { 18.0f32 } else { 22.0 }).min((col - 14.0) / (v.chars().count().max(1) as f32 * em)).max(11.0);
        ink::text(f, k.to_uppercase(), Point::new(x, strip.y + 12.0), 12.0, BODY, GREY, Alignment::Left, Vertical::Top);
        ink::text(f, v.clone(), Point::new(x, strip.y + 30.0), size, font, PAPER, Alignment::Left, Vertical::Top);
    }

    let upper = Rectangle::new(inner.position(), Size::new(inner.width, inner.height - strip_h - 20.0));
    // wide: art left, numbers right. narrow (tiled half-screen): art on top, numbers below.
    let narrow = upper.width < 760.0;
    let art = if narrow {
        Rectangle::new(upper.position(), Size::new(upper.width, (upper.height * 0.36).min(upper.width * 0.7)))
    } else {
        Rectangle::new(upper.position(), Size::new((upper.width * 0.42).min(upper.height * 1.15), upper.height))
    };
    ink::boxed(f, art, ink::ground(d.ground), 4.0, 5.0);
    // decorative halftone in the art box corner (no text sits on it)
    let corner = Rectangle::new(Point::new(art.x + art.width * 0.45, art.y), Size::new(art.width * 0.55, art.height * 0.5));
    ink::halftone_where(f, ink::inset(corner, 4.0), if pal.dark { Color { a: 0.55, ..d.ground } } else { Color { a: 0.22, ..INK } }, 12.0, 0.6, |q| (q.x - corner.x) > (q.y - corner.y) * 0.8);

    let right = if narrow {
        Rectangle::new(Point::new(upper.x, art.y + art.height + 22.0), Size::new(upper.width, upper.height - art.height - 22.0))
    } else {
        Rectangle::new(Point::new(art.x + art.width + 28.0, upper.y), Size::new(upper.width - art.width - 28.0, upper.height))
    };
    ink::text(f, d.label.to_uppercase(), Point::new(right.x, right.y + 4.0), 15.0, BODY, pal.text, Alignment::Left, Vertical::Top);
    let big_size = fit_size(&d.big, right.width - 20.0, (right.height * if narrow { 0.2 } else { 0.24 }).clamp(56.0, 120.0));
    ink::misprint(f, &d.big, Point::new(right.x + 4.0, right.y + 26.0), big_size, WHITE, Alignment::Left, pose / 4);
    let sub_y = right.y + 34.0 + big_size * 1.1;
    ink::text(f, d.sub.clone(), Point::new(right.x, sub_y), 16.0, BODY, pal.text, Alignment::Left, Vertical::Top);

    if let Some((word, age)) = d.burst {
        let c = Point::new(right.x + right.width - 90.0, right.y + 70.0);
        ink::burst(f, c, 78.0, word, age);
    }

    ink::caption(f, Point::new(p.x - 10.0, p.y - 20.0), d.title, 28.0, YELLOW);

    let charts = Rectangle::new(Point::new(right.x, sub_y + 34.0), Size::new(right.width, (right.y + right.height - sub_y - 34.0).max(40.0)));
    (ink::inset(art, 18.0), charts)
}

/// Shrink a display size so `s` fits `width` (never below the 48px misprint minimum).
fn fit_size(s: &str, width: f32, size: f32) -> f32 {
    let w = ink::approx_width(s, size, DISPLAY) * 1.1 + 12.0;
    if w > width { (size * width / w).max(48.0) } else { size }
}

fn split_v(r: Rectangle, top_frac: f32, gap: f32) -> (Rectangle, Rectangle) {
    let h = (r.height - gap) * top_frac;
    (
        Rectangle::new(r.position(), Size::new(r.width, h)),
        Rectangle::new(Point::new(r.x, r.y + h + gap), Size::new(r.width, r.height - h - gap)),
    )
}

fn chart_label(f: &mut Frame, r: Rectangle, s: &str) -> Rectangle {
    ink::text(f, s.to_uppercase(), Point::new(r.x, r.y), 12.0, BODY, ink::pal().text, Alignment::Left, Vertical::Top);
    Rectangle::new(Point::new(r.x, r.y + 18.0), Size::new(r.width, (r.height - 18.0).max(10.0)))
}

fn burst_age(a: &App, start: Option<u64>) -> Option<u64> {
    let age = a.pose.saturating_sub(start?);
    (age <= BURST_POSES).then_some(if a.motion() { age } else { 3 })
}

fn rate(v: Option<f64>) -> String {
    opt(v, |v| format!("{}/s", bytes(v)))
}

fn pct(v: Option<f32>) -> String {
    opt(v, |v| format!("{v:.0}%"))
}

fn dur(s: Option<u64>) -> String {
    opt(s, |s| format!("{}h {:02}m", s / 3600, s / 60 % 60))
}

/// Chart scale for byte rates: at least 1 MB/s so idle noise stays flat.
fn rate_max(a: &ember_core::series::Series, b: &ember_core::series::Series) -> f32 {
    a.max().unwrap_or(0.0).max(b.max().unwrap_or(0.0)).max(1e6) * 1.15
}

// ---------- pages ----------

fn cpu(f: &mut Frame, b: Rectangle, a: &App) {
    let s = &a.snap;
    let total = s.cpu_percent.unwrap_or(0.0);
    let hot = Level::from_percent(total) == Level::Hot;
    let d = Detail {
        title: "ENGINE ROOM",
        ground: if hot { RED } else { YELLOW },
        label: "CPU load",
        big: pct(s.cpu_percent),
        sub: format!("{}  ·  {} cores", s.cpu_brand, s.per_core.len()),
        details: vec![
            ("Clock", opt(s.cpu_mhz, |m| format!("{m} MHz"))),
            ("Max", opt(s.cpu_max_mhz, |m| format!("{m} MHz"))),
            ("Load 1/5/15", opt(s.load, |l| format!("{:.2} {:.2} {:.2}", l[0], l[1], l[2]))),
            ("Temp", opt(s.cpu_temp(), |c| format!("{c:.0}°C"))),
            ("Uptime", dur(s.uptime_s)),
        ],
        body_values: false,
        burst: burst_age(a, a.bursts.cpu).map(|age| ("SIZZLE!", age)),
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    let clock = match (s.cpu_mhz, s.cpu_max_mhz) {
        (Some(c), Some(m)) if m > 0 => c as f32 / m as f32,
        _ => 0.0,
    };
    art::cpu::draw(f, art_r, &art::cpu::Engine { total, cores: &s.per_core, clock }, a.anim());
    let (top, bot) = split_v(charts, 0.6, 18.0);
    let lr = chart_label(f, top, a.range_label());
    chart::area(f, lr, &a.hist().cpu, 100.0, RED);
    let lr = chart_label(f, bot, "Per core");
    chart::bars(f, lr, &s.per_core);
}

fn memory(f: &mut Frame, b: Rectangle, a: &App) {
    let s = &a.snap;
    let swap = (s.swap_total > 0).then(|| s.swap_used as f32 / s.swap_total as f32);
    let d = Detail {
        title: "INK TANK",
        ground: CYAN,
        label: "Memory in use",
        big: pct(s.mem_percent()),
        sub: format!("{} of {}", bytes(s.mem_used as f64), bytes(s.mem_total as f64)),
        details: vec![
            ("Used", bytes(s.mem_used as f64)),
            ("Free", bytes(s.mem_total.saturating_sub(s.mem_used) as f64)),
            ("Swap", format!("{} / {}", bytes(s.swap_used as f64), bytes(s.swap_total as f64))),
        ],
        body_values: false,
        burst: burst_age(a, a.bursts.mem).map(|age| ("GLUG!", age)),
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    art::mem::draw(f, art_r, s.mem_percent().unwrap_or(0.0) / 100.0, swap.unwrap_or(0.0), a.anim());
    let (top, bot) = split_v(charts, 0.62, 18.0);
    let lr = chart_label(f, top, &format!("RAM · {}", a.range_label()));
    chart::area(f, lr, &a.hist().mem, 100.0, MAGENTA);
    let bot = chart_label(f, bot, &format!("Swap · {}", a.range_label()));
    chart::area(f, bot, &a.hist().swap, 100.0, GREY);
}

fn disks(f: &mut Frame, b: Rectangle, a: &App) {
    let s = &a.snap;
    let mut list: Vec<_> = s.disks.iter().collect();
    list.sort_by_key(|d| std::cmp::Reverse(d.total));
    let main = list.first();
    let d = Detail {
        title: "FILING CABINET",
        ground: MAGENTA,
        label: "Main disk used",
        big: opt(main, |d| format!("{:.0}%", d.used as f64 / d.total.max(1) as f64 * 100.0)),
        sub: opt(main, |d| format!("{} of {}  ·  {}", bytes(d.used as f64), bytes(d.total as f64), d.mount)),
        details: vec![
            ("Read", rate(a.hist().disk_r.last().map(f64::from))),
            ("Write", rate(a.hist().disk_w.last().map(f64::from))),
            ("Disks", list.len().to_string()),
        ],
        body_values: false,
        burst: None,
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    let drawers: Vec<_> = list
        .iter()
        .map(|d| art::disk::Drawer {
            label: d.mount.clone(),
            used: d.used as f32 / d.total.max(1) as f32,
            busy: d.read_bps.unwrap_or(0.0) + d.write_bps.unwrap_or(0.0) > 64e3,
        })
        .collect();
    art::disk::draw(f, art_r, &drawers, a.anim());
    let max = rate_max(&a.hist().disk_r, &a.hist().disk_w);
    let lr = chart_label(f, charts, &format!("Read (up) · write (down) · {}", a.range_label()));
    chart::mirrored(f, lr, &a.hist().disk_r, &a.hist().disk_w, max, CYAN, RED);
}

fn network(f: &mut Frame, b: Rectangle, a: &App) {
    let s = &a.snap;
    let (rx, tx) = (s.net_rx.unwrap_or(0.0), s.net_tx.unwrap_or(0.0));
    let total = rx + tx;
    let level = match total {
        t if t < 1e3 => 0,
        t if t < 1e5 => 1,
        t if t < 2e6 => 2,
        _ => 3,
    };
    let d = Detail {
        title: "RADIO TOWER",
        ground: CYAN,
        label: "Download",
        big: rate(s.net_rx),
        sub: format!("Upload {}", rate(s.net_tx)),
        details: vec![("Down", rate(s.net_rx)), ("Up", rate(s.net_tx)), ("Peak down", rate(a.hist().rx.max().map(f64::from)))],
        body_values: false,
        burst: burst_age(a, a.bursts.net).map(|age| ("FWOOSH", age)),
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    art::net::draw(f, art_r, level, rx > 1e3, tx > 1e3, a.anim());
    let max = rate_max(&a.hist().rx, &a.hist().tx);
    let lr = chart_label(f, charts, &format!("Download (up) · upload (down) · {}", a.range_label()));
    chart::mirrored(f, lr, &a.hist().rx, &a.hist().tx, max, CYAN, RED);
}

fn temps(f: &mut Frame, b: Rectangle, a: &App) {
    let s = &a.snap;
    let groups = ember_core::temps::group(&s.temps);
    let hottest = groups.iter().map(|g| g.celsius).reduce(f32::max);
    let d = Detail {
        title: "THERMOMETER RACK",
        ground: if hottest.is_some_and(|c| Level::from_celsius(c) == Level::Hot) { RED } else { YELLOW },
        label: "CPU temperature",
        big: opt(s.cpu_temp(), |c| format!("{c:.0}°")),
        sub: format!("{} sensors · hottest {}", s.temps.len(), opt(hottest, |c| format!("{c:.0}°C"))),
        details: groups.iter().take(5).map(|g| (g.name, format!("{:.0}°C", g.celsius))).collect(),
        body_values: false,
        burst: burst_age(a, a.bursts.temp).map(|age| ("TOO HOT", age)),
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    let gauges: Vec<_> = groups
        .iter()
        .map(|g| art::temp::Gauge { label: g.name.to_string(), celsius: g.celsius, critical: g.critical.unwrap_or(100.0) })
        .collect();
    art::temp::draw(f, art_r, &gauges, a.anim());
    let (top, bot) = split_v(charts, 0.3, 18.0);
    let cells: Vec<_> = groups.iter().map(|g| (g.name.to_string(), g.celsius, g.critical.unwrap_or(100.0))).collect();
    let lr = chart_label(f, top, "Heat map");
    chart::heat_strip(f, lr, &cells);
    let lr = chart_label(f, bot, &format!("CPU °C · {}", a.range_label()));
    chart::area(f, lr, &a.hist().cpu_temp, 100.0, RED);
}

fn battery(f: &mut Frame, b: Rectangle, a: &App) {
    let bat = a.snap.batteries.first();
    let charging = bat.and_then(|b| b.status.as_deref()) == Some("Charging");
    let d = Detail {
        title: "POWER CELL",
        ground: YELLOW,
        label: "Battery",
        big: opt(bat.and_then(|b| b.percent), |p| format!("{p:.0}%")),
        sub: bat.map_or_else(|| "No battery found".into(), |b| b.status.clone().unwrap_or_else(|| "--".into())),
        details: vec![
            ("Health", opt(bat.and_then(|b| b.health), |h| format!("{:.0}%", h * 100.0))),
            ("Cycles", opt(bat.and_then(|b| b.cycles), |c| c.to_string())),
            ("Power", opt(bat.and_then(|b| b.power_w), |w| format!("{w:.1} W"))),
            ("Cell", bat.map_or("--".into(), |b| b.name.clone())),
        ],
        body_values: false,
        burst: burst_age(a, a.bursts.battery).map(|age| ("PLINK", age)),
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    art::battery::draw(f, art_r, bat.and_then(|b| b.percent), charging, a.anim());
    let (top, bot) = split_v(charts, 0.55, 18.0);
    let top = chart_label(f, top, "Health and power");
    let r = (top.height / 2.0).min(top.width / 5.0);
    chart::ring(f, Point::new(top.x + r + 6.0, top.y + top.height / 2.0), r, bat.and_then(|b| b.health).unwrap_or(0.0), CYAN, &opt(bat.and_then(|b| b.health), |h| format!("{:.0}%", h * 100.0)));
    let w = bat.and_then(|b| b.power_w).unwrap_or(0.0);
    chart::gauge(f, Point::new(top.x + r * 2.0 + 40.0 + r * 1.2, top.y + top.height * 0.85), r * 1.2, w / 60.0, &format!("{w:.0} W"));
    let lr = chart_label(f, bot, &format!("Charge · {}", a.range_label()));
    chart::area(f, lr, &a.hist().battery, 100.0, YELLOW);
}

fn gpu(f: &mut Frame, b: Rectangle, a: &App) {
    let gpus = &a.snap.gpus;
    let lead = gpus.iter().find(|g| g.driver.is_some());
    let d = Detail {
        title: "GRAPHICS CARD",
        ground: CYAN,
        label: lead.map_or("GPU", |g| g.vendor),
        big: lead.map_or_else(|| "OFF".into(), |g| g.usage.map_or_else(|| opt(g.cur_mhz, |m| format!("{m}")), |u| format!("{u:.0}%"))),
        sub: lead.map_or_else(|| "No GPU driver loaded".into(), |g| if g.usage.is_some() { "usage".into() } else { format!("MHz now (max {})", opt(g.max_mhz, |m| m.to_string())) }),
        details: gpus
            .iter()
            .map(|g| (g.vendor, g.driver.as_ref().map_or_else(|| "switched off".into(), |d| format!("{d} · {}", opt(g.cur_mhz, |m| format!("{m} MHz"))))))
            .collect(),
        body_values: true,
        burst: None,
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    let n = gpus.len().max(1) as f32;
    let h = art_r.height / n;
    for (i, g) in gpus.iter().enumerate() {
        let r = Rectangle::new(Point::new(art_r.x, art_r.y + i as f32 * h), Size::new(art_r.width, h - 10.0));
        let speed = g.usage.map(|u| u / 100.0).or_else(|| Some(g.cur_mhz? as f32 / g.max_mhz? as f32)).unwrap_or(0.0);
        art::gpu::draw(f, Rectangle::new(r.position(), Size::new(r.width, r.height - 22.0)), g.driver.is_some(), speed, g.usage.is_some_and(|u| u > 85.0), a.anim());
        let note = if g.driver.is_some() { g.name.clone() } else { format!("{} · switched off (integrated mode)", g.vendor) };
        ink::text(f, note, Point::new(r.center_x(), r.y + r.height - 4.0), 13.0, BODY, ink::pal().scene_line, Alignment::Center, Vertical::Bottom);
    }
    if gpus.is_empty() {
        ink::bubble(f, ink::inset(art_r, 30.0), Point::new(art_r.x, art_r.y + art_r.height), "No display adapter found", 18.0);
    }
    let max = lead.and_then(|g| g.max_mhz).map_or(2000.0, |m| m as f32);
    let lr = chart_label(f, charts, &format!("Clock MHz · {}", a.range_label()));
    chart::area(f, lr, &a.hist().gpu_clock, max, CYAN);
}

fn fans(f: &mut Frame, b: Rectangle, a: &App) {
    let fans = &a.snap.fans;
    let top_rpm = fans.iter().filter_map(|f| f.rpm).max();
    let d = Detail {
        title: "FAN BOX",
        ground: if fans.is_empty() { GREY } else { CYAN },
        label: "Fan speed",
        big: opt(top_rpm, |r| r.to_string()),
        sub: if fans.is_empty() { "Your firmware doesn't expose fan sensors".into() } else { "RPM".into() },
        details: if fans.is_empty() { vec![("Fans", "none exposed".into())] } else { fans.iter().take(5).map(|f| ("Fan", format!("{} {}", f.label, opt(f.rpm, |r| format!("{r} rpm"))))).collect() },
        body_values: false,
        burst: None,
    };
    let (art_r, charts) = frame(f, b, &d, a.anim());
    let speed = top_rpm.map(|r| (r as f32 / 5000.0).min(1.0));
    art::fan::draw(f, art_r, if fans.is_empty() { None } else { speed }, a.anim());
    if fans.is_empty() {
        let bub = Rectangle::new(Point::new(charts.x, charts.y + 44.0), Size::new(charts.width.min(420.0), 130.0));
        ink::bubble(f, bub, art_r.center(), "Firmware keeps these locked. Nothing to read or control here.", 20.0);
    }
}

// ---------- overview ----------

const OVERVIEW: [Page; 6] = [Page::Cpu, Page::Memory, Page::Gpu, Page::Temps, Page::Battery, Page::Network];

fn overview_cells(b: Rectangle) -> Vec<(Page, Rectangle)> {
    let pad = 28.0;
    let gap = 26.0;
    let area = Rectangle::new(Point::new(b.x + pad, b.y + pad + 14.0), Size::new(b.width - 2.0 * pad - 6.0, b.height - 2.0 * pad - 20.0));
    let cols = if area.width > 900.0 { 3 } else { 2 };
    let rows = OVERVIEW.len().div_ceil(cols);
    let w = (area.width - gap * (cols - 1) as f32) / cols as f32;
    let h = (area.height - gap * (rows - 1) as f32) / rows as f32;
    OVERVIEW
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (c, r) = (i % cols, i / cols);
            (*p, Rectangle::new(Point::new(area.x + c as f32 * (w + gap), area.y + r as f32 * (h + gap)), Size::new(w, h)))
        })
        .collect()
}

fn overview(f: &mut Frame, b: Rectangle, a: &App) {
    let s = &a.snap;
    let pose = a.anim();
    for (page, r) in overview_cells(b) {
        ink::panel(f, r, ink::pal().panel);
        let inner = ink::inset(r, 14.0);
        let art_r = Rectangle::new(Point::new(inner.x, inner.y + 18.0), Size::new(inner.width * 0.5, inner.height - 18.0));
        let num_r = Rectangle::new(Point::new(inner.x + inner.width * 0.52, inner.y + 18.0), Size::new(inner.width * 0.48, inner.height - 18.0));
        // tinted art box, same grounds as the detail pages
        let accent = match page {
            Page::Memory | Page::Gpu | Page::Network => CYAN,
            _ => YELLOW,
        };
        ink::boxed(f, art_r, ink::ground(accent), 3.0, 4.0);
        let art_r = ink::inset(art_r, 8.0);
        let (title, big, sub, series, max, color): (&str, String, String, Option<&ember_core::series::Series>, f32, Color) = match page {
            Page::Cpu => {
                let clock = match (s.cpu_mhz, s.cpu_max_mhz) {
                    (Some(c), Some(m)) if m > 0 => c as f32 / m as f32,
                    _ => 0.0,
                };
                art::cpu::draw(f, art_r, &art::cpu::Engine { total: s.cpu_percent.unwrap_or(0.0), cores: &s.per_core, clock }, pose);
                ("CPU", pct(s.cpu_percent), opt(s.cpu_mhz, |m| format!("{m} MHz")), Some(&a.hist().cpu), 100.0, RED)
            }
            Page::Memory => {
                let swap = if s.swap_total > 0 { s.swap_used as f32 / s.swap_total as f32 } else { 0.0 };
                art::mem::draw(f, art_r, s.mem_percent().unwrap_or(0.0) / 100.0, swap, pose);
                ("MEMORY", pct(s.mem_percent()), format!("{} used", bytes(s.mem_used as f64)), Some(&a.hist().mem), 100.0, MAGENTA)
            }
            Page::Gpu => {
                let g = s.gpus.iter().find(|g| g.driver.is_some()).or(s.gpus.first());
                let speed = g.and_then(|g| Some(g.cur_mhz? as f32 / g.max_mhz? as f32)).unwrap_or(0.0);
                art::gpu::draw(f, art_r, g.is_some_and(|g| g.driver.is_some()), speed, g.and_then(|g| g.usage).is_some_and(|u| u > 85.0), pose);
                let off = s.gpus.iter().filter(|g| g.driver.is_none()).count();
                let big = g.and_then(|g| g.cur_mhz).map_or_else(|| "OFF".into(), |m| m.to_string());
                let sub = if off > 0 { format!("MHz · {off} card asleep") } else { "MHz".into() };
                ("GPU", big, sub, Some(&a.hist().gpu_clock), g.and_then(|g| g.max_mhz).map_or(2000.0, |m| m as f32), CYAN)
            }
            Page::Temps => {
                let groups = ember_core::temps::group(&s.temps);
                let gauges: Vec<_> = groups.iter().take(3).map(|g| art::temp::Gauge { label: g.name.into(), celsius: g.celsius, critical: g.critical.unwrap_or(100.0) }).collect();
                art::temp::draw(f, art_r, &gauges, pose);
                ("TEMPS", opt(s.cpu_temp(), |c| format!("{c:.0}°")), "CPU".into(), Some(&a.hist().cpu_temp), 100.0, RED)
            }
            Page::Battery => {
                let bat = s.batteries.first();
                let charging = bat.and_then(|b| b.status.as_deref()) == Some("Charging");
                art::battery::draw(f, art_r, bat.and_then(|b| b.percent), charging, pose);
                ("BATTERY", opt(bat.and_then(|b| b.percent), |p| format!("{p:.0}%")), bat.and_then(|b| b.status.clone()).unwrap_or_else(|| "--".into()), Some(&a.hist().battery), 100.0, YELLOW)
            }
            _ => {
                let (rx, tx) = (s.net_rx.unwrap_or(0.0), s.net_tx.unwrap_or(0.0));
                let lvl = match rx + tx {
                    t if t < 1e3 => 0,
                    t if t < 1e5 => 1,
                    t if t < 2e6 => 2,
                    _ => 3,
                };
                art::net::draw(f, art_r, lvl, rx > 1e3, tx > 1e3, pose);
                {
                    // number big, unit in the subtitle so narrow cards don't overflow
                    let full = opt(s.net_rx, bytes);
                    let (num, unit) = full.split_once(' ').map_or((full.clone(), String::new()), |(n, u)| (n.to_string(), u.to_string()));
                    ("NETWORK", num, format!("{unit}/s down · up {}", rate(s.net_tx)), Some(&a.hist().rx), rate_max(&a.hist().rx, &a.hist().tx), CYAN)
                }
            }
        };
        let big_size = fit_size(&big, num_r.width, (num_r.height * 0.26).clamp(48.0, 84.0));
        ink::misprint(f, &big, Point::new(num_r.x, num_r.y + 6.0), big_size, WHITE, Alignment::Left, pose / 4);
        let sub_y = num_r.y + 14.0 + big_size * 1.05;
        ink::text(f, sub, Point::new(num_r.x, sub_y), 13.0, BODY, ink::pal().text, Alignment::Left, Vertical::Top);
        if let Some(series) = series {
            let top = sub_y + 26.0;
            let ch = Rectangle::new(Point::new(num_r.x, top), Size::new(num_r.width - 6.0, (num_r.y + num_r.height - top - 6.0).max(30.0)));
            chart::area(f, ch, series, max, color);
        }
        ink::caption(f, Point::new(r.x - 8.0, r.y - 16.0), title, 22.0, if page == Page::Temps && s.cpu_temp().is_some_and(|c| c > 80.0) { RED } else { YELLOW });
    }
}
