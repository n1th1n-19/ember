// SPDX-License-Identifier: GPL-3.0-or-later
//! Apps page: every installed app from every source; uninstall, hide/disable, or copy
//! the command when a button isn't safe or possible.
use cosmic::Element;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, column, row};
use ember_core::apps::{self, App, Plan, Source};
use ember_core::control::CtlResult;
use ember_ui::ink::{CYAN, MAGENTA, RED, YELLOW, bytes};

use crate::Message;
use ember_ui::widgets::{app_icon, banner, body, caption, card, comic_button, heading, muted, page, row_button};

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Vec<App>),
    Search(String),
    Filter(Option<Source>),
    BySize(bool),
    Select(String),
    Plan,
    Planned(String, Plan),
    Confirm,
    Toggle,
    DeleteData,
    Copy(String),
    Done(String, CtlResult),
}

#[derive(Default)]
pub struct Apps {
    apps: Vec<App>,
    loading: bool,
    loaded: bool,
    search: String,
    filter: Option<Source>,
    by_size: bool,
    selected: Option<String>,
    plan: Option<(String, Plan)>,
    busy: bool,
    status: Option<(String, bool)>,
}

fn wrap(m: Msg) -> cosmic::Action<Message> {
    cosmic::Action::App(Message::Apps(m))
}

fn key(a: &App) -> String {
    format!("{:?}:{}", a.source, a.id)
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) -> Msg + Send + 'static) -> Task<Message> {
    cosmic::task::future(async move {
        match tokio::task::spawn_blocking(f).await {
            Ok(v) => wrap(done(v)),
            Err(e) => wrap(Msg::Done("Task".into(), Err(e.to_string()))),
        }
    })
}

impl Apps {
    pub fn open(&mut self) -> Task<Message> {
        if self.loaded || self.loading {
            return Task::none();
        }
        self.reload()
    }

    fn reload(&mut self) -> Task<Message> {
        self.loading = true;
        blocking(apps::list, Msg::Loaded)
    }

    fn selected(&self) -> Option<&App> {
        let k = self.selected.as_ref()?;
        self.apps.iter().find(|a| &key(a) == k)
    }

    pub fn update(&mut self, msg: Msg) -> Task<Message> {
        match msg {
            Msg::Loaded(v) => {
                self.apps = v;
                self.loading = false;
                self.loaded = true;
            }
            Msg::Search(s) => self.search = s,
            Msg::Filter(f) => self.filter = f,
            Msg::BySize(b) => self.by_size = b,
            Msg::Select(k) => {
                self.selected = (self.selected.as_ref() != Some(&k)).then_some(k);
                self.plan = None;
            }
            Msg::Plan => {
                let Some(app) = self.selected().cloned() else { return Task::none() };
                self.busy = true;
                let k = key(&app);
                return blocking(move || apps::plan(&app), move |p| Msg::Planned(k, p));
            }
            Msg::Planned(k, p) => {
                self.busy = false;
                if self.selected.as_ref() == Some(&k) {
                    self.plan = Some((k, p));
                }
            }
            Msg::Confirm => {
                let Some(app) = self.selected().cloned() else { return Task::none() };
                self.busy = true;
                let what = format!("Uninstall {}", app.name);
                return blocking(move || apps::uninstall(&app), move |r| Msg::Done(what, r));
            }
            Msg::Toggle => {
                let Some(app) = self.selected().cloned() else { return Task::none() };
                let off = !app.disabled;
                let verb = match (app.source == Source::Snap, off) {
                    (true, true) => "Disable",
                    (true, false) => "Enable",
                    (false, true) => "Hide",
                    (false, false) => "Show",
                };
                let what = format!("{verb} {}", app.name);
                self.busy = true;
                return blocking(move || apps::set_disabled(&app, off), move |r| Msg::Done(what, r));
            }
            Msg::DeleteData => {
                let Some(app) = self.selected().cloned() else { return Task::none() };
                let what = format!("Move {}'s data to trash", app.name);
                self.busy = true;
                return blocking(move || apps::delete_data(&app), move |r| Msg::Done(what, r));
            }
            Msg::Copy(cmd) => {
                self.status = Some(("Command copied. Paste it into a terminal.".into(), false));
                return cosmic::iced::clipboard::write(cmd);
            }
            Msg::Done(what, r) => {
                self.busy = false;
                let failed = r.is_err();
                self.status = Some(match r {
                    Ok(()) => (format!("Done: {what}"), false),
                    // the plan (with its command) stays on screen as the fallback
                    Err(e) => (format!("{what} failed: {e}  Use the command below instead."), true),
                });
                if !failed {
                    self.plan = None;
                }
                return self.reload();
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let q = self.search.to_lowercase();
        let mut shown: Vec<&App> = self
            .apps
            .iter()
            .filter(|a| self.filter.is_none_or(|f| a.source == f))
            .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q) || a.id.to_lowercase().contains(&q))
            .collect();
        if self.by_size {
            shown.sort_by_key(|a| std::cmp::Reverse(a.size.unwrap_or(0)));
        }

        let count = |s: Option<Source>| self.apps.iter().filter(|a| s.is_none_or(|s| a.source == s)).count();
        let mut chips = row::with_capacity(10).spacing(10).push(comic_button(
            &format!("All {}", count(None)),
            Some(Message::Apps(Msg::Filter(None))),
            YELLOW,
            self.filter.is_none(),
        ));
        for s in Source::ALL {
            let n = count(Some(s));
            if n > 0 {
                chips = chips.push(comic_button(&format!("{} {n}", s.name()), Some(Message::Apps(Msg::Filter(Some(s)))), CYAN, self.filter == Some(s)));
            }
        }
        let tools = row::with_capacity(4)
            .spacing(12)
            .align_y(Alignment::Center)
            .push(widget::text_input::search_input("Search apps", &self.search).on_input(|s| Message::Apps(Msg::Search(s))).width(Length::Fixed(320.0)))
            .push(comic_button("By name", Some(Message::Apps(Msg::BySize(false))), MAGENTA, !self.by_size))
            .push(comic_button("By size", Some(Message::Apps(Msg::BySize(true))), MAGENTA, self.by_size))
            .push(muted(if self.loading {
                "Scanning APT, Flatpak, Snap, AppImages, cargo, npm, bun…".to_string()
            } else {
                format!("{} shown", shown.len())
            }));

        let header = row::with_capacity(5)
            .spacing(10)
            .push(widget::Space::new().width(Length::Fixed(24.0)))
            .push(widget::container(heading("App")).width(Length::FillPortion(4)))
            .push(widget::container(heading("Source")).width(Length::FillPortion(1)))
            .push(widget::container(heading("Version")).width(Length::FillPortion(2)))
            .push(widget::container(heading("Size")).width(Length::FillPortion(1)));
        let mut list = column::with_capacity(shown.len() + 2).spacing(2).push(header);
        for a in shown {
            let k = key(a);
            let selected = self.selected.as_ref() == Some(&k);
            let name = if a.disabled { format!("{}  (hidden)", a.name) } else { a.name.clone() };
            let line = row::with_capacity(4)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(app_icon(a.icon.as_deref(), 24))
                .push(widget::container(body(name)).width(Length::FillPortion(4)).clip(true))
                .push(widget::container(body(a.source.name())).width(Length::FillPortion(1)))
                .push(widget::container(muted(a.version.clone().unwrap_or_else(|| "--".into()))).width(Length::FillPortion(2)).clip(true))
                .push(widget::container(body(a.size.map_or("--".into(), |s| bytes(s as f64)))).width(Length::FillPortion(1)));
            list = list.push(row_button(line, Message::Apps(Msg::Select(k.clone())), selected));
            if selected {
                list = list.push(self.details(a));
            }
        }

        let mut col = column::with_capacity(4).spacing(18).push(caption("App Shelf"));
        if let Some((t, e)) = &self.status {
            col = col.push(banner(t, *e));
        }
        page(col.push(card(column::with_capacity(4).spacing(14).push(tools).push(chips.wrap()).push(list))))
    }

    fn details<'a>(&'a self, a: &'a App) -> Element<'a, Message> {
        let busy = self.busy;
        let on = |m: Msg| (!busy).then_some(Message::Apps(m));
        let mut col = column::with_capacity(6).spacing(10);
        let where_ = a.path.as_ref().or(a.desktop.as_ref()).map(|p| p.display().to_string()).unwrap_or_else(|| a.id.clone());
        col = col.push(muted(where_));

        let toggle_label = match (a.source == Source::Snap, a.disabled) {
            (true, false) => Some("Disable"),
            (true, true) => Some("Enable"),
            (false, _) if a.desktop.is_none() => None,
            (false, false) => Some("Hide from launcher"),
            (false, true) => Some("Show in launcher"),
        };
        let mut actions = row::with_capacity(4).spacing(10).align_y(Alignment::Center);
        actions = actions.push(comic_button("Uninstall…", on(Msg::Plan), RED, true));
        if let Some(l) = toggle_label {
            actions = actions.push(comic_button(l, on(Msg::Toggle), CYAN, false));
        }
        if a.source == Source::Flatpak {
            actions = actions.push(comic_button("Delete app data", on(Msg::DeleteData), MAGENTA, false));
        }
        if busy {
            actions = actions.push(muted("Working… (a password prompt may be open)"));
        }
        col = col.push(actions);

        if let Some((k, p)) = &self.plan
            && *k == key(a)
        {
            let mut confirm = column::with_capacity(5).spacing(8);
            if let Some(why) = &p.blocked {
                confirm = confirm.push(banner(why, true));
            } else if !p.removes.is_empty() {
                let what = if p.removes.len() > 1 {
                    format!("This removes {} items: {}", p.removes.len(), p.removes.join(", "))
                } else {
                    format!("This removes {}.", p.removes[0])
                };
                confirm = confirm.push(body(what));
            }
            confirm = confirm.push(
                row::with_capacity(3)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(widget::container(widget::text(p.command.clone()).font(cosmic::iced::Font::MONOSPACE).size(13)).padding([6, 10]).width(Length::Fill))
                    .push(comic_button("Copy command", Some(Message::Apps(Msg::Copy(p.command.clone()))), YELLOW, false)),
            );
            if p.button {
                confirm = confirm.push(comic_button(&format!("Yes, uninstall {}", a.name), on(Msg::Confirm), RED, true));
            }
            col = col.push(confirm);
        }
        widget::container(col).padding([6, 12, 14, 12]).into()
    }
}
