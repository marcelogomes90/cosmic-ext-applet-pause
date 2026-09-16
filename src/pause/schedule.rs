use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::pause::model::{Moment, PauseRecord, Reminder, ReminderKind, Settings, Snapshot};

pub const HEARTBEAT: Duration = Duration::from_secs(20);
pub const SUSPEND_GAP: Duration = Duration::from_mins(2);
pub const RESUME_SETTLE: Duration = Duration::from_mins(1);
pub const STARTUP_GRACE: Duration = Duration::from_mins(2);
pub const STAGGER: Duration = Duration::from_mins(1);
pub const MIN_LEAD: Duration = Duration::from_secs(30);
pub const PENDING_DISPLAY: Duration = Duration::from_mins(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Fire(ReminderKind),
    Persist,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Record {
    pub due: [Moment; ReminderKind::COUNT],
    pub pending_until: [Moment; ReminderKind::COUNT],
    pub pause: Option<PauseRecord>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Schedule {
    record: Record,
    settle_until: Option<Moment>,
}

impl Schedule {
    pub fn from_record(record: Record) -> Self {
        Self {
            record,
            settle_until: None,
        }
    }

    pub fn record(&self) -> Record {
        self.record
    }

    pub fn pause(&self) -> Option<PauseRecord> {
        self.record.pause
    }

    pub fn due_at(&self, kind: ReminderKind) -> Moment {
        self.record.due[kind.index()]
    }

    pub fn is_pending(&self, kind: ReminderKind, now: Moment) -> bool {
        self.record.pending_until[kind.index()].is_after(now)
    }

    pub fn snapshot(
        &self,
        settings: &Settings,
        now: Moment,
        leader: bool,
        revision: u64,
    ) -> Snapshot {
        Snapshot {
            reminders: ReminderKind::ALL
                .iter()
                .map(|kind| Reminder {
                    kind: *kind,
                    setting: settings.get(*kind),
                    due_at: self.due_at(*kind),
                    pending: self.is_pending(*kind, now),
                })
                .collect(),
            pause: self.record.pause,
            leader,
            notifier: crate::pause::model::NotifierState::default(),
            revision,
        }
    }

    pub fn adopt_record(&mut self, record: Record) -> bool {
        if self.record == record {
            return false;
        }

        self.record = record;
        true
    }

    pub fn start(&mut self, settings: &Settings, now: Moment) -> bool {
        let mut changed = false;

        for kind in ReminderKind::ALL {
            let interval = settings.interval(kind);
            let fresh = now.saturating_add(interval);
            let index = kind.index();

            if !settings.enabled(kind) {
                changed |= self.reset(index, fresh);
                continue;
            }

            let due = self.record.due[index];

            if due.is_after(fresh) {
                changed |= self.reset(index, fresh);
                continue;
            }

            if !due.is_after(now) && now.saturating_duration_since(due) > STARTUP_GRACE {
                changed |= self.reset(index, fresh);
            }
        }

        changed
    }

    pub fn apply_settings(&mut self, previous: &Settings, next: &Settings, now: Moment) -> bool {
        let mut changed = false;

        for kind in ReminderKind::ALL {
            let was = previous.get(kind);
            let is = next.get(kind);
            let index = kind.index();
            let fresh = now.saturating_add(is.interval());

            if !is.enabled {
                changed |= self.reset(index, fresh);
                continue;
            }

            if !was.enabled || was.interval_secs != is.interval_secs {
                changed |= self.reset(index, fresh);
            }
        }

        changed
    }

    pub fn pause_for(&mut self, now: Moment, span: Option<Duration>) -> bool {
        let record = PauseRecord {
            started_at: now,
            until: span.map(|span| now.saturating_add(span)),
        };

        if self.record.pause == Some(record) {
            return false;
        }

        self.record.pause = Some(record);
        true
    }

    pub fn extend(&mut self, now: Moment, span: Duration) -> bool {
        let Some(pause) = self.record.pause else {
            return false;
        };

        let from = pause
            .until
            .filter(|until| until.is_after(now))
            .unwrap_or(now);

        self.record.pause = Some(PauseRecord {
            started_at: pause.started_at,
            until: Some(from.saturating_add(span)),
        });

        true
    }

    pub fn resume(&mut self, settings: &Settings, now: Moment) -> bool {
        let Some(pause) = self.record.pause.take() else {
            return false;
        };

        let slept = now.saturating_duration_since(pause.started_at);

        for kind in ReminderKind::ALL {
            let index = kind.index();
            let interval = settings.interval(kind);
            let low = now.saturating_add(MIN_LEAD);
            let high = now.saturating_add(interval);

            self.record.due[index] = self.record.due[index]
                .saturating_add(slept)
                .clamp_between(low, high);
        }

        true
    }

    pub fn done(&mut self, kind: ReminderKind) -> bool {
        let index = kind.index();

        if self.record.pending_until[index] == Moment::EPOCH {
            return false;
        }

        self.record.pending_until[index] = Moment::EPOCH;
        true
    }

    pub fn snooze(&mut self, kind: ReminderKind, settings: &Settings, now: Moment) -> bool {
        let index = kind.index();

        self.record.pending_until[index] = Moment::EPOCH;
        self.record.due[index] = now.saturating_add(settings.snooze());
        true
    }

    pub fn skip_next(&mut self, settings: &Settings, now: Moment) -> bool {
        let Some(kind) = self.next_due(settings) else {
            return false;
        };

        let index = kind.index();
        let interval = settings.interval(kind);

        self.record.pending_until[index] = Moment::EPOCH;
        self.record.due[index] = self.record.due[index]
            .saturating_add(interval)
            .max(now.saturating_add(interval));

        true
    }

    pub fn restart_all(&mut self, settings: &Settings, now: Moment) -> bool {
        let mut changed = false;

        for kind in ReminderKind::ALL {
            changed |= self.reset(kind.index(), now.saturating_add(settings.interval(kind)));
        }

        changed
    }

    pub fn next_due(&self, settings: &Settings) -> Option<ReminderKind> {
        ReminderKind::ALL
            .into_iter()
            .filter(|kind| settings.enabled(*kind))
            .min_by_key(|kind| self.due_at(*kind))
    }

    pub fn next_wake(&self, settings: &Settings, now: Moment) -> Moment {
        let heartbeat = now.saturating_add(HEARTBEAT);

        if self.record.pause.is_some() {
            let expiry = self
                .record
                .pause
                .and_then(|pause| pause.until)
                .unwrap_or(heartbeat);

            return expiry.min(heartbeat);
        }

        let settle = self.settle_until.unwrap_or(heartbeat);
        let due = self
            .next_due(settings)
            .map_or(heartbeat, |kind| self.due_at(kind));

        due.min(settle).min(heartbeat)
    }

    pub fn advance(&mut self, settings: &Settings, now: Moment, gap: Duration) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut changed = self.retire_badges(now);

        if gap >= SUSPEND_GAP {
            changed |= self.rebase_stale(settings, now);
            self.settle_until = Some(now.saturating_add(RESUME_SETTLE));
        }

        if self
            .record
            .pause
            .is_some_and(|pause| pause.has_expired(now))
        {
            changed |= self.resume(settings, now);
        }

        if self.record.pause.is_some() {
            if changed {
                effects.push(Effect::Persist);
            }
            return effects;
        }

        if self.settle_until.is_some_and(|until| until.is_after(now)) {
            if changed {
                effects.push(Effect::Persist);
            }
            return effects;
        }
        self.settle_until = None;

        let mut overdue: Vec<ReminderKind> = ReminderKind::ALL
            .into_iter()
            .filter(|kind| settings.enabled(*kind))
            .filter(|kind| !self.due_at(*kind).is_after(now))
            .collect();

        if overdue.is_empty() {
            if changed {
                effects.push(Effect::Persist);
            }
            return effects;
        }

        overdue.sort_by_key(|kind| {
            (
                std::cmp::Reverse(now.saturating_duration_since(self.due_at(*kind))),
                *kind,
            )
        });

        let first = overdue[0];
        let index = first.index();
        self.record.pending_until[index] = now.saturating_add(PENDING_DISPLAY);
        self.record.due[index] = now.saturating_add(settings.interval(first));
        effects.push(Effect::Fire(first));

        for kind in &overdue[1..] {
            self.record.due[kind.index()] = now.saturating_add(STAGGER);
        }

        effects.push(Effect::Persist);
        effects
    }

    fn rebase_stale(&mut self, settings: &Settings, now: Moment) -> bool {
        let mut changed = false;

        for kind in ReminderKind::ALL {
            let index = kind.index();

            if self.record.due[index].is_after(now) {
                continue;
            }

            changed |= self.reset(index, now.saturating_add(settings.interval(kind)));
        }

        changed
    }

    fn reset(&mut self, index: usize, due: Moment) -> bool {
        let changed =
            self.record.due[index] != due || self.record.pending_until[index] != Moment::EPOCH;
        self.record.due[index] = due;
        self.record.pending_until[index] = Moment::EPOCH;
        changed
    }

    fn retire_badges(&mut self, now: Moment) -> bool {
        let mut changed = false;

        for kind in ReminderKind::ALL {
            let index = kind.index();

            if self.record.pending_until[index] != Moment::EPOCH
                && !self.record.pending_until[index].is_after(now)
            {
                self.record.pending_until[index] = Moment::EPOCH;
                changed = true;
            }
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause::model::ReminderSetting;

    const NOW: i64 = 1_700_000_000;

    fn at(offset: i64) -> Moment {
        Moment::from_epoch_seconds(NOW + offset)
    }

    fn minutes(count: u64) -> Duration {
        Duration::from_mins(count)
    }

    fn only(kinds: &[ReminderKind]) -> Settings {
        let mut settings = Settings::default();

        for kind in ReminderKind::ALL {
            let mut setting = settings.get(kind);
            setting.enabled = kinds.contains(&kind);
            settings.set(kind, setting);
        }

        settings
    }

    fn started(settings: &Settings) -> Schedule {
        let mut schedule = Schedule::default();
        schedule.start(settings, at(0));
        schedule
    }

    fn fired(effects: &[Effect]) -> Vec<ReminderKind> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Fire(kind) => Some(*kind),
                Effect::Persist => None,
            })
            .collect()
    }

    #[test]
    fn a_first_run_gives_every_reminder_a_full_interval_and_fires_nothing() {
        let settings = Settings::default();
        let mut schedule = started(&settings);

        for kind in ReminderKind::ALL {
            assert_eq!(
                schedule.due_at(kind),
                at(0).saturating_add(settings.interval(kind)),
                "{kind:?} did not start on a full interval"
            );
        }

        assert!(fired(&schedule.advance(&settings, at(0), Duration::ZERO)).is_empty());
    }

    #[test]
    fn a_reminder_fires_once_its_moment_arrives() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        let effects = schedule.advance(&settings, at(20 * 60), Duration::ZERO);

        assert_eq!(fired(&effects), vec![ReminderKind::Eyes]);
        assert!(schedule.is_pending(ReminderKind::Eyes, at(20 * 60)));
        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(40 * 60));
    }

    #[test]
    fn nothing_fires_for_the_reminders_that_came_due_while_the_machine_slept() {
        let settings = Settings::default();
        let mut schedule = started(&settings);

        let resumed = at(2 * 60 * 60);
        let effects = schedule.advance(&settings, resumed, Duration::from_hours(2));

        assert!(
            fired(&effects).is_empty(),
            "a two hour suspend must not wake the user up with a backlog"
        );

        for kind in ReminderKind::ALL {
            assert_eq!(
                schedule.due_at(kind),
                resumed.saturating_add(settings.interval(kind)),
                "{kind:?} was not rebased onto a full interval"
            );
            assert!(!schedule.is_pending(kind, resumed));
        }
    }

    #[test]
    fn an_overnight_hibernation_is_not_a_backlog_of_notifications() {
        let settings = Settings::default();
        let mut schedule = started(&settings);

        let morning = at(10 * 60 * 60);
        let effects = schedule.advance(&settings, morning, Duration::from_hours(10));

        assert!(
            fired(&effects).is_empty(),
            "ten hours asleep must not owe the user ten hours of reminders"
        );

        for kind in ReminderKind::ALL {
            assert_eq!(
                schedule.due_at(kind),
                morning.saturating_add(settings.interval(kind))
            );
        }
    }

    #[test]
    fn waking_and_sleeping_over_and_over_never_queues_anything_up() {
        let settings = Settings::default();
        let mut schedule = started(&settings);
        let mut clock = 0;
        let mut total = 0;

        for _ in 0..8 {
            clock += 3 * 60 * 60;
            let now = at(clock);
            total += fired(&schedule.advance(&settings, now, Duration::from_hours(3))).len();
            clock += 90;
            total += fired(&schedule.advance(&settings, at(clock), Duration::ZERO)).len();
        }

        assert_eq!(
            total, 0,
            "a day of sleeping and waking owed the user {total} reminders"
        );
    }

    #[test]
    fn the_settle_window_holds_everything_back_for_a_minute_after_a_gap() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        let resumed = at(2 * 60 * 60);
        schedule.advance(&settings, resumed, Duration::from_hours(2));

        schedule.snooze(ReminderKind::Eyes, &settings, resumed);
        schedule.done(ReminderKind::Eyes);

        let within = resumed.saturating_add(Duration::from_secs(30));
        assert!(fired(&schedule.advance(&settings, within, Duration::ZERO)).is_empty());
    }

    #[test]
    fn a_reminder_that_is_merely_a_little_late_still_fires() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        let effects = schedule.advance(&settings, at(20 * 60 + 90), Duration::from_secs(90));

        assert_eq!(fired(&effects), vec![ReminderKind::Eyes]);
    }

    #[test]
    fn two_reminders_due_at_the_same_minute_do_not_arrive_together() {
        let settings = only(&[ReminderKind::Move, ReminderKind::Wrists]);
        let mut schedule = started(&settings);

        let due = at(60 * 60);
        let effects = schedule.advance(&settings, due, Duration::ZERO);

        assert_eq!(fired(&effects).len(), 1, "only one may arrive per tick");
        let first = fired(&effects)[0];
        let second = if first == ReminderKind::Move {
            ReminderKind::Wrists
        } else {
            ReminderKind::Move
        };

        assert_eq!(
            schedule.due_at(second),
            due.saturating_add(STAGGER),
            "the one held back waits a stagger, not a whole interval"
        );

        let effects = schedule.advance(&settings, due.saturating_add(STAGGER), Duration::ZERO);
        assert_eq!(fired(&effects), vec![second]);
    }

    #[test]
    fn the_most_overdue_reminder_is_the_one_that_gets_through() {
        let settings = only(&[ReminderKind::Eyes, ReminderKind::Water]);
        let mut schedule = started(&settings);

        let effects = schedule.advance(&settings, at(46 * 60), Duration::ZERO);

        assert_eq!(fired(&effects), vec![ReminderKind::Eyes]);
    }

    #[test]
    fn a_reminder_ignored_for_hours_never_builds_up_a_backlog() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        let late = at(5 * 60 * 60);
        let effects = schedule.advance(&settings, late, Duration::ZERO);

        assert_eq!(fired(&effects).len(), 1);
        assert_eq!(
            schedule.due_at(ReminderKind::Eyes),
            late.saturating_add(minutes(20)),
            "the next one is a full interval away, not a queue of missed ones"
        );

        let effects = schedule.advance(&settings, late, Duration::ZERO);
        assert!(fired(&effects).is_empty());
    }

    #[test]
    fn nothing_fires_while_the_reminders_are_paused() {
        let settings = Settings::default();
        let mut schedule = started(&settings);
        schedule.pause_for(at(0), Some(minutes(30)));

        for step in 1..=5 {
            let effects = schedule.advance(&settings, at(step * 5 * 60), Duration::ZERO);
            assert!(fired(&effects).is_empty(), "step {step} fired while paused");
        }
    }

    #[test]
    fn resuming_gives_back_the_time_that_was_left_not_a_fresh_interval() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        schedule.pause_for(at(5 * 60), None);
        schedule.resume(&settings, at(25 * 60));

        assert_eq!(
            schedule.due_at(ReminderKind::Eyes),
            at(40 * 60),
            "the fifteen minutes that were left must still be fifteen minutes"
        );
    }

    #[test]
    fn a_pause_that_expires_on_its_own_resumes_without_being_asked() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);
        schedule.pause_for(at(0), Some(minutes(30)));

        schedule.advance(&settings, at(30 * 60), Duration::ZERO);

        assert!(schedule.pause().is_none());
        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(50 * 60));
    }

    #[test]
    fn a_pause_that_outlives_a_reboot_does_not_push_everything_days_away() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        schedule.pause_for(at(0), None);
        schedule.resume(&settings, at(3 * 24 * 60 * 60));

        let resumed_at = at(3 * 24 * 60 * 60);
        assert_eq!(
            schedule.due_at(ReminderKind::Eyes),
            resumed_at.saturating_add(minutes(20)),
            "a three day pause must not become a three day countdown"
        );
    }

    #[test]
    fn resuming_never_fires_a_reminder_the_instant_the_user_comes_back() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = Schedule::default();
        schedule.start(&settings, at(0));
        schedule.advance(&settings, at(20 * 60), Duration::ZERO);
        schedule.pause_for(at(39 * 60), None);

        schedule.resume(&settings, at(60 * 60));

        assert!(
            schedule
                .due_at(ReminderKind::Eyes)
                .is_after(at(60 * 60).saturating_add(Duration::from_secs(20))),
            "there must be a moment of grace after resuming"
        );
    }

    #[test]
    fn extending_a_pause_pushes_its_end_further_out() {
        let settings = Settings::default();
        let mut schedule = started(&settings);
        schedule.pause_for(at(0), Some(minutes(30)));

        schedule.extend(at(10 * 60), minutes(30));

        assert_eq!(schedule.pause().and_then(|p| p.until), Some(at(60 * 60)));
    }

    #[test]
    fn extending_an_indefinite_pause_gives_it_an_end_measured_from_now() {
        let settings = Settings::default();
        let mut schedule = started(&settings);
        schedule.pause_for(at(0), None);

        schedule.extend(at(10 * 60), minutes(30));

        assert_eq!(schedule.pause().and_then(|p| p.until), Some(at(40 * 60)));
    }

    #[test]
    fn a_panel_restart_keeps_the_timers_where_they_were() {
        let settings = Settings::default();
        let schedule = started(&settings);
        let before = schedule.record();

        let mut restarted = Schedule::from_record(before);
        let changed = restarted.start(&settings, at(5));

        assert!(!changed, "a restart seconds later must not move anything");
        assert_eq!(restarted.record(), before);
    }

    #[test]
    fn a_reminder_that_came_due_seconds_before_a_restart_still_gets_through() {
        let settings = only(&[ReminderKind::Eyes]);
        let schedule = started(&settings);

        let restart = at(20 * 60 + 30);
        let mut restarted = Schedule::from_record(schedule.record());
        restarted.start(&settings, restart);

        assert_eq!(
            fired(&restarted.advance(&settings, restart, Duration::ZERO)),
            vec![ReminderKind::Eyes]
        );
    }

    #[test]
    fn a_night_away_from_the_machine_starts_the_timers_over_in_silence() {
        let settings = Settings::default();
        let schedule = started(&settings);

        let morning = at(10 * 60 * 60);
        let mut restarted = Schedule::from_record(schedule.record());
        restarted.start(&settings, morning);

        assert!(fired(&restarted.advance(&settings, morning, Duration::ZERO)).is_empty());

        for kind in ReminderKind::ALL {
            assert_eq!(
                restarted.due_at(kind),
                morning.saturating_add(settings.interval(kind))
            );
        }
    }

    #[test]
    fn a_clock_that_jumped_backwards_can_never_leave_a_reminder_stranded() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = Schedule::from_record(Record {
            due: [at(365 * 24 * 60 * 60); ReminderKind::COUNT],
            pending_until: [Moment::EPOCH; ReminderKind::COUNT],
            pause: None,
        });

        schedule.start(&settings, at(0));

        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(20 * 60));
    }

    #[test]
    fn changing_an_interval_starts_that_countdown_over() {
        let previous = only(&[ReminderKind::Eyes]);
        let mut next = previous;
        next.set(
            ReminderKind::Eyes,
            ReminderSetting {
                enabled: true,
                interval_secs: 30 * 60,
            },
        );

        let mut schedule = started(&previous);
        schedule.apply_settings(&previous, &next, at(10 * 60));

        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(40 * 60));
    }

    #[test]
    fn leaving_a_reminder_untouched_does_not_disturb_its_countdown() {
        let previous = Settings::default();
        let mut next = previous;
        next.snooze_secs = 5 * 60;

        let mut schedule = started(&previous);
        let before = schedule.record();
        let changed = schedule.apply_settings(&previous, &next, at(10 * 60));

        assert!(!changed);
        assert_eq!(schedule.record(), before);
    }

    #[test]
    fn switching_a_reminder_back_on_gives_it_a_whole_interval() {
        let off = only(&[]);
        let on = only(&[ReminderKind::Eyes]);

        let mut schedule = started(&off);
        schedule.apply_settings(&off, &on, at(10 * 60));

        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(30 * 60));
    }

    #[test]
    fn a_reminder_switched_off_never_fires_however_long_it_waits() {
        let settings = only(&[]);
        let mut schedule = started(&settings);

        assert!(fired(&schedule.advance(&settings, at(10 * 60 * 60), Duration::ZERO)).is_empty());
    }

    #[test]
    fn putting_a_reminder_off_moves_it_by_the_snooze_and_clears_the_badge() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);
        schedule.advance(&settings, at(20 * 60), Duration::ZERO);

        schedule.snooze(ReminderKind::Eyes, &settings, at(20 * 60));

        assert!(!schedule.is_pending(ReminderKind::Eyes, at(20 * 60)));
        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(30 * 60));
    }

    #[test]
    fn marking_a_reminder_done_only_clears_its_badge() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);
        schedule.advance(&settings, at(20 * 60), Duration::ZERO);
        let due = schedule.due_at(ReminderKind::Eyes);

        assert!(schedule.done(ReminderKind::Eyes));
        assert!(!schedule.done(ReminderKind::Eyes));
        assert_eq!(schedule.due_at(ReminderKind::Eyes), due);
    }

    #[test]
    fn skipping_the_next_break_passes_over_exactly_one_of_them() {
        let settings = only(&[ReminderKind::Eyes, ReminderKind::Water]);
        let mut schedule = started(&settings);

        assert!(schedule.skip_next(&settings, at(0)));

        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(40 * 60));
        assert_eq!(schedule.due_at(ReminderKind::Water), at(45 * 60));
        assert_eq!(schedule.next_due(&settings), Some(ReminderKind::Eyes));
    }

    #[test]
    fn skipping_an_overdue_break_still_lands_a_full_interval_ahead() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);

        schedule.skip_next(&settings, at(60 * 60));

        assert_eq!(schedule.due_at(ReminderKind::Eyes), at(80 * 60));
    }

    #[test]
    fn skipping_does_nothing_when_every_reminder_is_switched_off() {
        let settings = only(&[]);
        let mut schedule = started(&settings);

        assert!(!schedule.skip_next(&settings, at(0)));
    }

    #[test]
    fn the_badge_goes_away_on_its_own_so_the_panel_returns_to_rest() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);
        let fired_at = at(20 * 60);
        schedule.advance(&settings, fired_at, Duration::ZERO);

        assert!(schedule.is_pending(ReminderKind::Eyes, fired_at));

        let later = fired_at.saturating_add(PENDING_DISPLAY);
        schedule.advance(&settings, later, Duration::ZERO);

        assert!(
            !schedule.is_pending(ReminderKind::Eyes, later),
            "the panel must stop shouting after a couple of minutes"
        );
    }

    #[test]
    fn the_scheduler_wakes_often_enough_to_put_the_badge_away_on_time() {
        let settings = only(&[ReminderKind::Eyes]);
        let mut schedule = started(&settings);
        let fired_at = at(20 * 60);
        schedule.advance(&settings, fired_at, Duration::ZERO);

        assert!(
            !schedule
                .next_wake(&settings, fired_at)
                .is_after(fired_at.saturating_add(PENDING_DISPLAY)),
            "the panel would keep its badge past the moment it should have gone"
        );
    }

    #[test]
    fn the_scheduler_never_sleeps_past_its_own_heartbeat() {
        let settings = Settings::default();
        let schedule = started(&settings);

        assert_eq!(
            schedule.next_wake(&settings, at(0)),
            at(0).saturating_add(HEARTBEAT)
        );
    }

    #[test]
    fn the_scheduler_wakes_exactly_when_the_next_reminder_is_due() {
        let settings = only(&[ReminderKind::Eyes]);
        let schedule = started(&settings);

        assert_eq!(schedule.next_wake(&settings, at(19 * 60 + 50)), at(20 * 60));
    }

    #[test]
    fn a_pause_with_an_end_wakes_the_scheduler_when_it_runs_out() {
        let settings = Settings::default();
        let mut schedule = started(&settings);
        schedule.pause_for(at(0), Some(Duration::from_secs(10)));

        assert_eq!(schedule.next_wake(&settings, at(0)), at(10));
    }
}
