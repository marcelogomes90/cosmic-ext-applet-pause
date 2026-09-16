use cosmic::widget;

macro_rules! bundled {
    ($($name:ident => $file:literal,)*) => {
        $(
            pub fn $name() -> widget::icon::Handle {
                widget::icon::from_svg_bytes(
                    include_bytes!(concat!("../../resources/icons/", $file)).as_slice(),
                )
                .symbolic(true)
            }
        )*
    };
}

bundled! {
    app => "app-symbolic.svg",
    back => "back-symbolic.svg",
    breathe => "breathe-symbolic.svg",
    bug => "bug-symbolic.svg",
    clock => "clock-symbolic.svg",
    code => "code-symbolic.svg",
    eyes => "eyes-symbolic.svg",
    link => "link-symbolic.svg",
    moon => "moon-symbolic.svg",
    r#move => "move-symbolic.svg",
    panel => "panel-symbolic.svg",
    panel_paused => "panel-paused-symbolic.svg",
    pause => "pause-symbolic.svg",
    person => "person-symbolic.svg",
    play => "play-symbolic.svg",
    posture => "posture-symbolic.svg",
    settings => "settings-symbolic.svg",
    skip => "skip-symbolic.svg",
    water => "water-symbolic.svg",
    wrists => "wrists-symbolic.svg",
}

pub fn sized(handle: widget::icon::Handle, size: u16) -> widget::icon::Icon {
    widget::icon::icon(handle).size(size)
}
