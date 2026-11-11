// SPDX-License-Identifier: GPL-3.0-or-later
//! Ember versions of the COSMIC panel applets. Each kind is its own applet (own ID and
//! launcher) sharing one popup implementation: a section of Ember's Quick panel.
use std::marker::PhantomData;
use std::time::Duration;

use cosmic::Element;
use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{Alignment, Length, Rectangle, Subscription};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, row};
use ember_core::calendar::{self, MONTHS};
use ember_ui::glyphs::{Glyph, Icon};
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
    /// Drawn panel glyph reflecting current state (None for the clock: text only).
    fn glyph(&self) -> Option<Glyph> {
        let q = &self.quick;
        Some(match K::SECTION {
            Section::Sound => match q.output() {
                Some(d) => Glyph::Speaker { level: if d.volume <= 0.001 { 0 } else { (d.volume * 3.0).ceil().min(3.0) as u8 }, muted: d.muted },
                None => Glyph::Speaker { level: 0, muted: true },
            },
            Section::Bluetooth => match q.bt() {
                Some(b) => Glyph::Bluetooth { on: b.powered && b.adapter.is_some(), connected: b.devices.iter().any(|d| d.connected) },
                None => Glyph::Bluetooth { on: true, connected: false },
            },
            Section::Network => match q.net() {
                Some(n) if n.airplane => Glyph::Airplane,
                Some(n) if n.links.iter().any(|l| l.kind == "ethernet") => Glyph::Wired,
                Some(n) if n.links.iter().any(|l| l.kind == "wifi") => {
                    Glyph::Wifi { bars: n.networks.iter().find(|w| w.in_use).map_or(3, |w| (w.signal / 25).min(3)) }
                }
                Some(_) => Glyph::Offline,
                None => Glyph::Wired,
            },
            Section::Power => {
                let b = q.battery();
                Glyph::Battery { frac: b.and_then(|b| b.percent).map(|p| p / 100.0), charging: b.and_then(|b| b.status.as_deref()) == Some("Charging") }
            }
            Section::Dnd => Glyph::Bell { dnd: Quick::dnd() },
            Section::Tiling => Glyph::Tiles { tiled: Quick::tiled() },
            Section::Session => Glyph::Power,
            Section::Media => Glyph::Note { playing: q.player().is_some_and(|p| p.playing) },
            Section::Awake => Glyph::Cup { on: q.awake() },
            Section::Clock => return None,
        })
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
            Section::Sound => {
                q.output().map_or("Sound".into(), |d| format!("{} · {:.0}%{}", d.description, d.volume * 100.0, if d.muted { " (muted)" } else { "" }))
            }
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
            Section::Awake => {
                if q.awake() {
                    "Keep awake: on (click to allow sleep)".into()
                } else {
                    "Keep awake: off (click to stay awake)".into()
                }
            }
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
            s.positioner.anchor_rect =
                Rectangle { x: (bounds.x - offset.x) as i32, y: (bounds.y - offset.y) as i32, width: bounds.width as i32, height: bounds.height as i32 };
            // Quick panel cards need more room than the stock 360px popups
            s.positioner.size_limits = cosmic::iced::Limits::NONE.min_width(POPUP_W).max_width(POPUP_W).min_height(1.0).max_height(1000.0);
            s
        },
        Some(Box::new(|state: &Applet<K>| Element::from(popup_box(state.quick.section(K::SECTION).map(Message::Quick))).map(cosmic::Action::App))),
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
        let icon = f32::from(applet.suggested_size(true).0);
        let mut content = row::with_capacity(2).spacing(6).align_y(Alignment::Center);
        if let Some(g) = self.glyph() {
            content = content.push(widget::canvas(Icon { glyph: g, color: None }).width(Length::Fixed(icon * g.aspect())).height(Length::Fixed(icon)));
        }
        if let Some(t) = self.label() {
            content = content.push(widget::text(t).font(ink::DISPLAY).size(icon.max(14.0)));
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
        // autosize: the panel gives the applet exactly the room its content needs
        applet.autosize_window(applet.applet_tooltip::<Message>(btn, self.tooltip(), self.popup.is_some(), Message::Surface, None)).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        "".into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

/// Popup frame in Ember's look. libcosmic's `popup_container` is capped at 360 px,
/// which crushed the cards; this one is 440 px and grows with its content's height.
fn popup_box<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    let framed = widget::container(content).padding(14).width(Length::Fixed(POPUP_W)).class(cosmic::theme::Container::custom(|_| widget::container::Style {
        text_color: Some(ink::pal().text),
        background: Some(cosmic::iced::Background::Color(ink::pal().page)),
        border: cosmic::iced::Border { color: ink::INK, width: 3.0, radius: 0.0.into() },
        ..Default::default()
    }));
    cosmic::widget::autosize::autosize(framed, cosmic::widget::Id::new("cosmic-applet-autosize"))
        .limits(cosmic::iced::Limits::NONE.min_width(POPUP_W).max_width(POPUP_W).min_height(1.0).max_height(1000.0))
        .into()
}

const POPUP_W: f32 = 440.0;

pub fn run<K: Kind>() -> cosmic::iced::Result {
    cosmic::applet::run::<Applet<K>>(())
}
