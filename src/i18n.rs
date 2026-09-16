use std::sync::LazyLock;

use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{DefaultLocalizer, LanguageLoader, Localizer};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    loader
        .load_fallback_language(&Localizations)
        .expect("the fallback language is embedded in the binary");
    loader
});

pub fn init() {
    let localizer = DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations);
    let requested = i18n_embed::DesktopLanguageRequester::requested_languages();

    match localizer.select(&requested) {
        Ok(selected) => tracing::info!(?selected, ?requested, "loaded translations"),
        Err(error) => tracing::warn!(%error, "keeping English, the locale did not load"),
    }
}

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        ::i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),*) => {{
        ::i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    fn message_ids(catalogue: &str) -> BTreeSet<&str> {
        catalogue
            .lines()
            .filter(|line| !line.starts_with([' ', '\t', '#', '-', '*', '[', ']']))
            .filter_map(|line| line.split_once('='))
            .map(|(id, _)| id.trim())
            .filter(|id| !id.is_empty())
            .collect()
    }

    fn assert_matches_english(language: &str, catalogue: &str) {
        let english = message_ids(include_str!("../i18n/en/pause.ftl"));
        let translated = message_ids(catalogue);

        assert!(
            translated == english,
            "{language} is out of step with en: missing {:?}, unknown {:?}",
            english.difference(&translated).collect::<Vec<_>>(),
            translated.difference(&english).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn every_language_translates_exactly_the_same_messages() {
        for (language, catalogue) in [
            ("cs", include_str!("../i18n/cs/pause.ftl")),
            ("de", include_str!("../i18n/de/pause.ftl")),
            ("es", include_str!("../i18n/es/pause.ftl")),
            ("fr", include_str!("../i18n/fr/pause.ftl")),
            ("it", include_str!("../i18n/it/pause.ftl")),
            ("nl", include_str!("../i18n/nl/pause.ftl")),
            ("pt-BR", include_str!("../i18n/pt-BR/pause.ftl")),
            ("pl", include_str!("../i18n/pl/pause.ftl")),
            ("ru", include_str!("../i18n/ru/pause.ftl")),
            ("uk", include_str!("../i18n/uk/pause.ftl")),
            ("zh-CN", include_str!("../i18n/zh-CN/pause.ftl")),
        ] {
            assert_matches_english(language, catalogue);
        }
    }

    #[test]
    fn every_catalogue_parses_and_renders() {
        use i18n_embed::LanguageLoader as _;
        use i18n_embed::unic_langid::LanguageIdentifier;

        for language in [
            "cs", "de", "es", "fr", "it", "nl", "pl", "pt-BR", "ru", "uk", "zh-CN",
        ] {
            let id: LanguageIdentifier = language.parse().expect("a well-formed language tag");
            let loader = super::fluent_language_loader!();
            loader
                .load_languages(&super::Localizations, &[id])
                .unwrap_or_else(|error| panic!("{language} failed to load: {error}"));

            let rendered = i18n_embed_fl::fl!(loader, "duration-minutes", minutes = 7);
            assert!(
                !rendered.is_empty() && !rendered.contains('{'),
                "{language} left a placeholder unresolved: {rendered}"
            );
        }
    }

    const TIGHT_SLOTS: [&str; 8] = [
        "action-pause-30m",
        "action-pause-1h",
        "action-pause-2h",
        "action-until-resumed",
        "action-resume",
        "action-extend",
        "action-reset-timers",
        "duration-compact",
    ];

    fn message(catalogue: &str, wanted: &str) -> String {
        for line in catalogue.lines() {
            let Some((id, value)) = line.split_once('=') else {
                continue;
            };

            if id.trim() == wanted {
                return value.trim().replace("{ $minutes }", "120");
            }
        }

        panic!("{wanted} is missing from a catalogue");
    }

    #[test]
    fn nothing_in_a_fixed_width_slot_outgrows_the_language_the_layout_was_drawn_for() {
        let base = include_str!("../i18n/pt-BR/pause.ftl");

        for (language, catalogue) in [
            ("cs", include_str!("../i18n/cs/pause.ftl")),
            ("de", include_str!("../i18n/de/pause.ftl")),
            ("en", include_str!("../i18n/en/pause.ftl")),
            ("es", include_str!("../i18n/es/pause.ftl")),
            ("fr", include_str!("../i18n/fr/pause.ftl")),
            ("it", include_str!("../i18n/it/pause.ftl")),
            ("nl", include_str!("../i18n/nl/pause.ftl")),
            ("pl", include_str!("../i18n/pl/pause.ftl")),
            ("ru", include_str!("../i18n/ru/pause.ftl")),
            ("uk", include_str!("../i18n/uk/pause.ftl")),
            ("zh-CN", include_str!("../i18n/zh-CN/pause.ftl")),
        ] {
            for id in TIGHT_SLOTS {
                let room = message(base, id);
                let translated = message(catalogue, id);

                assert!(
                    translated.chars().count() <= room.chars().count(),
                    "{language} would overflow the slot for {id}: {translated:?} \
                     is longer than the pt-BR {room:?} the layout was drawn for"
                );
            }
        }
    }

    #[test]
    fn attributes_and_comments_are_not_mistaken_for_messages() {
        let ids = message_ids("# a comment = not a message\nreal = yes\n    .tooltip = no\n");

        assert_eq!(ids, BTreeSet::from(["real"]));
    }
}
