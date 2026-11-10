// SPDX-License-Identifier: GPL-3.0-or-later
//! "Filing Cabinet": one drawer per disk; fill bar = used, drawer slides out while busy.
use cosmic::iced::advanced::text::Alignment;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::widget::canvas::Frame;
use cosmic::iced::{Point, Rectangle, Size, Vector};

use super::{fit, sub};
use crate::ink::{self, Level, BODY, GREY, INK, PAPER, WHITE};

pub struct Drawer {
    pub label: String,
    pub used: f32,
    pub busy: bool,
}

pub fn draw(f: &mut Frame, r: Rectangle, drawers: &[Drawer], pose: u64) {
    let r = fit(r, 0.8);
    ink::boxed(f, sub(r, 0.08, 0.04, 0.84, 0.92), GREY, 4.0, 6.0);
    let n = drawers.len().clamp(1, 4);
    let area = sub(r, 0.13, 0.08, 0.74, 0.84);
    let h = area.height / n as f32;
    for (i, d) in drawers.iter().take(n).enumerate() {
        let slide = if d.busy && pose.is_multiple_of(2) { r.width * 0.05 } else { 0.0 };
        let front = Rectangle::new(Point::new(area.x, area.y + i as f32 * h + h * 0.06), Size::new(area.width, h * 0.88)) + Vector::new(0.0, 0.0);
        if slide > 0.0 {
            // drawer side visible when pulled out
            ink::boxed(f, Rectangle::new(front.position() + Vector::new(slide * 0.5, -slide * 0.4), front.size()), INK, 2.0, 0.0);
        }
        let front = front + Vector::new(-slide * 0.3, slide * 0.4);
        ink::boxed(f, front, PAPER, 3.0, 3.0);
        // label card
        let card = sub(front, 0.08, 0.14, 0.5, 0.3);
        ink::boxed(f, card, WHITE, 2.0, 0.0);
        ink::text(f, d.label.clone(), Point::new(card.center_x(), card.center_y()), (card.height * 0.6).min(14.0), BODY, INK, Alignment::Center, Vertical::Center);
        // handle
        ink::boxed(f, sub(front, 0.66, 0.2, 0.24, 0.16), INK, 2.0, 0.0);
        // used bar along the bottom of the drawer
        crate::chart::hbar(f, sub(front, 0.08, 0.6, 0.84, 0.24), d.used, Level::from_percent(d.used * 100.0).color());
    }
}
