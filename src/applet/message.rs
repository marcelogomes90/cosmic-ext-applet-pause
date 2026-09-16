use std::sync::Arc;

use cosmic::iced::window;

use crate::pause::model::{ReminderKind, Settings, Snapshot};
use crate::pause::schedule::Record;

#[derive(Clone, Debug)]
pub enum Message {
    TogglePopup,
    SurfaceClosed(window::Id),
    Surface(cosmic::surface::Action<Message>),
    Snapshot(Arc<Snapshot>),
    SettingsChanged(Box<Settings>),
    ScheduleChanged(Box<Record>),
    DoNotDisturbChanged(bool),
    Tick,
    ShowSettings(bool),
    Toggle(ReminderKind, bool),
    Setting(Box<Settings>),
    ResetTimers,
    PauseFor(Option<std::time::Duration>),
    Extend(std::time::Duration),
    Resume,
    OpenLink(&'static str),
    Relayout,
}
