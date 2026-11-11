// SPDX-License-Identifier: GPL-3.0-or-later
//! ember-widget: comic readings card pinned to the desktop. A wlr layer surface on the
//! Bottom layer: above the wallpaper, below every window, no keyboard focus, no
//! reserved space. Click it to open Ember.
//!
//! `ember-widget [--corner top-right|top-left|bottom-right|bottom-left] [--on-top]`
//! `--on-top` floats it above windows instead of on the desktop.
use std::time::Duration;

use cosmic::app::{Core, Settings, Task};
use cosmic::cctk::sctk::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings};
use cosmic::iced::widget::canvas::{self, Frame, Geometry};
use cosmic::iced::window::Id;
use cosmic::iced::{mouse, Length, Limits, Point, Rectangle, Size, Subscription};
use cosmic::surface::action::app_layer_shell;
use cosmic::{Element, Renderer, Theme};
use ember_core::{Sampler, Snapshot};
use ember_ui::ink;
use ember_ui::mini::{self, Key};

const ID: &str = "io.github.n1th1n_19.Ember.Widget";
const W: u32 = 340;
const H: u32 = 380;
// ponytail: fixed readings until the Settings page writes them to cosmic-config
const KEYS: [Key; 7] = [Key::Cpu, Key::Ram, Key::Temp, Key::Gpu, Key::Battery, Key::Net, Key::Disk];

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Open,
}

struct Widget {
    core: Core,
    sampler: Sampler,
    snap: Snapshot,
}

struct Card<'a>(&'a Snapshot);

impl canvas::Program<Message, Theme, Renderer> for Card<'_> {
    type State = ();

    fn update(&self, _: &mut (), event: &canvas::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(bounds) => Some(canvas::Action::publish(Message::Open).and_capture()),
            _ => None,
        }
    }

    fn mouse_interaction(&self, _: &(), bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let readings: Vec<_> = KEYS.iter().map(|k| mini::read(self.0, *k)).collect();
        // leave room for the caption above and the hard shadow below-right
        mini::card(&mut f, Rectangle::new(Point::new(10.0, 20.0), bounds.size() - Size::new(20.0, 30.0)), "EMBER", &readings);
        vec![f.into_geometry()]
    }
}

fn corner() -> (Anchor, IcedMargin) {
    let arg = std::env::args().skip_while(|a| a != "--corner").nth(1).unwrap_or_else(|| "top-right".into());
    // clear of the top panel and screen edges
    let (top, side) = (56, 28);
    match arg.as_str() {
        "top-left" => (Anchor::TOP | Anchor::LEFT, IcedMargin { top, left: side, ..Default::default() }),
        "bottom-left" => (Anchor::BOTTOM | Anchor::LEFT, IcedMargin { bottom: side, left: side, ..Default::default() }),
        "bottom-right" => (Anchor::BOTTOM | Anchor::RIGHT, IcedMargin { bottom: side, right: side, ..Default::default() }),
        _ => (Anchor::TOP | Anchor::RIGHT, IcedMargin { top, right: side, ..Default::default() }),
    }
}

fn open_ember() {
    let sibling = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("ember"))).filter(|p| p.exists());
    let cmd = sibling.unwrap_or_else(|| "ember".into());
    if let Err(e) = std::process::Command::new(&cmd).spawn() {
        eprintln!("ember-widget: couldn't start {}: {e}", cmd.display());
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = ID;

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _: ()) -> (Self, Task<Message>) {
        let mut sampler = Sampler::new();
        std::thread::sleep(Duration::from_millis(200));
        let snap = sampler.sample();
        let (anchor, margin) = corner();
        let layer = if std::env::args().any(|a| a == "--on-top") { Layer::Top } else { Layer::Bottom };
        let surface = app_layer_shell::<Widget>(
            |_| Default::default(),
            move |_| SctkLayerSurfaceSettings {
                id: Id::unique(),
                layer,
                keyboard_interactivity: KeyboardInteractivity::None,
                anchor,
                output: IcedOutput::Active,
                namespace: "ember-widget".into(),
                margin,
                size: Some((Some(W), Some(H))),
                exclusive_zone: 0,
                size_limits: Limits::NONE.min_width(1.0).min_height(1.0).max_width(W as f32).max_height(H as f32),
                ..Default::default()
            },
            Some(Box::new(|state: &Widget| Element::from(state.card()).map(cosmic::Action::App))),
        );
        (Self { core, sampler, snap }, cosmic::task::message(cosmic::Action::Surface(surface)))
    }

    fn subscription(&self) -> Subscription<Message> {
        cosmic::iced::time::every(Duration::from_secs(2)).map(|_| Message::Tick)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick => self.snap = self.sampler.sample(),
            Message::Open => open_ember(),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        self.card()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        self.card()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        // transparent surface: only the card's ink shows on the wallpaper
        Some(cosmic::iced::theme::Style { background_color: cosmic::iced::Color::TRANSPARENT, text_color: ink::pal().text, icon_color: ink::pal().text })
    }
}

impl Widget {
    fn card(&self) -> Element<'_, Message> {
        cosmic::widget::canvas(Card(&self.snap)).width(Length::Fill).height(Length::Fill).into()
    }
}

fn main() -> cosmic::iced::Result {
    ink::load_fonts();
    cosmic::app::run::<Widget>(Settings::default().no_main_window(true).transparent(true).exit_on_close(false), ())
}
