use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use cosmic::iced::Subscription;
use futures::Stream;

use crate::applet::message::Message;
use crate::config::CONFIG_VERSION;
use crate::pause::PauseHandle;
use crate::pause::model::{Settings, Snapshot};
use crate::pause::schedule::Record;

pub const UI_TICK: Duration = Duration::from_secs(1);

struct Source(PauseHandle);

impl Hash for Source {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "cosmic-ext-applet-pause-snapshots".hash(state);
    }
}

pub fn snapshots(handle: &PauseHandle) -> Subscription<Message> {
    Subscription::run_with(Source(handle.clone()), |Source(handle)| {
        stream(handle.clone())
    })
}

#[allow(clippy::needless_pass_by_value)]
fn stream(handle: PauseHandle) -> impl Stream<Item = Message> + Send + 'static {
    futures::stream::unfold(
        (handle.subscribe(), true),
        |(mut receiver, first)| async move {
            if !first && receiver.changed().await.is_err() {
                return None;
            }

            let snapshot = Arc::clone(&receiver.borrow_and_update());
            Some((Message::Snapshot(snapshot), (receiver, false)))
        },
    )
}

pub fn settings() -> Subscription<Message> {
    cosmic::cosmic_config::config_subscription::<_, Settings>(
        "cosmic-ext-applet-pause-settings",
        crate::APP_ID.into(),
        CONFIG_VERSION,
    )
    .map(|update| Message::SettingsChanged(Box::new(update.config)))
}

pub fn schedule() -> Subscription<Message> {
    cosmic::cosmic_config::config_subscription::<_, Record>(
        "cosmic-ext-applet-pause-schedule",
        crate::APP_ID.into(),
        CONFIG_VERSION,
    )
    .map(|update| Message::ScheduleChanged(Box::new(update.config)))
}

pub fn do_not_disturb() -> Subscription<Message> {
    cosmic::cosmic_config::config_subscription::<_, crate::config::DoNotDisturb>(
        "cosmic-ext-applet-pause-do-not-disturb",
        crate::config::NOTIFICATIONS_APP_ID.into(),
        crate::config::NOTIFICATIONS_VERSION,
    )
    .map(|update| Message::DoNotDisturbChanged(update.config.0))
}

pub fn tick() -> Subscription<Message> {
    cosmic::iced::time::every(UI_TICK).map(|_| Message::Tick)
}

pub fn now() -> crate::pause::model::Moment {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        });

    crate::pause::model::Moment::from_epoch_seconds(seconds)
}

pub fn initial(handle: &PauseHandle) -> Arc<Snapshot> {
    handle.snapshot()
}
