// SPDX-License-Identifier: GPL-3.0-or-later
//! Printed-style charts: white paper box, ink axis, flat fills + halftone, 3px ink lines.
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::{Color, Point, Radians, Rectangle, Size, Vector};
use ember_core::series::Series;
use std::f32::consts::PI;

use crate::ink::{self, BODY, INK, PAPER, WHITE};

fn y_of(r: Rectangle, v: f32, max: f32) -> f32 {
    r.y + r.height - (v / max).clamp(0.0, 1.0) * r.height
}

/// Points of a series spread over the full width (right-aligned: newest at the right edge).
fn points(r: Rectangle, s: &Series, max: f32, flip: bool) -> Vec<Option<Point>> {
    let n = s.cap().max(2);
    let dx = r.width / (n - 1) as f32;
    let start = n - s.len();
    s.iter()
        .enumerate()
        .map(|(i, v)| {
            v.map(|v| {
                let x = r.x + (start + i) as f32 * dx;
                let frac = (v / max).clamp(0.0, 1.0);
                let y = if flip { r.y + frac * r.height } else { r.y + r.height - frac * r.height };
                Point::new(x, y)
            })
        })
        .collect()
}

/// Contiguous runs of Some points (gaps break the line).
fn runs(pts: &[Option<Point>]) -> Vec<Vec<Point>> {
    pts.split(Option::is_none).map(|r| r.iter().flatten().copied().collect::<Vec<_>>()).filter(|r| r.len() > 1).collect()
}

fn paper_box(f: &mut Frame, r: Rectangle) -> Rectangle {
    let p = ink::pal();
    ink::boxed(f, r, p.chart_bg, 3.0, 4.0);
    let inner = ink::inset(r, 8.0);
    // faint guide lines at 25/50/75%
    let guides = Path::new(|b| {
        for k in 1..4 {
            let y = inner.y + inner.height * k as f32 / 4.0;
            b.move_to(Point::new(inner.x, y));
            b.line_to(Point::new(inner.x + inner.width, y));
        }
    });
    f.stroke(&guides, Stroke::default().with_color(Color { a: 0.15, ..p.text }).with_width(1.0));
    inner
}

/// Filled area under one series, halftone dots in the fill, ink line on top.
pub fn area(f: &mut Frame, r: Rectangle, s: &Series, max: f32, color: Color) {
    let inner = paper_box(f, r);
    area_in(f, inner, s, max, color, false);
}

fn area_in(f: &mut Frame, r: Rectangle, s: &Series, max: f32, color: Color, flip: bool) {
    let base = if flip { r.y } else { r.y + r.height };
    for run in runs(&points(r, s, max, flip)) {
        let shape = Path::new(|b| {
            b.move_to(Point::new(run[0].x, base));
            run.iter().for_each(|p| b.line_to(*p));
            b.line_to(Point::new(run[run.len() - 1].x, base));
            b.close();
        });
        f.fill(&shape, Color { a: 0.35, ..color });
        // halftone only below the line: interpolate the run's height at each dot's x
        let (x0, x1) = (run[0].x, run[run.len() - 1].x);
        let bounds = Rectangle::new(Point::new(x0, r.y), Size::new(x1 - x0, r.height));
        ink::halftone_where(f, bounds, color, 7.0, 0.8, |p| {
            let i = run.partition_point(|q| q.x < p.x).clamp(1, run.len() - 1);
            let (a, b) = (run[i - 1], run[i]);
            let t = if b.x > a.x { (p.x - a.x) / (b.x - a.x) } else { 0.0 };
            let y = a.y + (b.y - a.y) * t;
            if flip { p.y < y } else { p.y > y }
        });
        let line = Path::new(|b| {
            b.move_to(run[0]);
            run[1..].iter().for_each(|p| b.line_to(*p));
        });
        f.stroke(&line, ink::stroke(3.0).with_color(ink::pal().chart_line));
    }
}

/// Two series around a middle axis: `up` above (e.g. download), `down` below (upload).
pub fn mirrored(f: &mut Frame, r: Rectangle, up: &Series, down: &Series, max: f32, c_up: Color, c_down: Color) {
    let inner = paper_box(f, r);
    let half = inner.height / 2.0;
    let top = Rectangle::new(inner.position(), Size::new(inner.width, half));
    let bot = Rectangle::new(Point::new(inner.x, inner.y + half), Size::new(inner.width, half));
    area_in(f, top, up, max, c_up, false);
    area_in(f, bot, down, max, c_down, true);
    f.stroke(&Path::line(Point::new(inner.x, inner.y + half), Point::new(inner.x + inner.width, inner.y + half)), ink::stroke(2.0).with_color(ink::pal().chart_line));
}

/// Vertical bars 0..100 (per-core strip). Bar color follows its own level.
pub fn bars(f: &mut Frame, r: Rectangle, values: &[f32]) {
    let inner = paper_box(f, r);
    if values.is_empty() {
        return;
    }
    let gap = 4.0;
    let w = ((inner.width - gap * (values.len() - 1) as f32) / values.len() as f32).max(2.0);
    for (i, v) in values.iter().enumerate() {
        let x = inner.x + i as f32 * (w + gap);
        let y = y_of(inner, *v, 100.0);
        let bar = Rectangle::new(Point::new(x, y), Size::new(w, inner.y + inner.height - y));
        f.fill(&ink::rect(bar), ink::Level::from_percent(*v).color());
        f.stroke(&ink::rect(bar), ink::stroke(2.0));
    }
}

/// Horizontal progress bar with hatch on the empty part.
pub fn hbar(f: &mut Frame, r: Rectangle, frac: f32, color: Color) {
    ink::boxed(f, r, PAPER, 3.0, 3.0);
    let inner = ink::inset(r, 3.0);
    ink::hatch(f, inner, 8.0, 1.0, Color { a: 0.35, ..INK });
    let filled = Rectangle::new(inner.position(), Size::new(inner.width * frac.clamp(0.0, 1.0), inner.height));
    f.fill(&ink::rect(filled), color);
    f.stroke(&Path::line(Point::new(filled.x + filled.width, inner.y), Point::new(filled.x + filled.width, inner.y + inner.height)), ink::stroke(3.0));
}

fn arc_path(c: Point, r: f32, a0: f32, a1: f32) -> Path {
    Path::new(|b| b.arc(cosmic::iced::widget::canvas::path::Arc { center: c, radius: r, start_angle: Radians(a0), end_angle: Radians(a1) }))
}

/// Thick ring (donut) showing a fraction, with a label in the middle.
pub fn ring(f: &mut Frame, c: Point, r: f32, frac: f32, color: Color, label: &str) {
    let w = r * 0.32;
    f.fill(&Path::circle(c + Vector::new(4.0, 4.0), r), INK);
    f.fill(&Path::circle(c, r), WHITE);
    let a0 = -PI / 2.0;
    let a1 = a0 + frac.clamp(0.0, 1.0) * 2.0 * PI;
    f.stroke(&arc_path(c, r - w / 2.0, a0, a1), Stroke::default().with_color(color).with_width(w));
    f.stroke(&Path::circle(c, r), ink::stroke(3.0));
    f.stroke(&Path::circle(c, r - w), ink::stroke(3.0));
    ink::text(f, label, c, r * 0.42, ink::DISPLAY, INK, Alignment::Center, Vertical::Center);
}

/// Half-circle dial with 10 ticks and a needle. `frac` 0..1.
pub fn gauge(f: &mut Frame, c: Point, r: f32, frac: f32, label: &str) {
    let face_at = |c: Point| {
        Path::new(|b| {
            b.arc(cosmic::iced::widget::canvas::path::Arc { center: c, radius: r, start_angle: Radians(PI), end_angle: Radians(2.0 * PI) });
            b.close();
        })
    };
    let face = face_at(c);
    f.fill(&face_at(c + Vector::new(4.0, 4.0)), INK);
    f.fill(&face, WHITE);
    // red zone last 20%
    f.stroke(&arc_path(c, r * 0.85, PI + 0.8 * PI, 2.0 * PI), Stroke::default().with_color(ink::RED).with_width(r * 0.12));
    f.stroke(&face, ink::stroke(3.0));
    let ticks = Path::new(|b| {
        for i in 0..=10 {
            let a = PI + i as f32 / 10.0 * PI;
            let d = Vector::new(a.cos(), a.sin());
            b.move_to(c + d * (r * 0.72));
            b.line_to(c + d * (r * 0.92));
        }
    });
    f.stroke(&ticks, ink::stroke(2.0));
    let a = PI + frac.clamp(0.0, 1.0) * PI;
    f.stroke(&Path::line(c, c + Vector::new(a.cos(), a.sin()) * (r * 0.8)), ink::stroke(4.0));
    f.fill(&Path::circle(c, r * 0.08), INK);
    ink::text(f, label, c + Vector::new(0.0, -r * 0.35), r * 0.22, BODY, INK, Alignment::Center, Vertical::Center);
}

/// Row of cells, each colored by its own level. `cells` = (label, value, max).
pub fn heat_strip(f: &mut Frame, r: Rectangle, cells: &[(String, f32, f32)]) {
    if cells.is_empty() {
        return;
    }
    let w = r.width / cells.len() as f32;
    for (i, (label, v, max)) in cells.iter().enumerate() {
        let cell = Rectangle::new(Point::new(r.x + i as f32 * w, r.y), Size::new(w, r.height));
        let lvl = if *v >= max * 0.9 { ink::Level::Hot } else { ink::Level::from_celsius(*v) };
        f.fill(&ink::rect(cell), lvl.color());
        f.stroke(&ink::rect(cell), ink::stroke(2.0));
        ink::text(f, format!("{v:.0}°"), Point::new(cell.center_x(), cell.y + cell.height * 0.38), (w * 0.32).min(22.0), ink::DISPLAY, INK, Alignment::Center, Vertical::Center);
        ink::text(f, label.clone(), Point::new(cell.center_x(), cell.y + cell.height * 0.78), (w * 0.16).clamp(8.0, 12.0), BODY, INK, Alignment::Center, Vertical::Center);
    }
    f.stroke(&ink::rect(r), ink::stroke(3.0));
}
