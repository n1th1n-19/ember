// SPDX-License-Identifier: GPL-3.0-or-later
//! Ember core: system sampling, no UI. Every reading is `Option` — `None` renders as `--`.

pub mod alerts;
pub mod control;
pub mod battery;
pub mod fans;
pub mod gpu;
pub mod history;
pub mod sample;
pub mod series;
pub mod temps;

pub use sample::{Sampler, Snapshot};
