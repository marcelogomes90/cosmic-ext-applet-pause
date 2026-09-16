pub mod overview;
pub mod settings;

use cosmic::Element;
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use crate::applet::message::Message;
use crate::applet::{Page, Pause, popup, style, symbols};
use crate::fl;
use crate::pause::model::ReminderKind;

pub const PAD: u16 = 16;
pub const PAD_ROW_H: u16 = 12;
pub const LIST_INSET: u16 = 16;
pub const GAP: u16 = 8;
pub const GAP_TIGHT: u16 = 4;
pub const ROW_PAD: u16 = 10;
pub const CONTROL_HEIGHT: u16 = 32;
pub const ICON: u16 = 16;
pub const ICON_SMALL: u16 = 14;
pub const HEADER_RESERVE: f32 = 76.0;
pub const PAGE_HEADER_RESERVE: f32 = 64.0;
pub const DIVIDER_RESERVE: f32 = 1.0;
pub const FOOTER_RESERVE: f32 = 12.0;

pub fn kind_icon(kind: ReminderKind) -> widget::icon::Handle {
    match kind {
        ReminderKind::Eyes => symbols::eyes(),
        ReminderKind::Water => symbols::water(),
        ReminderKind::Breathe => symbols::breathe(),
        ReminderKind::Move => symbols::r#move(),
        ReminderKind::Posture => symbols::posture(),
        ReminderKind::Wrists => symbols::wrists(),
    }
}

pub fn popup(app: &Pause) -> Element<'_, Message> {
    let body = match app.page() {
        Page::Overview => overview::page(app),
        Page::Settings => settings::page(app),
    };

    let surface = widget::container(body)
        .width(Length::Fixed(popup::SURFACE_WIDTH))
        .style(style::surface);

    widget::autosize::autosize(surface, popup::SURFACE_ID.clone())
        .limits(popup::surface_limits())
        .into()
}

pub const APP_ICON: u16 = 30;

pub fn header(
    title: String,
    subtitle: String,
    trailing: Element<'_, Message>,
) -> Element<'_, Message> {
    let badge = symbols::sized(symbols::app(), APP_ICON);

    let words = widget::column::with_children(vec![
        widget::text::heading(title).into(),
        widget::text::caption(subtitle)
            .class(style::dimmed_text())
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
    ])
    .spacing(1)
    .width(Length::Fill);

    widget::container(
        widget::row::with_children(vec![badge.into(), words.into(), trailing])
            .spacing(GAP + GAP_TIGHT)
            .align_y(Alignment::Center),
    )
    .padding([PAD, PAD, 0, PAD])
    .width(Length::Fill)
    .into()
}

pub fn page_header<'a>(title: String, back: Message) -> Element<'a, Message> {
    let back = widget::button::text(fl!("back"))
        .class(style::quiet())
        .height(Length::Fixed(f32::from(CONTROL_HEIGHT)))
        .on_press(back);

    widget::container(
        cosmic::iced::widget::stack(vec![
            widget::container(back).into(),
            widget::container(
                widget::text::heading(title)
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
            )
            .center(Length::Fill)
            .into(),
        ])
        .width(Length::Fill),
    )
    .padding(PAD)
    .width(Length::Fill)
    .into()
}

pub fn card<'a>() -> widget::ListColumn<'a, Message> {
    widget::list_column().list_item_padding([cosmic::theme::spacing().space_xxs, LIST_INSET])
}

pub fn heading<'a>(handle: widget::icon::Handle, title: String) -> Element<'a, Message> {
    widget::row::with_children(vec![
        symbols::sized(handle, ICON_SMALL).into(),
        widget::text::heading(title)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
    ])
    .spacing(GAP)
    .align_y(Alignment::Center)
    .into()
}

pub fn section(
    handle: widget::icon::Handle,
    title: String,
    controls: Element<'_, Message>,
) -> Element<'_, Message> {
    widget::column::with_children(vec![heading(handle, title), controls])
        .spacing(PAD_ROW_H)
        .into()
}

pub fn setting_row<'a>(
    handle: widget::icon::Handle,
    label: String,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    widget::row::with_children(vec![
        symbols::sized(handle, ICON).into(),
        widget::text::body(label)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .width(Length::Fill)
            .into(),
        control.into(),
    ])
    .spacing(GAP + 4)
    .align_y(Alignment::Center)
    .into()
}

pub fn settings_button<'a>() -> Element<'a, Message> {
    widget::button::icon(symbols::settings())
        .class(style::quiet())
        .width(Length::Fixed(f32::from(CONTROL_HEIGHT)))
        .height(Length::Fixed(f32::from(CONTROL_HEIGHT)))
        .on_press(Message::ShowSettings(true))
        .into()
}

pub fn divider<'a>() -> Element<'a, Message> {
    widget::container(widget::divider::horizontal::default())
        .padding([0, PAD])
        .into()
}

pub fn section_title<'a>(title: String) -> Element<'a, Message> {
    widget::container(
        widget::text::heading(title).ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
    )
    .padding([0, PAD])
    .width(Length::Fill)
    .into()
}

pub fn action<'a>(
    icon: Option<widget::icon::Handle>,
    label: String,
    message: Option<Message>,
    prominent: bool,
    portion: u16,
) -> widget::Button<'a, Message> {
    let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);

    if let Some(icon) = icon {
        children.push(symbols::sized(icon, ICON_SMALL).into());
    }

    children.push(
        widget::text::body(label)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
    );

    let content = widget::row::with_children(children)
        .spacing(GAP_TIGHT + 2)
        .align_y(Alignment::Center);

    widget::button::custom(widget::container(content).center(Length::Fill))
        .class(if prominent {
            style::suggested()
        } else {
            style::standard()
        })
        .padding(0)
        .height(Length::Fixed(f32::from(CONTROL_HEIGHT + 4)))
        .width(Length::FillPortion(portion))
        .on_press_maybe(message)
}

pub fn scroll<'a>(
    content: impl Into<Element<'a, Message>>,
) -> cosmic::iced::widget::Scrollable<'a, Message, cosmic::Theme, cosmic::Renderer> {
    widget::scrollable(content)
        .scrollbar_width(0.0)
        .scroller_width(0.0)
        .scrollbar_padding(0.0)
}

pub fn body_budget(reserved: f32) -> f32 {
    popup::SURFACE_MAX_HEIGHT - reserved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reminder_has_an_icon_of_its_own() {
        let handles: Vec<_> = ReminderKind::ALL
            .iter()
            .map(|kind| format!("{:?}", kind_icon(*kind).data))
            .collect();

        for (index, handle) in handles.iter().enumerate() {
            for other in handles.iter().skip(index + 1) {
                assert_ne!(handle, other, "two reminders share an icon");
            }
        }
    }

    #[test]
    fn the_body_always_leaves_room_for_the_header_it_sits_under() {
        assert!(body_budget(HEADER_RESERVE + FOOTER_RESERVE) > 0.0);
        assert!(body_budget(HEADER_RESERVE + FOOTER_RESERVE) < popup::SURFACE_MAX_HEIGHT);
    }
}
