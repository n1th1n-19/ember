// SPDX-License-Identifier: GPL-3.0-or-later
//! Ember versions of the COSMIC panel applets. Each kind is its own applet (own ID and
//! launcher) sharing one popup implementation: a section of Ember's Quick panel.
use std::marker::PhantomData;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{Alignment, Length, Rectangle, Subscription};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, row};
use cosmic::Element;
use ember_core::calendar::{self, MONTHS};
use ember_ui::ink;
use ember_ui::quick::{self, Quick, Section};

pub trait Kind: 'static {
    const ID: &'static str;
    const SECTION: Section;
}

macro_rules! kinds {
    ($($t:ident $id:literal $sec:ident),* $(,)?) => {
        $(pub struct $t; impl Kind for $t { const ID: &'static str = $id; const SECTION: Section = Section::$sec; })*
    };
}

kinds! {
    Clock "io.github.n1th1n_19.Ember.Applet.Clock" Clock,
    Sound "io.github.n1th1n_19.Ember.Applet.Sound" Sound,
    Bluetooth "io.github.n1th1n_19.Ember.Applet.Bluetooth" Bluetooth,
    Network "io.github.n1th1n_19.Ember.Applet.Network" Network,
    Power "io.github.n1th1n_19.Ember.Applet.Power" Power,
    Dnd "io.github.n1th1n_19.Ember.Applet.Dnd" Dnd,
    Tiling "io.github.n1th1n_19.Ember.Applet.Tiling" Tiling,
    Session "io.github.n1th1n_19.Ember.Applet.Session" Session,
    Media "io.github.n1th1n_19.Ember.Applet.Media" Media,
    Awake "io.github.n1th1n_19.Ember.Applet.Awake" Awake,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Quick(quick::Msg),
    PopupClosed(Id),
    Surface(cosmic::surface::Action<Message>),
}

pub struct Applet<K: Kind> {
    core: Core,
    popup: Option<Id>,
    quick: Quick,
    ticks: u64,
    _k: PhantomData<K>,
}

fn lift(t: cosmic::iced::Task<quick::Msg>) -> Task<Message> {
    t.map(|m| cosmic::Action::App(Message::Quick(m)))
}

impl<K: Kind> Applet<K> {
    /// Theme icon for the panel button, reflecting current state.
    fn icon(&self) -> &'static str {
        let q = &self.quick;
        match K::SECTION {
            Section::Sound => match q.output() {
                Some(d) if d.muted || d.volume <= 0.001 => "audio-volume-muted-symbolic",
                Some(d) if d.volume < 0.34 => "audio-volume-low-symbolic",
                Some(d) if d.volume < 0.67 => "audio-volume-medium-symbolic",
                Some(_) => "audio-volume-high-symbolic",
                None => "audio-volume-muted-symbolic",
            },
            Section::Bluetooth => match q.bt() {
                Some(b) if !b.powered || b.adapter.is_none() => "bluetooth-disabled-symbolic",
                Some(b) if b.devices.iter().any(|d| d.connected) => "bluetooth-active-symbolic",
                _ => "bluetooth-symbolic",
            },
            Section::Network => match q.net() {
                Some(n) if n.airplane => "airplane-mode-symbolic",
                Some(n) if n.links.iter().any(|l| l.kind == "ethernet") => "network-wired-symbolic",
                Some(n) if n.links.iter().any(|l| l.kind == "wifi") => "network-wireless-signal-excellent-symbolic",
                Some(_) => "network-offline-symbolic",
                None => "network-wired-symbolic",
            },
            Section::Power => {
                let b = q.battery();
                let charging = b.and_then(|b| b.status.as_deref()).is_some_and(|s| s == "Charging");
                match (b.and_then(|b| b.percent), charging) {
                    (None, _) => "battery-missing-symbolic",
                    (Some(p), true) if p >= 95.0 => "battery-full-charging-symbolic",
                    (Some(_), true) => "battery-good-charging-symbolic",
                    (Some(p), false) if p < 10.0 => "battery-empty-symbolic",
                    (Some(p), false) if p < 30.0 => "battery-low-symbolic",
                    (Some(p), false) if p < 70.0 => "battery-good-symbolic",
                    _ => "battery-full-symbolic",
                }
            }
            Section::Dnd => {
                if Quick::dnd() { "notifications-disabled-symbolic" } else { "preferences-system-notifications-symbolic" }
            }
            Section::Tiling => {
                if Quick::tiled() { "view-grid-symbolic" } else { "focus-windows-symbolic" }
            }
            Section::Session => "system-shutdown-symbolic",
            Section::Media => match q.player() {
                Some(p) if p.playing => "media-playback-start-symbolic",
                Some(_) => "media-playback-pause-symbolic",
                None => "audio-x-generic-symbolic",
            },
            Section::Awake => {
                if q.awake() { "view-reveal-symbolic" } else { "view-conceal-symbolic" }
            }
            Section::Clock => "",
        }
    }

    /// Short Bangers text shown next to the icon (clock, battery %, track title).
    fn label(&self) -> Option<String> {
        match K::SECTION {
            Section::Clock => {
                let n = calendar::now();
                let (h, ap) = (if n.hour.is_multiple_of(12) { 12 } else { n.hour % 12 }, if n.hour < 12 { "AM" } else { "PM" });
                Some(format!("{} {}, {h}:{:02} {ap}", &MONTHS[n.month as usize - 1][..3], n.day, n.minute))
            }
            Section::Power => self.quick.battery().and_then(|b| b.percent).map(|p| format!("{p:.0}%")),
            Section::Media => self.quick.player().filter(|p| !p.title.is_empty()).map(|p| p.title.chars().take(22).collect()),
            _ => None,
        }
    }

    fn tooltip(&self) -> String {
        let q = &self.quick;
        match K::SECTION {
            Section::Sound => q.output().map_or("Sound".into(), |d| format!("{} · {:.0}%{}", d.description, d.volume * 100.0, if d.muted { " (muted)" } else { "" })),
            Section::Bluetooth => q.bt().map_or("Bluetooth".into(), |b| {
                let on: Vec<_> = b.devices.iter().filter(|d| d.connected).map(|d| d.name.as_str()).collect();
                if on.is_empty() { "Bluetooth".into() } else { format!("Bluetooth · {}", on.join(", ")) }
            }),
            Section::Network => q.net().and_then(|n| n.links.first()).map_or("Network".into(), |l| l.connection.clone()),
            Section::Power => "Power & battery".into(),
            Section::Dnd => format!("Do Not Disturb {}", if Quick::dnd() { "on" } else { "off" }),
            Section::Tiling => "Tiling".into(),
            Section::Session => "Lock, log out, suspend, restart, shut down".into(),
            Section::Media => q.player().map_or("Nothing playing".into(), |p| format!("{} · {}", p.app, p.title)),
            Section::Awake => if q.awake() { "Keep awake: on (click to allow sleep)".into() } else { "Keep awake: off (click to stay awake)".into() },
            Section::Clock => "Calendar".into(),
        }
    }
}

/// Popup with this applet's Quick panel section, anchored to the clicked button.
fn popup<K: Kind>(offset: cosmic::iced::Vector, bounds: Rectangle) -> Message {
    Message::Surface(app_popup::<Applet<K>>(
        |_| Default::default(),
        move |state: &mut Applet<K>| {
            let id = Id::unique();
            state.popup = Some(id);
            let mut s = state.core.applet.get_popup_settings(state.core.main_window_id().unwrap(), id, None, None, None);
            s.positioner.anchor_rect = Rectangle { x: (bounds.x - offset.x) as i32, y: (bounds.y - offset.y) as i32, width: bounds.width as i32, height: bounds.height as i32 };
            // Quick panel cards need more room than the stock 360px popups
            s.positioner.size_limits = cosmic::iced::Limits::NONE.min_width(380.0).max_width(560.0).min_height(1.0).max_height(1000.0);
            s
        },
        Some(Box::new(|state: &Applet<K>| {
            let content = widget::container(state.quick.section(K::SECTION).map(Message::Quick)).padding(10).width(Length::Fixed(520.0));
            Element::from(state.core.applet.popup_container(content)).map(cosmic::Action::App)
        })),
    ))
}

impl<K: Kind> cosmic::Application for Applet<K> {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = K::ID;

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _: ()) -> (Self, Task<Message>) {
        let mut quick = Quick::only(&[K::SECTION]);
        let first = lift(quick.refresh());
        (Self { core, popup: None, quick, ticks: 0, _k: PhantomData }, first)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        let secs = match K::SECTION {
            Section::Clock => 1,
            Section::Session | Section::Awake => 3600,
            _ => 3,
        };
        cosmic::iced::time::every(Duration::from_secs(secs)).map(|_| Message::Tick)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick => {
                self.ticks += 1;
                // network reads spawn nmcli several times: every 9 s unless the popup is open
                let due = K::SECTION != Section::Network || self.popup.is_some() || self.ticks.is_multiple_of(3);
                if due && K::SECTION != Section::Clock {
                    return lift(self.quick.refresh());
                }
            }
            Message::Quick(m) => return lift(self.quick.update(m)),
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Surface(a) => return cosmic::task::message(cosmic::Action::Surface(a)),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let applet = &self.core.applet;
        let have_popup = self.popup;
        let mut content = row::with_capacity(2).spacing(6).align_y(Alignment::Center);
        if K::SECTION != Section::Clock {
            content = content.push(widget::icon::from_name(self.icon()).size(applet.suggested_size(true).0).symbolic(true).icon());
        }
        if let Some(t) = self.label() {
            content = content.push(widget::text(t).font(if K::SECTION == Section::Clock { ink::BODY } else { ink::DISPLAY }).size(14));
        }
        let (px, py) = applet.suggested_padding(true);
        let mut btn = widget::button::custom(content).class(cosmic::theme::Button::AppletIcon).padding([py, px]);
        btn = if K::SECTION == Section::Awake {
            // one click toggles; no popup needed
            btn.on_press(Message::Quick(quick::Msg::Awake(!self.quick.awake())))
        } else {
            btn.on_press_with_rectangle(move |offset, bounds| match have_popup {
                Some(id) => Message::Surface(destroy_popup(id)),
                None => popup::<K>(offset, bounds),
            })
        };
        applet.applet_tooltip::<Message>(btn, self.tooltip(), self.popup.is_some(), Message::Surface, None).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        "".into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

pub fn run<K: Kind>() -> cosmic::iced::Result {
    cosmic::applet::run::<Applet<K>>(())
}
