mod common;

use std::time::Duration;

use common::{VirtualClock, wait_for};
use cosmic_ext_applet_pause::pause::model::{ReminderKind, ReminderSetting, Settings, Snapshot};
use cosmic_ext_applet_pause::pause::notify::{RecordingNotifier, Request};
use cosmic_ext_applet_pause::pause::schedule::Record;
use cosmic_ext_applet_pause::pause::{Builder, Command, Event, MemoryScheduleStore, Phrasebook};
use cosmic_ext_applet_pause::pause::{PauseHandle, ScheduleStore};

#[derive(Clone, Copy, Debug, Default)]
struct Plain;

impl Phrasebook for Plain {
    fn request(&self, kind: ReminderKind) -> Request {
        Request {
            kind,
            summary: format!("{kind:?}"),
            body: String::new(),
            action_label: "Done".to_owned(),
            icon: String::new(),
            timeout_ms: 0,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SharedStore(std::sync::Arc<std::sync::Mutex<Record>>);

impl SharedStore {
    fn record(&self) -> Record {
        *self.0.lock().expect("the store is not poisoned")
    }
}

impl ScheduleStore for SharedStore {
    fn load(&self) -> Record {
        self.record()
    }

    fn store(&mut self, record: &Record) {
        *self.0.lock().expect("the store is not poisoned") = *record;
    }
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

fn short(kind: ReminderKind, minutes: u32) -> Settings {
    let mut settings = only(&[kind]);
    settings.set(
        kind,
        ReminderSetting {
            enabled: true,
            interval_secs: minutes * 60,
        },
    );
    settings
}

#[tokio::test(start_paused = true)]
async fn the_first_snapshot_is_ready_before_anyone_subscribes() {
    let settings = Settings::default();
    let (handle, _events, _join) = Builder::new(MemoryScheduleStore::default(), Plain, settings)
        .clock(VirtualClock::default())
        .spawn(&tokio::runtime::Handle::current());

    let snapshot = handle.snapshot();

    assert_eq!(snapshot.reminders.len(), ReminderKind::COUNT);
    assert!(snapshot.leader);
    assert!(!snapshot.is_paused());
}

#[tokio::test(start_paused = true)]
async fn a_reminder_reaches_the_desktop_when_its_interval_runs_out() {
    let notifier = RecordingNotifier::default();
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .notifier(notifier.clone())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    wait_for(
        &mut snapshots,
        "the eyes reminder to come due",
        |snapshot| {
            snapshot
                .reminders
                .iter()
                .any(|reminder| reminder.kind == ReminderKind::Eyes && reminder.pending)
        },
    )
    .await;

    assert_eq!(notifier.kinds(), vec![ReminderKind::Eyes]);
}

#[tokio::test(start_paused = true)]
async fn a_reminder_carries_the_duration_the_settings_asked_for() {
    let notifier = RecordingNotifier::default();
    let mut settings = short(ReminderKind::Eyes, 1);
    settings.notification_secs = 10;

    let (handle, _events, _join) = Builder::new(MemoryScheduleStore::default(), Plain, settings)
        .clock(VirtualClock::default())
        .notifier(notifier.clone())
        .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    wait_for(&mut snapshots, "the first reminder", |s| {
        s.reminders.iter().any(|reminder| reminder.pending)
    })
    .await;

    assert_eq!(notifier.delivered()[0].timeout_ms, 10_000);
}

#[tokio::test(start_paused = true)]
async fn a_follower_never_sends_a_notification() {
    let notifier = RecordingNotifier::default();
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .notifier(notifier.clone())
    .leader(false)
    .spawn(&tokio::runtime::Handle::current());

    let _ = handle.snapshot();
    tokio::time::sleep(Duration::from_mins(5)).await;

    assert!(
        notifier.delivered().is_empty(),
        "only the instance that holds the name may speak"
    );
}

#[tokio::test(start_paused = true)]
async fn a_follower_promoted_to_leader_takes_over_the_reminders() {
    let notifier = RecordingNotifier::default();
    let (handle, events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .notifier(notifier.clone())
    .leader(false)
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    tokio::time::sleep(Duration::from_mins(5)).await;
    assert!(notifier.delivered().is_empty());

    events.post(Event::Leader(true));

    wait_for(&mut snapshots, "the promoted instance to speak up", |s| {
        s.leader && s.reminders.iter().any(|reminder| reminder.pending)
    })
    .await;

    assert_eq!(notifier.kinds(), vec![ReminderKind::Eyes]);
}

#[tokio::test(start_paused = true)]
async fn pausing_everything_keeps_the_desktop_quiet() {
    let notifier = RecordingNotifier::default();
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .notifier(notifier.clone())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    handle.send(Command::PauseFor(None));
    wait_for(
        &mut snapshots,
        "the pause to take hold",
        Snapshot::is_paused,
    )
    .await;

    tokio::time::sleep(Duration::from_mins(10)).await;

    assert!(notifier.delivered().is_empty());
}

#[tokio::test(start_paused = true)]
async fn resuming_lets_the_reminders_through_again() {
    let notifier = RecordingNotifier::default();
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .notifier(notifier.clone())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    handle.send(Command::PauseFor(None));
    wait_for(
        &mut snapshots,
        "the pause to take hold",
        Snapshot::is_paused,
    )
    .await;
    tokio::time::sleep(Duration::from_mins(10)).await;

    handle.send(Command::Resume);
    wait_for(&mut snapshots, "a reminder after resuming", |s| {
        !s.is_paused() && s.reminders.iter().any(|reminder| reminder.pending)
    })
    .await;

    assert_eq!(notifier.kinds(), vec![ReminderKind::Eyes]);
}

#[tokio::test(start_paused = true)]
async fn a_desktop_that_asked_for_quiet_is_left_alone_until_it_changes_its_mind() {
    let notifier = RecordingNotifier::default();
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .notifier(notifier.clone())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    handle.send(Command::DoNotDisturb(true));
    wait_for(
        &mut snapshots,
        "do not disturb to take hold",
        Snapshot::is_quiet,
    )
    .await;
    tokio::time::sleep(Duration::from_mins(10)).await;

    assert!(
        notifier.delivered().is_empty(),
        "ten minutes of do not disturb owed the user nothing"
    );

    handle.send(Command::DoNotDisturb(false));
    wait_for(&mut snapshots, "a reminder once the quiet lifts", |s| {
        !s.is_quiet() && s.reminders.iter().any(|reminder| reminder.pending)
    })
    .await;

    assert_eq!(notifier.kinds(), vec![ReminderKind::Eyes]);
}

#[tokio::test(start_paused = true)]
async fn clicking_the_notification_clears_the_badge_it_left_behind() {
    let (handle, events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 1),
    )
    .clock(VirtualClock::default())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    wait_for(&mut snapshots, "the badge to appear", |s| {
        s.reminders.iter().any(|reminder| reminder.pending)
    })
    .await;

    events.post(Event::Activated(ReminderKind::Eyes));

    wait_for(&mut snapshots, "the badge to clear", |s| {
        s.reminders.iter().all(|reminder| !reminder.pending)
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn what_the_scheduler_decides_is_written_down_for_the_next_run() {
    let store = SharedStore::default();
    let (handle, _events, _join) = Builder::new(store.clone(), Plain, short(ReminderKind::Eyes, 1))
        .clock(VirtualClock::default())
        .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    wait_for(&mut snapshots, "the first reminder", |s| {
        s.reminders.iter().any(|reminder| reminder.pending)
    })
    .await;

    let stored = store.record();

    assert_ne!(
        stored.pending_until[ReminderKind::Eyes.index()],
        cosmic_ext_applet_pause::pause::model::Moment::EPOCH
    );
    assert_eq!(
        stored.due[ReminderKind::Eyes.index()],
        snapshots.borrow().reminders[ReminderKind::Eyes.index()].due_at
    );
}

#[tokio::test(start_paused = true)]
async fn a_settings_change_reaches_the_countdown_without_a_restart() {
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 60),
    )
    .clock(VirtualClock::default())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    let before = snapshots.borrow().reminders[ReminderKind::Eyes.index()].due_at;

    handle.send(Command::Settings(Box::new(short(ReminderKind::Eyes, 5))));

    let snapshot = wait_for(&mut snapshots, "the shorter interval to apply", |s| {
        s.reminders[ReminderKind::Eyes.index()].due_at != before
    })
    .await;

    assert_eq!(
        snapshot.reminders[ReminderKind::Eyes.index()]
            .setting
            .interval_secs,
        5 * 60
    );
}

#[tokio::test(start_paused = true)]
async fn skipping_the_next_break_moves_it_out_of_the_way() {
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        short(ReminderKind::Eyes, 10),
    )
    .clock(VirtualClock::default())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    let before = snapshots.borrow().reminders[ReminderKind::Eyes.index()].due_at;

    handle.send(Command::SkipNext);

    let snapshot = wait_for(&mut snapshots, "the break to be skipped", |s| {
        s.reminders[ReminderKind::Eyes.index()].due_at != before
    })
    .await;

    assert!(
        snapshot.reminders[ReminderKind::Eyes.index()]
            .due_at
            .is_after(before)
    );
}

#[tokio::test(start_paused = true)]
async fn a_schedule_written_by_another_instance_is_adopted_here_too() {
    let (handle, _events, _join) = Builder::new(
        MemoryScheduleStore::default(),
        Plain,
        only(&[ReminderKind::Eyes]),
    )
    .clock(VirtualClock::default())
    .spawn(&tokio::runtime::Handle::current());

    let mut snapshots = handle.subscribe();
    let mut record =
        snapshots
            .borrow()
            .reminders
            .iter()
            .fold(Record::default(), |mut record, reminder| {
                record.due[reminder.kind.index()] = reminder.due_at;
                record
            });
    record.pending_until[ReminderKind::Eyes.index()] =
        cosmic_ext_applet_pause::pause::model::Moment::from_epoch_seconds(i64::MAX / 2);

    handle.send(Command::Schedule(Box::new(record)));

    wait_for(&mut snapshots, "the other instance's badge", |s| {
        s.reminders[ReminderKind::Eyes.index()].pending
    })
    .await;
}

#[allow(dead_code)]
fn handle_is_shareable(handle: &PauseHandle) -> PauseHandle {
    handle.clone()
}
