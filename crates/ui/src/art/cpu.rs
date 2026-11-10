// SPDX-License-Identifier: GPL-3.0-or-later
//! "Engine Room": boiler with firebox flames = total CPU, pistons = cores, gauge = clock.
use cosmic::iced::widget::canvas::{Frame, Path};
use cosmic::iced::{Point, Rectangle, Size, Vector};

use super::{at, fit, sub};
use crate::ink::{self, Level, GREY, INK, PAPER, RED, WHITE, YELLOW};

pub struct Engine<'a> {
    pub total: f32,
    pub cores: &'a [f32],
    /// clock / max clock, 0..1
    pub clock: f32,
}

pub fn draw(f: &mut Frame, r: Rectangle, e: &Engine, pose: u64) {
    let r = fit(r, 1.1);
    let level = Level::from_percent(e.total);
    let body = sub(r, 0.12, 0.42, 0.76, 0.5);

    // chimney + smoke
    let chim = sub(r, 0.16, 0.22, 0.1, 0.22);
    ink::boxed(f, chim, GREY, 3.0, 3.0);
    let puffs = 1 + (e.total / 34.0) as usize;
    for i in 0..puffs {
        let rise = ((pose / 3 + i as u64 * 2) % 6) as f32;
        let c = at(r, 0.21 + i as f32 * 0.05, 0.18) + Vector::new(0.0, -rise * r.height * 0.018);
        let rad = r.width * (0.03 + i as f32 * 0.008);
        f.fill(&Path::circle(c, rad), WHITE);
        f.stroke(&Path::circle(c, rad), ink::stroke(2.0));
    }

    // pistons on top: one per core, rod height = that core's usage
    let n = e.cores.len().clamp(1, 16);
    let row = sub(r, 0.3, 0.06, 0.56, 0.36);
    let w = row.width / n as f32;
    for (i, u) in e.cores.iter().take(n).enumerate() {
        let x = row.x + i as f32 * w;
        let cyl = Rectangle::new(Point::new(x + w * 0.15, row.y + row.height * 0.55), Size::new(w * 0.7, row.height * 0.45));
        // rod + head (head rises with load, snapped to 4 positions per pose step)
        let lift = (u / 100.0).clamp(0.0, 1.0);
        let head_y = cyl.y - row.height * 0.5 * lift;
        f.fill(&ink::rect(Rectangle::new(Point::new(x + w * 0.42, head_y), Size::new(w * 0.16, cyl.y - head_y + 2.0))), GREY);
        let head = Rectangle::new(Point::new(x + w * 0.22, head_y - row.height * 0.08), Size::new(w * 0.56, row.height * 0.08));
        ink::boxed(f, head, Level::from_percent(*u).color(), 2.0, 0.0);
        ink::boxed(f, cyl, PAPER, 2.5, 2.0);
    }

    // boiler body with rivets
    ink::boxed(f, body, PAPER, 4.0, 6.0);
    let rivets = Path::new(|b| {
        for k in 0..9 {
            b.circle(at(body, 0.06 + k as f32 * 0.11, 0.07), body.width * 0.012);
        }
    });
    f.fill(&rivets, INK);
    if level == Level::Hot {
        ink::hatch(f, sub(body, 0.0, 0.0, 1.0, 0.16), 6.0, 1.5, RED);
    }

    // firebox + flames (height = total CPU)
    let fire = sub(body, 0.08, 0.25, 0.5, 0.62);
    f.fill(&ink::rect(fire), INK);
    let tongues = 5;
    let flame = |scale: f32, jitter: u64| {
        let h = fire.height * (0.12 + 0.86 * (e.total / 100.0).clamp(0.0, 1.0)) * scale;
        let base = fire.y + fire.height;
        let tw = fire.width / tongues as f32;
        let mut pts = vec![Point::new(fire.x, base)];
        for t in 0..tongues {
            let wob = [1.0, 0.78, 0.9][((t as u64 + jitter) % 3) as usize];
            pts.push(Point::new(fire.x + tw * (t as f32 + 0.5), base - h * wob));
            pts.push(Point::new(fire.x + tw * (t as f32 + 1.0), base - h * 0.45 * wob));
        }
        pts.push(Point::new(fire.x + fire.width, base));
        ink::poly(&pts)
    };
    f.fill(&flame(1.0, pose), RED);
    f.fill(&flame(0.6, pose + 1), YELLOW);
    f.stroke(&ink::rect(fire), ink::stroke(4.0));
    // grate bars
    let grate = Path::new(|b| {
        for k in 1..4 {
            let x = fire.x + fire.width * k as f32 / 4.0;
            b.move_to(Point::new(x, fire.y));
            b.line_to(Point::new(x, fire.y + fire.height * 0.25));
        }
    });
    f.stroke(&grate, ink::stroke(3.0));

    // pressure gauge = clock
    let gc = at(body, 0.8, 0.55);
    crate::chart::gauge(f, gc, body.width * 0.15, e.clock, "MHz");

    // steam + speed lines when hot
    if level == Level::Hot {
        ink::speed_lines(f, r.center(), r.width * 0.55, r.width * 0.75, 28, pose);
        for k in 0..3 {
            let c = at(r, 0.9, 0.45 - k as f32 * 0.1) + Vector::new(((pose + k) % 2) as f32 * 4.0, 0.0);
            f.fill(&Path::circle(c, r.width * 0.035), WHITE);
            f.stroke(&Path::circle(c, r.width * 0.035), ink::stroke(2.0));
        }
    }
}
