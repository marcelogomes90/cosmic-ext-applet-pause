pub mod call;
pub mod clock;
pub mod leader;
pub mod model;
pub mod notify;
pub mod proxies;
pub mod schedule;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::pause::clock::{Clock, GapDetector, SystemClock};
use crate::pause::model::{Moment, NotifierState, ReminderKind, Settings, Snapshot};
use crate::pause::notify::{DroppingNotifier, Notifier, Request};
use crate::pause::schedule::{Effect, Record, Schedule};

const FLOOR: Duration = Duration::from_millis(250);

pub trait ScheduleStore: Send + 'static {
    fn load(&self) -> Record;
    fn store(&mut self, record: &Record);

    fn adopted(&mut self, record: &Record) {
        let _ = record;
    }
}

#[derive(Debug, Default)]
pub struct MemoryScheduleStore(Record);

impl ScheduleStore for MemoryScheduleStore {
    fn load(&self) -> Record {
        self.0
    }

    fn store(&mut self, record: &Record) {
        self.0 = *record;
    }
}

pub trait Phrasebook: Send + 'static {
    fn request(&self, kind: ReminderKind) -> Request;
}

#[derive(Clone, Debug)]
pub enum Command {
    Settings(Box<Settings>),
    Schedule(Box<Record>),
    Done(ReminderKind),
    Snooze(ReminderKind),
    SkipNext,
    ResetTimers,
    PauseFor(Option<Duration>),
    Extend(Duration),
    Resume,
    DoNotDisturb(bool),
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Activated(ReminderKind),
    Notifier(NotifierState),
    Leader(bool),
}

#[derive(Clone, Debug)]
pub struct PauseHandle {
    snapshots: watch::Receiver<Arc<Snapshot>>,
    commands: mpsc::Sender<Command>,
}

impl PauseHandle {
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshots.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.snapshots.clone()
    }

    pub fn send(&self, command: Command) {
        if let Err(error) = self.commands.try_send(command) {
            tracing::warn!(error = %error, "dropping a command, the scheduler is not keeping up");
        }
    }
}

pub fn channel() -> (Events, Inbox) {
    let (sender, receiver) = mpsc::channel(64);

    (Events(sender.clone()), Inbox { sender, receiver })
}

#[derive(Debug)]
pub struct Inbox {
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

pub struct Builder<S, C, N, P> {
    store: S,
    clock: C,
    notifier: N,
    phrasebook: P,
    settings: Settings,
    leader: bool,
    inbox: Option<Inbox>,
}

impl<S, P> Builder<S, SystemClock, DroppingNotifier, P>
where
    S: ScheduleStore,
    P: Phrasebook,
{
    pub fn new(store: S, phrasebook: P, settings: Settings) -> Self {
        Self {
            store,
            clock: SystemClock::default(),
            notifier: DroppingNotifier,
            phrasebook,
            settings,
            leader: true,
            inbox: None,
        }
    }
}

impl<S, C, N, P> Builder<S, C, N, P>
where
    S: ScheduleStore,
    C: Clock,
    N: Notifier,
    P: Phrasebook,
{
    pub fn clock<D: Clock>(self, clock: D) -> Builder<S, D, N, P> {
        Builder {
            store: self.store,
            clock,
            notifier: self.notifier,
            phrasebook: self.phrasebook,
            settings: self.settings,
            leader: self.leader,
            inbox: self.inbox,
        }
    }

    pub fn notifier<M: Notifier>(self, notifier: M) -> Builder<S, C, M, P> {
        Builder {
            store: self.store,
            clock: self.clock,
            notifier,
            phrasebook: self.phrasebook,
            settings: self.settings,
            leader: self.leader,
            inbox: self.inbox,
        }
    }

    pub fn leader(mut self, leader: bool) -> Self {
        self.leader = leader;
        self
    }

    pub fn inbox(mut self, inbox: Inbox) -> Self {
        self.inbox = Some(inbox);
        self
    }

    pub fn spawn(self, runtime: &tokio::runtime::Handle) -> (PauseHandle, Events, JoinHandle<()>) {
        let (command_tx, command_rx) = mpsc::channel(64);
        let inbox = self.inbox.unwrap_or_else(|| channel().1);
        let events = Events(inbox.sender.clone());

        let mut scheduler = Scheduler {
            store: self.store,
            clock: self.clock,
            notifier: self.notifier,
            phrasebook: self.phrasebook,
            settings: self.settings,
            schedule: Schedule::default(),
            gap: GapDetector::default(),
            notifier_state: NotifierState::default(),
            leader: self.leader,
            revision: 0,
            commands: command_rx,
            events: inbox.receiver,
        };

        let first = scheduler.begin();
        let (snapshot_tx, snapshot_rx) = watch::channel(Arc::new(first));
        let join = runtime.spawn(async move { scheduler.run(&snapshot_tx).await });

        (
            PauseHandle {
                snapshots: snapshot_rx,
                commands: command_tx,
            },
            events,
            join,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Events(mpsc::Sender<Event>);

impl Events {
    pub fn post(&self, event: Event) {
        if let Err(error) = self.0.try_send(event) {
            tracing::warn!(error = %error, "dropping an event, the scheduler is not keeping up");
        }
    }

    pub fn sender(&self) -> mpsc::Sender<Event> {
        self.0.clone()
    }
}

struct Scheduler<S, C, N, P> {
    store: S,
    clock: C,
    notifier: N,
    phrasebook: P,
    settings: Settings,
    schedule: Schedule,
    gap: GapDetector,
    notifier_state: NotifierState,
    leader: bool,
    revision: u64,
    commands: mpsc::Receiver<Command>,
    events: mpsc::Receiver<Event>,
}

impl<S, C, N, P> Scheduler<S, C, N, P>
where
    S: ScheduleStore,
    C: Clock,
    N: Notifier,
    P: Phrasebook,
{
    fn begin(&mut self) -> Snapshot {
        self.schedule = Schedule::from_record(self.store.load());
        let now = self.clock.wall();

        if self.schedule.start(&self.settings, now) && self.leader {
            self.store.store(&self.schedule.record());
        }

        self.gap.observe(now, self.clock.uptime());
        self.snapshot()
    }

    async fn run(mut self, snapshots: &watch::Sender<Arc<Snapshot>>) {
        loop {
            let now = self.clock.wall();
            let delay = if self.leader {
                self.schedule
                    .next_wake(&self.settings, now)
                    .saturating_duration_since(now)
                    .max(FLOOR)
            } else {
                schedule::HEARTBEAT
            };

            tokio::select! {
                biased;

                event = self.events.recv() => match event {
                    Some(event) => self.on_event(event),
                    None => break,
                },

                command = self.commands.recv() => match command {
                    Some(command) => self.on_command(command),
                    None => break,
                },

                () = tokio::time::sleep(delay) => self.on_tick(),
            }

            self.publish(snapshots);
        }

        tracing::info!("the scheduler is shutting down");
    }

    fn on_tick(&mut self) {
        let now = self.clock.wall();
        let gap = self.gap.observe(now, self.clock.uptime());

        if !self.leader {
            return;
        }

        if gap >= schedule::SUSPEND_GAP {
            tracing::info!(
                seconds = gap.as_secs(),
                "the machine was away, rescheduling instead of catching up"
            );
        }

        let effects = self.schedule.advance(&self.settings, now, gap);
        self.carry_out(&effects);
    }

    fn on_command(&mut self, command: Command) {
        let now = self.clock.wall();

        let changed = match command {
            Command::Settings(settings) => {
                let next = settings.sanitised();
                let previous = std::mem::replace(&mut self.settings, next);
                self.schedule.apply_settings(&previous, &next, now)
            }
            Command::Schedule(record) => return self.adopt(*record),
            Command::Done(kind) => self.schedule.done(kind),
            Command::Snooze(kind) => self.schedule.snooze(kind, &self.settings, now),
            Command::SkipNext => self.schedule.skip_next(&self.settings, now),
            Command::ResetTimers => self.schedule.restart_all(&self.settings, now),
            Command::PauseFor(span) => self.schedule.pause_for(now, span),
            Command::Extend(span) => self.schedule.extend(now, span),
            Command::Resume => self.schedule.resume(&self.settings, now),
            Command::DoNotDisturb(quiet) => return self.quiet(quiet, now),
        };

        if changed {
            self.store.store(&self.schedule.record());
        }
    }

    fn quiet(&mut self, quiet: bool, now: Moment) {
        if !self.schedule.set_quiet(&self.settings, now, quiet) {
            return;
        }

        tracing::info!(quiet, "the desktop changed do not disturb");

        if self.leader {
            self.store.store(&self.schedule.record());
        }
    }

    fn adopt(&mut self, record: Record) {
        if !self.schedule.adopt_record(record) {
            return;
        }

        tracing::debug!("another instance moved the schedule");
        self.store.adopted(&record);
        self.schedule.start(&self.settings, self.clock.wall());
    }

    fn on_event(&mut self, event: Event) {
        match event {
            Event::Activated(kind) => {
                if self.schedule.done(kind) {
                    self.store.store(&self.schedule.record());
                }
            }
            Event::Notifier(state) => self.notifier_state = state,
            Event::Leader(leader) => {
                if self.leader != leader {
                    tracing::info!(leader, "the scheduler changed role");
                    self.leader = leader;
                }
            }
        }
    }

    fn carry_out(&mut self, effects: &[Effect]) {
        let mut persist = false;

        for effect in effects {
            match effect {
                Effect::Fire(kind) => {
                    let mut request = self.phrasebook.request(*kind);
                    request.timeout_ms =
                        i32::try_from(self.settings.notification().as_millis()).unwrap_or(5_000);
                    self.notifier.deliver(request);
                }
                Effect::Persist => persist = true,
            }
        }

        if persist {
            self.store.store(&self.schedule.record());
        }
    }

    fn snapshot(&self) -> Snapshot {
        let mut snapshot = self.schedule.snapshot(
            &self.settings,
            self.clock.wall(),
            self.leader,
            self.revision,
        );
        snapshot.notifier = self.notifier_state;
        snapshot
    }

    fn publish(&mut self, snapshots: &watch::Sender<Arc<Snapshot>>) {
        let mut next = self.snapshot();

        if snapshots.borrow().as_ref().same_state(&next) {
            return;
        }

        self.revision += 1;
        next.revision = self.revision;
        let _ = snapshots.send(Arc::new(next));
    }
}
