// SPDX-License-Identifier: GPL-3.0-or-later
//! "Fan Box": spinning fan by RPM, or a taped-shut fan when firmware exposes none.
use cosmic::iced::widget::canvas::{path::Arc, Frame, Path};
use cosmic::iced::{Radians, Rectangle, Vector};
use std::f32::consts::TAU;

use super::{fit, sub};
use crate::ink::{self, GREY, INK, PAPER, YELLOW};

/// `speed` None = no fans exposed (taped). Some(0..1) = spinning.
pub fn draw(f: &mut Frame, r: Rectangle, speed: Option<f32>, pose: u64) {
    let r = fit(r, 1.0);
    let frame = sub(r, 0.1, 0.1, 0.8, 0.8);
    ink::boxed(f, frame, GREY, 4.0, 6.0);
    let c = frame.center();
    let rad = frame.width * 0.4;
    f.fill(&Path::circle(c, rad), INK);
    let rot = speed.map_or(0.0, |s| (pose as f32 * (0.5 + s * 2.0)).floor() * TAU / 40.0);
    let blades = Path::new(|b| {
        for k in 0..5 {
            let a = rot + k as f32 * TAU / 5.0;
            b.move_to(c);
            b.arc(Arc { center: c, radius: rad * 0.92, start_angle: Radians(a), end_angle: Radians(a + 0.8) });
            b.close();
        }
    });
    f.fill(&blades, PAPER);
    f.stroke(&blades, ink::stroke(2.0));
    f.fill(&Path::circle(c, rad * 0.2), GREY);
    f.stroke(&Path::circle(c, rad * 0.2), ink::stroke(2.0));
    if speed.is_none() {
        // two strips of tape in an X
        for angle in [0.6f32, -0.6] {
            f.with_save(|f| {
                f.translate(Vector::new(c.x, c.y));
                f.rotate(Radians(angle));
                let tape = cosmic::iced::Rectangle::new(cosmic::iced::Point::new(-rad * 1.25, -rad * 0.16), cosmic::iced::Size::new(rad * 2.5, rad * 0.32));
                ink::boxed(f, tape, YELLOW, 2.5, 3.0);
                // tape texture: short vertical ticks (no clip; clip ignores transforms)
                let ticks = Path::new(|b| {
                    let mut x = tape.x + 6.0;
                    while x < tape.x + tape.width - 4.0 {
                        b.move_to(cosmic::iced::Point::new(x, tape.y + 4.0));
                        b.line_to(cosmic::iced::Point::new(x - 3.0, tape.y + tape.height - 4.0));
                        x += 9.0;
                    }
                });
                f.stroke(&ticks, ink::stroke(1.0));
            });
        }
    }
}
