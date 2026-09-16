use crate::fl;
use crate::pause::Phrasebook;
use crate::pause::model::ReminderKind;
use crate::pause::notify::Request;

pub const NOTIFICATION_ICON: &str = "io.github.marcelogomes90.cosmic-ext-applet-pause-symbolic";

pub fn icon_name(kind: ReminderKind) -> String {
    format!("{}-{}-symbolic", crate::APP_ID, kind.key())
}

pub fn name(kind: ReminderKind) -> String {
    match kind {
        ReminderKind::Eyes => fl!("reminder-eyes"),
        ReminderKind::Water => fl!("reminder-water"),
        ReminderKind::Breathe => fl!("reminder-breathe"),
        ReminderKind::Move => fl!("reminder-move"),
        ReminderKind::Posture => fl!("reminder-posture"),
        ReminderKind::Wrists => fl!("reminder-wrists"),
    }
}

pub fn detail(kind: ReminderKind) -> String {
    match kind {
        ReminderKind::Eyes => fl!("reminder-eyes-detail"),
        ReminderKind::Water => fl!("reminder-water-detail"),
        ReminderKind::Breathe => fl!("reminder-breathe-detail"),
        ReminderKind::Move => fl!("reminder-move-detail"),
        ReminderKind::Posture => fl!("reminder-posture-detail"),
        ReminderKind::Wrists => fl!("reminder-wrists-detail"),
    }
}

pub fn summary(kind: ReminderKind) -> String {
    match kind {
        ReminderKind::Eyes => fl!("notify-eyes-title"),
        ReminderKind::Water => fl!("notify-water-title"),
        ReminderKind::Breathe => fl!("notify-breathe-title"),
        ReminderKind::Move => fl!("notify-move-title"),
        ReminderKind::Posture => fl!("notify-posture-title"),
        ReminderKind::Wrists => fl!("notify-wrists-title"),
    }
}

pub fn body(kind: ReminderKind) -> String {
    match kind {
        ReminderKind::Eyes => fl!("notify-eyes-body"),
        ReminderKind::Water => fl!("notify-water-body"),
        ReminderKind::Breathe => fl!("notify-breathe-body"),
        ReminderKind::Move => fl!("notify-move-body"),
        ReminderKind::Posture => fl!("notify-posture-body"),
        ReminderKind::Wrists => fl!("notify-wrists-body"),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Fluent;

impl Phrasebook for Fluent {
    fn request(&self, kind: ReminderKind) -> Request {
        Request {
            kind,
            summary: summary(kind),
            body: body(kind),
            action_label: fl!("action-done"),
            icon: icon_name(kind),
            timeout_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reminder_has_something_to_say_in_every_place_the_ui_asks() {
        for kind in ReminderKind::ALL {
            for phrase in [name(kind), detail(kind), summary(kind), body(kind)] {
                assert!(!phrase.is_empty(), "{kind:?} left a phrase empty");
                assert!(
                    !phrase.contains('{'),
                    "{kind:?} left a placeholder unresolved: {phrase}"
                );
            }
        }
    }

    #[test]
    fn a_notification_request_carries_everything_the_daemon_needs() {
        for kind in ReminderKind::ALL {
            let request = Fluent.request(kind);

            assert_eq!(request.kind, kind);
            assert!(!request.summary.is_empty());
            assert!(!request.body.is_empty());
            assert!(!request.action_label.is_empty());
            assert_eq!(
                request.icon,
                format!("{}-{}-symbolic", crate::APP_ID, kind.key())
            );
        }
    }

    #[test]
    fn the_notification_icon_is_the_one_the_packaging_installs() {
        assert_eq!(NOTIFICATION_ICON, format!("{}-symbolic", crate::APP_ID));
    }

    #[test]
    fn every_reminder_asks_the_desktop_for_an_icon_of_its_own() {
        let mut names: Vec<String> = ReminderKind::ALL.iter().map(|k| icon_name(*k)).collect();
        names.sort();
        names.dedup();

        assert_eq!(names.len(), ReminderKind::COUNT);

        for kind in ReminderKind::ALL {
            let installed = std::path::Path::new("resources/icons")
                .join(format!("{}-symbolic.svg", kind.key()));
            assert!(
                installed.exists(),
                "{kind:?} asks for an icon the packaging does not install: {}",
                installed.display()
            );
        }
    }
}
