// SPDX-License-Identifier: GPL-3.0-or-later
//! "Filing Cabinet": steel cabinet in 3/4 view, one drawer per disk. A busy drawer
//! slides out and shows its files; the label card carries mount + used bar.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::Frame;
use cosmic::iced::{Point, Rectangle, Size, Vector};

use super::{cast_shadow, darker, fit, floor, sub, Block};
use crate::ink::{self, Level, BODY, CYAN, GREY, INK, MAGENTA, PAPER, WHITE, YELLOW};

pub struct Drawer {
    pub label: String,
    pub used: f32,
    pub busy: bool,
}

const STEEL: cosmic::iced::Color = cosmic::iced::Color { r: 0.80, g: 0.78, b: 0.74, a: 1.0 };

pub fn draw(f: &mut Frame, r: Rectangle, drawers: &[Drawer], pose: u64) {
    let r = fit(r, 0.95);
    let floor_y = r.y + r.height * 0.95;
    floor(f, r, floor_y);
    let front = sub(r, 0.14, 0.14, 0.6, 0.81);
    let cab = Block::new(front, Vector::new(r.width * 0.16, -r.height * 0.08));
    cast_shadow(f, Point::new(front.x + front.width, floor_y), Point::new(front.x + front.width + cab.depth.x, floor_y - 2.0), r.width * 0.12);
    cab.draw(f, STEEL, darker(STEEL, 0.93), darker(STEEL, 0.72));

    // paper stack + mug on the top face
    for k in 0..4 {
        let a = cab.on_top(0.12, 0.35) + Vector::new(0.0, -(k as f32) * 3.0);
        let sheet = [a, a + Vector::new(r.width * 0.16, 0.0), a + Vector::new(r.width * 0.16, 0.0) + cab.depth * 0.4, a + cab.depth * 0.4];
        f.fill(&ink::poly(&sheet), WHITE);
        f.stroke(&ink::poly(&sheet), ink::stroke(1.5));
    }
    let mug = cab.on_top(0.75, 0.5);
    super::cylinder(f, mug + Vector::new(0.0, -r.height * 0.06), r.width * 0.035, r.width * 0.012, r.height * 0.06, ink::RED, INK);

    let n = drawers.len().clamp(1, 4);
    let inner = ink::inset(front, 10.0);
    let h = inner.height / n as f32;
    let slot = |i: usize| Rectangle::new(Point::new(inner.x, inner.y + i as f32 * h + 4.0), Size::new(inner.width, h - 8.0));

    // closed fronts (and dark holes for open ones) first, open drawers on top
    for (i, d) in drawers.iter().take(n).enumerate() {
        let s = slot(i);
        if d.busy {
            f.fill(&ink::rect(s), INK);
        } else {
            drawer_front(f, s, d, r);
        }
    }
    for (i, d) in drawers.iter().take(n).enumerate().filter(|(_, d)| d.busy) {
        let s = slot(i);
        // slides out 1 notch every other pose
        let out = if pose.is_multiple_of(2) { 0.55 } else { 0.4 };
        let pull = -cab.depth * out;
        let drawer = Block::new(s + pull, cab.depth * out);
        // folders poking up from the open drawer's top face
        let colors = [YELLOW, CYAN, MAGENTA, WHITE];
        for k in 0..5 {
            let u = 0.1 + k as f32 * 0.18;
            let base = drawer.on_top(u, 0.5);
            let tab = [base, base + Vector::new(0.0, -s.height * 0.28), base + Vector::new(s.width * 0.12, -s.height * 0.28), base + Vector::new(s.width * 0.14, 0.0)];
            f.fill(&ink::poly(&tab), colors[k % 4]);
            f.stroke(&ink::poly(&tab), ink::stroke(1.5));
        }
        drawer.draw(f, STEEL, darker(STEEL, 0.6), darker(STEEL, 0.75));
        drawer_front(f, drawer.front, d, r);
    }
}

fn drawer_front(f: &mut Frame, s: Rectangle, d: &Drawer, r: Rectangle) {
    ink::boxed(f, s, STEEL, 2.5, 0.0);
    // recessed panel line
    f.stroke(&ink::rect(ink::inset(s, 5.0)), ink::stroke(1.2));
    // label holder with mount name + used bar
    let card = sub(s, 0.1, 0.16, 0.5, 0.4);
    ink::boxed(f, card, GREY, 2.0, 0.0);
    let paper = ink::inset(card, 3.0);
    f.fill(&ink::rect(paper), WHITE);
    ink::text(f, d.label.clone(), Point::new(paper.center_x(), paper.center_y()), (paper.height * 0.7).clamp(8.0, 14.0), BODY, INK, Alignment::Center, Vertical::Center);
    crate::chart::hbar(f, sub(s, 0.1, 0.64, 0.5, 0.18), d.used, Level::from_percent(d.used * 100.0).color());
    ink::text(f, format!("{:.0}%", d.used * 100.0), Point::new(s.x + s.width * 0.64, s.y + s.height * 0.73), (s.height * 0.22).clamp(9.0, 14.0), BODY, INK, Alignment::Left, Vertical::Center);
    // cup handle
    let handle = sub(s, 0.7, 0.22, 0.2, 0.2);
    ink::boxed(f, handle, darker(GREY, 0.8), 2.0, 2.0);
    f.fill(&ink::rect(Rectangle::new(Point::new(handle.x + 3.0, handle.y + handle.height * 0.55), Size::new(handle.width - 6.0, 3.0))), INK);
    let _ = (PAPER, r);
}
