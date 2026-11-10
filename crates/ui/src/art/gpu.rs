// SPDX-License-Identifier: GPL-3.0-or-later
//! "Graphics Card": fan spins with load; a card with no driver sleeps (ZZZ).
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{path::Arc, Frame, Path};
use cosmic::iced::{Point, Radians, Rectangle, Vector};
use std::f32::consts::{PI, TAU};

use super::{at, fit, sub};
use crate::ink::{self, CYAN, DISPLAY, GREY, INK, PAPER, YELLOW};

/// `speed` 0..1 drives fan rotation; `awake=false` draws the sleeping card.
pub fn draw(f: &mut Frame, r: Rectangle, awake: bool, speed: f32, pose: u64) {
    let r = fit(r, 1.7);
    let pcb = sub(r, 0.02, 0.1, 0.96, 0.72);
    ink::boxed(f, pcb, if awake { CYAN } else { GREY }, 4.0, 6.0);
    // gold fingers
    let fingers = sub(r, 0.18, 0.82, 0.5, 0.08);
    f.fill(&ink::rect(fingers), YELLOW);
    let pins = Path::new(|b| {
        for k in 1..12 {
            let x = fingers.x + fingers.width * k as f32 / 12.0;
            b.move_to(Point::new(x, fingers.y));
            b.line_to(Point::new(x, fingers.y + fingers.height));
        }
    });
    f.stroke(&pins, ink::stroke(1.5));
    f.stroke(&ink::rect(fingers), ink::stroke(2.5));
    // shroud
    let shroud = sub(pcb, 0.05, 0.1, 0.62, 0.8);
    ink::boxed(f, shroud, PAPER, 3.0, 0.0);
    // fan: 7 blades, rotation stepped in 8 poses, faster steps with speed
    let c = shroud.center();
    let rad = shroud.height * 0.42;
    f.fill(&Path::circle(c, rad), INK);
    let step = if awake { (pose as f32 * (0.5 + speed * 2.0)).floor() } else { 0.0 };
    let rot = step * (TAU / 7.0) / 8.0 * 3.0;
    let blades = Path::new(|b| {
        for k in 0..7 {
            let a = rot + k as f32 * TAU / 7.0;
            b.move_to(c);
            b.arc(Arc { center: c, radius: rad * 0.9, start_angle: Radians(a), end_angle: Radians(a + 0.55) });
            b.close();
        }
    });
    f.fill(&blades, if awake { PAPER } else { GREY });
    f.stroke(&blades, ink::stroke(1.5));
    f.fill(&Path::circle(c, rad * 0.22), INK);
    // chip
    let chip = sub(pcb, 0.73, 0.25, 0.2, 0.5);
    ink::boxed(f, chip, INK, 2.0, 0.0);

    if !awake {
        // sleepy eyes on the shroud
        for x in [0.3, 0.7] {
            let e = at(shroud, x, 0.18);
            let lid = Path::new(|b| b.arc(Arc { center: e, radius: shroud.width * 0.06, start_angle: Radians(0.0), end_angle: Radians(PI) }));
            f.stroke(&lid, ink::stroke(3.0));
        }
        let float = (pose / 6 % 3) as f32 * 6.0;
        for (k, s) in ["Z", "Z", "Z"].iter().enumerate() {
            let p = at(r, 0.78 + k as f32 * 0.07, 0.15 - k as f32 * 0.06) + Vector::new(0.0, -float);
            ink::text(f, *s, p, 16.0 + k as f32 * 6.0, DISPLAY, INK, Alignment::Center, Vertical::Center);
        }
    }
}
