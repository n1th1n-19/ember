// SPDX-License-Identifier: GPL-3.0-or-later
//! "Ink Tank": glass cylinder on legs; ink level = RAM, bubbles rise while it moves.
//! Overflow pipe drips into the swap bucket.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::{Point, Rectangle, Size, Vector};
use std::f32::consts::PI;

use super::{at, cast_shadow, cylinder, darker, disc, ellipse_pts, fit, floor, polyline};
use crate::ink::{self, Level, BODY, GREY, INK, MAGENTA, PAPER, WHITE};

pub fn draw(f: &mut Frame, r: Rectangle, ram: f32, swap: f32, pose: u64) {
    let r = fit(r, 1.05);
    let floor_y = r.y + r.height * 0.94;
    floor(f, r, floor_y);

    let c = Point::new(r.x + r.width * 0.36, r.y + r.height * 0.16);
    let (rx, ry) = (r.width * 0.22, r.width * 0.055);
    let h = r.height * 0.58;
    let bot = c + Vector::new(0.0, h);

    // legs + cross brace
    let leg_w = 7.0;
    for x in [-0.75, 0.75] {
        let top = Point::new(c.x + rx * x, bot.y);
        f.fill(&ink::rect(Rectangle::new(Point::new(top.x - leg_w / 2.0, top.y), Size::new(leg_w, floor_y - top.y))), GREY);
        f.stroke(&ink::rect(Rectangle::new(Point::new(top.x - leg_w / 2.0, top.y), Size::new(leg_w, floor_y - top.y))), ink::stroke(2.0));
    }
    f.stroke(&Path::line(Point::new(c.x - rx * 0.75, floor_y - 12.0), Point::new(c.x + rx * 0.75, bot.y + 10.0)), ink::stroke(2.0));
    cast_shadow(f, Point::new(c.x - rx * 0.2, floor_y), Point::new(c.x + rx * 1.1, floor_y), r.width * 0.14);

    // glass body
    cylinder(f, c, rx, ry, h, WHITE, darker(GREY, 0.9));

    // ink inside the glass, surface wobbles 2 poses
    let lvl = ram.clamp(0.0, 1.0);
    let inner_rx = rx - 5.0;
    let surf_y = bot.y - (h - 6.0) * lvl;
    if lvl > 0.01 {
        let mut body = vec![Point::new(c.x + inner_rx, surf_y)];
        body.extend(ellipse_pts(Point::new(c.x, bot.y - 3.0), inner_rx, ry * 0.85, 0.0, PI, 20));
        body.push(Point::new(c.x - inner_rx, surf_y));
        let body_poly = body.clone();
        f.fill(&ink::poly(&body), darker(MAGENTA, 0.85));
        super::halftone_poly(f, &body_poly, darker(MAGENTA, 0.55), 9.0, 0.45);
        let wob = if pose.is_multiple_of(2) { 1.0 } else { 0.8 };
        disc(f, Point::new(c.x, surf_y), inner_rx, ry * 0.8 * wob, MAGENTA, 2.0);
        // bubbles rising inside the ink
        for k in 0..3u64 {
            let t = ((pose + k * 3) % 8) as f32 / 8.0;
            let y = bot.y - 8.0 - (bot.y - surf_y - 12.0).max(0.0) * t;
            let x = c.x + inner_rx * [-0.4, 0.15, 0.5][k as usize];
            f.fill(&Path::circle(Point::new(x, y), 3.5), WHITE);
            f.stroke(&Path::circle(Point::new(x, y), 3.5), ink::stroke(1.2));
        }
    }
    // glass shine strip (flat white)
    f.stroke(&Path::line(Point::new(c.x - rx * 0.6, c.y + h * 0.12), Point::new(c.x - rx * 0.6, c.y + h * 0.55)), Stroke::default().with_color(WHITE).with_width(6.0));

    // two metal bands (front half of an ellipse)
    for k in [0.33f32, 0.7] {
        let band = ellipse_pts(c + Vector::new(0.0, h * k), rx + 1.0, ry, 0.0, PI, 20);
        polyline(f, &band, Stroke::default().with_color(INK).with_width(7.0));
        polyline(f, &band, Stroke::default().with_color(GREY).with_width(3.5));
    }

    // scale plate on the left
    let plate = Rectangle::new(Point::new(c.x - rx - r.width * 0.09, c.y + h * 0.1), Size::new(r.width * 0.07, h * 0.8));
    ink::boxed(f, plate, PAPER, 2.0, 2.0);
    let ticks = Path::new(|b| {
        for k in 0..=10 {
            let y = plate.y + plate.height * k as f32 / 10.0;
            let len = if k % 5 == 0 { 0.6 } else { 0.3 };
            b.move_to(Point::new(plate.x + plate.width, y));
            b.line_to(Point::new(plate.x + plate.width * (1.0 - len), y));
        }
    });
    f.stroke(&ticks, ink::stroke(1.5));
    let marker_y = plate.y + plate.height * (1.0 - lvl);
    f.fill(&ink::poly(&[Point::new(plate.x - 2.0, marker_y - 6.0), Point::new(plate.x + 8.0, marker_y), Point::new(plate.x - 2.0, marker_y + 6.0)]), MAGENTA);

    // valve wheel on the lid
    let wc = c + Vector::new(0.0, -ry * 0.4);
    f.stroke(&Path::line(c, wc + Vector::new(0.0, -10.0)), ink::stroke(4.0));
    let wheel = wc + Vector::new(0.0, -14.0);
    disc(f, wheel, rx * 0.35, ry * 0.5, ink::RED, 2.5);
    disc(f, wheel, rx * 0.12, ry * 0.18, INK, 0.0);

    if Level::from_percent(ram * 100.0) == Level::Hot {
        let crack = [at(r, 0.46, 0.24), at(r, 0.42, 0.31), at(r, 0.49, 0.36), at(r, 0.44, 0.44)];
        polyline(f, &crack, ink::stroke(2.5));
    }

    // overflow pipe: out of the tank's right side, elbow down to the bucket
    let bucket_c = Point::new(r.x + r.width * 0.8, r.y + r.height * 0.7);
    let pipe = [Point::new(c.x + rx, c.y + h * 0.12), Point::new(bucket_c.x, c.y + h * 0.12), Point::new(bucket_c.x, bucket_c.y - r.height * 0.07)];
    polyline(f, &pipe, Stroke::default().with_color(INK).with_width(r.width * 0.05));
    polyline(f, &pipe, Stroke::default().with_color(GREY).with_width(r.width * 0.03));
    // flange joints
    for p in [pipe[0], pipe[1]] {
        f.fill(&ink::rect(Rectangle::new(p + Vector::new(-6.0, -r.width * 0.032), Size::new(12.0, r.width * 0.064))), INK);
    }

    // swap bucket
    let (brx, bry, bh) = (r.width * 0.12, r.width * 0.03, r.height * 0.2);
    let s = swap.clamp(0.0, 1.0);
    cylinder(f, bucket_c, brx, bry, bh, PAPER, darker(PAPER, 0.7));
    if s > 0.0 {
        let y = bucket_c.y + bh - (bh - 4.0) * s;
        disc(f, Point::new(bucket_c.x, y.max(bucket_c.y + 2.0)), brx - 4.0, bry * 0.8, MAGENTA, 1.5);
        // drip: falls in 3 of 6 poses
        let phase = pose % 6;
        if phase < 3 {
            let d = Point::new(bucket_c.x, pipe[2].y + 6.0 + phase as f32 * 7.0);
            let drop = ink::poly(&[d + Vector::new(0.0, -6.0), d + Vector::new(4.0, 2.0), d + Vector::new(0.0, 6.0), d + Vector::new(-4.0, 2.0)]);
            f.fill(&drop, MAGENTA);
            f.stroke(&drop, ink::stroke(1.5));
        }
    }
    let tag = Rectangle::new(Point::new(bucket_c.x - brx * 0.7, bucket_c.y + bh * 0.4), Size::new(brx * 1.4, bh * 0.32));
    ink::boxed(f, tag, WHITE, 2.0, 0.0);
    ink::text(f, "SWAP", tag.center(), (tag.height * 0.6).max(8.0), BODY, INK, Alignment::Center, Vertical::Center);
    cast_shadow(f, Point::new(bucket_c.x, floor_y), Point::new(bucket_c.x + brx, floor_y), r.width * 0.08);
    // bucket sits on the floor: short stand
    f.fill(&ink::rect(Rectangle::new(Point::new(bucket_c.x - brx * 0.6, bucket_c.y + bh + bry * 0.5), Size::new(brx * 1.2, floor_y - bucket_c.y - bh - bry * 0.5))), INK);
}
