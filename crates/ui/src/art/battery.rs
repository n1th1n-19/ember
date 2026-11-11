// SPDX-License-Identifier: GPL-3.0-or-later
//! "Power Cell": upright battery cell in 3/4 view on the floor. The glass charge window
//! fills with the charge; a cable snakes in from a wall plug while charging (bolt flickers);
//! under 15% the cell shakes and hazard stripes appear.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::{Color, Point, Rectangle, Size, Vector};
use std::f32::consts::PI;

use super::{Block, cast_shadow, cylinder, darker, ellipse_pts, fit, floor, polyline, sub};
use crate::ink::{self, DISPLAY, GREY, INK, PAPER, RED, WHITE, YELLOW};

const WRAP: Color = Color { r: 0.16, g: 0.15, b: 0.19, a: 1.0 };

pub fn draw(f: &mut Frame, r: Rectangle, pct: Option<f32>, charging: bool, pose: u64) {
    let low = pct.is_some_and(|p| p < 15.0);
    let shake = if low && !charging { [0.0, -3.0, 3.0, -1.0][(pose % 4) as usize] } else { 0.0 };
    let r = fit(r, 0.95);
    let floor_y = r.y + r.height * 0.94;
    floor(f, r, floor_y);

    let c = Point::new(r.x + r.width * 0.4 + shake, r.y + r.height * 0.2);
    let (rx, ry) = (r.width * 0.25, r.width * 0.065);
    let h = floor_y - c.y - ry;
    let bot = c + Vector::new(0.0, h);
    cast_shadow(f, Point::new(c.x, floor_y), Point::new(c.x + rx, floor_y), r.width * 0.14);

    // wall socket + cable into the terminal while charging (drawn behind the cell)
    let socket = Block::new(sub(r, 0.8, 0.7, 0.13, 0.17), Vector::new(r.width * 0.03, -r.height * 0.02));
    socket.draw(f, PAPER, darker(PAPER, 0.9), darker(PAPER, 0.7));
    for dx in [0.33, 0.67] {
        f.fill(&ink::rect(Rectangle::new(super::at(socket.front, dx, 0.3) + Vector::new(-2.0, 0.0), Size::new(4.0, socket.front.height * 0.3))), INK);
    }

    // body: dark wrapper with a yellow label band
    cylinder(f, c, rx, ry, h, WRAP, darker(GREY, 0.95));
    let band_top = c.y + h * 0.18;
    let band_bot = c.y + h * 0.42;
    let mut band = ellipse_pts(Point::new(c.x, band_bot), rx, ry, 0.0, PI, 20);
    band.extend(ellipse_pts(Point::new(c.x, band_top), rx, ry, PI, 0.0, 20));
    f.fill(&ink::poly(&band), if low { RED } else { YELLOW });
    f.stroke(&ink::poly(&band), ink::stroke(2.5));
    // shading band on the right of the wrapper (flat hatch, like the other machines)
    let x0 = 0.4f32;
    let mut shade = vec![Point::new(c.x + rx * x0, band_top + ry * 0.9), Point::new(c.x + rx, band_top)];
    shade.extend(ellipse_pts(Point::new(c.x, band_bot), rx, ry, 0.0, x0.acos(), 6));
    super::hatch_poly(f, &shade, 5.0, 1.2, Color { a: 0.45, ..INK });
    ink::text(
        f,
        "EMBER CELL",
        Point::new(c.x - rx * 0.1, (band_top + band_bot) / 2.0 + ry * 0.5),
        (rx * 0.2).clamp(10.0, 22.0),
        DISPLAY,
        INK,
        Alignment::Center,
        Vertical::Center,
    );

    // charge window: glass strip on the front, fills from the bottom
    let win = Rectangle::new(Point::new(c.x - rx * 0.32, band_bot + ry + h * 0.06), Size::new(rx * 0.5, bot.y - band_bot - ry - h * 0.12));
    ink::boxed(f, win, WHITE, 3.0, 0.0);
    let glass = ink::inset(win, 4.0);
    match pct {
        Some(p) => {
            let frac = (p / 100.0).clamp(0.0, 1.0);
            // creeps up one notch per pose while charging (stepped)
            let shown = if charging { (frac - 0.05 * (3 - pose % 4) as f32).max(0.0) } else { frac };
            let fill = Rectangle::new(Point::new(glass.x, glass.y + glass.height * (1.0 - shown)), Size::new(glass.width, glass.height * shown));
            let color = if low { RED } else { YELLOW };
            f.fill(&ink::rect(fill), color);
            ink::halftone(f, fill, darker(color, 0.7), 8.0, 0.5);
        }
        None => ink::hatch(f, glass, 8.0, 2.0, GREY),
    }
    let ticks = Path::new(|b| {
        for k in 1..5 {
            let y = glass.y + glass.height * k as f32 / 5.0;
            b.move_to(Point::new(glass.x, y));
            b.line_to(Point::new(glass.x + glass.width * 0.4, y));
        }
    });
    f.stroke(&ticks, ink::stroke(1.5));
    f.stroke(
        &Path::line(Point::new(glass.x + glass.width * 0.8, glass.y + 6.0), Point::new(glass.x + glass.width * 0.8, glass.y + glass.height * 0.4)),
        Stroke::default().with_color(WHITE).with_width(3.0),
    );

    // + terminal on the cap
    let term = c + Vector::new(0.0, -r.height * 0.05);
    cylinder(f, term, rx * 0.28, ry * 0.3, r.height * 0.05, GREY, darker(GREY, 1.1));
    ink::text(f, "+", c + Vector::new(rx * 0.6, 0.0), (rx * 0.3).max(12.0), DISPLAY, PAPER, Alignment::Center, Vertical::Center);

    if charging {
        // cable: socket → over the cell → clipped onto the terminal
        let plug = super::at(socket.front, 0.5, 0.45);
        let cable = [
            plug,
            plug + Vector::new(-r.width * 0.05, -r.height * 0.08),
            Point::new(c.x + rx * 0.9, r.y + r.height * 0.06),
            term + Vector::new(rx * 0.1, -4.0),
        ];
        polyline(f, &cable, Stroke::default().with_color(INK).with_width(7.0));
        polyline(f, &cable, Stroke::default().with_color(GREY).with_width(3.0));
        ink::boxed(f, Rectangle::new(term + Vector::new(-8.0, -14.0), Size::new(16.0, 10.0)), RED, 2.0, 0.0);
        // bolt on the label band flickers 2 poses
        if pose.is_multiple_of(2) {
            let bc = Point::new(c.x + rx * 0.62, (band_top + band_bot) / 2.0 + ry * 0.3);
            let s = (band_bot - band_top) * 0.42;
            let pts = [
                bc + Vector::new(s * 0.2, -s),
                bc + Vector::new(-s * 0.45, s * 0.1),
                bc + Vector::new(-s * 0.02, s * 0.1),
                bc + Vector::new(-s * 0.2, s),
                bc + Vector::new(s * 0.45, -s * 0.1),
                bc + Vector::new(s * 0.02, -s * 0.1),
            ];
            f.fill(&ink::poly(&pts.map(|p| p + Vector::new(3.0, 3.0))), INK);
            f.fill(&ink::poly(&pts), WHITE);
            f.stroke(&ink::poly(&pts), ink::stroke(2.0));
        }
    }

    if low && !charging {
        // hazard stripes across the base
        let y0 = bot.y - h * 0.1;
        let stripe = Path::new(|b| {
            let mut x = c.x - rx;
            while x < c.x + rx {
                b.move_to(Point::new(x, y0 + h * 0.08));
                b.line_to(Point::new(x + 8.0, y0));
                b.line_to(Point::new((x + 16.0).min(c.x + rx), y0));
                b.line_to(Point::new(x + 8.0, y0 + h * 0.08));
                b.close();
                x += 24.0;
            }
        });
        f.fill(&stripe, YELLOW);
    }
}
