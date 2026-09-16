pub mod message;
pub mod popup;
pub mod style;
pub mod subscription;
pub mod symbols;
pub mod view;

use std::sync::Arc;

use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::shell::commands::popup::destroy_popup;
use cosmic::iced::{Subscription, window};
use cosmic::widget;
use cosmic::{Application, Element};

use crate::APP_ID;
use crate::config::SettingsStore;
use crate::pause::model::{Moment, Settings, Snapshot};
use crate::pause::{Command, PauseHandle};

pub use message::Message;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PopupState {
    #[default]
    Closed,
    Open {
        id: window::Id,
        closing: bool,
    },
}

impl PopupState {
    fn id(self) -> Option<window::Id> {
        match self {
            Self::Closed => None,
            Self::Open { id, .. } => Some(id),
        }
    }
}

pub struct Pause {
    core: Core,
    scheduler: PauseHandle,
    store: SettingsStore,
    snapshot: Arc<Snapshot>,
    settings: Settings,
    now: Moment,
    popup: PopupState,
    page: Page,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Page {
    #[default]
    Overview,
    Settings,
}

impl Pause {
    fn open_popup(&mut self) -> Task<Message> {
        let id = window::Id::unique();
        self.popup = PopupState::Open { id, closing: false };
        self.now = subscription::now();

        let parent = self
            .core
            .main_window_id()
            .expect("an applet always has a main window");

        cosmic::surface::surface_task(cosmic::surface::action::app_popup::<Self>(
            |_| cosmic::surface::action::LiveSettings::default(),
            move |app| {
                let mut settings = app
                    .core
                    .applet
                    .get_popup_settings(parent, id, None, None, None);
                settings.positioner.size_limits = popup::surface_limits();
                settings
            },
            None,
        ))
    }

    fn close_popup(&mut self) -> Task<Message> {
        match self.popup {
            PopupState::Open { id, closing: false } => {
                self.popup = PopupState::Open { id, closing: true };
                destroy_popup(id)
            }
            PopupState::Closed | PopupState::Open { closing: true, .. } => Task::none(),
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn now(&self) -> Moment {
        self.now
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    fn command(&self, command: Command) -> Task<Message> {
        self.scheduler.send(command);
        Task::none()
    }

    fn commit(&mut self, settings: Settings) -> Task<Message> {
        let settings = settings.sanitised();

        if self.settings == settings {
            return cosmic::task::message(Message::Relayout);
        }

        self.settings = settings;
        self.store.save(&settings);
        self.scheduler.send(Command::Settings(Box::new(settings)));

        cosmic::task::message(Message::Relayout)
    }

    fn toggle_popup(&mut self) -> Task<Message> {
        if self.popup == PopupState::Closed {
            self.open_popup()
        } else {
            self.close_popup()
        }
    }
}

fn panel_icon(snapshot: &Snapshot) -> widget::icon::Handle {
    if snapshot.is_paused() {
        symbols::panel_paused()
    } else {
        symbols::panel()
    }
}

fn reconcile_surface_closed(id: window::Id, state: &mut PopupState) -> bool {
    if state.id() == Some(id) {
        *state = PopupState::Closed;
        return true;
    }

    false
}

impl Application for Pause {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = PauseHandle;
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, scheduler: Self::Flags) -> (Self, Task<Message>) {
        let store = SettingsStore::open(APP_ID);
        let settings = store.load();
        let snapshot = scheduler.snapshot();

        (
            Self {
                core,
                scheduler,
                store,
                snapshot,
                settings,
                now: subscription::now(),
                popup: PopupState::Closed,
                page: Page::Overview,
            },
            Task::none(),
        )
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::SurfaceClosed(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut sources = vec![
            subscription::snapshots(&self.scheduler),
            subscription::settings(),
            subscription::schedule(),
            subscription::do_not_disturb(),
        ];

        if self.popup != PopupState::Closed {
            sources.push(subscription::tick());
        }

        Subscription::batch(sources)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => self.toggle_popup(),
            Message::SurfaceClosed(id) => {
                if reconcile_surface_closed(id, &mut self.popup) {
                    self.page = Page::Overview;
                }
                Task::none()
            }
            Message::Surface(action) => cosmic::task::message(cosmic::Action::Surface(action)),
            Message::Snapshot(snapshot) => {
                self.snapshot = snapshot;
                self.now = subscription::now();
                Task::none()
            }
            Message::SettingsChanged(settings) => {
                let settings = settings.sanitised();
                if self.settings != settings {
                    self.settings = settings;
                    self.scheduler.send(Command::Settings(Box::new(settings)));
                }
                Task::none()
            }
            Message::ScheduleChanged(record) => {
                self.scheduler.send(Command::Schedule(record));
                Task::none()
            }
            Message::DoNotDisturbChanged(quiet) => self.command(Command::DoNotDisturb(quiet)),
            Message::Tick => {
                self.now = subscription::now();
                Task::none()
            }
            Message::ShowSettings(showing) => {
                self.page = if showing {
                    Page::Settings
                } else {
                    Page::Overview
                };
                cosmic::task::message(Message::Relayout)
            }
            Message::Toggle(kind, enabled) => {
                let mut next = self.settings;
                let mut setting = next.get(kind);
                setting.enabled = enabled;
                next.set(kind, setting);
                self.commit(next)
            }
            Message::Setting(settings) => self.commit(*settings),
            Message::ResetTimers => self.command(Command::ResetTimers),
            Message::PauseFor(span) => self.command(Command::PauseFor(span)),
            Message::Extend(span) => self.command(Command::Extend(span)),
            Message::Resume => self.command(Command::Resume),
            Message::OpenLink(url) => cosmic::task::future(async move {
                crate::links::open(url).await;
                Message::Relayout
            }),
            Message::Relayout => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let button = self
            .core
            .applet
            .icon_button_from_handle(panel_icon(&self.snapshot))
            .on_press(Message::TogglePopup);

        widget::autosize::autosize(button, popup::PANEL_ID.clone())
            .limits(popup::panel_limits(
                self.core.applet.suggested_bounds,
                self.core.applet.is_horizontal(),
            ))
            .into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.popup.id() != Some(id) {
            return widget::text::body("").into();
        }

        view::popup(self)
    }
}

pub fn run(scheduler: PauseHandle) -> cosmic::iced::Result {
    cosmic::applet::run::<Pause>(scheduler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_popup_leaves_no_surface() {
        let id = window::Id::unique();
        let mut state = PopupState::Open { id, closing: true };

        assert!(reconcile_surface_closed(id, &mut state));
        assert_eq!(state, PopupState::Closed);
    }

    #[test]
    fn a_reminder_coming_due_leaves_the_panel_icon_alone() {
        use crate::pause::model::{Reminder, ReminderKind, ReminderSetting};

        let face = |snapshot: &Snapshot| format!("{:?}", panel_icon(snapshot).data);

        let resting = Snapshot::default();
        let due = Snapshot {
            reminders: vec![Reminder {
                kind: ReminderKind::Eyes,
                setting: ReminderSetting {
                    enabled: true,
                    interval_secs: 20 * 60,
                },
                due_at: Moment::EPOCH,
                pending: true,
            }],
            ..Snapshot::default()
        };
        let frozen = Snapshot {
            quiet_since: Some(Moment::from_epoch_seconds(1_700_000_000)),
            ..Snapshot::default()
        };

        assert_eq!(
            face(&due),
            face(&resting),
            "the notification is the announcement, the panel is not"
        );
        assert_ne!(
            face(&frozen),
            face(&resting),
            "but a frozen schedule has to be visible at a glance"
        );
    }

    #[test]
    fn an_unrelated_surface_does_not_change_popup_state() {
        let id = window::Id::unique();
        let other = window::Id::unique();
        let mut state = PopupState::Open { id, closing: false };

        assert!(!reconcile_surface_closed(other, &mut state));
        assert_eq!(state, PopupState::Open { id, closing: false });
    }
}
