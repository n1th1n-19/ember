// SPDX-License-Identifier: GPL-3.0-or-later
//! "Fan Box": box fan in 3/4 view with grille. Spins by RPM and blows wind lines,
//! or sits taped shut with an "OUT OF ORDER" tag when firmware exposes no fans.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path};
use cosmic::iced::{Point, Rectangle, Size, Vector};
use std::f32::consts::TAU;

use super::{Block, at, blades, cast_shadow, darker, ellipse_pts, fit, floor, polyline, rot_rect, sub};
use crate::ink::{self, DISPLAY, GREY, INK, PAPER, WHITE, YELLOW};

/// `speed` None = no fans exposed (taped). Some(0..1) = spinning.
pub fn draw(f: &mut Frame, r: Rectangle, speed: Option<f32>, pose: u64) {
    let r = fit(r, 1.1);
    let floor_y = r.y + r.height * 0.93;
    floor(f, r, floor_y);
    let front = sub(r, 0.12, 0.1, 0.64, 0.74);
    let bx = Block::new(front, Vector::new(r.width * 0.12, -r.height * 0.08));
    // feet
    for x in [0.15, 0.75] {
        let foot =
            Rectangle::new(Point::new(front.x + front.width * x, front.y + front.height), Size::new(front.width * 0.12, floor_y - front.y - front.height));
        ink::boxed(f, foot, INK, 1.0, 0.0);
    }
    cast_shadow(f, Point::new(front.x + front.width, floor_y), Point::new(front.x + front.width + bx.depth.x, floor_y), r.width * 0.12);
    bx.draw(f, GREY, darker(GREY, 0.9), darker(GREY, 0.7));
    // carry handle on top
    let h0 = bx.on_top(0.35, 0.5);
    let h1 = bx.on_top(0.65, 0.5);
    polyline(f, &[h0, h0 + Vector::new(0.0, -12.0), h1 + Vector::new(0.0, -12.0), h1], ink::stroke(4.0));

    let c = front.center();
    let rad = front.width.min(front.height) * 0.42;
    f.fill(&Path::circle(c, rad), INK);
    let rot = speed.map_or(0.0, |s| (pose as f32 * (0.5 + s * 2.5)).floor() * TAU / 30.0);
    blades(f, c, rad * 0.2, rad * 0.93, 5, rot, PAPER);
    // grille: rings + spokes over the blades
    for k in 1..4 {
        f.stroke(&Path::circle(c, rad * k as f32 / 3.6), ink::stroke(1.2).with_color(darker(GREY, 0.85)));
    }
    let spokes = Path::new(|b| {
        for k in 0..12 {
            let a = k as f32 * TAU / 12.0;
            b.move_to(c + Vector::new(a.cos(), a.sin()) * (rad * 0.22));
            b.line_to(c + Vector::new(a.cos(), a.sin()) * rad);
        }
    });
    f.stroke(&spokes, ink::stroke(1.2).with_color(darker(GREY, 0.85)));
    f.stroke(&Path::circle(c, rad), ink::stroke(3.0));
    f.fill(&Path::circle(c, rad * 0.2), YELLOW);
    f.stroke(&Path::circle(c, rad * 0.2), ink::stroke(2.0));
    // speed knob
    let knob = at(front, 0.9, 0.9);
    f.fill(&Path::circle(knob, 7.0), INK);

    match speed {
        Some(s) => {
            // wind lines blowing right, more with speed
            let n = 1 + (s * 3.0) as usize;
            let shift = (pose % 3) as f32 * 6.0;
            for k in 0..n {
                let y = c.y - rad * 0.5 + k as f32 * rad * 0.35;
                let x0 = front.x + front.width + bx.depth.x + 8.0 + shift;
                let w: Vec<_> = ellipse_pts(Point::new(x0 + 20.0, y), 20.0, 6.0, std::f32::consts::PI, TAU, 6);
                polyline(f, &w, ink::stroke(2.5));
            }
        }
        None => {
            // tape X across the face (rotated rects as polygons)
            for a in [0.7f32, -0.7] {
                let tape = rot_rect(c, rad * 2.5, rad * 0.32, a);
                f.fill(&ink::poly(&tape.map(|p| p + Vector::new(3.0, 3.0))), INK);
                f.fill(&ink::poly(&tape), YELLOW);
                f.stroke(&ink::poly(&tape), ink::stroke(2.0));
                // torn-tape ticks
                let ticks = Path::new(|b| {
                    for k in 1..9 {
                        let t = k as f32 / 9.0;
                        let p0 = Point::new(tape[0].x + (tape[1].x - tape[0].x) * t, tape[0].y + (tape[1].y - tape[0].y) * t);
                        let p1 = Point::new(tape[3].x + (tape[2].x - tape[3].x) * t, tape[3].y + (tape[2].y - tape[3].y) * t);
                        b.move_to(p0);
                        b.line_to(Point::new(p0.x + (p1.x - p0.x) * 0.35, p0.y + (p1.y - p0.y) * 0.35));
                    }
                });
                f.stroke(&ticks, ink::stroke(1.0));
            }
            // OUT OF ORDER tag swinging on a string from the top-right corner (2 poses)
            let hook = Point::new(front.x + front.width * 0.88, front.y + 4.0);
            let swing = if (pose / 3).is_multiple_of(2) { 0.12 } else { -0.08 };
            let tag_c = hook + Vector::new(swing * 90.0, r.height * 0.2);
            polyline(f, &[hook, tag_c + Vector::new(0.0, -r.height * 0.05)], ink::stroke(1.5));
            let tag = rot_rect(tag_c, r.width * 0.26, r.height * 0.1, swing);
            f.fill(&ink::poly(&tag.map(|p| p + Vector::new(3.0, 3.0))), INK);
            f.fill(&ink::poly(&tag), WHITE);
            f.stroke(&ink::poly(&tag), ink::stroke(2.0));
            ink::text(f, "OUT OF ORDER", tag_c, (r.height * 0.045).max(10.0), DISPLAY, ink::RED, Alignment::Center, Vertical::Center);
            let _ = PAPER;
        }
    }
}
