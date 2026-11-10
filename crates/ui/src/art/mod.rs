// SPDX-License-Identifier: GPL-3.0-or-later
//! One ink illustration per metric, drawn as detailed machines in 3/4 view:
//! light from top-left, right faces hatched, cast shadows on a floor line.
//! Each art draws inside `r` and takes `pose` (83ms counter, 0 when motion is reduced).
//!
//! Every shape here is an explicit polygon (no arcs, no clip regions): the renderer
//! misplaces clipped content and the arc builder dropped blade fills.
use cosmic::iced::widget::canvas::{Frame, Path, Stroke};
use cosmic::iced::{Color, Point, Rectangle, Size, Vector};
use std::f32::consts::TAU;

use crate::ink::{self, INK, WHITE};

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod fan;
pub mod gpu;
pub mod mem;
pub mod net;
pub mod temp;

/// Largest rect of `aspect` (w/h) inside `r`, centered horizontally and standing on the
/// bottom edge (machines sit on the floor; spare space becomes sky).
pub fn fit(r: Rectangle, aspect: f32) -> Rectangle {
    let (w, h) = if r.width / r.height > aspect { (r.height * aspect, r.height) } else { (r.width, r.width / aspect) };
    Rectangle::new(Point::new(r.x + (r.width - w) / 2.0, r.y + r.height - h), Size::new(w, h))
}

/// Point at fractional coords inside `r` (0..1, 0..1).
pub fn at(r: Rectangle, x: f32, y: f32) -> Point {
    Point::new(r.x + r.width * x, r.y + r.height * y)
}

/// Rect from fractional coords inside `r`.
pub fn sub(r: Rectangle, x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(at(r, x, y), Size::new(r.width * w, r.height * h))
}

/// Flat darker tone of a color for shadow faces (still a flat ink, not a gradient).
pub fn darker(c: Color, k: f32) -> Color {
    Color { r: c.r * k, g: c.g * k, b: c.b * k, a: c.a }
}

pub fn ellipse_pts(c: Point, rx: f32, ry: f32, a0: f32, a1: f32, n: usize) -> Vec<Point> {
    (0..=n).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        Point::new(c.x + a.cos() * rx, c.y + a.sin() * ry)
    }).collect()
}

pub fn ellipse(c: Point, rx: f32, ry: f32) -> Path {
    ink::poly(&ellipse_pts(c, rx, ry, 0.0, TAU, 40))
}

/// Filled + outlined ellipse.
pub fn disc(f: &mut Frame, c: Point, rx: f32, ry: f32, fill: Color, line: f32) {
    let e = ellipse(c, rx, ry);
    f.fill(&e, fill);
    if line > 0.0 {
        f.stroke(&e, ink::stroke(line));
    }
}

fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Point inside a convex polygon (either winding).
pub fn convex_contains(poly: &[Point], p: Point) -> bool {
    let n = poly.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let c = cross(poly[i], poly[(i + 1) % n], p);
        if c.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

/// Clip segment a→b to a convex polygon (Cyrus–Beck).
pub fn clip_convex(a: Point, b: Point, poly: &[Point]) -> Option<(Point, Point)> {
    let n = poly.len();
    // orientation so "inside" is consistent
    let area: f32 = (0..n).map(|i| cross(Point::ORIGIN, poly[i], poly[(i + 1) % n])).sum();
    let s = area.signum();
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let e = q - p;
        // inward normal
        let nrm = Vector::new(-e.y * s, e.x * s);
        let num = nrm.x * (a.x - p.x) + nrm.y * (a.y - p.y);
        let den = nrm.x * d.x + nrm.y * d.y;
        if den.abs() < 1e-9 {
            if num < 0.0 {
                return None;
            }
        } else {
            let t = -num / den;
            if den > 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) }
        }
    }
    (t1 > t0).then(|| (a + d * t0, a + d * t1))
}

fn bbox(poly: &[Point]) -> Rectangle {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in poly {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}

/// 45° hatch lines inside a convex polygon.
pub fn hatch_poly(f: &mut Frame, poly: &[Point], gap: f32, width: f32, color: Color) {
    let r = bbox(poly);
    let lines = Path::new(|b| {
        let mut d = gap / 2.0;
        while d < r.width + r.height {
            let a = Point::new(r.x + d, r.y);
            let z = Point::new(r.x + d - r.height, r.y + r.height);
            if let Some((p, q)) = clip_convex(a, z, poly) {
                b.move_to(p);
                b.line_to(q);
            }
            d += gap;
        }
    });
    f.stroke(&lines, Stroke::default().with_color(color).with_width(width));
}

/// Halftone dots inside a convex polygon.
pub fn halftone_poly(f: &mut Frame, poly: &[Point], color: Color, spacing: f32, density: f32) {
    ink::halftone_where(f, bbox(poly), color, spacing, density, |p| convex_contains(poly, p));
}

/// A box in 3/4 view: `front` face plus a depth offset going up-right.
#[derive(Clone, Copy)]
pub struct Block {
    pub front: Rectangle,
    pub depth: Vector,
}

impl Block {
    pub fn new(front: Rectangle, depth: Vector) -> Self {
        Self { front, depth }
    }
    pub fn top(&self) -> [Point; 4] {
        let r = self.front;
        let (tl, tr) = (r.position(), Point::new(r.x + r.width, r.y));
        [tl, tl + self.depth, tr + self.depth, tr]
    }
    pub fn side(&self) -> [Point; 4] {
        let r = self.front;
        let (tr, br) = (Point::new(r.x + r.width, r.y), Point::new(r.x + r.width, r.y + r.height));
        [tr, tr + self.depth, br + self.depth, br]
    }
    /// Point on the top face: u along width (0..1), v into depth (0..1).
    pub fn on_top(&self, u: f32, v: f32) -> Point {
        Point::new(self.front.x + self.front.width * u, self.front.y) + self.depth * v
    }
    pub fn draw(&self, f: &mut Frame, face: Color, top: Color, side: Color) {
        let side_p = ink::poly(&self.side());
        f.fill(&side_p, side);
        hatch_poly(f, &self.side(), 6.0, 1.2, Color { a: 0.55, ..INK });
        f.stroke(&side_p, ink::stroke(3.0));
        let top_p = ink::poly(&self.top());
        f.fill(&top_p, top);
        f.stroke(&top_p, ink::stroke(3.0));
        f.fill(&ink::rect(self.front), face);
        f.stroke(&ink::rect(self.front), ink::stroke(3.5));
    }
}

/// Floor line across `r` at height `y` with short perspective dashes below.
pub fn floor(f: &mut Frame, r: Rectangle, y: f32) {
    f.stroke(&Path::line(Point::new(r.x, y), Point::new(r.x + r.width, y)), ink::stroke(3.0));
    let dashes = Path::new(|b| {
        let mut x = r.x + 12.0;
        while x < r.x + r.width - 12.0 {
            b.move_to(Point::new(x, y + 8.0));
            b.line_to(Point::new(x - 10.0, y + 14.0));
            x += 26.0;
        }
    });
    f.stroke(&dashes, ink::stroke(1.5));
}

/// Hatched cast shadow on the floor, thrown to the right of an object's base.
pub fn cast_shadow(f: &mut Frame, base_l: Point, base_r: Point, reach: f32) {
    let k = Vector::new(reach, reach * 0.18);
    let poly = [base_l, base_r, base_r + k, base_l + k * 0.6];
    hatch_poly(f, &poly, 5.0, 1.5, INK);
}

/// Vertical cylinder seen slightly from above. `top` = center of the top ellipse.
pub fn cylinder(f: &mut Frame, top: Point, rx: f32, ry: f32, h: f32, body: Color, cap: Color) {
    let bot = top + Vector::new(0.0, h);
    let mut pts = vec![Point::new(top.x + rx, top.y)];
    pts.extend(ellipse_pts(bot, rx, ry, 0.0, std::f32::consts::PI, 20));
    pts.push(Point::new(top.x - rx, top.y));
    let shape = ink::poly(&pts);
    f.fill(&shape, body);
    // shadow band on the right third, following the bottom curve
    let x0 = 0.35f32;
    let mut band = vec![Point::new(top.x + rx * x0, top.y), Point::new(top.x + rx, top.y)];
    band.extend(ellipse_pts(bot, rx, ry, 0.0, x0.acos(), 8));
    hatch_poly(f, &band, 5.0, 1.2, Color { a: 0.6, ..INK });
    f.stroke(&shape, ink::stroke(3.0));
    disc(f, top, rx, ry, cap, 3.0);
}

/// Ring of `n` fan blades between r_in and r_out, rotated by `rot` radians.
pub fn blades(f: &mut Frame, c: Point, r_in: f32, r_out: f32, n: usize, rot: f32, color: Color) {
    let sweep = TAU / n as f32 * 0.62;
    for k in 0..n {
        let a = rot + k as f32 * TAU / n as f32;
        // swept blade: outer edge curves ahead of the inner edge
        let mut pts = ellipse_pts(c, r_in, r_in, a, a + sweep * 0.55, 5);
        let mut outer = ellipse_pts(c, r_out, r_out, a + sweep * 0.25, a + sweep, 8);
        outer.reverse();
        pts.extend(outer);
        let p = ink::poly(&pts);
        f.fill(&p, color);
        f.stroke(&p, ink::stroke(2.0));
    }
}

/// Little cloud of steam or smoke: three overlapping circles.
pub fn puff(f: &mut Frame, c: Point, r: f32) {
    let parts = [(Vector::new(-r * 0.6, r * 0.15), 0.7), (Vector::new(r * 0.55, r * 0.2), 0.65), (Vector::new(0.0, -r * 0.2), 1.0)];
    for (d, k) in parts {
        f.fill(&Path::circle(c + d, r * k + 3.0), INK);
    }
    for (d, k) in parts {
        f.fill(&Path::circle(c + d, r * k), WHITE);
    }
}

/// Round bezel gauge face with ticks and a needle; frac 0..1 sweeps 270°.
pub fn dial(f: &mut Frame, c: Point, r: f32, frac: f32) {
    f.fill(&Path::circle(c + Vector::new(3.0, 3.0), r), INK);
    f.fill(&Path::circle(c, r), ink::GREY);
    f.stroke(&Path::circle(c, r), ink::stroke(3.0));
    f.fill(&Path::circle(c, r * 0.8), WHITE);
    f.stroke(&Path::circle(c, r * 0.8), ink::stroke(2.0));
    let start = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let ticks = Path::new(|b| {
        for i in 0..=10 {
            let a = start + span * i as f32 / 10.0;
            let d = Vector::new(a.cos(), a.sin());
            b.move_to(c + d * (r * 0.58));
            b.line_to(c + d * (r * 0.74));
        }
    });
    f.stroke(&ticks, ink::stroke(1.5));
    // red zone
    let red = ellipse_pts(c, r * 0.7, r * 0.7, start + span * 0.8, start + span, 6);
    let red_path = Path::new(|b| {
        b.move_to(red[0]);
        red[1..].iter().for_each(|p| b.line_to(*p));
    });
    f.stroke(&red_path, Stroke::default().with_color(ink::RED).with_width(r * 0.12));
    let a = start + span * frac.clamp(0.0, 1.0);
    f.stroke(&Path::line(c, c + Vector::new(a.cos(), a.sin()) * (r * 0.66)), ink::stroke(3.0));
    f.fill(&Path::circle(c, r * 0.1), INK);
}

/// Corners of a w×h rectangle centered at `c`, rotated by `angle` (no frame transform needed).
pub fn rot_rect(c: Point, w: f32, h: f32, angle: f32) -> [Point; 4] {
    let (s, co) = angle.sin_cos();
    [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(x, y)| {
        let (x, y) = (x / 2.0, y / 2.0);
        Point::new(c.x + x * co - y * s, c.y + x * s + y * co)
    })
}

/// Open polyline stroke.
pub fn polyline(f: &mut Frame, pts: &[Point], stroke: Stroke<'_>) {
    if pts.len() < 2 {
        return;
    }
    let p = Path::new(|b| {
        b.move_to(pts[0]);
        pts[1..].iter().for_each(|q| b.line_to(*q));
    });
    f.stroke(&p, stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convex_geometry() {
        let sq = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)];
        assert!(convex_contains(&sq, Point::new(5.0, 5.0)));
        assert!(!convex_contains(&sq, Point::new(15.0, 5.0)));
        let (a, b) = clip_convex(Point::new(-5.0, 5.0), Point::new(15.0, 5.0), &sq).unwrap();
        assert!((a.x - 0.0).abs() < 1e-4 && (b.x - 10.0).abs() < 1e-4);
        let mut rev = sq;
        rev.reverse();
        assert!(clip_convex(Point::new(-5.0, 5.0), Point::new(15.0, 5.0), &rev).is_some());
        assert!(clip_convex(Point::new(-5.0, 20.0), Point::new(15.0, 20.0), &sq).is_none());
    }
}
