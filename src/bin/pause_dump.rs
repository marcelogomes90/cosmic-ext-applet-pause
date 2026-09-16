use std::time::Duration;

use cosmic_ext_applet_pause::config::{ConfigScheduleStore, DoNotDisturb, SettingsStore};
use cosmic_ext_applet_pause::pause::model::{Moment, ReminderKind, Settings};
use cosmic_ext_applet_pause::pause::notify::RecordingNotifier;
use cosmic_ext_applet_pause::pause::schedule::{Effect, Schedule};
use cosmic_ext_applet_pause::pause::{Builder, Command, ScheduleStore};
use cosmic_ext_applet_pause::{APP_ID, i18n, init_tracing, phrases};

const USAGE: &str = "\
usage: cosmic-ext-applet-pause-dump [command]

  simulate [--hours N] [--suspend AT:FOR] [--quiet AT:FOR]
                                            replay a timeline with no waiting (default)
  watch [--seconds N] [--notify] [--elect]  follow the real scheduler and print every change
";

fn main() {
    init_tracing();
    i18n::init();

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();

    match words.split_first() {
        None | Some((&"simulate", _)) => {
            simulate(words.split_first().map_or(&[], |(_, rest)| rest));
        }
        Some((&"watch", rest)) => watch(rest),
        Some((&("--help" | "-h"), _)) => print!("{USAGE}"),
        Some((other, _)) => {
            eprintln!("unknown command: {other}\n\n{USAGE}");
            std::process::exit(2);
        }
    }
}

fn number(words: &[&str], flag: &str) -> Option<u64> {
    words
        .iter()
        .position(|word| *word == flag)
        .and_then(|index| words.get(index + 1))
        .and_then(|value| value.parse().ok())
}

fn pair(words: &[&str], flag: &str) -> Option<(u64, u64)> {
    let value = words
        .iter()
        .position(|word| *word == flag)
        .and_then(|index| words.get(index + 1))?;
    let (at, span) = value.split_once(':')?;

    Some((at.parse().ok()?, span.parse().ok()?))
}

fn simulate(words: &[&str]) {
    let hours = number(words, "--hours").unwrap_or(3);
    let suspend = pair(words, "--suspend");
    let quiet = pair(words, "--quiet");

    let settings = Settings::default();
    let start = Moment::from_epoch_seconds(1_700_000_000);
    let mut schedule = Schedule::default();
    schedule.start(&settings, start);

    println!("simulating {hours}h at one minute per step");
    if let Some((at, span)) = suspend {
        println!("the machine sleeps for {span} minutes after {at} minutes");
    }
    if let Some((at, span)) = quiet {
        println!("do not disturb is on for {span} minutes after {at} minutes");
    }
    println!();
    print!("{:>7}  ", "time");
    for kind in ReminderKind::ALL {
        print!("{:>9}", format!("{kind:?}"));
    }
    println!("   event");

    let mut minute = 0;
    let mut fired = 0usize;

    while minute < hours * 60 {
        let mut gap = Duration::ZERO;

        if let Some((at, span)) = suspend
            && minute == at
        {
            minute += span;
            gap = Duration::from_mins(span);
        }

        let now = start.saturating_add(Duration::from_mins(minute));

        let quiet_event = quiet.and_then(|(at, span)| {
            if minute == at {
                schedule.set_quiet(&settings, now, true);
                Some("do not disturb on".to_owned())
            } else if minute == at + span {
                schedule.set_quiet(&settings, now, false);
                Some("do not disturb off".to_owned())
            } else {
                None
            }
        });

        let effects = schedule.advance(&settings, now, gap);

        let mut events: Vec<String> = Vec::new();
        if gap > Duration::ZERO {
            events.push(format!("woke after {} min asleep", gap.as_secs() / 60));
        }
        events.extend(quiet_event);
        for effect in &effects {
            if let Effect::Fire(kind) = effect {
                fired += 1;
                events.push(format!("FIRED {kind:?}"));
            }
        }

        if !events.is_empty() || minute % 15 == 0 {
            print!("{minute:>6}m  ");
            let reference = schedule.reference(now);
            for kind in ReminderKind::ALL {
                let left = schedule.due_at(kind).saturating_duration_since(reference);
                print!("{:>9}", format!("{}m", left.as_secs() / 60));
            }
            println!("   {}", events.join(", "));
        }

        minute += 1;
    }

    println!("\n{fired} notifications over {hours}h");
}

fn watch(words: &[&str]) {
    let seconds = number(words, "--seconds").unwrap_or(120);
    let live = words.contains(&"--notify");
    let elect = words.contains(&"--elect");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("pause-scheduler")
        .enable_all()
        .build()
        .expect("failed to start the scheduler runtime");

    let settings = SettingsStore::open(APP_ID).load();
    let store = ConfigScheduleStore::open(APP_ID);
    let notifier = RecordingNotifier::default();

    println!("schedule on disk: {:?}", store.load());
    println!("settings on disk: {settings:?}");
    if live {
        println!("watching for {seconds}s, reminders WILL reach the desktop\n");
    } else {
        println!("watching for {seconds}s, nothing will reach the desktop\n");
    }

    let (events, inbox) = cosmic_ext_applet_pause::pause::channel();
    let builder = Builder::new(store, phrases::Fluent, settings)
        .inbox(inbox)
        .leader(!elect);

    if elect {
        cosmic_ext_applet_pause::pause::leader::spawn(
            runtime.handle(),
            APP_ID.to_owned(),
            events.sender(),
        );
    }

    let (handle, _events, _join) = if live {
        let notifier = cosmic_ext_applet_pause::pause::notify::FreedesktopNotifier::spawn(
            runtime.handle(),
            "Pause".to_owned(),
            events.sender(),
        );
        builder.notifier(notifier).spawn(runtime.handle())
    } else {
        builder.notifier(notifier.clone()).spawn(runtime.handle())
    };

    let quiet = DoNotDisturb::load();
    println!("do not disturb: {}", quiet.0);
    handle.send(Command::DoNotDisturb(quiet.0));

    let mut snapshots = handle.subscribe();
    let inside = notifier.clone();

    runtime.block_on(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);

        loop {
            let snapshot = snapshots.borrow_and_update().clone();
            println!("revision {} leader={}", snapshot.revision, snapshot.leader);
            let now = Moment::from_epoch_seconds(
                i64::try_from(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |since| since.as_secs()),
                )
                .unwrap_or(i64::MAX),
            );
            let reference = snapshot.reference(now);
            for reminder in &snapshot.reminders {
                println!(
                    "  {:<8} {:>5}m left  enabled={} pending={}",
                    format!("{:?}", reminder.kind),
                    reminder.remaining(reference).as_secs() / 60,
                    reminder.setting.enabled,
                    reminder.pending,
                );
            }
            if let Some(pause) = snapshot.pause {
                println!("  paused since {:?}", pause.started_at);
            }
            if let Some(since) = snapshot.quiet_since {
                println!("  do not disturb since {since:?}");
            }
            for request in inside.take() {
                println!("  >> {} — {}", request.summary, request.body);
            }
            println!();

            if tokio::time::timeout_at(deadline, snapshots.changed())
                .await
                .is_err()
            {
                break;
            }
        }
    });

    println!(
        "{} notifications were delivered",
        notifier.delivered().len()
    );
}
