use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ReminderKind {
    Eyes,
    Water,
    Breathe,
    Move,
    Posture,
    Wrists,
}

impl ReminderKind {
    pub const COUNT: usize = 6;

    pub const ALL: [Self; Self::COUNT] = [
        Self::Eyes,
        Self::Water,
        Self::Breathe,
        Self::Move,
        Self::Posture,
        Self::Wrists,
    ];

    pub fn index(self) -> usize {
        match self {
            Self::Eyes => 0,
            Self::Water => 1,
            Self::Breathe => 2,
            Self::Move => 3,
            Self::Posture => 4,
            Self::Wrists => 5,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Eyes => "eyes",
            Self::Water => "water",
            Self::Breathe => "breathe",
            Self::Move => "move",
            Self::Posture => "posture",
            Self::Wrists => "wrists",
        }
    }

    pub fn due_key(self) -> &'static str {
        match self {
            Self::Eyes => "due-eyes",
            Self::Water => "due-water",
            Self::Breathe => "due-breathe",
            Self::Move => "due-move",
            Self::Posture => "due-posture",
            Self::Wrists => "due-wrists",
        }
    }

    pub fn pending_key(self) -> &'static str {
        match self {
            Self::Eyes => "pending-eyes",
            Self::Water => "pending-water",
            Self::Breathe => "pending-breathe",
            Self::Move => "pending-move",
            Self::Posture => "pending-posture",
            Self::Wrists => "pending-wrists",
        }
    }

    pub fn default_interval(self) -> Duration {
        let minutes = match self {
            Self::Eyes => 20,
            Self::Breathe => 30,
            Self::Water => 45,
            Self::Move | Self::Wrists => 60,
            Self::Posture => 90,
        };

        Duration::from_mins(minutes)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Moment(i64);

impl Moment {
    pub const EPOCH: Self = Self(0);

    pub fn from_epoch_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    pub fn epoch_seconds(self) -> i64 {
        self.0
    }

    pub fn saturating_add(self, span: Duration) -> Self {
        let seconds = i64::try_from(span.as_secs()).unwrap_or(i64::MAX);
        Self(self.0.saturating_add(seconds))
    }

    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_secs(u64::try_from(self.0.saturating_sub(earlier.0)).unwrap_or(0))
    }

    pub fn is_after(self, other: Self) -> bool {
        self.0 > other.0
    }

    pub fn clamp_between(self, low: Self, high: Self) -> Self {
        if high < low {
            low
        } else {
            self.clamp(low, high)
        }
    }
}

pub const MIN_INTERVAL: Duration = Duration::from_mins(1);
pub const MAX_INTERVAL: Duration = Duration::from_hours(4);
pub const MIN_SNOOZE: Duration = Duration::from_mins(1);
pub const MIN_NOTIFICATION: Duration = Duration::from_secs(3);
pub const MAX_NOTIFICATION: Duration = Duration::from_mins(1);
pub const MAX_SNOOZE: Duration = Duration::from_hours(1);

pub const INTERVAL_STEP_MINUTES: u32 = 10;
pub const MIN_INTERVAL_MINUTES: u32 = 10;
pub const MAX_INTERVAL_MINUTES: u32 = 120;
pub const SNOOZE_PRESETS_MINUTES: [u32; 4] = [5, 10, 15, 30];
pub const NOTIFICATION_PRESETS_SECONDS: [u32; 2] = [5, 10];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReminderSetting {
    pub enabled: bool,
    pub interval_secs: u32,
}

impl ReminderSetting {
    pub fn interval(self) -> Duration {
        Duration::from_secs(u64::from(self.interval_secs))
    }

    fn sanitised(mut self) -> Self {
        let low = u32::try_from(MIN_INTERVAL.as_secs()).unwrap_or(u32::MAX);
        let high = u32::try_from(MAX_INTERVAL.as_secs()).unwrap_or(u32::MAX);
        self.interval_secs = self.interval_secs.clamp(low, high);
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Settings {
    reminders: [ReminderSetting; ReminderKind::COUNT],
    pub snooze_secs: u32,
    pub notification_secs: u32,
}

impl Default for Settings {
    fn default() -> Self {
        let mut reminders = [ReminderSetting {
            enabled: true,
            interval_secs: 0,
        }; ReminderKind::COUNT];

        for kind in ReminderKind::ALL {
            reminders[kind.index()].interval_secs =
                u32::try_from(kind.default_interval().as_secs()).unwrap_or(u32::MAX);
        }

        Self {
            reminders,
            snooze_secs: 10 * 60,
            notification_secs: 10,
        }
    }
}

impl Settings {
    pub fn get(&self, kind: ReminderKind) -> ReminderSetting {
        self.reminders[kind.index()]
    }

    pub fn set(&mut self, kind: ReminderKind, setting: ReminderSetting) {
        self.reminders[kind.index()] = setting.sanitised();
    }

    pub fn interval(&self, kind: ReminderKind) -> Duration {
        self.get(kind).interval()
    }

    pub fn enabled(&self, kind: ReminderKind) -> bool {
        self.get(kind).enabled
    }

    pub fn snooze(&self) -> Duration {
        Duration::from_secs(u64::from(self.snooze_secs))
    }

    pub fn notification(&self) -> Duration {
        Duration::from_secs(u64::from(self.notification_secs))
    }

    pub fn any_enabled(&self) -> bool {
        ReminderKind::ALL.iter().any(|kind| self.enabled(*kind))
    }

    pub fn sanitised(mut self) -> Self {
        for index in 0..self.reminders.len() {
            self.reminders[index] = self.reminders[index].sanitised();
        }

        let low = u32::try_from(MIN_SNOOZE.as_secs()).unwrap_or(u32::MAX);
        let high = u32::try_from(MAX_SNOOZE.as_secs()).unwrap_or(u32::MAX);
        self.snooze_secs = self.snooze_secs.clamp(low, high);

        let low = u32::try_from(MIN_NOTIFICATION.as_secs()).unwrap_or(u32::MAX);
        let high = u32::try_from(MAX_NOTIFICATION.as_secs()).unwrap_or(u32::MAX);
        self.notification_secs = self.notification_secs.clamp(low, high);

        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PauseRecord {
    pub started_at: Moment,
    pub until: Option<Moment>,
}

impl PauseRecord {
    pub fn has_expired(&self, now: Moment) -> bool {
        self.until.is_some_and(|until| !until.is_after(now))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reminder {
    pub kind: ReminderKind,
    pub setting: ReminderSetting,
    pub due_at: Moment,
    pub pending: bool,
}

impl Reminder {
    pub fn remaining(&self, now: Moment) -> Duration {
        self.due_at.saturating_duration_since(now)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotifierState {
    #[default]
    Unknown,
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub reminders: Vec<Reminder>,
    pub pause: Option<PauseRecord>,
    pub leader: bool,
    pub notifier: NotifierState,
    pub revision: u64,
}

impl Snapshot {
    pub fn next_due(&self) -> Option<&Reminder> {
        self.reminders
            .iter()
            .filter(|reminder| reminder.setting.enabled)
            .min_by_key(|reminder| reminder.due_at)
    }

    pub fn first_pending(&self) -> Option<&Reminder> {
        self.reminders
            .iter()
            .find(|reminder| reminder.pending && reminder.setting.enabled)
    }

    pub fn is_paused(&self) -> bool {
        self.pause.is_some()
    }

    pub fn reference(&self, now: Moment) -> Moment {
        self.pause.map_or(now, |pause| pause.started_at.min(now))
    }

    pub fn same_state(&self, other: &Self) -> bool {
        self.reminders == other.reminders
            && self.pause == other.pause
            && self.leader == other.leader
            && self.notifier == other.notifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_own_slot_and_its_own_config_key() {
        let mut indexes: Vec<usize> = ReminderKind::ALL.iter().map(|k| k.index()).collect();
        indexes.sort_unstable();
        assert_eq!(indexes, vec![0, 1, 2, 3, 4, 5]);

        let mut keys: Vec<&str> = ReminderKind::ALL
            .iter()
            .flat_map(|k| [k.key(), k.due_key(), k.pending_key()])
            .collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(
            keys.len(),
            ReminderKind::COUNT * 3,
            "every config key must be its own"
        );
    }

    #[test]
    fn the_defaults_match_the_intervals_the_product_promises() {
        let settings = Settings::default();

        for (kind, minutes) in [
            (ReminderKind::Eyes, 20),
            (ReminderKind::Breathe, 30),
            (ReminderKind::Water, 45),
            (ReminderKind::Move, 60),
            (ReminderKind::Posture, 90),
            (ReminderKind::Wrists, 60),
        ] {
            assert_eq!(settings.interval(kind), Duration::from_mins(minutes));
            assert!(settings.enabled(kind));
        }

        assert_eq!(settings.snooze(), Duration::from_mins(10));
        assert_eq!(settings.notification(), Duration::from_secs(10));
    }

    #[test]
    fn a_hand_edited_config_is_clamped_instead_of_trusted() {
        let mut settings = Settings::default();
        settings.reminders[0].interval_secs = 0;
        settings.reminders[1].interval_secs = u32::MAX;
        settings.snooze_secs = 0;
        settings.notification_secs = u32::MAX;

        let settings = settings.sanitised();

        assert_eq!(settings.interval(ReminderKind::Eyes), MIN_INTERVAL);
        assert_eq!(settings.interval(ReminderKind::Water), MAX_INTERVAL);
        assert_eq!(settings.snooze(), MIN_SNOOZE);
        assert_eq!(settings.notification(), MAX_NOTIFICATION);
    }

    #[test]
    fn a_moment_never_travels_backwards_when_a_duration_is_added() {
        let now = Moment::from_epoch_seconds(1_700_000_000);
        assert_eq!(
            now.saturating_add(Duration::from_mins(1)).epoch_seconds(),
            1_700_000_060
        );
        assert_eq!(now.saturating_duration_since(now), Duration::ZERO);
        assert_eq!(
            now.saturating_duration_since(now.saturating_add(Duration::from_mins(1))),
            Duration::ZERO
        );
    }

    #[test]
    fn a_paused_countdown_is_frozen_where_the_user_left_it() {
        let paused_at = Moment::from_epoch_seconds(1_700_000_000);
        let snapshot = Snapshot {
            pause: Some(PauseRecord {
                started_at: paused_at,
                until: None,
            }),
            ..Snapshot::default()
        };

        let much_later = paused_at.saturating_add(Duration::from_hours(3));

        assert_eq!(
            snapshot.reference(much_later),
            paused_at,
            "three hours of pause must not eat three hours off the countdown"
        );
    }

    #[test]
    fn a_running_countdown_is_measured_against_the_present() {
        let now = Moment::from_epoch_seconds(1_700_000_000);

        assert_eq!(Snapshot::default().reference(now), now);
    }

    #[test]
    fn an_impossible_clamp_range_settles_on_the_lower_bound() {
        let low = Moment::from_epoch_seconds(100);
        let high = Moment::from_epoch_seconds(50);
        let value = Moment::from_epoch_seconds(10);

        assert_eq!(value.clamp_between(low, high), low);
    }
}
