// SPDX-License-Identifier: GPL-3.0-or-later
//! emberd: headless sampler. Every 2 s it appends one record to the history file
//! (read by the Ember app for 1-hour charts) and checks alert rules, sending a
//! desktop notification when one fires. Runs as the systemd user unit `ember.service`.
use std::time::{Duration, Instant};

use ember_core::alerts::{should_fire, AlertState, Metric, Rule};
use ember_core::history::{self, Record, Writer};
use ember_core::series::Series;
use ember_core::{Sampler, Snapshot};
use notify_rust::{Notification, Urgency};

// ponytail: fixed default rules; the Settings page will move these to cosmic-config.
const RULES: [Rule; 4] = [
    Rule { metric: Metric::Cpu, threshold: 90.0, hysteresis: 15.0, cooldown: Duration::from_secs(300) },
    Rule { metric: Metric::Ram, threshold: 90.0, hysteresis: 5.0, cooldown: Duration::from_secs(600) },
    Rule { metric: Metric::CpuTemp, threshold: 90.0, hysteresis: 8.0, cooldown: Duration::from_secs(300) },
    Rule { metric: Metric::BatteryLow, threshold: 15.0, hysteresis: 5.0, cooldown: Duration::from_secs(900) },
];

/// Rules judge a 10 s rolling mean (5 samples), so one spiky sample never alerts.
const WINDOW: usize = 5;

fn value(m: Metric, s: &Snapshot) -> Option<f32> {
    match m {
        Metric::Cpu => s.cpu_percent,
        Metric::Ram => s.mem_percent(),
        Metric::CpuTemp => s.cpu_temp(),
        // only a discharging battery is worth warning about
        Metric::BatteryLow => s.batteries.first().filter(|b| b.status.as_deref() == Some("Discharging")).and_then(|b| b.percent),
    }
}

fn notify(m: Metric, v: f32) {
    let (summary, body, urgency) = match m {
        Metric::Cpu => (format!("CPU at {v:.0}%"), "SIZZLE! The engine room has been flat out for a while.", Urgency::Normal),
        Metric::Ram => (format!("Memory at {v:.0}%"), "GLUG! The ink tank is nearly full. Close something heavy?", Urgency::Normal),
        Metric::CpuTemp => (format!("CPU at {v:.0}°C"), "TOO HOT! Check vents and what's running.", Urgency::Critical),
        Metric::BatteryLow => (format!("Battery at {v:.0}%"), "PLINK! Plug in soon.", Urgency::Critical),
    };
    let sent = Notification::new()
        .appname("Ember")
        .summary(&summary)
        .body(body)
        .icon("io.github.n1th1n_19.Ember")
        .urgency(urgency)
        .show();
    if let Err(e) = sent {
        eprintln!("emberd: notification failed: {e}");
    }
}

fn main() {
    // `emberd --test-notify` checks that desktop notifications reach the user.
    if std::env::args().any(|a| a == "--test-notify") {
        notify(Metric::Cpu, 95.0);
        return;
    }
    let path = history::default_path();
    let mut writer = match Writer::open(path.clone()) {
        Ok(w) => Some(w),
        Err(e) => {
            // keep alerting even if history can't be written
            eprintln!("emberd: history disabled ({}): {e}", path.display());
            None
        }
    };
    let mut sampler = Sampler::new();
    let mut states = [AlertState::default(); RULES.len()];
    let mut windows: [Series; RULES.len()] = std::array::from_fn(|_| Series::new(WINDOW));
    let interval = Duration::from_secs(history::INTERVAL_S);
    let mut next = Instant::now() + interval;

    loop {
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        next += interval;
        let snap = sampler.sample();

        if let Some(w) = writer.as_mut()
            && let Err(e) = w.append(&Record::from_snapshot(&snap, history::now()))
        {
            eprintln!("emberd: history write failed: {e}");
        }

        for (i, rule) in RULES.iter().enumerate() {
            windows[i].push(value(rule.metric, &snap));
            let vals: Vec<f32> = windows[i].iter().flatten().collect();
            if vals.len() < WINDOW {
                continue; // not enough real readings yet
            }
            let mean = vals.iter().sum::<f32>() / vals.len() as f32;
            if should_fire(rule, mean, &mut states[i], Instant::now()) {
                notify(rule.metric, mean);
            }
        }
    }
}
