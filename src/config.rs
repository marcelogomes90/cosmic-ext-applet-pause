use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet, CosmicConfigEntry, Error};

use crate::pause::model::{PauseRecord, ReminderKind, ReminderSetting, Settings};
use crate::pause::schedule::Record;

pub const CONFIG_VERSION: u64 = 1;
pub const SNOOZE_KEY: &str = "snooze-secs";
pub const NOTIFICATION_KEY: &str = "notification-secs";
pub const PAUSE_KEY: &str = "pause";

impl CosmicConfigEntry for Settings {
    const VERSION: u64 = CONFIG_VERSION;

    fn write_entry(&self, config: &Config) -> Result<(), Error> {
        let mut first: Option<Error> = None;

        for kind in ReminderKind::ALL {
            if let Err(error) = config.set(kind.key(), self.get(kind))
                && first.is_none()
            {
                first = Some(error);
            }
        }

        if let Err(error) = config.set(SNOOZE_KEY, self.snooze_secs)
            && first.is_none()
        {
            first = Some(error);
        }

        if let Err(error) = config.set(NOTIFICATION_KEY, self.notification_secs)
            && first.is_none()
        {
            first = Some(error);
        }

        match first {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn get_entry(config: &Config) -> Result<Self, (Vec<Error>, Self)> {
        let mut settings = Self::default();
        let mut errors = Vec::new();

        for kind in ReminderKind::ALL {
            match config.get::<ReminderSetting>(kind.key()) {
                Ok(setting) => settings.set(kind, setting),
                Err(error) => errors.push(error),
            }
        }

        match config.get::<u32>(SNOOZE_KEY) {
            Ok(snooze) => settings.snooze_secs = snooze,
            Err(error) => errors.push(error),
        }

        match config.get::<u32>(NOTIFICATION_KEY) {
            Ok(seconds) => settings.notification_secs = seconds,
            Err(error) => errors.push(error),
        }

        let settings = settings.sanitised();

        if errors.is_empty() {
            Ok(settings)
        } else {
            Err((errors, settings))
        }
    }

    fn update_keys<T: AsRef<str>>(
        &mut self,
        config: &Config,
        changed_keys: &[T],
    ) -> (Vec<Error>, Vec<&'static str>) {
        let mut errors = Vec::new();
        let mut updated = Vec::new();

        for key in changed_keys {
            let key = key.as_ref();

            if key == NOTIFICATION_KEY {
                match config.get::<u32>(NOTIFICATION_KEY) {
                    Ok(seconds) if self.notification_secs != seconds => {
                        self.notification_secs = seconds;
                        updated.push(NOTIFICATION_KEY);
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(error),
                }
                continue;
            }

            if key == SNOOZE_KEY {
                match config.get::<u32>(SNOOZE_KEY) {
                    Ok(snooze) if self.snooze_secs != snooze => {
                        self.snooze_secs = snooze;
                        updated.push(SNOOZE_KEY);
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(error),
                }
                continue;
            }

            let Some(kind) = ReminderKind::ALL.into_iter().find(|kind| kind.key() == key) else {
                continue;
            };

            match config.get::<ReminderSetting>(key) {
                Ok(setting) if self.get(kind) != setting => {
                    self.set(kind, setting);
                    updated.push(kind.key());
                }
                Ok(_) => {}
                Err(error) => errors.push(error),
            }
        }

        if !updated.is_empty() {
            *self = self.sanitised();
        }

        (errors, updated)
    }
}

impl CosmicConfigEntry for Record {
    const VERSION: u64 = CONFIG_VERSION;

    fn write_entry(&self, config: &Config) -> Result<(), Error> {
        let mut first: Option<Error> = None;

        for kind in ReminderKind::ALL {
            let index = kind.index();

            if let Err(error) = config.set(kind.due_key(), self.due[index])
                && first.is_none()
            {
                first = Some(error);
            }

            if let Err(error) = config.set(kind.pending_key(), self.pending_until[index])
                && first.is_none()
            {
                first = Some(error);
            }
        }

        if let Err(error) = config.set(PAUSE_KEY, self.pause)
            && first.is_none()
        {
            first = Some(error);
        }

        match first {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn get_entry(config: &Config) -> Result<Self, (Vec<Error>, Self)> {
        let mut record = Self::default();
        let mut errors = Vec::new();

        for kind in ReminderKind::ALL {
            let index = kind.index();

            match config.get(kind.due_key()) {
                Ok(due) => record.due[index] = due,
                Err(error) => errors.push(error),
            }

            match config.get(kind.pending_key()) {
                Ok(pending) => record.pending_until[index] = pending,
                Err(error) => errors.push(error),
            }
        }

        match config.get::<Option<PauseRecord>>(PAUSE_KEY) {
            Ok(pause) => record.pause = pause,
            Err(error) => errors.push(error),
        }

        if errors.is_empty() {
            Ok(record)
        } else {
            Err((errors, record))
        }
    }

    fn update_keys<T: AsRef<str>>(
        &mut self,
        config: &Config,
        changed_keys: &[T],
    ) -> (Vec<Error>, Vec<&'static str>) {
        let mut errors = Vec::new();
        let mut updated = Vec::new();

        for key in changed_keys {
            let key = key.as_ref();

            if key == PAUSE_KEY {
                match config.get::<Option<PauseRecord>>(PAUSE_KEY) {
                    Ok(pause) if self.pause != pause => {
                        self.pause = pause;
                        updated.push(PAUSE_KEY);
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(error),
                }
                continue;
            }

            for kind in ReminderKind::ALL {
                let index = kind.index();

                if key == kind.due_key() {
                    match config.get(key) {
                        Ok(due) if self.due[index] != due => {
                            self.due[index] = due;
                            updated.push(kind.due_key());
                        }
                        Ok(_) => {}
                        Err(error) => errors.push(error),
                    }
                } else if key == kind.pending_key() {
                    match config.get(key) {
                        Ok(pending) if self.pending_until[index] != pending => {
                            self.pending_until[index] = pending;
                            updated.push(kind.pending_key());
                        }
                        Ok(_) => {}
                        Err(error) => errors.push(error),
                    }
                }
            }
        }

        (errors, updated)
    }
}

pub struct SettingsStore {
    config: Option<Config>,
}

impl SettingsStore {
    pub fn open(app_id: &str) -> Self {
        Self {
            config: open_config(app_id, "settings"),
        }
    }

    pub fn load(&self) -> Settings {
        let Some(config) = self.config.as_ref() else {
            return Settings::default();
        };

        match Settings::get_entry(config) {
            Ok(settings) => settings,
            Err((errors, settings)) => {
                tracing::debug!(
                    unset = errors.len(),
                    "using defaults for settings that have never been written"
                );
                settings
            }
        }
    }

    pub fn save(&self, settings: &Settings) {
        let Some(config) = self.config.as_ref() else {
            return;
        };

        if let Err(error) = settings.write_entry(config) {
            tracing::warn!(%error, "could not save the settings");
        }
    }
}

pub struct ConfigScheduleStore {
    config: Option<Config>,
    written: Option<Record>,
}

impl ConfigScheduleStore {
    pub fn open(app_id: &str) -> Self {
        Self {
            config: open_config(app_id, "schedule"),
            written: None,
        }
    }
}

impl crate::pause::ScheduleStore for ConfigScheduleStore {
    fn load(&self) -> Record {
        let Some(config) = self.config.as_ref() else {
            return Record::default();
        };

        match Record::get_entry(config) {
            Ok(record) => record,
            Err((errors, record)) => {
                tracing::debug!(unset = errors.len(), "no schedule has been stored yet");
                record
            }
        }
    }

    fn store(&mut self, record: &Record) {
        let Some(config) = self.config.as_ref() else {
            return;
        };

        for key in keys_to_write(self.written.as_ref(), record) {
            let outcome = if key == PAUSE_KEY {
                config.set(key, record.pause)
            } else if let Some(kind) = ReminderKind::ALL
                .into_iter()
                .find(|kind| kind.due_key() == key)
            {
                config.set(key, record.due[kind.index()])
            } else if let Some(kind) = ReminderKind::ALL
                .into_iter()
                .find(|kind| kind.pending_key() == key)
            {
                config.set(key, record.pending_until[kind.index()])
            } else {
                Ok(())
            };

            if let Err(error) = outcome {
                tracing::warn!(%error, key, "could not store a part of the schedule");
            }
        }

        self.written = Some(*record);
    }

    fn adopted(&mut self, record: &Record) {
        self.written = Some(*record);
    }
}

fn keys_to_write(previous: Option<&Record>, next: &Record) -> Vec<&'static str> {
    let mut keys = Vec::new();

    for kind in ReminderKind::ALL {
        let index = kind.index();

        if previous.is_none_or(|before| before.due[index] != next.due[index]) {
            keys.push(kind.due_key());
        }

        if previous.is_none_or(|before| before.pending_until[index] != next.pending_until[index]) {
            keys.push(kind.pending_key());
        }
    }

    if previous.is_none_or(|before| before.pause != next.pause) {
        keys.push(PAUSE_KEY);
    }

    keys
}

fn open_config(app_id: &str, what: &'static str) -> Option<Config> {
    match Config::new(app_id, CONFIG_VERSION) {
        Ok(config) => Some(config),
        Err(error) => {
            tracing::warn!(%error, what, "settings will not persist, cosmic-config is unavailable");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause::ScheduleStore as _;
    use crate::pause::model::Moment;

    fn temporary(name: &str) -> (std::path::PathBuf, Config) {
        let path = std::env::temp_dir().join(format!(
            "cosmic-ext-applet-pause-{}-{name}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let config = Config::with_custom_path("pause-test", CONFIG_VERSION, path.clone())
            .expect("a config under a temporary path");

        (path, config)
    }

    #[test]
    fn settings_that_were_never_written_come_back_as_the_defaults() {
        let (path, config) = temporary("settings-empty");

        let (errors, settings) = Settings::get_entry(&config).expect_err("nothing is stored yet");

        assert_eq!(
            errors.len(),
            ReminderKind::COUNT + 2,
            "one per key that has never been written"
        );
        assert_eq!(settings, Settings::default());

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn every_setting_survives_a_round_trip_through_the_config() {
        let (path, config) = temporary("settings-round-trip");

        let mut stored = Settings::default();
        stored.set(
            ReminderKind::Eyes,
            ReminderSetting {
                enabled: false,
                interval_secs: 30 * 60,
            },
        );
        stored.snooze_secs = 5 * 60;

        stored
            .write_entry(&config)
            .expect("the settings are stored");
        let loaded = Settings::get_entry(&config).expect("the settings are read back");

        assert_eq!(loaded, stored);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_hand_edited_interval_is_clamped_on_the_way_back_in() {
        let (path, config) = temporary("settings-clamp");

        config
            .set(
                ReminderKind::Eyes.key(),
                ReminderSetting {
                    enabled: true,
                    interval_secs: 1,
                },
            )
            .expect("the raw value is stored");

        let (_errors, settings) = Settings::get_entry(&config).expect_err("other keys are unset");

        assert_eq!(
            settings.interval(ReminderKind::Eyes),
            crate::pause::model::MIN_INTERVAL
        );

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn a_changed_key_is_reported_only_when_the_value_really_moved() {
        let (path, config) = temporary("settings-updates");

        let mut settings = Settings::default();
        settings
            .write_entry(&config)
            .expect("the settings are stored");

        let (errors, updated) = settings.update_keys(&config, &[ReminderKind::Eyes.key()]);
        assert!(errors.is_empty());
        assert!(updated.is_empty(), "an identical reload is not a change");

        config
            .set(
                ReminderKind::Eyes.key(),
                ReminderSetting {
                    enabled: false,
                    interval_secs: 45 * 60,
                },
            )
            .expect("the new value is stored");

        let (errors, updated) = settings.update_keys(&config, &[ReminderKind::Eyes.key()]);
        assert!(errors.is_empty());
        assert_eq!(updated, vec![ReminderKind::Eyes.key()]);
        assert!(!settings.enabled(ReminderKind::Eyes));

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn the_whole_schedule_survives_a_round_trip_through_the_config() {
        let (path, config) = temporary("schedule-round-trip");

        let stored = Record {
            due: [
                Moment::from_epoch_seconds(1),
                Moment::from_epoch_seconds(2),
                Moment::from_epoch_seconds(3),
                Moment::from_epoch_seconds(4),
                Moment::from_epoch_seconds(5),
                Moment::from_epoch_seconds(6),
            ],
            pending_until: [
                Moment::from_epoch_seconds(90),
                Moment::EPOCH,
                Moment::from_epoch_seconds(70),
                Moment::EPOCH,
                Moment::EPOCH,
                Moment::EPOCH,
            ],
            pause: Some(PauseRecord {
                started_at: Moment::from_epoch_seconds(10),
                until: Some(Moment::from_epoch_seconds(20)),
            }),
        };

        stored.write_entry(&config).expect("the schedule is stored");
        let loaded = Record::get_entry(&config).expect("the schedule is read back");

        assert_eq!(loaded, stored);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn an_indefinite_pause_survives_a_round_trip() {
        let (path, config) = temporary("schedule-indefinite");

        let stored = Record {
            pause: Some(PauseRecord {
                started_at: Moment::from_epoch_seconds(10),
                until: None,
            }),
            ..Record::default()
        };

        stored.write_entry(&config).expect("the schedule is stored");
        let loaded = Record::get_entry(&config).expect("the schedule is read back");

        assert_eq!(loaded.pause, stored.pause);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn the_very_first_store_writes_every_key_it_owns() {
        let keys = keys_to_write(None, &Record::default());

        assert_eq!(
            keys.len(),
            ReminderKind::COUNT * 2 + 1,
            "one due and one badge per reminder, plus the pause"
        );
    }

    #[test]
    fn storing_the_same_schedule_again_writes_nothing_at_all() {
        let record = Record::default();

        assert!(keys_to_write(Some(&record), &record).is_empty());
    }

    #[test]
    fn one_reminder_moving_never_rewrites_another_reminders_key() {
        let before = Record::default();
        let mut after = before;
        after.due[ReminderKind::Eyes.index()] = Moment::from_epoch_seconds(42);

        assert_eq!(
            keys_to_write(Some(&before), &after),
            vec![ReminderKind::Eyes.due_key()]
        );
    }

    #[test]
    fn an_unavailable_config_simply_keeps_the_defaults_and_drops_the_writes() {
        let settings = SettingsStore { config: None };
        assert_eq!(settings.load(), Settings::default());
        settings.save(&Settings::default());

        let mut schedule = ConfigScheduleStore {
            config: None,
            written: None,
        };
        assert_eq!(schedule.load(), Record::default());
        schedule.store(&Record::default());
    }
}

#[cfg(test)]
mod cross_instance {
    use super::*;
    use crate::pause::ScheduleStore as _;
    use crate::pause::model::{Moment, PauseRecord, Settings};
    use crate::pause::schedule::Schedule;

    fn store_on(path: &std::path::Path) -> ConfigScheduleStore {
        ConfigScheduleStore {
            config: Some(
                Config::with_custom_path("pause-cross", CONFIG_VERSION, path.to_path_buf())
                    .expect("a config under a temporary path"),
            ),
            written: None,
        }
    }

    #[test]
    fn a_pause_started_on_one_screen_can_be_resumed_from_the_other() {
        let path = std::env::temp_dir().join(format!(
            "cosmic-ext-applet-pause-cross-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);

        let settings = Settings::default();
        let now = Moment::from_epoch_seconds(1_700_000_000);

        let mut first = store_on(&path);
        let mut second = store_on(&path);

        let mut here = Schedule::from_record(first.load());
        here.start(&settings, now);
        first.store(&here.record());

        let mut there = Schedule::from_record(second.load());
        there.start(&settings, now);
        second.store(&there.record());

        here.pause_for(now, None);
        first.store(&here.record());

        let from_disk = second.load();
        assert_eq!(
            from_disk.pause,
            Some(PauseRecord {
                started_at: now,
                until: None
            }),
            "the second screen never saw the pause"
        );
        there.adopt_record(from_disk);
        second.adopted(&from_disk);

        there.resume(&settings, now);
        second.store(&there.record());

        assert!(
            first.load().pause.is_none(),
            "resuming from the second screen never reached the stored schedule"
        );

        let _ = std::fs::remove_dir_all(path);
    }
}
