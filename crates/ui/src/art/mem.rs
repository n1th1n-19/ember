// SPDX-License-Identifier: GPL-3.0-or-later
//! "Ink Tank": glass tank filled to RAM %, overflow pipe drips into a swap bucket.
use cosmic::iced::widget::canvas::{Frame, Path};
use cosmic::iced::{Point, Rectangle, Vector};

use super::{at, fit, sub};
use crate::ink::{self, Level, GREY, INK, MAGENTA, PAPER, WHITE};

pub fn draw(f: &mut Frame, r: Rectangle, ram: f32, swap: f32, pose: u64) {
    let r = fit(r, 1.0);
    let tank = sub(r, 0.1, 0.08, 0.55, 0.82);
    ink::boxed(f, tank, WHITE, 4.0, 6.0);
    let glass = ink::inset(tank, 4.0);

    // ink level with a stepped 2-pose wobble on the surface
    let lvl = ram.clamp(0.0, 1.0);
    let top = glass.y + glass.height * (1.0 - lvl);
    let wob = if pose.is_multiple_of(2) { 1.0 } else { -1.0 } * glass.height * 0.012;
    let waves = 6;
    let mut pts = vec![Point::new(glass.x, glass.y + glass.height)];
    for k in 0..=waves {
        let x = glass.x + glass.width * k as f32 / waves as f32;
        pts.push(Point::new(x, top + if k % 2 == 0 { wob } else { -wob }));
    }
    pts.push(Point::new(glass.x + glass.width, glass.y + glass.height));
    let liquid = ink::poly(&pts);
    f.fill(&liquid, MAGENTA);
    ink::halftone_where(f, ink::inset(glass, 3.0), INK, 9.0, 0.45, |p| p.y > top + glass.height * 0.04);
    f.stroke(&liquid, ink::stroke(3.0));

    // measurement ticks
    let ticks = Path::new(|b| {
        for k in 1..10 {
            let y = glass.y + glass.height * k as f32 / 10.0;
            let len = if k == 5 { 0.25 } else { 0.12 };
            b.move_to(Point::new(glass.x, y));
            b.line_to(Point::new(glass.x + glass.width * len, y));
        }
    });
    f.stroke(&ticks, ink::stroke(2.0));
    // glass shine: two short white strokes (flat, not a gradient)
    f.stroke(&Path::line(at(glass, 0.82, 0.08), at(glass, 0.82, 0.3)), cosmic::iced::widget::canvas::Stroke::default().with_color(WHITE).with_width(5.0));

    if Level::from_percent(ram * 100.0) == Level::Hot {
        let crack = Path::new(|b| {
            b.move_to(at(glass, 0.62, 0.0));
            b.line_to(at(glass, 0.55, 0.12));
            b.line_to(at(glass, 0.66, 0.2));
            b.line_to(at(glass, 0.58, 0.34));
        });
        f.stroke(&crack, ink::stroke(2.5));
    }

    // overflow pipe from tank top-right down to bucket
    let pipe = Path::new(|b| {
        b.move_to(at(tank, 1.0, 0.12));
        b.line_to(at(r, 0.82, 0.08 + 0.82 * 0.12));
        b.line_to(at(r, 0.82, 0.6));
    });
    f.stroke(&pipe, cosmic::iced::widget::canvas::Stroke::default().with_color(INK).with_width(r.width * 0.045));
    f.stroke(&pipe, cosmic::iced::widget::canvas::Stroke::default().with_color(GREY).with_width(r.width * 0.025));

    // swap bucket
    let bk = sub(r, 0.7, 0.68, 0.24, 0.22);
    let shape = |b: Rectangle| ink::poly(&[Point::new(b.x, b.y), Point::new(b.x + b.width, b.y), Point::new(b.x + b.width * 0.88, b.y + b.height), Point::new(b.x + b.width * 0.12, b.y + b.height)]);
    f.fill(&shape(bk + Vector::new(4.0, 4.0)), INK);
    f.fill(&shape(bk), PAPER);
    let s = swap.clamp(0.0, 1.0);
    if s > 0.0 {
        // trapezoid cut at the swap level: side edges narrow from 0 to 12% inset
        let y = bk.y + bk.height * (1.0 - s);
        let edge = |t: f32| bk.width * 0.12 * t;
        let t = 1.0 - s;
        let fill = ink::poly(&[
            Point::new(bk.x + edge(t), y),
            Point::new(bk.x + bk.width - edge(t), y),
            Point::new(bk.x + bk.width * 0.88, bk.y + bk.height),
            Point::new(bk.x + bk.width * 0.12, bk.y + bk.height),
        ]);
        f.fill(&fill, MAGENTA);
        // drip falls 3 poses out of 6
        let phase = pose % 6;
        if phase < 3 {
            let d = at(r, 0.82, 0.62) + Vector::new(0.0, phase as f32 * bk.height * 0.12);
            f.fill(&Path::circle(d, r.width * 0.014), MAGENTA);
            f.stroke(&Path::circle(d, r.width * 0.014), ink::stroke(1.5));
        }
    }
    f.stroke(&shape(bk), ink::stroke(3.0));
}
