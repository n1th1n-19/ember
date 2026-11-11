// SPDX-License-Identifier: GPL-3.0-or-later
//! Panel icons drawn in code: thick strokes in the panel's text color, Ember accents for
//! state (battery fill, connected dot, DND slash, steam). One look across all applets,
//! and nothing depends on which icon theme happens to be installed.
use cosmic::iced::widget::canvas::{self, path::Arc, Frame, Geometry, Path, Stroke};
use cosmic::iced::{mouse, Color, Point, Radians, Rectangle, Size, Vector};
use cosmic::{Renderer, Theme};
use std::f32::consts::PI;

use crate::ink::{CYAN, RED, YELLOW};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Glyph {
    /// level 0..=3 sound waves
    Speaker { level: u8, muted: bool },
    Mic { muted: bool },
    Bluetooth { on: bool, connected: bool },
    Wired,
    /// bars 0..=3
    Wifi { bars: u8 },
    Airplane,
    Offline,
    Battery { frac: Option<f32>, charging: bool },
    Bell { dnd: bool },
    Tiles { tiled: bool },
    Power,
    Note { playing: bool },
    Cup { on: bool },
    Bulb,
    Keyboard,
}

impl Glyph {
    /// width / height of the drawing
    pub fn aspect(self) -> f32 {
        match self {
            Glyph::Battery { .. } => 1.9,
            _ => 1.0,
        }
    }
}

/// Canvas program; `color` None = the theme's foreground.
pub struct Icon {
    pub glyph: Glyph,
    pub color: Option<Color>,
}

impl<M> canvas::Program<M, Theme, Renderer> for Icon {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let fg = self.color.unwrap_or_else(|| theme.cosmic().on_bg_color().into());
        draw(&mut f, Rectangle::new(Point::ORIGIN, bounds.size()), self.glyph, fg);
        vec![f.into_geometry()]
    }
}

fn line(w: f32, c: Color) -> Stroke<'static> {
    Stroke::default().with_color(c).with_width(w).with_line_cap(canvas::LineCap::Round).with_line_join(canvas::LineJoin::Round)
}

fn arc(c: Point, r: f32, a0: f32, a1: f32) -> Path {
    Path::new(|b| b.arc(Arc { center: c, radius: r, start_angle: Radians(a0), end_angle: Radians(a1) }))
}

fn poly(pts: &[Point], close: bool) -> Path {
    Path::new(|b| {
        b.move_to(pts[0]);
        pts[1..].iter().for_each(|p| b.line_to(*p));
        if close {
            b.close();
        }
    })
}

/// Draw `g` centered in `r`, keeping its aspect.
pub fn draw(f: &mut Frame, r: Rectangle, g: Glyph, fg: Color) {
    let a = g.aspect();
    let (w, h) = if r.width / r.height > a { (r.height * a, r.height) } else { (r.width, r.width / a) };
    let b = Rectangle::new(Point::new(r.x + (r.width - w) / 2.0, r.y + (r.height - h) / 2.0), Size::new(w, h));
    let s = b.height;
    let p = |x: f32, y: f32| Point::new(b.x + b.width * x, b.y + b.height * y);
    let lw = (s * 0.11).max(1.6);
    let st = line(lw, fg);
    let slash = |f: &mut Frame| {
        f.stroke(&Path::line(p(0.12, 0.12), p(0.88, 0.88)), line(lw * 1.1, RED));
    };

    match g {
        Glyph::Speaker { level, muted } => {
            let body = poly(&[p(0.1, 0.38), p(0.28, 0.38), p(0.5, 0.16), p(0.5, 0.84), p(0.28, 0.62), p(0.1, 0.62)], true);
            f.fill(&body, fg);
            for k in 0..level.min(3) {
                let rad = s * (0.16 + 0.13 * k as f32);
                f.stroke(&arc(p(0.5, 0.5), rad, -PI / 4.0, PI / 4.0), st);
            }
            if muted {
                slash(f);
            }
        }
        Glyph::Mic { muted } => {
            f.fill(&Path::rounded_rectangle(p(0.36, 0.08), Size::new(s * 0.28, s * 0.5), (s * 0.14).into()), fg);
            f.stroke(&arc(p(0.5, 0.42), s * 0.25, 0.0, PI), st);
            f.stroke(&Path::line(p(0.5, 0.68), p(0.5, 0.9)), st);
            if muted {
                slash(f);
            }
        }
        Glyph::Bluetooth { on, connected } => {
            let rune = poly(&[p(0.28, 0.3), p(0.68, 0.68), p(0.48, 0.88), p(0.48, 0.12), p(0.68, 0.32), p(0.28, 0.7)], false);
            f.stroke(&rune, st);
            if connected {
                f.fill(&Path::circle(p(0.86, 0.5), s * 0.1), CYAN);
            }
            if !on {
                slash(f);
            }
        }
        Glyph::Wired => {
            // three linked boxes: a small network
            for (x, y) in [(0.5, 0.22), (0.2, 0.78), (0.8, 0.78)] {
                f.fill(&Path::rectangle(p(x - 0.13, y - 0.13), Size::new(s * 0.26, s * 0.26)), fg);
            }
            f.stroke(&poly(&[p(0.5, 0.35), p(0.5, 0.5), p(0.2, 0.5), p(0.2, 0.65)], false), st);
            f.stroke(&poly(&[p(0.5, 0.5), p(0.8, 0.5), p(0.8, 0.65)], false), st);
        }
        Glyph::Wifi { bars } => {
            let c = p(0.5, 0.85);
            f.fill(&Path::circle(c, s * 0.08), fg);
            for k in 0..3u8 {
                let col = if k < bars { fg } else { Color { a: 0.3, ..fg } };
                f.stroke(&arc(c, s * (0.26 + 0.2 * k as f32), -PI * 0.75, -PI * 0.25), line(lw, col));
            }
        }
        Glyph::Airplane => {
            let plane = poly(&[p(0.5, 0.08), p(0.58, 0.4), p(0.92, 0.6), p(0.58, 0.55), p(0.56, 0.8), p(0.68, 0.9), p(0.32, 0.9), p(0.44, 0.8), p(0.42, 0.55), p(0.08, 0.6), p(0.42, 0.4)], true);
            f.fill(&plane, fg);
        }
        Glyph::Offline => {
            Glyph::Wifi { bars: 0 }.draw_into(f, b, fg);
            slash(f);
        }
        Glyph::Battery { frac, charging } => {
            let body = Rectangle::new(p(0.04, 0.16), Size::new(b.width * 0.82, b.height * 0.68));
            f.stroke(&Path::rounded_rectangle(body.position(), body.size(), (s * 0.12).into()), st);
            f.fill(&Path::rectangle(p(0.88, 0.36), Size::new(b.width * 0.08, b.height * 0.28)), fg);
            if let Some(fr) = frac {
                let inner = Rectangle::new(body.position() + Vector::new(lw * 1.3, lw * 1.3), body.size() - Size::new(lw * 2.6, lw * 2.6));
                let fill = Rectangle::new(inner.position(), Size::new(inner.width * fr.clamp(0.0, 1.0), inner.height));
                f.fill(&Path::rectangle(fill.position(), fill.size()), if fr < 0.15 && !charging { RED } else { YELLOW });
            }
            if charging {
                let c = body.center();
                let u = body.height * 0.42;
                let bolt = poly(&[c + Vector::new(u * 0.25, -u), c + Vector::new(-u * 0.45, u * 0.12), c + Vector::new(0.0, u * 0.12), c + Vector::new(-u * 0.25, u), c + Vector::new(u * 0.45, -u * 0.12), c + Vector::new(0.0, -u * 0.12)], true);
                f.fill(&bolt, fg);
            }
        }
        Glyph::Bell { dnd } => {
            let bell = Path::new(|bd| {
                bd.move_to(p(0.2, 0.72));
                bd.quadratic_curve_to(p(0.3, 0.6), p(0.3, 0.42));
                bd.quadratic_curve_to(p(0.3, 0.14), p(0.5, 0.14));
                bd.quadratic_curve_to(p(0.7, 0.14), p(0.7, 0.42));
                bd.quadratic_curve_to(p(0.7, 0.6), p(0.8, 0.72));
                bd.close();
            });
            f.fill(&bell, fg);
            f.fill(&Path::circle(p(0.5, 0.84), s * 0.08), fg);
            if dnd {
                slash(f);
            }
        }
        Glyph::Tiles { tiled } => {
            if tiled {
                f.stroke(&Path::rectangle(p(0.1, 0.12), Size::new(s * 0.36, s * 0.76)), st);
                f.stroke(&Path::rectangle(p(0.56, 0.12), Size::new(s * 0.34, s * 0.34)), st);
                f.stroke(&Path::rectangle(p(0.56, 0.54), Size::new(s * 0.34, s * 0.34)), st);
                f.fill(&Path::rectangle(p(0.1, 0.12), Size::new(s * 0.36, s * 0.76)), Color { a: 0.35, ..YELLOW });
            } else {
                f.stroke(&Path::rectangle(p(0.1, 0.3), Size::new(s * 0.5, s * 0.5)), st);
                f.fill(&Path::rectangle(p(0.38, 0.12), Size::new(s * 0.52, s * 0.5)), fg);
            }
        }
        Glyph::Power => {
            f.stroke(&arc(p(0.5, 0.55), s * 0.32, -PI * 0.3, PI * 1.3), st);
            f.stroke(&Path::line(p(0.5, 0.1), p(0.5, 0.48)), st);
        }
        Glyph::Note { playing } => {
            f.fill(&Path::circle(p(0.3, 0.76), s * 0.14), fg);
            f.fill(&Path::circle(p(0.74, 0.66), s * 0.14), fg);
            f.stroke(&poly(&[p(0.42, 0.76), p(0.42, 0.18), p(0.86, 0.1), p(0.86, 0.66)], false), st);
            if playing {
                f.fill(&Path::circle(p(0.12, 0.2), s * 0.07), CYAN);
            }
        }
        Glyph::Cup { on } => {
            let cup = poly(&[p(0.14, 0.42), p(0.7, 0.42), p(0.64, 0.88), p(0.2, 0.88)], true);
            if on {
                f.fill(&cup, YELLOW);
            }
            f.stroke(&cup, st);
            f.stroke(&arc(p(0.72, 0.6), s * 0.12, -PI / 2.0, PI / 2.0), st);
            if on {
                for x in [0.3, 0.52] {
                    f.stroke(&poly(&[p(x, 0.34), p(x - 0.06, 0.24), p(x, 0.14), p(x - 0.05, 0.05)], false), line(lw * 0.8, fg));
                }
            }
        }
        Glyph::Bulb => {
            f.stroke(&Path::circle(p(0.5, 0.4), s * 0.26), st);
            f.stroke(&Path::line(p(0.38, 0.8), p(0.62, 0.8)), st);
            f.stroke(&Path::line(p(0.42, 0.92), p(0.58, 0.92)), st);
            f.fill(&Path::circle(p(0.5, 0.4), s * 0.12), YELLOW);
        }
        Glyph::Keyboard => {
            f.stroke(&Path::rounded_rectangle(p(0.06, 0.24), Size::new(s * 0.88, s * 0.54), (s * 0.08).into()), st);
            let keys = Path::new(|bd| {
                for (x, y) in [(0.24, 0.42), (0.42, 0.42), (0.6, 0.42), (0.78, 0.42)] {
                    bd.circle(p(x, y), s * 0.045);
                }
            });
            f.fill(&keys, fg);
            f.stroke(&Path::line(p(0.3, 0.62), p(0.7, 0.62)), st);
        }
    }
}

impl Glyph {
    fn draw_into(self, f: &mut Frame, r: Rectangle, fg: Color) {
        draw(f, r, self, fg);
    }
}
