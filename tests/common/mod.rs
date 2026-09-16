#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use cosmic_ext_applet_pause::pause::clock::Clock;
use cosmic_ext_applet_pause::pause::model::{Moment, Snapshot};
use tokio::sync::watch;

pub const SETTLE: Duration = Duration::from_hours(2);
pub const BASE: i64 = 1_700_000_000;

pub struct VirtualClock {
    wall: Moment,
    started: tokio::time::Instant,
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self {
            wall: Moment::from_epoch_seconds(BASE),
            started: tokio::time::Instant::now(),
        }
    }
}

impl Clock for VirtualClock {
    fn wall(&self) -> Moment {
        self.wall.saturating_add(self.started.elapsed())
    }

    fn uptime(&self) -> Duration {
        self.started.elapsed()
    }
}

pub type Snapshots = watch::Receiver<Arc<Snapshot>>;

pub async fn wait_for(
    snapshots: &mut Snapshots,
    what: &str,
    mut predicate: impl FnMut(&Snapshot) -> bool,
) -> Arc<Snapshot> {
    let deadline = tokio::time::Instant::now() + SETTLE;

    loop {
        {
            let current = snapshots.borrow_and_update().clone();
            if predicate(&current) {
                return current;
            }
        }

        if tokio::time::timeout_at(deadline, snapshots.changed())
            .await
            .is_err()
        {
            panic!(
                "timed out waiting for {what}; the last snapshot was {}",
                describe(&snapshots.borrow())
            );
        }
    }
}

pub fn describe(snapshot: &Snapshot) -> String {
    let reminders: Vec<String> = snapshot
        .reminders
        .iter()
        .map(|reminder| {
            format!(
                "{:?}(due {} enabled={} pending={})",
                reminder.kind,
                reminder.due_at.epoch_seconds() - BASE,
                reminder.setting.enabled,
                reminder.pending
            )
        })
        .collect();

    format!(
        "revision {} leader={} paused={} [{}]",
        snapshot.revision,
        snapshot.leader,
        snapshot.is_paused(),
        reminders.join(", ")
    )
}
