// SPDX-License-Identifier: GPL-3.0-or-later
//! "Engine Room": a riveted boiler in 3/4 view. Firebox flames = total CPU,
//! one piston per core on the top deck, pressure dial = clock, chimney smoke = load.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path};
use cosmic::iced::{Point, Rectangle, Size, Vector};

use super::{Block, at, cast_shadow, cylinder, darker, dial, fit, floor, puff, sub};
use crate::ink::{self, BODY, GREY, INK, Level, PAPER, RED, WHITE, YELLOW};

pub struct Engine<'a> {
    pub total: f32,
    pub cores: &'a [f32],
    /// clock / max clock, 0..1
    pub clock: f32,
}

pub fn draw(f: &mut Frame, r: Rectangle, e: &Engine, pose: u64) {
    let r = fit(r, 1.15);
    let level = Level::from_percent(e.total);
    let hot = level == Level::Hot;
    // whole machine rattles when hot (stepped, 4 poses)
    let shake = if hot { Vector::new([0.0, -2.0, 2.0, -1.0][(pose % 4) as usize], 0.0) } else { Vector::new(0.0, 0.0) };
    let floor_y = r.y + r.height * 0.93;

    if hot {
        ink::speed_lines(f, at(r, 0.45, 0.55), r.width * 0.52, r.width * 0.7, 30, pose);
    }
    floor(f, r, floor_y);

    // coal pile on the floor, left
    let coal = Path::new(|b| {
        for (x, s) in [(0.06, 0.035), (0.1, 0.045), (0.14, 0.03), (0.09, 0.03)] {
            let c = Point::new(r.x + r.width * x, floor_y - r.width * s * 0.7);
            b.circle(c, r.width * s);
        }
    });
    f.fill(&coal, INK);
    f.fill(&Path::circle(at(r, 0.095, 0.875), r.width * 0.008), WHITE);

    let front = sub(r, 0.2, 0.5, 0.56, 0.43) + shake;
    let body = Block::new(front, Vector::new(r.width * 0.13, -r.height * 0.1));
    cast_shadow(f, Point::new(front.x + front.width, floor_y), Point::new(front.x + front.width + body.depth.x, floor_y), r.width * 0.12);

    // chimney stands on the back-left of the deck, drawn before the boiler top
    let chim_base = body.on_top(0.1, 0.65);
    let chim_top = Point::new(chim_base.x, r.y + r.height * 0.12);
    cylinder(f, chim_top, r.width * 0.045, r.width * 0.014, chim_base.y - chim_top.y, GREY, INK);
    // smoke: more puffs with more load, rising 1 step every 3 poses
    let puffs = 1 + (e.total / 30.0) as usize;
    for i in 0..puffs.min(4) {
        let rise = ((pose / 3 + i as u64 * 2) % 6) as f32 * r.height * 0.015;
        let c = chim_top + Vector::new(r.width * (0.03 + i as f32 * 0.035), -r.height * (0.03 + i as f32 * 0.025) - rise);
        puff(f, c, r.width * (0.025 + i as f32 * 0.006));
    }

    body.draw(f, PAPER, darker(PAPER, 0.92), darker(PAPER, 0.75));

    // rivets along the front top and bottom
    let rivets = Path::new(|b| {
        for k in 0..10 {
            let x = front.x + front.width * (0.05 + k as f32 * 0.1);
            b.circle(Point::new(x, front.y + 7.0), 2.2);
            b.circle(Point::new(x, front.y + front.height - 7.0), 2.2);
        }
    });
    f.fill(&rivets, INK);
    if hot {
        ink::hatch(f, Rectangle::new(front.position() + Vector::new(4.0, 14.0), Size::new(front.width - 8.0, front.height * 0.08)), 6.0, 1.5, RED);
    }

    // firebox door with flames behind the glass
    let door = sub(front, 0.07, 0.24, 0.48, 0.64);
    ink::boxed(f, door, GREY, 3.0, 0.0);
    let glass = ink::inset(door, door.width * 0.08);
    f.fill(&ink::rect(glass), INK);
    let load = (e.total / 100.0).clamp(0.0, 1.0);
    let flame = |scale: f32, jitter: u64| {
        let h = glass.height * (0.1 + 0.85 * load) * scale;
        let base = glass.y + glass.height;
        let tongues = 6;
        let tw = glass.width / tongues as f32;
        let mut pts = vec![Point::new(glass.x, base)];
        for t in 0..tongues {
            let wob = [1.0, 0.75, 0.9][((t as u64 + jitter) % 3) as usize];
            pts.push(Point::new(glass.x + tw * (t as f32 + 0.5), base - h * wob));
            pts.push(Point::new(glass.x + tw * (t as f32 + 1.0), base - h * 0.4 * wob));
        }
        pts.push(Point::new(glass.x + glass.width, base));
        ink::poly(&pts)
    };
    f.fill(&flame(1.0, pose), RED);
    f.fill(&flame(0.62, pose + 1), YELLOW);
    f.fill(&flame(0.3, pose + 2), WHITE);
    // sparks when busy: little 4-point stars
    if load > 0.5 {
        for k in 0..3u64 {
            let x = glass.x + glass.width * (0.2 + 0.3 * k as f32);
            let y = glass.y + glass.height * (0.15 + ((pose + k * 2) % 4) as f32 * 0.08);
            let s = 4.0;
            let star = ink::poly(&[
                Point::new(x, y - s),
                Point::new(x + 1.2, y - 1.2),
                Point::new(x + s, y),
                Point::new(x + 1.2, y + 1.2),
                Point::new(x, y + s),
                Point::new(x - 1.2, y + 1.2),
                Point::new(x - s, y),
                Point::new(x - 1.2, y - 1.2),
            ]);
            f.fill(&star, YELLOW);
        }
    }
    // grate bars + door handle + hinges
    let bars = Path::new(|b| {
        for k in 1..5 {
            let x = glass.x + glass.width * k as f32 / 5.0;
            b.move_to(Point::new(x, glass.y));
            b.line_to(Point::new(x, glass.y + glass.height));
        }
    });
    f.stroke(&bars, ink::stroke(2.0).with_color(darker(GREY, 0.7)));
    f.stroke(&ink::rect(glass), ink::stroke(3.0));
    ink::boxed(f, Rectangle::new(Point::new(door.x + door.width - 4.0, door.center_y() - 6.0), Size::new(14.0, 12.0)), INK, 2.0, 0.0);
    for y in [0.2, 0.8] {
        f.fill(&ink::rect(Rectangle::new(Point::new(door.x - 5.0, door.y + door.height * y - 4.0), Size::new(8.0, 8.0))), INK);
    }

    // pressure dial = clock, with a brass plate under it
    let dc = at(front, 0.77, 0.42);
    let dr = (front.height * 0.26).min(front.width * 0.17);
    dial(f, dc, dr, e.clock);
    let plate = Rectangle::new(Point::new(dc.x - dr * 0.9, dc.y + dr + 8.0), Size::new(dr * 1.8, dr * 0.5));
    ink::boxed(f, plate, YELLOW, 2.0, 2.0);
    ink::text(f, "CLOCK", plate.center(), (dr * 0.32).max(8.0), BODY, INK, Alignment::Center, Vertical::Center);

    // pistons on the deck: 2 rows, back row first (painter's order)
    let n = e.cores.len().clamp(1, 16);
    let per_row = n.div_ceil(2);
    let cyl_rx = (front.width * 0.36 / per_row as f32).min(r.width * 0.03);
    let cyl_ry = cyl_rx * 0.35;
    for row in [1usize, 0] {
        for i in 0..per_row {
            let idx = row * per_row + i;
            let Some(u) = e.cores.get(idx) else { continue };
            let v = if row == 1 { 0.72 } else { 0.3 };
            let base = body.on_top(0.32 + 0.62 * (i as f32 + 0.5) / per_row as f32, v);
            let housing_h = r.height * 0.05;
            let top = base + Vector::new(0.0, -housing_h);
            // rod rises with that core's load
            let lift = (u / 100.0).clamp(0.0, 1.0) * r.height * 0.12;
            let rod_top = top + Vector::new(0.0, -lift - 4.0);
            f.fill(&ink::rect(Rectangle::new(Point::new(top.x - cyl_rx * 0.18, rod_top.y), Size::new(cyl_rx * 0.36, top.y - rod_top.y))), GREY);
            f.stroke(&ink::rect(Rectangle::new(Point::new(top.x - cyl_rx * 0.18, rod_top.y), Size::new(cyl_rx * 0.36, top.y - rod_top.y))), ink::stroke(1.5));
            super::disc(f, rod_top, cyl_rx * 0.7, cyl_ry * 0.7, Level::from_percent(*u).color(), 2.0);
            cylinder(f, top, cyl_rx, cyl_ry, housing_h, PAPER, darker(GREY, 0.8));
        }
    }

    // hot: whistle valve venting steam
    if hot {
        let v = body.on_top(0.95, 0.2);
        for k in 0..3 {
            puff(f, v + Vector::new(r.width * 0.05 + ((pose + k) % 2) as f32 * 3.0, -r.height * (0.05 + k as f32 * 0.06)), r.width * 0.03);
        }
    }
}
