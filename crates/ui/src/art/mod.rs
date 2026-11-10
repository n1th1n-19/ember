// SPDX-License-Identifier: GPL-3.0-or-later
//! One ink illustration per metric. Each draws inside `r`, keeps its own aspect,
//! and takes `pose` (83ms counter, frozen at 0 when motion is reduced).
use cosmic::iced::{Point, Rectangle, Size};

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod fan;
pub mod gpu;
pub mod mem;
pub mod net;
pub mod temp;

/// Largest centered rect of `aspect` (w/h) inside `r`.
pub fn fit(r: Rectangle, aspect: f32) -> Rectangle {
    let (w, h) = if r.width / r.height > aspect { (r.height * aspect, r.height) } else { (r.width, r.width / aspect) };
    Rectangle::new(Point::new(r.x + (r.width - w) / 2.0, r.y + (r.height - h) / 2.0), Size::new(w, h))
}

/// Point at fractional coords inside `r` (0..1, 0..1).
pub fn at(r: Rectangle, x: f32, y: f32) -> Point {
    Point::new(r.x + r.width * x, r.y + r.height * y)
}

/// Rect from fractional coords inside `r`.
pub fn sub(r: Rectangle, x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(at(r, x, y), Size::new(r.width * w, r.height * h))
}
