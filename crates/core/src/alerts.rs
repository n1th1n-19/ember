// SPDX-License-Identifier: GPL-3.0-or-later
//! Threshold alerts. `emberd` feeds values every 2s; a `true` from `should_fire` sends one notification.
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cpu,
    Ram,
    CpuTemp,
    BatteryLow,
}

#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub metric: Metric,
    /// Fire when value goes above this (or below, for BatteryLow).
    pub threshold: f32,
    /// Re-arm only after value returns this far past the threshold (prevents flapping at 89.9/90.1).
    pub hysteresis: f32,
    /// Minimum gap between two notifications for this rule.
    pub cooldown: Duration,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AlertState {
    /// True while the value is in the "alerting" zone and hasn't re-armed yet.
    pub tripped: bool,
    pub last_fired: Option<Instant>,
}

/// Decide whether to notify now, updating `state`.
///
/// Expected behavior (see tests):
/// - crossing the threshold while armed → fire once
/// - staying above → no repeat
/// - must drop below `threshold - hysteresis` to re-arm
/// - never fire twice within `cooldown`, even if re-armed
/// - BatteryLow is inverted (fires when value goes *below* threshold)
pub fn should_fire(rule: &Rule, value: f32, state: &mut AlertState, now: Instant) -> bool {
    // BatteryLow alerts below the threshold; flip it so one rule works for both.
    let (v, t) = if rule.metric == Metric::BatteryLow { (-value, -rule.threshold) } else { (value, rule.threshold) };
    if state.tripped {
        // re-arm only once the value has backed off past the hysteresis band
        if v < t - rule.hysteresis {
            state.tripped = false;
        }
        return false;
    }
    if v <= t {
        return false;
    }
    state.tripped = true;
    let cooled = state.last_fired.is_none_or(|last| now.duration_since(last) >= rule.cooldown);
    if cooled {
        state.last_fired = Some(now);
    }
    cooled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu() -> Rule {
        Rule { metric: Metric::Cpu, threshold: 90.0, hysteresis: 10.0, cooldown: Duration::from_secs(60) }
    }

    #[test]
    fn fires_once_then_needs_rearm() {
        let (r, mut s, t0) = (cpu(), AlertState::default(), Instant::now());
        assert!(!should_fire(&r, 50.0, &mut s, t0));
        assert!(should_fire(&r, 95.0, &mut s, t0));
        assert!(!should_fire(&r, 97.0, &mut s, t0 + Duration::from_secs(120))); // still high
        assert!(!should_fire(&r, 85.0, &mut s, t0 + Duration::from_secs(122))); // inside hysteresis band
        assert!(!should_fire(&r, 95.0, &mut s, t0 + Duration::from_secs(124))); // never re-armed
        assert!(!should_fire(&r, 70.0, &mut s, t0 + Duration::from_secs(126))); // re-arm
        assert!(should_fire(&r, 95.0, &mut s, t0 + Duration::from_secs(128)));
    }

    #[test]
    fn cooldown_blocks_fast_repeat() {
        let (r, mut s, t0) = (cpu(), AlertState::default(), Instant::now());
        assert!(should_fire(&r, 95.0, &mut s, t0));
        assert!(!should_fire(&r, 10.0, &mut s, t0 + Duration::from_secs(2)));
        assert!(!should_fire(&r, 95.0, &mut s, t0 + Duration::from_secs(4))); // re-armed but in cooldown
    }

    #[test]
    fn battery_low_is_inverted() {
        let r = Rule { metric: Metric::BatteryLow, threshold: 15.0, hysteresis: 5.0, cooldown: Duration::ZERO };
        let (mut s, t0) = (AlertState::default(), Instant::now());
        assert!(!should_fire(&r, 50.0, &mut s, t0));
        assert!(should_fire(&r, 12.0, &mut s, t0));
        assert!(!should_fire(&r, 18.0, &mut s, t0)); // below 15+5, not re-armed
        assert!(!should_fire(&r, 25.0, &mut s, t0));
        assert!(should_fire(&r, 14.0, &mut s, t0));
    }
}
