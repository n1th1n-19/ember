// SPDX-License-Identifier: GPL-3.0-or-later
//! "Thermometer Rack": one thermometer per sensor group; mercury = °C / critical.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path};
use cosmic::iced::{Point, Rectangle, Size, Vector};

use super::fit;
use crate::ink::{self, Level, BODY, INK, RED, WHITE};

pub struct Gauge {
    pub label: String,
    pub celsius: f32,
    pub critical: f32,
}

pub fn draw(f: &mut Frame, r: Rectangle, gauges: &[Gauge], pose: u64) {
    let n = gauges.len().clamp(1, 6);
    let r = fit(r, (n as f32 * 0.32).max(0.6));
    let w = r.width / n as f32;
    for (i, g) in gauges.iter().take(n).enumerate() {
        let cx = r.x + w * (i as f32 + 0.5);
        let tube_w = (w * 0.3).min(r.height * 0.12);
        let bulb_r = tube_w * 0.95;
        let top = r.y + r.height * 0.05;
        let bulb_c = Point::new(cx, r.y + r.height * 0.78);
        let tube = Rectangle::new(Point::new(cx - tube_w / 2.0, top), Size::new(tube_w, bulb_c.y - top));
        let frac = (g.celsius / g.critical.max(1.0)).clamp(0.0, 1.0);
        let lvl = if frac >= 0.9 { Level::Hot } else { Level::from_celsius(g.celsius) };

        f.fill(&Path::rounded_rectangle(tube.position() + Vector::new(4.0, 4.0), tube.size(), (tube_w / 2.0).into()), INK);
        f.fill(&Path::circle(bulb_c + Vector::new(4.0, 4.0), bulb_r), INK);
        f.fill(&Path::rounded_rectangle(tube.position(), tube.size(), (tube_w / 2.0).into()), WHITE);
        let merc_top = tube.y + tube.height * (1.0 - frac);
        let merc = Rectangle::new(Point::new(cx - tube_w * 0.22, merc_top), Size::new(tube_w * 0.44, bulb_c.y - merc_top));
        f.fill(&ink::rect(merc), RED);
        f.fill(&Path::circle(bulb_c, bulb_r), RED);
        f.stroke(&Path::rounded_rectangle(tube.position(), tube.size(), (tube_w / 2.0).into()), ink::stroke(3.0));
        f.stroke(&Path::circle(bulb_c, bulb_r), ink::stroke(3.0));
        // ticks
        let ticks = Path::new(|b| {
            for k in 1..8 {
                let y = tube.y + tube.height * k as f32 / 8.0;
                b.move_to(Point::new(tube.x + tube.width, y));
                b.line_to(Point::new(tube.x + tube.width + tube_w * 0.35, y));
            }
        });
        f.stroke(&ticks, ink::stroke(1.5));
        // sweat drops when hot (alternate sides each pose)
        if lvl == Level::Hot {
            let side = if pose.is_multiple_of(2) { -1.0 } else { 1.0 };
            for k in 0..2 {
                let d = Point::new(cx + side * tube_w * 1.1, top + tube.height * (0.25 + k as f32 * 0.25));
                let drop = ink::poly(&[d + Vector::new(0.0, -6.0), d + Vector::new(4.0, 2.0), d + Vector::new(0.0, 5.0), d + Vector::new(-4.0, 2.0)]);
                f.fill(&drop, ink::CYAN);
                f.stroke(&drop, ink::stroke(1.5));
            }
        }
        ink::text(f, format!("{:.0}°", g.celsius), Point::new(cx, bulb_c.y + bulb_r + 4.0), (w * 0.28).min(26.0), ink::DISPLAY, INK, Alignment::Center, Vertical::Top);
        ink::text(f, g.label.clone(), Point::new(cx, r.y + r.height - 2.0), (w * 0.14).clamp(9.0, 13.0), BODY, INK, Alignment::Center, Vertical::Bottom);
    }
}
