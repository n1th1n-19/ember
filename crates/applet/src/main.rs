// SPDX-License-Identifier: GPL-3.0-or-later
//! ember-applet: Ember's COSMIC panel applets in one binary.
//!
//! `ember-applet`             live readings meters
//! `ember-applet --setup-panel [names|all]` / `--restore-panel [names|all]`
//! `ember-applet <kind>`      clock, sound, bluetooth, network, power, dnd, tiling,
//!                            session, media, awake
mod panel;
mod readings;

/// `ember-applet --setup-panel [names|all]` / `--restore-panel [names|all]`: put applets in
/// the COSMIC top panel or take them out (COSMIC's stock applet comes back).
fn setup(args: &[String]) -> Option<i32> {
    let on = match args.get(1).map(String::as_str)? {
        "--setup-panel" => true,
        "--restore-panel" => false,
        _ => return None,
    };
    let r = ember_ui::setup::parse_list(args.get(2).map_or("all", String::as_str)).and_then(|l| ember_ui::setup::set_panel(&l, on));
    match r {
        Ok(()) => {
            println!("ember-applet: panel {}", if on { "set up" } else { "restored" });
            Some(0)
        }
        Err(e) => {
            eprintln!("ember-applet: {e}");
            Some(1)
        }
    }
}

fn main() -> cosmic::iced::Result {
    if let Some(code) = setup(&std::env::args().collect::<Vec<_>>()) {
        std::process::exit(code);
    }
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
