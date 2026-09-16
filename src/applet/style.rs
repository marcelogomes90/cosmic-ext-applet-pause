use cosmic::iced::{Background, Border, Color, Shadow};
use cosmic::widget;

pub fn surface(theme: &cosmic::Theme) -> widget::container::Style {
    let cosmic = theme.cosmic();
    let background = cosmic.background(theme.transparent);

    widget::container::Style {
        text_color: Some(background.on.into()),
        icon_color: Some(background.on.into()),
        background: Some(Color::from(background.base).into()),
        border: Border {
            radius: cosmic.corner_radii.radius_m.into(),
            width: 1.0,
            color: background.divider.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn card(theme: &cosmic::Theme) -> widget::container::Style {
    let cosmic = theme.cosmic();
    let component = &cosmic.background(theme.transparent).component;

    widget::container::Style {
        text_color: Some(Color::from(component.on)),
        icon_color: Some(Color::from(component.on)),
        background: Some(Color::from(component.base).into()),
        border: Border {
            radius: cosmic.corner_radii.radius_s.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn highlight(theme: &cosmic::Theme) -> widget::container::Style {
    let cosmic = theme.cosmic();
    let accent = cosmic.accent_color();

    widget::container::Style {
        text_color: Some(Color::from(cosmic.on_bg_color())),
        icon_color: Some(Color::from(accent)),
        background: Some(Background::Color(with_alpha(accent.into(), 0.12))),
        border: Border {
            radius: cosmic.corner_radii.radius_s.into(),
            width: 1.0,
            color: with_alpha(accent.into(), 0.45),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn chip(theme: &cosmic::Theme, lively: bool) -> widget::container::Style {
    let cosmic = theme.cosmic();

    let tint = if lively {
        Color::from(cosmic.accent_color())
    } else {
        Color::from(cosmic.on_bg_color())
    };

    widget::container::Style {
        text_color: None,
        icon_color: None,
        background: Some(Background::Color(with_alpha(
            tint,
            if lively { 0.16 } else { 0.07 },
        ))),
        border: Border {
            radius: cosmic.corner_radii.radius_s.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn quiet() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_, theme| neutral_style(theme, 0.0)),
        disabled: Box::new(|theme| neutral_style(theme, 0.0)),
        hovered: Box::new(|_, theme| neutral_style(theme, 0.08)),
        pressed: Box::new(|_, theme| neutral_style(theme, 0.14)),
    }
}

pub fn suggested() -> cosmic::theme::Button {
    cosmic::theme::Button::Suggested
}

pub fn standard() -> cosmic::theme::Button {
    cosmic::theme::Button::Standard
}

pub fn dimmed_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Custom(|theme| cosmic::iced::widget::text::Style {
        color: Some(dimmed(theme)),
        ..Default::default()
    })
}

pub fn dimmed_icon() -> cosmic::theme::Svg {
    cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
        color: Some(dimmed(theme)),
    })
}

pub fn accent_icon() -> cosmic::theme::Svg {
    cosmic::theme::Svg::custom(|theme| cosmic::iced::widget::svg::Style {
        color: Some(theme.cosmic().accent_color().into()),
    })
}

pub fn dimmed(theme: &cosmic::Theme) -> Color {
    let cosmic = theme.cosmic();

    with_alpha(Color::from(cosmic.on_bg_color()), 0.55)
}

fn neutral_style(theme: &cosmic::Theme, alpha: f32) -> widget::button::Style {
    let cosmic = theme.cosmic();
    let ink = Color::from(cosmic.on_bg_color());

    widget::button::Style {
        background: (alpha > 0.0).then(|| Background::Color(with_alpha(ink, alpha))),
        border_radius: cosmic.corner_radii.radius_s.into(),
        icon_color: Some(ink),
        text_color: Some(ink),
        ..widget::button::Style::new()
    }
}

fn with_alpha(colour: Color, alpha: f32) -> Color {
    Color { a: alpha, ..colour }
}
