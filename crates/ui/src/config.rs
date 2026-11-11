// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings shared by the app, applet and widget through cosmic-config
//! (`~/.config/cosmic/io.github.n1th1n_19.Ember/v1/`). Every process watches it, so a
//! change in the Settings page shows up in the panel and on the desktop right away.
use cosmic::cosmic_config::{self, Config, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

use crate::mini::Key;

pub const ID: &str = "io.github.n1th1n_19.Ember";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Corner; 4] = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight];
    pub fn name(self) -> &'static str {
        match self {
            Corner::TopLeft => "Top left",
            Corner::TopRight => "Top right",
            Corner::BottomLeft => "Bottom left",
            Corner::BottomRight => "Bottom right",
        }
    }
    pub fn top(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::TopRight)
    }
    pub fn left(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::BottomLeft)
    }
    /// Layer-surface margins (top, right, bottom, left) for an offset of (x, y) from the corner.
    pub fn margins(self, x: i32, y: i32) -> (i32, i32, i32, i32) {
        let (x, y) = (x.max(0), y.max(0));
        match self {
            Corner::TopLeft => (y, 0, 0, x),
            Corner::TopRight => (y, x, 0, 0),
            Corner::BottomLeft => (0, 0, y, x),
            Corner::BottomRight => (0, x, y, 0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, CosmicConfigEntry, Serialize, Deserialize)]
#[version = 1]
pub struct EmberConfig {
    pub dark: bool,
    pub motion: bool,
    pub widget_corner: Corner,
    /// distance from the corner's side edge, px
    pub widget_x: i32,
    /// distance from the corner's top/bottom edge, px
    pub widget_y: i32,
    pub widget_on_top: bool,
    pub widget_keys: Vec<String>,
    pub applet_keys: Vec<String>,
}

impl Default for EmberConfig {
    fn default() -> Self {
        Self {
            dark: true,
            motion: true,
            widget_corner: Corner::TopRight,
            widget_x: 28,
            widget_y: 56,
            widget_on_top: false,
            widget_keys: Key::ALL.iter().map(|k| k.id().to_string()).collect(),
            applet_keys: [Key::Cpu, Key::Ram, Key::Temp].iter().map(|k| k.id().to_string()).collect(),
        }
    }
}

impl EmberConfig {
    pub fn handle() -> Option<Config> {
        Config::new(ID, Self::VERSION).ok()
    }

    /// Saved config, or defaults (missing/broken keys fall back individually).
    pub fn load() -> Self {
        Self::handle().map(|c| Self::get_entry(&c).unwrap_or_else(|(_, partial)| partial)).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(c) = Self::handle()
            && let Err(e) = self.write_entry(&c)
        {
            eprintln!("ember: couldn't save settings: {e}");
        }
    }

    fn keys(ids: &[String]) -> Vec<Key> {
        ids.iter().filter_map(|s| Key::parse(s)).collect()
    }
    pub fn widget(&self) -> Vec<Key> {
        Self::keys(&self.widget_keys)
    }
    pub fn applet(&self) -> Vec<Key> {
        Self::keys(&self.applet_keys)
    }

    /// Add or remove a reading; keeps the canonical order.
    pub fn toggle(list: &mut Vec<String>, key: Key) {
        let mut keys = Self::keys(list);
        if let Some(i) = keys.iter().position(|k| *k == key) {
            keys.remove(i);
        } else {
            keys.push(key);
        }
        keys.sort_by_key(|k| Key::ALL.iter().position(|a| a == k));
        *list = keys.iter().map(|k| k.id().to_string()).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn margins_and_toggle() {
        assert_eq!(Corner::TopRight.margins(28, 56), (56, 28, 0, 0));
        assert_eq!(Corner::BottomLeft.margins(10, 20), (0, 0, 20, 10));
        assert_eq!(Corner::TopLeft.margins(-5, 3), (3, 0, 0, 0));
        let mut v = vec!["temp".to_string(), "bogus".to_string()];
        EmberConfig::toggle(&mut v, Key::Cpu);
        assert_eq!(v, ["cpu", "temp"]);
        EmberConfig::toggle(&mut v, Key::Temp);
        assert_eq!(v, ["cpu"]);
        assert_eq!(EmberConfig::default().applet(), [Key::Cpu, Key::Ram, Key::Temp]);
    }
}
