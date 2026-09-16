use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget;

use crate::applet::message::Message;
use crate::applet::view::{
    DIVIDER_RESERVE, PAD, PAGE_HEADER_RESERVE, body_budget, card, divider, kind_icon, page_header,
    scroll, section, setting_row,
};
use crate::applet::{Pause, symbols};
use crate::pause::model::{
    INTERVAL_STEP_MINUTES, MAX_INTERVAL_MINUTES, MIN_INTERVAL_MINUTES, ReminderKind,
    ReminderSetting, Settings,
};
use crate::{fl, links, phrases};

pub fn page(app: &Pause) -> Element<'_, Message> {
    let sections = widget::column::with_children(vec![
        section(symbols::app(), fl!("settings-reminders"), reminders(app)),
        section(symbols::link(), fl!("links"), link_controls()),
    ])
    .spacing(PAD);

    widget::column::with_children(vec![
        page_header(fl!("settings"), Message::ShowSettings(false)),
        widget::container(divider()).into(),
        widget::container(scroll(widget::container(sections).padding(PAD)))
            .max_height(body_budget(PAGE_HEADER_RESERVE + DIVIDER_RESERVE))
            .width(Length::Fill)
            .into(),
    ])
    .into()
}

fn reminders(app: &Pause) -> Element<'_, Message> {
    let settings = *app.settings();

    ReminderKind::ALL
        .into_iter()
        .fold(card(), |card, kind| card.add(interval(settings, kind)))
        .into()
}

fn interval<'a>(settings: Settings, kind: ReminderKind) -> Element<'a, Message> {
    let minutes = settings.interval(kind).as_secs() / 60;
    let minutes = u32::try_from(minutes).unwrap_or(MIN_INTERVAL_MINUTES);

    let stepper = widget::spin_button(
        fl!("duration-compact", minutes = u64::from(minutes)),
        minutes,
        INTERVAL_STEP_MINUTES,
        MIN_INTERVAL_MINUTES,
        MAX_INTERVAL_MINUTES,
        move |value| {
            let mut next = settings;
            next.set(
                kind,
                ReminderSetting {
                    enabled: settings.enabled(kind),
                    interval_secs: value * 60,
                },
            );
            Message::Setting(Box::new(next))
        },
    );

    setting_row(kind_icon(kind), phrases::name(kind), stepper)
}

fn link_controls<'a>() -> Element<'a, Message> {
    card()
        .add(link(symbols::bug(), fl!("link-issues"), links::ISSUES))
        .add(link(
            symbols::person(),
            fl!("link-developer"),
            links::DEVELOPER,
        ))
        .add(link(
            symbols::code(),
            fl!("link-repository"),
            links::REPOSITORY,
        ))
        .into()
}

fn link<'a>(
    handle: widget::icon::Handle,
    label: String,
    url: &'static str,
) -> widget::list::ListButton<'a, Message> {
    widget::list::button(setting_row(
        handle,
        label,
        symbols::sized(symbols::link(), crate::applet::view::ICON),
    ))
    .on_press(Message::OpenLink(url))
}
