// SPDX-License-Identifier: GPL-3.0-or-later
//! "Radio Tower": lattice tower in 3/4 view on a hill, dish + blinking beacon,
//! radio shed with a lit window, signal rings = traffic level, packets flying in/out.
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::{Point, Rectangle, Size, Vector};
use std::f32::consts::PI;

use super::{Block, darker, disc, ellipse_pts, fit, polyline, sub};
use crate::ink::{self, CYAN, GREY, INK, PAPER, RED, WHITE, YELLOW};

/// `level` 0..=3 from traffic; rx/tx light the incoming/outgoing packets.
pub fn draw(f: &mut Frame, r: Rectangle, level: u8, rx: bool, tx: bool, pose: u64) {
    let r = fit(r, 0.95);
    let apex = Point::new(r.x + r.width * 0.42, r.y + r.height * 0.16);

    // rings behind everything, expanding 1 step per pose while active
    let pulse = if rx || tx { (pose % 4) as f32 * r.width * 0.02 } else { 0.0 };
    for k in 0..level.min(3) {
        let rad = r.width * (0.1 + k as f32 * 0.09) + pulse;
        for (a0, a1) in [(-PI * 0.42, -PI * 0.05), (-PI * 0.95, -PI * 0.58)] {
            let arc = ellipse_pts(apex, rad, rad, a0, a1, 10);
            polyline(f, &arc, Stroke::default().with_color(INK).with_width(7.0));
            polyline(f, &arc, Stroke::default().with_color(if k % 2 == 0 { CYAN } else { WHITE }).with_width(3.5));
        }
    }

    // hill
    let ground_y = r.y + r.height * 0.9;
    let mut hill = vec![Point::new(r.x, r.y + r.height)];
    hill.extend(ellipse_pts(Point::new(r.x + r.width * 0.5, ground_y + r.height * 0.06), r.width * 0.5, r.height * 0.08, PI, 2.0 * PI, 20));
    hill.push(Point::new(r.x + r.width, r.y + r.height));
    f.fill(&ink::poly(&hill), darker(PAPER, 0.9));
    f.stroke(&ink::poly(&hill), ink::stroke(3.0));
    // grass tufts
    let tufts = Path::new(|b| {
        for k in 0..7 {
            let x = r.x + r.width * (0.08 + k as f32 * 0.13);
            let y = ground_y + r.height * 0.02 + ((k % 3) as f32) * 3.0;
            b.move_to(Point::new(x - 4.0, y));
            b.line_to(Point::new(x, y - 7.0));
            b.line_to(Point::new(x + 4.0, y));
        }
    });
    f.stroke(&tufts, ink::stroke(1.5));

    // tower: back leg (thin) then front legs + X braces
    let fl = Point::new(r.x + r.width * 0.25, ground_y);
    let fr = Point::new(r.x + r.width * 0.55, ground_y);
    let back = Point::new(r.x + r.width * 0.5, ground_y - r.height * 0.05);
    let lerp = |a: Point, b: Point, t: f32| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    f.stroke(&Path::line(back, apex), ink::stroke(2.0));
    let levels = 7;
    let braces = Path::new(|b| {
        for k in 0..levels {
            let (t0, t1) = (k as f32 / levels as f32, (k + 1) as f32 / levels as f32);
            let (l0, r0, l1, r1) = (lerp(fl, apex, t0), lerp(fr, apex, t0), lerp(fl, apex, t1), lerp(fr, apex, t1));
            b.move_to(l0);
            b.line_to(r1);
            b.move_to(r0);
            b.line_to(l1);
            b.move_to(l1);
            b.line_to(r1);
            // side braces toward the back leg
            let (b0, b1) = (lerp(back, apex, t0), lerp(back, apex, t1));
            b.move_to(r0);
            b.line_to(b1);
            b.move_to(b0);
            b.line_to(r1);
        }
    });
    f.stroke(&braces, ink::stroke(2.0));
    f.stroke(&Path::line(fl, apex), ink::stroke(4.5));
    f.stroke(&Path::line(fr, apex), ink::stroke(4.5));
    // platform ring near the top
    let plat = lerp(Point::new((fl.x + fr.x) / 2.0, ground_y), apex, 0.72);
    polyline(f, &ellipse_pts(plat, r.width * 0.07, r.width * 0.02, 0.0, PI, 12), ink::stroke(3.0));
    f.stroke(&Path::line(plat + Vector::new(-r.width * 0.07, 0.0), plat + Vector::new(-r.width * 0.07, -10.0)), ink::stroke(2.0));
    f.stroke(&Path::line(plat + Vector::new(r.width * 0.07, 0.0), plat + Vector::new(r.width * 0.07, -10.0)), ink::stroke(2.0));

    // dish on an arm to the right
    let dish_c = lerp(Point::new((fl.x + fr.x) / 2.0, ground_y), apex, 0.5) + Vector::new(r.width * 0.12, 0.0);
    f.stroke(&Path::line(dish_c + Vector::new(-r.width * 0.12, 0.0), dish_c), ink::stroke(3.0));
    disc(f, dish_c + Vector::new(3.0, 3.0), r.width * 0.035, r.width * 0.075, INK, 0.0);
    disc(f, dish_c, r.width * 0.035, r.width * 0.075, WHITE, 3.0);
    disc(f, dish_c, r.width * 0.015, r.width * 0.035, darker(WHITE, 0.85), 1.5);
    f.stroke(&Path::line(dish_c, dish_c + Vector::new(r.width * 0.07, -r.width * 0.02)), ink::stroke(2.0));

    // mast + beacon: blinks 3 poses on / 3 off
    let tip = apex + Vector::new(0.0, -r.height * 0.07);
    f.stroke(&Path::line(apex, tip), ink::stroke(3.5));
    let on = (pose % 6) < 3;
    f.fill(&Path::circle(tip, r.width * 0.03), if on { RED } else { darker(RED, 0.5) });
    f.stroke(&Path::circle(tip, r.width * 0.03), ink::stroke(3.0));
    if on {
        let rays = Path::new(|b| {
            for k in 0..8 {
                let a = k as f32 * PI / 4.0;
                let d = Vector::new(a.cos(), a.sin());
                b.move_to(tip + d * (r.width * 0.045));
                b.line_to(tip + d * (r.width * 0.065));
            }
        });
        f.stroke(&rays, ink::stroke(2.0));
    }

    // radio shed with lit window when traffic flows
    let shed = Block::new(sub(r, 0.64, 0.74, 0.22, 0.16), Vector::new(r.width * 0.07, -r.height * 0.04));
    shed.draw(f, PAPER, RED, darker(PAPER, 0.75));
    let door = sub(shed.front, 0.12, 0.3, 0.26, 0.7);
    ink::boxed(f, door, GREY, 2.0, 0.0);
    let win = sub(shed.front, 0.52, 0.25, 0.34, 0.36);
    ink::boxed(f, win, if rx || tx { YELLOW } else { darker(GREY, 0.7) }, 2.0, 0.0);
    f.stroke(&Path::line(Point::new(win.center_x(), win.y), Point::new(win.center_x(), win.y + win.height)), ink::stroke(1.5));
    // cable shed → tower foot
    polyline(
        f,
        &[
            Point::new(shed.front.x, shed.front.y + shed.front.height * 0.3),
            Point::new(fr.x + 8.0, ground_y - r.height * 0.06),
            Point::new(fr.x, ground_y - 4.0),
        ],
        ink::stroke(2.0),
    );

    // packets: envelopes flying down (rx, cyan) and up (tx, red)
    let envelope = |f: &mut Frame, c: Point, color| {
        let s = Size::new(r.width * 0.07, r.width * 0.045);
        let b = Rectangle::new(Point::new(c.x - s.width / 2.0, c.y - s.height / 2.0), s);
        ink::boxed(f, b, color, 2.0, 2.0);
        polyline(f, &[b.position(), Point::new(b.center_x(), b.center_y() + 1.0), Point::new(b.x + b.width, b.y)], ink::stroke(1.5));
    };
    let t = (pose % 8) as f32 / 8.0;
    if rx {
        envelope(f, Point::new(r.x + r.width * (0.86 - 0.1 * t), r.y + r.height * (0.05 + 0.25 * t)), CYAN);
    }
    if tx {
        envelope(f, Point::new(r.x + r.width * (0.1 + 0.05 * t), r.y + r.height * (0.45 - 0.3 * t)), RED);
    }
}
