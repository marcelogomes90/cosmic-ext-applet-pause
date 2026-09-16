use std::time::Duration;

use cosmic::Element;
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use crate::applet::message::Message;
use crate::applet::view::{
    FOOTER_RESERVE, GAP, GAP_TIGHT, HEADER_RESERVE, ICON, PAD, ROW_PAD, action, body_budget,
    divider, header, kind_icon, scroll, section_title, settings_button,
};
use crate::applet::{Pause, style, symbols};
use crate::pause::model::{Moment, Reminder, Snapshot};
use crate::{fl, format, phrases};

pub fn page(app: &Pause) -> Element<'_, Message> {
    let snapshot = app.snapshot();

    let children: Vec<Element<'_, Message>> = vec![
        header(fl!("app-title"), fl!("app-tagline"), settings_button()),
        status(snapshot, snapshot.reference(app.now())),
        divider(),
        widget::container(reminders(app))
            .max_height(body_budget(HEADER_RESERVE + FOOTER_RESERVE + 200.0))
            .into(),
        reset_timers(),
        divider(),
        section_title(fl!("quick-actions")),
        quick_actions(snapshot),
    ];

    widget::column::with_children(children)
        .spacing(GAP + GAP_TIGHT)
        .padding([0, 0, PAD, 0])
        .into()
}

fn status(snapshot: &Snapshot, now: Moment) -> Element<'_, Message> {
    let (icon, headline, detail) = if let Some(pause) = snapshot.pause {
        let until = pause.until.map_or_else(
            || fl!("paused-indefinitely"),
            |until| fl!("paused-until", time = format::clock(until)),
        );

        (symbols::app(), fl!("paused-title"), until)
    } else if let Some(reminder) = snapshot.next_due() {
        let left = reminder.remaining(now);
        let headline = if left == Duration::ZERO {
            fl!("next-break-now")
        } else {
            fl!("next-break-in", time = format::remaining(left))
        };

        (
            kind_icon(reminder.kind),
            headline,
            phrases::detail(reminder.kind),
        )
    } else {
        (
            symbols::app(),
            fl!("nothing-scheduled"),
            fl!("nothing-scheduled-detail"),
        )
    };

    let words = widget::column::with_children(vec![
        widget::text::body(headline)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
        widget::text::caption(detail)
            .class(style::dimmed_text())
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
    ])
    .spacing(1)
    .width(Length::Fill);

    let inner = widget::row::with_children(vec![
        symbols::sized(icon, ICON + 6)
            .class(style::accent_icon())
            .into(),
        words.into(),
    ])
    .spacing(GAP + GAP_TIGHT)
    .align_y(Alignment::Center);

    widget::container(
        widget::container(inner)
            .padding(ROW_PAD + 2)
            .width(Length::Fill)
            .style(style::highlight),
    )
    .padding([0, PAD])
    .width(Length::Fill)
    .into()
}

fn reminders(app: &Pause) -> Element<'_, Message> {
    let snapshot = app.snapshot();
    let now = snapshot.reference(app.now());
    let paused = snapshot.is_paused();

    let rows: Vec<Element<'_, Message>> = snapshot
        .reminders
        .iter()
        .map(|reminder| row(reminder, now, paused))
        .collect();

    scroll(
        widget::column::with_children(rows)
            .spacing(2)
            .padding([0, PAD]),
    )
    .into()
}

fn row<'a>(reminder: &Reminder, now: Moment, paused: bool) -> Element<'a, Message> {
    let kind = reminder.kind;
    let enabled = reminder.setting.enabled;
    let lively = enabled && !paused;

    let glyph = symbols::sized(kind_icon(kind), ICON).class(if lively {
        style::accent_icon()
    } else {
        style::dimmed_icon()
    });

    let words = widget::column::with_children(vec![
        widget::text::body(phrases::name(kind))
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
        widget::text::caption(format::interval(reminder.setting.interval()))
            .class(style::dimmed_text())
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
    ])
    .spacing(1)
    .width(Length::Fill);

    let left = widget::text::body(if enabled {
        format::remaining(reminder.remaining(now))
    } else {
        String::new()
    })
    .class(style::dimmed_text());

    let toggle = widget::toggler(enabled)
        .width(Length::Shrink)
        .on_toggle(move |value| Message::Toggle(kind, value));

    let content =
        widget::row::with_children(vec![glyph.into(), words.into(), left.into(), toggle.into()])
            .spacing(GAP + GAP_TIGHT)
            .align_y(Alignment::Center);

    widget::button::custom(content)
        .class(style::quiet())
        .padding([GAP_TIGHT + 2, GAP])
        .width(Length::Fill)
        .on_press(Message::Toggle(kind, !enabled))
        .into()
}

fn reset_timers<'a>() -> Element<'a, Message> {
    widget::container(action(
        Some(symbols::clock()),
        fl!("action-reset-timers"),
        Some(Message::ResetTimers),
        false,
        1,
    ))
    .padding([0, PAD])
    .width(Length::Fill)
    .into()
}

fn quick_actions(snapshot: &Snapshot) -> Element<'_, Message> {
    let buttons: Vec<Element<'_, Message>> = match snapshot.pause {
        Some(pause) if pause.until.is_some() => vec![
            action(
                Some(symbols::clock()),
                fl!("action-extend", minutes = 30),
                Some(Message::Extend(Duration::from_mins(30))),
                false,
                2,
            )
            .into(),
            action(
                Some(symbols::play()),
                fl!("action-resume"),
                Some(Message::Resume),
                true,
                2,
            )
            .into(),
        ],
        Some(_) => vec![
            action(
                Some(symbols::play()),
                fl!("action-resume"),
                Some(Message::Resume),
                true,
                1,
            )
            .into(),
        ],
        None => vec![
            action(
                Some(symbols::clock()),
                fl!("action-pause-30m"),
                Some(Message::PauseFor(Some(Duration::from_mins(30)))),
                false,
                1,
            )
            .into(),
            action(
                Some(symbols::clock()),
                fl!("action-pause-1h"),
                Some(Message::PauseFor(Some(Duration::from_hours(1)))),
                false,
                1,
            )
            .into(),
            action(
                Some(symbols::moon()),
                fl!("action-until-resumed"),
                Some(Message::PauseFor(None)),
                false,
                2,
            )
            .into(),
        ],
    };

    widget::container(widget::row::with_children(buttons).spacing(GAP))
        .padding([0, PAD])
        .width(Length::Fill)
        .into()
}
