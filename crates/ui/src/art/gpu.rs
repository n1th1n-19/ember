// SPDX-License-Identifier: GPL-3.0-or-later
//! "Graphics Card": dual-fan card in 3/4 view with bracket, heatsink fins on the top
//! edge and gold fingers. Fans spin with load; a card with no driver sleeps.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::{Point, Rectangle, Size, Vector};
use std::f32::consts::TAU;

use super::{Block, at, blades, darker, fit, polyline, sub};
use crate::ink::{self, CYAN, DISPLAY, GREY, INK, PAPER, RED, YELLOW};

/// `speed` 0..1 drives fan rotation; `awake=false` draws the sleeping card;
/// `hot` (real usage > 85%, not just a high clock) adds heat lines.
pub fn draw(f: &mut Frame, r: Rectangle, awake: bool, speed: f32, hot: bool, pose: u64) {
    let r = fit(r, 2.0);
    let accent = if awake { CYAN } else { GREY };
    let front = sub(r, 0.12, 0.24, 0.76, 0.58);
    let card = Block::new(front, Vector::new(r.width * 0.06, -r.height * 0.14));

    // PCIe bracket on the left (behind the card's front edge)
    let bracket = Rectangle::new(Point::new(front.x - r.width * 0.06, front.y - r.height * 0.1), Size::new(r.width * 0.05, front.height + r.height * 0.2));
    ink::boxed(f, bracket, GREY, 2.5, 3.0);
    let vents = Path::new(|b| {
        for k in 0..6 {
            let y = bracket.y + bracket.height * (0.18 + k as f32 * 0.1);
            b.move_to(Point::new(bracket.x + 4.0, y));
            b.line_to(Point::new(bracket.x + bracket.width - 4.0, y));
        }
    });
    f.stroke(&vents, ink::stroke(2.0));
    // screw tab
    ink::boxed(f, Rectangle::new(Point::new(bracket.x - 8.0, bracket.y), Size::new(bracket.width + 8.0, 10.0)), GREY, 2.0, 0.0);

    card.draw(f, if awake { PAPER } else { darker(PAPER, 0.85) }, darker(GREY, 0.9), INK);
    // heatsink fins across the top face
    let fins = Path::new(|b| {
        for k in 1..24 {
            let u = k as f32 / 24.0;
            b.move_to(card.on_top(u, 0.1));
            b.line_to(card.on_top(u, 0.9));
        }
    });
    f.stroke(&fins, ink::stroke(1.2));
    // shroud accent stripes
    let stripe = [at(front, 0.0, 0.0), at(front, 0.18, 0.0), at(front, 0.08, 1.0), at(front, 0.0, 1.0)];
    f.fill(&ink::poly(&stripe), accent);
    let stripe2 = [at(front, 0.82, 0.0), at(front, 1.0, 0.0), at(front, 1.0, 1.0), at(front, 0.92, 1.0)];
    f.fill(&ink::poly(&stripe2), accent);
    f.stroke(&ink::rect(front), ink::stroke(3.5));

    // two fans; rotation steps faster with load
    let step = if awake { (pose as f32 * (0.5 + speed * 2.0)).floor() } else { 0.0 };
    let rot = step * TAU / 28.0;
    let rad = front.height * 0.4;
    for (k, x) in [0.32f32, 0.68].iter().enumerate() {
        let c = at(front, *x, 0.5);
        f.fill(&Path::circle(c, rad + 4.0), darker(GREY, 0.6));
        f.stroke(&Path::circle(c, rad + 4.0), ink::stroke(2.5));
        f.fill(&Path::circle(c, rad), INK);
        blades(f, c, rad * 0.26, rad * 0.94, 9, rot + k as f32 * 0.3, if awake { PAPER } else { GREY });
        f.fill(&Path::circle(c, rad * 0.26), accent);
        f.stroke(&Path::circle(c, rad * 0.26), ink::stroke(2.0));
        if !awake {
            // closed sleepy eyes on each hub
            for dx in [-0.1, 0.1] {
                let e = c + Vector::new(rad * dx, 0.0);
                let lid = super::ellipse_pts(e, rad * 0.06, rad * 0.05, 0.0, std::f32::consts::PI, 6);
                polyline(f, &lid, ink::stroke(2.0));
            }
        }
    }

    // PCB edge + gold fingers below the shroud
    let pcb = Rectangle::new(Point::new(front.x, front.y + front.height), Size::new(front.width, r.height * 0.04));
    ink::boxed(f, pcb, darker(INK, 2.0), 2.0, 0.0);
    let fingers = Rectangle::new(Point::new(front.x + front.width * 0.1, pcb.y + pcb.height), Size::new(front.width * 0.5, r.height * 0.07));
    f.fill(&ink::rect(fingers), YELLOW);
    let pins = Path::new(|b| {
        for k in 1..18 {
            let x = fingers.x + fingers.width * k as f32 / 18.0;
            b.move_to(Point::new(x, fingers.y));
            b.line_to(Point::new(x, fingers.y + fingers.height));
        }
    });
    f.stroke(&pins, ink::stroke(1.2));
    f.stroke(&ink::rect(fingers), ink::stroke(2.5));
    // power connector on top edge
    let plug = card.on_top(0.85, 0.5);
    ink::boxed(f, Rectangle::new(plug + Vector::new(-10.0, -10.0), Size::new(20.0, 10.0)), INK, 1.5, 0.0);

    if awake && hot {
        // hot card: heat lines off the top
        let shift = if pose.is_multiple_of(2) { 3.0 } else { -3.0 };
        for k in 0..3 {
            let x0 = card.on_top(0.3 + k as f32 * 0.2, 0.5).x;
            let y0 = card.on_top(0.0, 1.0).y - 6.0;
            let wave: Vec<Point> = (0..4).map(|j| Point::new(x0 + if j % 2 == 0 { shift } else { -shift }, y0 - j as f32 * 6.0)).collect();
            polyline(f, &wave, Stroke::default().with_color(RED).with_width(2.0));
        }
    }
    if !awake {
        let float = (pose / 6 % 3) as f32 * 5.0;
        for k in 0..3 {
            let p = at(r, 0.86 + k as f32 * 0.05, 0.2 - k as f32 * 0.07) + Vector::new(0.0, -float);
            ink::text(f, "Z", p, 14.0 + k as f32 * 6.0, DISPLAY, ink::pal().scene_line, Alignment::Center, Vertical::Center);
        }
    }
}
