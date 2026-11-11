// SPDX-License-Identifier: GPL-3.0-or-later
//! Readings applet: live meters in the COSMIC dock or panel. Click for a comic popup
//! with more readings, a power-profile switch and a button that opens Ember.
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{CosmicConfigEntry, config_subscription};
use cosmic::iced::widget::canvas::{self, Frame, Geometry};
use cosmic::iced::window::Id;
use cosmic::iced::{Alignment, Length, Point, Rectangle, Subscription, mouse};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, column, row};
use cosmic::{Element, Renderer, Theme};
use ember_core::control::power::{self, Profile};
use ember_core::{Sampler, Snapshot};
use ember_ui::config::EmberConfig;
use ember_ui::ink;
use ember_ui::mini::{self, Key};

const ID: &str = "io.github.n1th1n_19.Ember.Applet";
const POPUP: [Key; 7] = [Key::Cpu, Key::Ram, Key::Temp, Key::Gpu, Key::Battery, Key::Net, Key::Disk];

#[derive(Debug, Clone)]
enum Message {
    Tick,
    PopupClosed(Id),
    Surface(cosmic::surface::Action<Message>),
    Profile(Option<Profile>),
    SetProfile(Profile),
    OpenEmber,
    Config(EmberConfig),
}

struct Applet {
    core: Core,
    popup: Option<Id>,
    sampler: Sampler,
    snap: Snapshot,
    profile: Option<Profile>,
    cfg: EmberConfig,
}

/// Canvas program for the tile in the dock.
struct Tile<'a> {
    snap: &'a Snapshot,
    keys: Vec<Key>,
    horizontal: bool,
}

impl canvas::Program<Message, Theme, Renderer> for Tile<'_> {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let readings: Vec<_> = self.keys.iter().map(|k| mini::read(self.snap, *k)).collect();
        mini::meters(&mut f, Rectangle::new(Point::new(1.0, 1.0), bounds.size() - cosmic::iced::Size::new(2.0, 2.0)), &readings, self.horizontal);
        vec![f.into_geometry()]
    }
}

/// Canvas program for the popup card.
struct Card<'a> {
    snap: &'a Snapshot,
}

impl canvas::Program<Message, Theme, Renderer> for Card<'_> {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        f.fill(&ink::rect(Rectangle::new(Point::ORIGIN, bounds.size())), ink::pal().page);
        let readings: Vec<_> = POPUP.iter().map(|k| mini::read(self.snap, *k)).collect();
        mini::card(&mut f, Rectangle::new(Point::new(14.0, 24.0), bounds.size() - cosmic::iced::Size::new(28.0, 34.0)), "EMBER", &readings);
        vec![f.into_geometry()]
    }
}

fn load_profile() -> Task<Message> {
    cosmic::task::future(async { cosmic::Action::App(Message::Profile(power::read().await.and_then(|p| p.profile))) })
}

/// Installed `ember`, or the one next to this binary in a dev build.
fn open_ember() {
    let sibling = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("ember"))).filter(|p| p.exists());
    let cmd = sibling.unwrap_or_else(|| "ember".into());
    if let Err(e) = std::process::Command::new(&cmd).spawn() {
        eprintln!("ember-applet: couldn't start {}: {e}", cmd.display());
    }
}

impl cosmic::Application for Applet {
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
        (Self { core, popup: None, sampler, snap, profile: None, cfg: EmberConfig::load() }, load_profile())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        // faster while the popup is open
        let every = if self.popup.is_some() { 1 } else { 2 };
        Subscription::batch([
            cosmic::iced::time::every(Duration::from_secs(every)).map(|_| Message::Tick),
            config_subscription::<_, EmberConfig>(0u8, ember_ui::config::ID.into(), EmberConfig::VERSION).map(|u| Message::Config(u.config)),
        ])
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick => self.snap = self.sampler.sample(),
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Surface(a) => return cosmic::task::message(cosmic::Action::Surface(a)),
            Message::Profile(p) => self.profile = p,
            Message::Config(c) => {
                ink::set_dark(c.dark);
                self.cfg = c;
            }
            Message::SetProfile(p) => {
                self.profile = Some(p);
                return cosmic::task::future(async move {
                    if let Err(e) = power::set_profile(p).await {
                        eprintln!("ember-applet: {e}");
                    }
                    cosmic::Action::App(Message::Profile(power::read().await.and_then(|s| s.profile)))
                });
            }
            Message::OpenEmber => {
                open_ember();
                if let Some(id) = self.popup.take() {
                    return cosmic::task::message(cosmic::Action::Surface(destroy_popup(id)));
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let applet = &self.core.applet;
        let horizontal = applet.is_horizontal();
        let keys = self.cfg.applet();
        let icon = f32::from(applet.suggested_size(true).0);
        // horizontal panel: a slim bar, short name and number for each reading ("▮ CPU 12%"), readable
        // even on an XS panel; vertical panel: stacked bars only (no room for text)
        let tile: Element<'_, Message> = if horizontal {
            let (bar_w, label_px, value_px) = ((icon * 0.45).max(7.0), (icon * 0.7).max(10.0), icon.max(13.0));
            let vals: Vec<_> = keys.iter().map(|k| mini::read(&self.snap, *k).value).collect();
            // rough width (Bangers ≈ 0.45 em/char, Space Grotesk ≈ 0.6) to see whether
            // names fit in the room the panel offers; if not, show bars and numbers only
            let est = |labels: bool| -> f32 {
                keys.iter()
                    .zip(&vals)
                    .map(|(k, v)| {
                        bar_w
                            + 12.0
                            + v.chars().count().max(3) as f32 * value_px * 0.45
                            + if labels { k.short().len() as f32 * label_px * 0.6 + 4.0 } else { 0.0 }
                    })
                    .sum()
            };
            let room = applet.suggested_bounds.as_ref().map(|b| b.width).filter(|w| *w > 0.0);
            let labels = room.is_none_or(|w| est(true) <= w);
            let mut r = row::with_capacity(keys.len() * 3).spacing(4).align_y(Alignment::Center);
            for (i, k) in keys.iter().enumerate() {
                r = r.push(widget::canvas(Tile { snap: &self.snap, keys: vec![*k], horizontal }).width(Length::Fixed(bar_w)).height(Length::Fixed(icon)));
                if labels {
                    r = r.push(widget::text(k.short()).font(ink::BODY).size(label_px));
                }
                r = r.push(widget::text(vals[i].clone()).font(ink::DISPLAY).size(value_px));
                if i + 1 < keys.len() {
                    r = r.push(widget::Space::new().width(Length::Fixed(4.0)));
                }
            }
            r.into()
        } else {
            let along = icon * (0.55 * keys.len().max(1) as f32).max(0.6);
            widget::canvas(Tile { snap: &self.snap, keys: keys.clone(), horizontal }).width(Length::Fixed(icon)).height(Length::Fixed(along)).into()
        };

        let have_popup = self.popup;
        let btn = cosmic::widget::button::custom(tile)
            .class(cosmic::theme::Button::AppletIcon)
            .padding({
                let (px, py) = applet.suggested_padding(true);
                [py, px]
            })
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    return Message::Surface(destroy_popup(id));
                }
                Message::Surface(app_popup::<Applet>(
                    |_| Default::default(),
                    move |state: &mut Applet| {
                        let id = Id::unique();
                        state.popup = Some(id);
                        let mut s = state.core.applet.get_popup_settings(state.core.main_window_id().unwrap(), id, Some((360, 470)), None, None);
                        s.positioner.anchor_rect = Rectangle {
                            x: (bounds.x - offset.x) as i32,
                            y: (bounds.y - offset.y) as i32,
                            width: bounds.width as i32,
                            height: bounds.height as i32,
                        };
                        s
                    },
                    Some(Box::new(|state: &Applet| Element::from(state.core.applet.popup_container(state.popup_view())).map(cosmic::Action::App))),
                ))
            });

        let summary = keys
            .iter()
            .map(|k| {
                let r = mini::read(&self.snap, *k);
                format!("{} {}", k.short(), r.value)
            })
            .collect::<Vec<_>>()
            .join("  ·  ");
        // autosize: the panel gives the applet exactly the room its content needs
        applet.autosize_window(applet.applet_tooltip::<Message>(btn, summary, self.popup.is_some(), Message::Surface, None)).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        "".into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl Applet {
    fn popup_view(&self) -> Element<'_, Message> {
        let card = widget::canvas(Card { snap: &self.snap }).width(Length::Fixed(332.0)).height(Length::Fixed(340.0));
        let mut profiles = row::with_capacity(3).spacing(6);
        for p in Profile::ALL {
            let b = widget::button::text(p.name()).on_press(Message::SetProfile(p));
            profiles =
                profiles.push(if self.profile == Some(p) { b.class(cosmic::theme::Button::Suggested) } else { b.class(cosmic::theme::Button::Standard) });
        }
        let mut col = column::with_capacity(3).spacing(10).padding(8).align_x(Alignment::Center).push(card);
        if self.profile.is_some() {
            col = col.push(profiles);
        }
        col.push(widget::button::suggested("Open Ember").on_press(Message::OpenEmber)).into()
    }
}

pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<Applet>(())
}
