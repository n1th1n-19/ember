// SPDX-License-Identifier: GPL-3.0-or-later
//! "Power Cell": yellow halftone fill = charge, bolt flickers while charging, shakes when low.
use cosmic::iced::widget::canvas::{Frame, Path};
use cosmic::iced::{Point, Rectangle, Size, Vector};

use super::{fit, sub};
use crate::ink::{self, GREY, INK, PAPER, RED, WHITE, YELLOW};

pub fn draw(f: &mut Frame, r: Rectangle, pct: Option<f32>, charging: bool, pose: u64) {
    let low = pct.is_some_and(|p| p < 15.0);
    let shake = if low && !charging { [0.0, -4.0, 3.0, -1.0][(pose % 4) as usize] } else { 0.0 };
    let r = fit(r, 1.9) + Vector::new(shake, 0.0);
    let body = sub(r, 0.04, 0.1, 0.84, 0.8);
    let nub = sub(r, 0.88, 0.34, 0.08, 0.32);
    ink::boxed(f, nub, GREY, 3.0, 4.0);
    ink::boxed(f, body, WHITE, 5.0, 6.0);
    let cell = ink::inset(body, 8.0);
    let frac = pct.unwrap_or(0.0) / 100.0;
    // fill creeps one notch per pose while charging (stepped)
    let shown = if charging { (frac - 0.04 * (3 - pose % 4) as f32).max(0.0) } else { frac };
    let fill = Rectangle::new(cell.position(), Size::new(cell.width * shown.clamp(0.0, 1.0), cell.height));
    let color = if low { RED } else { YELLOW };
    f.fill(&ink::rect(fill), color);
    ink::halftone(f, fill, Color8::shade(color), 10.0, 0.5);
    // 5 segment dividers
    let seg = Path::new(|b| {
        for k in 1..5 {
            let x = cell.x + cell.width * k as f32 / 5.0;
            b.move_to(Point::new(x, cell.y));
            b.line_to(Point::new(x, cell.y + cell.height));
        }
    });
    f.stroke(&seg, ink::stroke(2.0));
    if pct.is_none() {
        ink::hatch(f, cell, 10.0, 2.0, GREY);
    }
    if charging && pose.is_multiple_of(2) {
        let c = cell.center();
        let s = cell.height * 0.42;
        let pts = [
            c + Vector::new(s * 0.2, -s),
            c + Vector::new(-s * 0.45, s * 0.1),
            c + Vector::new(-s * 0.02, s * 0.1),
            c + Vector::new(-s * 0.2, s),
            c + Vector::new(s * 0.45, -s * 0.1),
            c + Vector::new(s * 0.02, -s * 0.1),
        ];
        f.fill(&ink::poly(&pts.map(|p| p + Vector::new(4.0, 4.0))), INK);
        f.fill(&ink::poly(&pts), PAPER);
        f.stroke(&ink::poly(&pts), ink::stroke(3.0));
    }
}

/// Darker dot color for halftone on a flat fill (still flat, just a second ink).
struct Color8;
impl Color8 {
    fn shade(c: cosmic::iced::Color) -> cosmic::iced::Color {
        cosmic::iced::Color { r: c.r * 0.8, g: c.g * 0.65, b: c.b * 0.4, a: 1.0 }
    }
}
