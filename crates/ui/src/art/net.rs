// SPDX-License-Identifier: GPL-3.0-or-later
//! "Radio Tower": lattice tower; signal rings = traffic level, pulsing while active.
use cosmic::iced::widget::canvas::{path::Arc, Frame, Path};
use cosmic::iced::{Point, Radians, Rectangle, Vector};
use std::f32::consts::PI;

use super::{at, fit};
use crate::ink::{self, CYAN, INK, PAPER, RED, YELLOW};

/// `level` 0..=3 from traffic; rx/tx active flags light the arrows.
pub fn draw(f: &mut Frame, r: Rectangle, level: u8, rx: bool, tx: bool, pose: u64) {
    let r = fit(r, 0.9);
    let top = at(r, 0.5, 0.18);
    let (bl, br) = (at(r, 0.25, 0.95), at(r, 0.75, 0.95));
    // legs + cross bracing
    let tower = Path::new(|b| {
        b.move_to(bl);
        b.line_to(top);
        b.line_to(br);
        for k in 1..6 {
            let t = k as f32 / 6.0;
            let l = Point::new(top.x + (bl.x - top.x) * t, top.y + (bl.y - top.y) * t);
            let rr = Point::new(top.x + (br.x - top.x) * t, top.y + (br.y - top.y) * t);
            let t2 = (k + 1) as f32 / 6.0;
            let l2 = Point::new(top.x + (bl.x - top.x) * t2, top.y + (bl.y - top.y) * t2);
            let r2 = Point::new(top.x + (br.x - top.x) * t2, top.y + (br.y - top.y) * t2);
            b.move_to(l);
            b.line_to(rr);
            b.move_to(l);
            b.line_to(r2);
            b.move_to(rr);
            b.line_to(l2);
        }
    });
    f.stroke(&tower, ink::stroke(3.5));
    f.stroke(&Path::line(top, top + Vector::new(0.0, -r.height * 0.08)), ink::stroke(4.0));
    let ball = top + Vector::new(0.0, -r.height * 0.1);
    f.fill(&Path::circle(ball, r.width * 0.035), RED);
    f.stroke(&Path::circle(ball, r.width * 0.035), ink::stroke(3.0));

    // rings: count = level, outward pulse in 4 poses while active
    let pulse = if rx || tx { (pose % 4) as f32 * r.width * 0.015 } else { 0.0 };
    for k in 0..level.min(3) {
        let rad = r.width * (0.12 + k as f32 * 0.1) + pulse;
        for (a0, a1) in [(-PI * 0.35, PI * 0.1), (PI * 0.9, PI * 1.35)] {
            let arc = Path::new(|b| b.arc(Arc { center: ball, radius: rad, start_angle: Radians(a0), end_angle: Radians(a1) }));
            f.stroke(&arc, ink::stroke(4.0).with_color(if k % 2 == 0 { CYAN } else { INK }));
        }
    }

    // down / up arrows
    let arrow = |f: &mut Frame, c: Point, down: bool, on: bool, color| {
        let s = r.width * 0.07;
        let d = if down { 1.0 } else { -1.0 };
        let pts = [
            c + Vector::new(-s * 0.4, -s * d),
            c + Vector::new(s * 0.4, -s * d),
            c + Vector::new(s * 0.4, 0.0),
            c + Vector::new(s * 0.8, 0.0),
            c + Vector::new(0.0, s * d),
            c + Vector::new(-s * 0.8, 0.0),
            c + Vector::new(-s * 0.4, 0.0),
        ];
        let p = ink::poly(&pts);
        f.fill(&ink::poly(&pts.map(|q| q + Vector::new(3.0, 3.0))), INK);
        f.fill(&p, if on { color } else { PAPER });
        f.stroke(&p, ink::stroke(2.5));
    };
    arrow(f, at(r, 0.12, 0.55), true, rx, CYAN);
    arrow(f, at(r, 0.88, 0.55), false, tx, RED);
    if level >= 3 {
        ink::speed_lines(f, ball, r.width * 0.46, r.width * 0.56, 20, pose);
        f.fill(&Path::circle(ball, r.width * 0.012), YELLOW);
    }
}
