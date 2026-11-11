// SPDX-License-Identifier: GPL-3.0-or-later
//! ember-applet: Ember's COSMIC panel applets in one binary.
//!
//! `ember-applet`             live readings meters
//! `ember-applet <kind>`      clock, sound, bluetooth, network, power, dnd, tiling,
//!                            session, media, awake
mod panel;
mod readings;

fn main() -> cosmic::iced::Result {
    ember_ui::ink::load_fonts();
    ember_ui::ink::set_dark(ember_ui::config::EmberConfig::load().dark);
    let kind = std::env::args().nth(1).unwrap_or_default();
    match kind.as_str() {
        "" | "readings" => readings::run(),
        "clock" => panel::run::<panel::Clock>(),
        "sound" => panel::run::<panel::Sound>(),
        "bluetooth" => panel::run::<panel::Bluetooth>(),
        "network" => panel::run::<panel::Network>(),
        "power" => panel::run::<panel::Power>(),
        "dnd" => panel::run::<panel::Dnd>(),
        "tiling" => panel::run::<panel::Tiling>(),
        "session" => panel::run::<panel::Session>(),
        "media" => panel::run::<panel::Media>(),
        "awake" => panel::run::<panel::Awake>(),
        other => {
            eprintln!("ember-applet: unknown applet {other:?}");
            std::process::exit(2);
        }
    }
}
