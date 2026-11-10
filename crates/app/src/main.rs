// SPDX-License-Identifier: GPL-3.0-or-later
//! Ember: comic-style system monitor for COSMIC.
mod pages;

use std::time::Duration;

use cosmic::app::{Settings, Task};
use cosmic::iced::{Length, Size, Subscription};
use cosmic::widget::{self, canvas, nav_bar};
use cosmic::{Core, Element};
use ember_core::{series::Series, Sampler, Snapshot};
use ember_ui::ink::{Level, POSE_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Cpu,
    Memory,
    Disks,
    Network,
    Temps,
    Battery,
    Gpu,
    Fans,
}

impl Page {
    pub const ALL: [Page; 9] = [Page::Overview, Page::Cpu, Page::Memory, Page::Disks, Page::Network, Page::Temps, Page::Battery, Page::Gpu, Page::Fans];
    pub fn name(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Cpu => "CPU",
            Page::Memory => "Memory",
            Page::Disks => "Disks",
            Page::Network => "Network",
            Page::Temps => "Temperatures",
            Page::Battery => "Battery",
            Page::Gpu => "GPU",
            Page::Fans => "Fans",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Sample,
    Pose,
    Open(Page),
    ToggleMotion,
    ToggleTheme,
    ToggleRange,
}

/// 60 one-second samples per chart.
const HISTORY: usize = 60;

pub struct History {
    pub cpu: Series,
    pub mem: Series,
    pub swap: Series,
    pub rx: Series,
    pub tx: Series,
    pub disk_r: Series,
    pub disk_w: Series,
    pub cpu_temp: Series,
    pub battery: Series,
    pub gpu_clock: Series,
}

impl History {
    fn new(cap: usize) -> Self {
        let s = || Series::new(cap);
        Self { cpu: s(), mem: s(), swap: s(), rx: s(), tx: s(), disk_r: s(), disk_w: s(), cpu_temp: s(), battery: s(), gpu_clock: s() }
    }

    /// 1-hour history from emberd's file (empty if the daemon isn't running).
    fn from_file() -> Self {
        use ember_core::history::{read_last, default_path, Field, KEEP};
        let mut h = Self::new(KEEP);
        for r in default_path().and_then(|p| read_last(&p, KEEP).ok()).unwrap_or_default() {
            h.cpu.push(r.get(Field::Cpu));
            h.mem.push(r.get(Field::Mem));
            h.swap.push(r.get(Field::Swap));
            h.rx.push(r.get(Field::Rx));
            h.tx.push(r.get(Field::Tx));
            h.disk_r.push(r.get(Field::DiskR));
            h.disk_w.push(r.get(Field::DiskW));
            h.cpu_temp.push(r.get(Field::CpuTemp));
            h.battery.push(r.get(Field::Battery));
            h.gpu_clock.push(r.get(Field::GpuClock));
        }
        h
    }

    fn push(&mut self, s: &Snapshot) {
        let pct = |used: u64, total: u64| (total > 0).then(|| used as f32 / total as f32 * 100.0);
        self.cpu.push(s.cpu_percent);
        self.mem.push(s.mem_percent());
        self.swap.push(pct(s.swap_used, s.swap_total));
        self.rx.push(s.net_rx.map(|v| v as f32));
        self.tx.push(s.net_tx.map(|v| v as f32));
        let sum = |f: fn(&ember_core::sample::DiskStat) -> Option<f64>| s.disks.iter().filter_map(f).reduce(|a, b| a + b).map(|v| v as f32);
        self.disk_r.push(sum(|d| d.read_bps));
        self.disk_w.push(sum(|d| d.write_bps));
        self.cpu_temp.push(s.cpu_temp());
        self.battery.push(s.batteries.first().and_then(|b| b.percent));
        self.gpu_clock.push(s.gpus.iter().find_map(|g| g.cur_mhz).map(|v| v as f32));
    }
}

/// Which SFX burst is playing, and since which pose.
#[derive(Default)]
pub struct Bursts {
    pub cpu: Option<u64>,
    pub mem: Option<u64>,
    pub net: Option<u64>,
    pub temp: Option<u64>,
    pub battery: Option<u64>,
    last: [bool; 5],
}

/// Poses a burst stays on screen (2 s).
pub const BURST_POSES: u64 = 24;

pub struct App {
    core: Core,
    nav: nav_bar::Model,
    page: Page,
    sampler: Sampler,
    pub snap: Snapshot,
    hist_short: History,
    hist_long: History,
    /// charts show the last hour (from emberd) instead of the last 60 s
    pub long_range: bool,
    samples: u64,
    pub pose: u64,
    pub motion: bool,
    pub bursts: Bursts,
}

impl App {
    /// History behind every chart for the current range.
    pub fn hist(&self) -> &History {
        if self.long_range { &self.hist_long } else { &self.hist_short }
    }

    pub fn range_label(&self) -> &'static str {
        if self.long_range { "last hour" } else { "last 60 s" }
    }

    /// Pose counter for art, frozen when motion is off.
    pub fn anim(&self) -> u64 {
        if self.motion { self.pose } else { 0 }
    }

    fn update_bursts(&mut self) {
        let s = &self.snap;
        let net = s.net_rx.unwrap_or(0.0) + s.net_tx.unwrap_or(0.0);
        let now = [
            s.cpu_percent.is_some_and(|v| Level::from_percent(v) == Level::Hot),
            s.mem_percent().is_some_and(|v| Level::from_percent(v) == Level::Hot),
            net > 10e6,
            s.cpu_temp().is_some_and(|c| Level::from_celsius(c) == Level::Hot),
            s.batteries.first().and_then(|b| b.percent).is_some_and(|p| p < 15.0),
        ];
        let slots = [&mut self.bursts.cpu, &mut self.bursts.mem, &mut self.bursts.net, &mut self.bursts.temp, &mut self.bursts.battery];
        for (i, slot) in slots.into_iter().enumerate() {
            // start on entering the state; clear once it has played
            if now[i] && !self.bursts.last[i] {
                *slot = Some(self.pose);
            } else if slot.is_some_and(|t| self.pose.saturating_sub(t) > BURST_POSES) {
                *slot = None;
            }
        }
        self.bursts.last = now;
    }

    fn open(&mut self, page: Page) {
        self.page = page;
        let id = self.nav.iter().find(|id| self.nav.data::<Page>(*id) == Some(&page));
        if let Some(id) = id {
            self.nav.activate(id);
        }
    }
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = (Page, bool);
    type Message = Message;
    const APP_ID: &'static str = "io.github.n1th1n_19.Ember";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, (start, long): (Page, bool)) -> (Self, Task<Message>) {
        let mut nav = nav_bar::Model::default();
        for p in Page::ALL {
            nav.insert().text(p.name()).data(p);
        }
        let mut sampler = Sampler::new();
        // first CPU % needs a short gap after priming
        std::thread::sleep(Duration::from_millis(250));
        let snap = sampler.sample();
        let mut hist_short = History::new(HISTORY);
        hist_short.push(&snap);
        let mut app = App { core, nav, page: start, sampler, snap, hist_short, hist_long: if long { History::from_file() } else { History::new(1) }, long_range: long, samples: 0, pose: 0, motion: true, bursts: Bursts::default() };
        app.open(start);
        (app, Task::none())
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Message> {
        self.nav.activate(id);
        if let Some(p) = self.nav.data::<Page>(id) {
            self.page = *p;
        }
        Task::none()
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        let motion = if self.motion { "Motion: on" } else { "Motion: off" };
        let theme = if ember_ui::ink::pal().dark { "Theme: dark" } else { "Theme: light" };
        let range = if self.long_range { "Range: 1 h" } else { "Range: 60 s" };
        vec![
            widget::button::text(range).on_press(Message::ToggleRange).into(),
            widget::button::text(theme).on_press(Message::ToggleTheme).into(),
            widget::button::text(motion).on_press(Message::ToggleMotion).into(),
        ]
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Sample => {
                self.snap = self.sampler.sample();
                self.hist_short.push(&self.snap);
                self.samples += 1;
                if self.long_range && self.samples.is_multiple_of(10) {
                    self.hist_long = History::from_file();
                }
                self.update_bursts();
            }
            Message::Pose => self.pose = self.pose.wrapping_add(1),
            Message::Open(p) => self.open(p),
            Message::ToggleMotion => self.motion = !self.motion,
            Message::ToggleRange => {
                self.long_range = !self.long_range;
                if self.long_range {
                    self.hist_long = History::from_file();
                }
            }
            Message::ToggleTheme => ember_ui::ink::set_dark(!ember_ui::ink::pal().dark),
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let sample = cosmic::iced::time::every(Duration::from_secs(1)).map(|_| Message::Sample);
        if self.motion {
            let pose = cosmic::iced::time::every(Duration::from_millis(POSE_MS)).map(|_| Message::Pose);
            Subscription::batch([sample, pose])
        } else {
            sample
        }
    }

    fn view(&self) -> Element<'_, Message> {
        canvas(pages::View { app: self, page: self.page }).width(Length::Fill).height(Length::Fill).into()
    }
}

fn main() -> cosmic::iced::Result {
    ember_ui::ink::load_fonts();
    // `ember --page cpu` opens straight to a page (used by the dock applet).
    let args: Vec<String> = std::env::args().collect();
    let start = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
        .and_then(|name| Page::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(name) || (name == "temps" && *p == Page::Temps)))
        .unwrap_or(Page::Overview);
    let long = args.windows(2).any(|w| w[0] == "--range" && w[1] == "1h");
    cosmic::app::run::<App>(Settings::default().size(Size::new(1180.0, 800.0)).size_limits(cosmic::iced::Limits::NONE.min_width(720.0).min_height(560.0)), (start, long))
}
