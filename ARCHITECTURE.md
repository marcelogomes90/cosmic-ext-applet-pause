# Architecture

Pause is a COSMIC panel applet that reminds you to look after yourself while you work.
This document carries the reasoning the code cannot: why the scheduler behaves the way it
does, what was tried and rejected, and which invariants must not be broken.

## Layout

```
src/
├── main.rs              runtime, stores, notifier, leader election, then the applet
├── lib.rs               APP_ID, init_tracing, module list
├── config.rs            cosmic-config: Settings and Schedule entries, and their stores
├── format.rs            durations and clock times as the user reads them
├── phrases.rs           the Fluent phrasebook the scheduler speaks through
├── i18n.rs  links.rs
├── bin/pause_dump.rs    headless scheduler: simulate a timeline, or watch the real one
├── pause/               the domain. No cosmic, no iced.
│   ├── mod.rs           the actor: PauseHandle, Command, Event, the select! loop
│   ├── model.rs         ReminderKind, Moment, Settings, Snapshot
│   ├── schedule.rs      the scheduling policy, as pure functions
│   ├── clock.rs         Clock and GapDetector
│   ├── notify.rs        the Notifier trait and its freedesktop implementation
│   ├── proxies.rs       org.freedesktop.Notifications
│   ├── leader.rs        one instance decides who speaks
│   └── call.rs          every remote call gets a timeout
└── applet/              the UI. libcosmic and iced live only here.
    ├── mod.rs           the cosmic::Application impl
    ├── subscription.rs  watch channel and cosmic-config into Messages
    ├── popup.rs         surface geometry, as pure functions
    ├── style.rs  symbols.rs
    └── view/{mod,overview,settings}.rs
```

## The flow

```
cosmic-config ──┐                              ┌── org.freedesktop.Notifications
                ▼                              ▼
  ScheduleStore ─► Scheduler (one tokio select!) ─► Notifier
        ▲              │
        │              ▼
   Command       watch::Sender<Arc<Snapshot>>
   (mpsc)             │
        │              ▼
        └──── Subscription ─► Message ─► update ─► view
```

The scheduler runs on its own two-worker tokio runtime, created in `main` and leaked so it
outlives the stack while the iced event loop drives the UI on `SingleThreadExecutor`.

## The scheduling policy

`schedule.rs` is the heart, and it is deliberately a set of pure functions: `advance` takes
the state, the settings, the current moment and the size of any gap since the last tick, and
returns the new state plus a list of effects. Nothing in it knows about D-Bus, the UI, or the
clock. That is what lets roughly forty tests pin the behaviour exhaustively in microseconds.

Four rules hold the whole product together:

**At most one notification per tick.** When several reminders come due at the same minute —
which happens constantly, since 20, 60 and 90 minute intervals coincide — only the most
overdue one fires. The rest are pushed out by a one minute stagger, not by a whole interval,
so they still arrive promptly but never as a burst.

**A reminder never accumulates a backlog.** Firing sets `due_at = now + interval`, not
`due_at += interval`. Ignoring a reminder for five hours owes you exactly one reminder, not
fifteen.

**Time that passed while you were away is time that passed.** See below.

**The badge retires itself.** `pending_until` is a moment, not a flag. The panel icon shows
the due mark for two minutes and then goes back on its own, without the user having to
acknowledge anything. `retire_badges` runs at the top of every `advance`.

## Suspend, hibernate, and the clock

`std::time::Instant` on Linux is `CLOCK_MONOTONIC`, which freezes while the machine sleeps,
and `tokio::time::Instant` is a newtype over it. A timer armed for twenty minutes across a
three hour suspend wakes up still believing it has nineteen minutes left. The wall clock, of
course, does not freeze.

The difference between the two deltas **is** the length of the sleep. `GapDetector` reports
it, and a gap beyond two minutes triggers a silent rebase: every reminder whose moment fell
inside the gap is given a fresh full interval, and a sixty second settle window keeps
anything from firing while the session is still coming back. Closing the laptop for two hours
costs you zero notifications.

This needs no D-Bus and no system bus permission. `PrepareForSleep` on `org.freedesktop.login1`
would be more precise and would react instantly instead of within one twenty second heartbeat,
but it buys little for a product whose smallest interval is ten minutes, and it would cost a
system bus grant in the Flatpak sandbox.

A large NTP step is indistinguishable from a sleep and is treated identically. That is a
deliberate trade: both mean "the wall clock moved without the user being here for it."

**Restarting follows the same rule.** On startup a stored moment is kept if it is still in the
future or overdue by less than the startup grace, rebased silently if it is older than that,
and clamped if it somehow sits more than a full interval ahead. A panel restart takes seconds
and keeps every countdown; a night logged out starts fresh in silence. One policy, two places.

## One process per screen

`cosmic-panel` spawns one applet process per output. Without coordination every notification
would arrive twice on a two monitor desk. Each instance asks for the well known name
`io.github.marcelogomes90.cosmic-ext-applet-pause`, with empty request flags so it queues
instead of stealing, and the `NameAcquired` stream is created *before* the request so the
signal cannot be missed. The owner fires notifications and advances the schedule; the others
render, act on what the user clicks, and take over automatically if the owner dies.

**The leader is invisible to the user.** An earlier version put a line in the settings page
saying another instance was sending the reminders. That was implementation detail leaking into
the product: there is one Pause, and which screen a notification lands on is the notification
daemon's decision, not ours.

**The write shadow must follow adopted state.** `ConfigScheduleStore` skips writing a key whose
value has not moved since it last wrote it. When an instance adopts a record another instance
wrote, that shadow has to be updated too — `ScheduleStore::adopted` exists for exactly this.
Without it, pausing on one screen and resuming on the other silently wrote nothing: the second
instance compared the resume against its own stale shadow, concluded nothing had changed, and
dropped it. The test `a_pause_started_on_one_screen_can_be_resumed_from_the_other` in
`config.rs` runs against a real cosmic-config on disk and fails if the call is removed.

## Notifications

`cosmic-notifications` advertises the `actions` capability, but its banner does not render one
button per action: the whole card carries a single activation, and the daemon picks the action
itself — the one named `default`, or failing that the first one declared. Two buttons in a
banner are therefore not achievable today. Pause declares one `default` action labelled Done,
and a click on the banner clears the badge.

`expire_timeout` is honoured but clamped per urgency, and passing `-1` lands on three seconds
rather than the server default. Pause asks explicitly for ten seconds at normal urgency. If the
user's `max_timeout_normal` is lower, the daemon clamps it; raising the ceiling is the user's
call. Marking a break reminder as critical to escape the clamp would also make it ignore Do Not
Disturb, which is the wrong trade for a nudge to drink water.

`notify-rust` was rejected: its blocking calls run `zbus::block_on`, which panics from inside a
tokio worker. A `zbus` proxy is less code and gives one stream for `ActionInvoked` and
`NotificationClosed`.

## Layer boundaries

`src/pause/` must never import `cosmic::` or `iced::`. It may use tokio and zbus. This is what
lets the whole scheduler be driven by `pause-dump` with no display server, and it is checked
mechanically by `just layering` and by CI.

`cosmic-config` is reached from the domain through the `ScheduleStore` trait, implemented in
`config.rs`, so the persistence format stays on the UI side of the line.

## Configuration, and why it is split by key

`Settings` holds what the user chooses; `Schedule` holds what the scheduler decides. Both live
under the same app id and version. Every reminder owns its own `due-*` and `pending-*` key, so
two instances writing about different reminders at the same moment cannot clobber each other —
cosmic-config stores one file per key.

`update_keys` reports a key as changed only when the value actually moved. Without that, each
instance's own write comes back through its subscription and the two ping-pong forever.

## The popup

One surface, painted once. An earlier version wrapped the content in a styled container *and*
in `core.applet.popup_container`, which paints its own background and border — two backgrounds
stacked, and the frosted panel setting had no visible effect because the opaque inner layer sat
on top of it. The popup now paints itself, the way Clip Keep does, and reads `theme.transparent`
so frosted works.

Colour comes from the theme, never from a literal. The reminder icons use the accent when they
are live and dimmed ink when they are off or paused. Buttons use `Button::Suggested` and
`Button::Standard` rather than hand mixed pairs: pairing `accent_color` with `on_accent_color`
looks right in the default theme and goes unreadable in others, because the matched pair lives
on the `accent_button` component.

## Icons

The COSMIC icon theme ships 672 icons and has no eye, droplet, walking figure, chair, lungs or
hand. All six reminder glyphs are drawn here and compiled into the binary with
`icon::from_svg_bytes(..).symbolic(true)`, which also means they render identically inside the
Flatpak sandbox. They are additionally installed into the hicolor theme under
`<app-id>-<kind>-symbolic`, because the notification daemon resolves `app_icon` by name and a
reminder about your eyes should arrive with an eye on it.

Symbolic icons are recoloured to a single colour, so a knocked-out shape needs a mask rather
than a second fill — the due variant of the panel icon is a filled circle with the pause bars
masked out.

## Text that has to fit

Most labels ellipsize, so a long translation degrades instead of breaking the layout. Eight
strings sit in slots that cannot ellipsize gracefully: the three pause buttons, the resume and
extend buttons, the reset button, the until-resumed label, and the spin button's value, whose
label area libcosmic fixes at 48 pixels. Those are held at or below the pt-BR length by
`nothing_in_a_fixed_width_slot_outgrows_the_language_the_layout_was_drawn_for`.

Applying that ceiling to every string was considered and rejected: "Punhos" is six characters,
and fitting under it would have turned wrists into hands in seven languages to save one
character.

## Failure handling

- No session bus, no notification service, or a daemon that has died: logged once, the applet
  keeps its countdowns, and `NotifierState` carries the fact.
- `cosmic_config` unavailable: settings do not persist, nothing panics, defaults apply.
- A corrupted or hand edited config is clamped by `sanitised`, never trusted.
- Every remote call goes through `call::with_timeout`. There are no bare awaits on D-Bus.

## How to test

```sh
just verify          # fmt, clippy -D warnings, layering, tests, desktop and metainfo
just run-dump        # simulate a timeline, or watch the real scheduler, with no display server
```

`pause-dump simulate --hours 10 --suspend 45:480` replays a night of hibernation in
milliseconds and prints every countdown and every notification, which is the fastest way to see
the policy behave. `pause-dump watch --notify --elect` runs the real thing against the real
session bus.

Unit tests live at the bottom of the module they cover and are named as sentences describing
the invariant, not after the function under test.
