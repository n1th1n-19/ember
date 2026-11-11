// SPDX-License-Identifier: GPL-3.0-or-later
//! "Thermometer Rack": wooden rack in 3/4 view holding glass thermometers on scale
//! plates. Mercury = °C vs critical; hot ones sweat and shimmer.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::{Color, Point, Rectangle, Size, Vector};

use super::{Block, darker, fit, polyline, sub};
use crate::ink::{self, BODY, CYAN, DISPLAY, GREY, INK, Level, PAPER, RED, WHITE};

pub struct Gauge {
    pub label: String,
    pub celsius: f32,
    pub critical: f32,
}

const WOOD: Color = Color { r: 0.80, g: 0.55, b: 0.30, a: 1.0 };

pub fn draw(f: &mut Frame, r: Rectangle, gauges: &[Gauge], pose: u64) {
    let n = gauges.len().clamp(1, 6);
    let r = fit(r, (0.28 * n as f32 + 0.2).clamp(0.7, 1.6));
    let depth = Vector::new(r.width * 0.05, -r.height * 0.035);

    // back board with grain + screws
    let board = Block::new(sub(r, 0.04, 0.04, 0.88, 0.8), depth);
    board.draw(f, WOOD, darker(WOOD, 0.9), darker(WOOD, 0.7));
    let grain = Path::new(|b| {
        for k in 0..6 {
            let y = board.front.y + board.front.height * (0.1 + k as f32 * 0.16);
            b.move_to(Point::new(board.front.x + 6.0, y));
            b.line_to(Point::new(board.front.x + board.front.width * 0.4, y + 3.0));
            b.line_to(Point::new(board.front.x + board.front.width - 6.0, y - 2.0));
        }
    });
    f.stroke(&grain, Stroke::default().with_color(darker(WOOD, 0.6)).with_width(1.2));
    for (x, y) in [(0.03, 0.03), (0.97, 0.03), (0.03, 0.97), (0.97, 0.97)] {
        let p = super::at(board.front, x, y) + Vector::new(if x < 0.5 { 6.0 } else { -6.0 }, if y < 0.5 { 6.0 } else { -6.0 });
        f.fill(&Path::circle(p, 4.0), GREY);
        f.stroke(&Path::circle(p, 4.0), ink::stroke(1.5));
        f.stroke(&Path::line(p + Vector::new(-2.5, 0.0), p + Vector::new(2.5, 0.0)), ink::stroke(1.2));
    }
    // shelf at the bottom
    let shelf = Block::new(sub(r, 0.0, 0.84, 0.94, 0.08), Vector::new(r.width * 0.06, -r.height * 0.05));
    shelf.draw(f, WOOD, darker(WOOD, 0.95), darker(WOOD, 0.7));

    let col = board.front.width / n as f32;
    for (i, g) in gauges.iter().take(n).enumerate() {
        let cx = board.front.x + col * (i as f32 + 0.5);
        let frac = (g.celsius / g.critical.max(1.0)).clamp(0.0, 1.0);
        let hot = frac >= 0.9 || Level::from_celsius(g.celsius) == Level::Hot;

        // scale plate
        let plate = Rectangle::new(Point::new(cx - col * 0.36, board.front.y + board.front.height * 0.07), Size::new(col * 0.72, board.front.height * 0.82));
        ink::boxed(f, plate, PAPER, 2.0, 3.0);
        let tube_w = (col * 0.18).min(r.height * 0.07);
        let top = plate.y + plate.height * 0.06;
        let bulb_r = tube_w * 1.05;
        let bulb_c = Point::new(cx, plate.y + plate.height - bulb_r * 1.3 - (col * 0.2).clamp(12.0, 24.0) - 6.0);
        let tube = Rectangle::new(Point::new(cx - tube_w / 2.0, top), Size::new(tube_w, bulb_c.y - top));
        // tick marks + 3 numbers (0, half, critical)
        let ticks = Path::new(|b| {
            for k in 0..=10 {
                let y = tube.y + tube.height * k as f32 / 10.0;
                let len = if k % 5 == 0 { 0.32 } else { 0.18 };
                b.move_to(Point::new(tube.x + tube.width + 3.0, y));
                b.line_to(Point::new(tube.x + tube.width + 3.0 + col * len * 0.5, y));
            }
        });
        f.stroke(&ticks, ink::stroke(1.2));
        let num_size = (col * 0.1).clamp(8.0, 11.0);
        for (k, v) in [(0.0, g.critical), (0.5, g.critical / 2.0), (1.0, 0.0)] {
            ink::text(f, format!("{v:.0}"), Point::new(tube.x - 8.0, tube.y + tube.height * k), num_size, BODY, INK, Alignment::Right, Vertical::Center);
        }

        // glass tube, mercury, bulb, metal cap
        let rr = (tube_w / 2.0).into();
        f.fill(&Path::rounded_rectangle(tube.position(), tube.size(), rr), WHITE);
        let merc_top = tube.y + tube.height * (1.0 - frac);
        f.fill(&ink::rect(Rectangle::new(Point::new(cx - tube_w * 0.22, merc_top), Size::new(tube_w * 0.44, bulb_c.y - merc_top))), RED);
        f.stroke(&Path::rounded_rectangle(tube.position(), tube.size(), rr), ink::stroke(2.5));
        f.fill(&Path::circle(bulb_c, bulb_r), RED);
        f.stroke(&Path::circle(bulb_c, bulb_r), ink::stroke(2.5));
        f.fill(&Path::circle(bulb_c + Vector::new(-bulb_r * 0.35, -bulb_r * 0.35), bulb_r * 0.22), WHITE);
        f.stroke(
            &Path::line(Point::new(tube.x + tube_w * 0.25, tube.y + 8.0), Point::new(tube.x + tube_w * 0.25, merc_top - 4.0)),
            Stroke::default().with_color(darker(WHITE, 0.85)).with_width(2.0),
        );
        ink::boxed(f, Rectangle::new(Point::new(cx - tube_w * 0.7, tube.y - 6.0), Size::new(tube_w * 1.4, 8.0)), GREY, 2.0, 0.0);

        if hot {
            // heat shimmer above the cap (stepped wiggle) + sweat drops
            let shift = if pose.is_multiple_of(2) { 3.0 } else { -3.0 };
            for k in 0..2 {
                let x0 = cx - tube_w * 0.6 + k as f32 * tube_w * 1.2;
                let wave: Vec<Point> = (0..5).map(|j| Point::new(x0 + if j % 2 == 0 { shift } else { -shift }, tube.y - 14.0 - j as f32 * 6.0)).collect();
                polyline(f, &wave, ink::stroke(2.0).with_color(RED));
            }
            let side = if pose.is_multiple_of(2) { -1.0 } else { 1.0 };
            let d = Point::new(cx + side * tube_w * 1.3, tube.y + tube.height * 0.35);
            let drop = ink::poly(&[d + Vector::new(0.0, -6.0), d + Vector::new(4.0, 2.0), d + Vector::new(0.0, 6.0), d + Vector::new(-4.0, 2.0)]);
            f.fill(&drop, CYAN);
            f.stroke(&drop, ink::stroke(1.5));
        }

        // reading + hanging label tag on the shelf front
        ink::text(
            f,
            format!("{:.0}°", g.celsius),
            Point::new(cx, plate.y + plate.height - 4.0),
            (col * 0.2).clamp(12.0, 24.0),
            DISPLAY,
            if hot { RED } else { INK },
            Alignment::Center,
            Vertical::Bottom,
        );
        let tag = Rectangle::new(Point::new(cx - col * 0.3, shelf.front.y + shelf.front.height * 0.15), Size::new(col * 0.6, shelf.front.height * 0.7));
        ink::boxed(f, tag, WHITE, 1.5, 0.0);
        ink::text(f, g.label.clone(), tag.center(), (tag.height * 0.65).clamp(8.0, 13.0), BODY, INK, Alignment::Center, Vertical::Center);
    }
}
