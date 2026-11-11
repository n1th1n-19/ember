// SPDX-License-Identifier: GPL-3.0-or-later
//! ember-widget: comic readings card pinned to the desktop. A wlr layer surface on the
//! Bottom layer: above the wallpaper, below every window, no keyboard focus, no
//! reserved space. Click it to open Ember.
//!
//! Placement, layer and readings come from Ember's settings (cosmic-config) and update
//! live when they change.
use std::time::Duration;

use cosmic::app::{Core, Settings, Task};
use cosmic::cctk::sctk::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use cosmic::cosmic_config::{CosmicConfigEntry, config_subscription};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{set_anchor, set_layer, set_margin, set_size};
use cosmic::iced::widget::canvas::{self, Frame, Geometry};
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Limits, Point, Rectangle, Size, Subscription, mouse};
use cosmic::surface::action::app_layer_shell;
use cosmic::{Element, Renderer, Theme};
use ember_core::{Sampler, Snapshot};
use ember_ui::config::{Corner, EmberConfig};
use ember_ui::ink;
use ember_ui::mini::{self, Key};

const ID: &str = "io.github.n1th1n_19.Ember.Widget";
const W: u32 = 340;

/// Card height for `n` readings (46px rows + caption and border room).
fn height(n: usize) -> u32 {
    70 + 46 * n.max(1) as u32
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Open,
    Config(EmberConfig),
}

struct Widget {
    core: Core,
    sampler: Sampler,
    snap: Snapshot,
    cfg: EmberConfig,
    surface: Option<Id>,
}

struct Card<'a>(&'a Snapshot, Vec<Key>);

impl canvas::Program<Message, Theme, Renderer> for Card<'_> {
    type State = ();

    fn update(&self, _: &mut (), event: &canvas::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(bounds) => {
                Some(canvas::Action::publish(Message::Open).and_capture())
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, _: &(), bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let readings: Vec<_> = self.1.iter().map(|k| mini::read(self.0, *k)).collect();
        // leave room for the caption above and the hard shadow below-right
        mini::card(&mut f, Rectangle::new(Point::new(10.0, 20.0), bounds.size() - Size::new(20.0, 30.0)), "EMBER", &readings);
        vec![f.into_geometry()]
    }
}

fn anchor(c: Corner) -> Anchor {
    (if c.top() { Anchor::TOP } else { Anchor::BOTTOM }) | (if c.left() { Anchor::LEFT } else { Anchor::RIGHT })
}

fn layer(on_top: bool) -> Layer {
    if on_top { Layer::Top } else { Layer::Bottom }
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
        let cfg = EmberConfig::load();
        let (top, right, bottom, left) = cfg.widget_corner.margins(cfg.widget_x, cfg.widget_y);
        let h = height(cfg.widget().len());
        let (anchor, margin, layer) = (anchor(cfg.widget_corner), IcedMargin { top, right, bottom, left }, layer(cfg.widget_on_top));
        let surface = app_layer_shell::<Widget>(
            |_| Default::default(),
            move |state: &mut Widget| {
                let id = Id::unique();
                state.surface = Some(id);
                SctkLayerSurfaceSettings {
                    id,
                    layer,
                    keyboard_interactivity: KeyboardInteractivity::None,
                    anchor,
                    output: IcedOutput::Active,
                    namespace: "ember-widget".into(),
                    margin,
                    size: Some((Some(W), Some(h))),
                    exclusive_zone: 0,
                    size_limits: Limits::NONE.min_width(1.0).min_height(1.0).max_width(W as f32).max_height(height(Key::ALL.len()) as f32),
                    ..Default::default()
                }
            },
            Some(Box::new(|state: &Widget| Element::from(state.card()).map(cosmic::Action::App))),
        );
        (Self { core, sampler, snap, cfg, surface: None }, cosmic::task::message(cosmic::Action::Surface(surface)))
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            cosmic::iced::time::every(Duration::from_secs(2)).map(|_| Message::Tick),
            config_subscription::<_, EmberConfig>(0u8, ember_ui::config::ID.into(), EmberConfig::VERSION).map(|u| Message::Config(u.config)),
        ])
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick => self.snap = self.sampler.sample(),
            Message::Open => open_ember(),
            Message::Config(new) => {
                let old = std::mem::replace(&mut self.cfg, new);
                ink::set_dark(self.cfg.dark);
                let Some(id) = self.surface else { return Task::none() };
                let c = &self.cfg;
                let mut tasks = vec![];
                if (old.widget_corner, old.widget_x, old.widget_y) != (c.widget_corner, c.widget_x, c.widget_y) {
                    let (t, r, b, l) = c.widget_corner.margins(c.widget_x, c.widget_y);
                    tasks.push(set_anchor(id, anchor(c.widget_corner)));
                    tasks.push(set_margin(id, t, r, b, l));
                }
                if old.widget_keys != c.widget_keys {
                    tasks.push(set_size(id, Some(W), Some(height(c.widget().len()))));
                }
                if old.widget_on_top != c.widget_on_top {
                    tasks.push(set_layer(id, layer(c.widget_on_top)));
                }
                return Task::batch(tasks);
            }
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
        cosmic::widget::canvas(Card(&self.snap, self.cfg.widget())).width(Length::Fill).height(Length::Fill).into()
    }
}

fn main() -> cosmic::iced::Result {
    ink::load_fonts();
    ink::set_dark(EmberConfig::load().dark);
    cosmic::app::run::<Widget>(Settings::default().no_main_window(true).transparent(true).exit_on_close(false), ())
}
