#[allow(dead_code)]
#[rustfmt::skip]
#[path = "generated/third_locale.rs"]
mod generated;
use generated::{Locale, Message};

#[test]
fn resource_only_third_locale_has_real_translations_and_registry_choices() {
    assert_eq!(
        Locale::supported().map(Locale::lang).collect::<Vec<_>>(),
        ["vi-VN", "en", "ja"]
    );
    assert_eq!(Locale::from_lang("ja-JP"), Some(Locale::Japanese));
    assert_eq!(
        generated::probe::text(generated::probe::Key::Language, Locale::Japanese),
        "言語"
    );
    assert_eq!(Locale::Japanese.autonym(), "日本語");
    let title = "Lời hẹn {time} <b> %1$s";
    assert_eq!(
        Message::CommonProbeBookmark {
            arg_title: title,
            arg_time: "12:05"
        }
        .render(Locale::Japanese)
        .unwrap(),
        "12:05にLời hẹn {time} <b> %1$sを聴く"
    );
    for (count, english, japanese) in [
        (0, "0 episodes", "0話"),
        (1, "1 episode", "1話"),
        (2, "2 episodes", "2話"),
    ] {
        let message = Message::CommonProbeEpisodes { arg_count: count };
        assert_eq!(message.render(Locale::English).unwrap(), english);
        assert_eq!(message.render(Locale::Japanese).unwrap(), japanese);
    }
}

#[test]
fn preference_saved_choice_and_refusals_do_not_negotiate_device_language() {
    use cantos_localization::{select_preference, PreferenceWarning, StoredPreference};
    let select = |state| select_preference(state, Locale::default(), Locale::from_lang);
    assert_eq!(
        select(StoredPreference::Saved("en-GB")).locale,
        Locale::English
    );
    assert_eq!(select(StoredPreference::Missing).locale, Locale::Vietnamese);
    assert_eq!(
        select(StoredPreference::Saved("zh-Hant")).warning,
        Some(PreferenceWarning::Unsupported)
    );
    assert_eq!(
        select(StoredPreference::Unavailable).warning,
        Some(PreferenceWarning::StorageUnavailable)
    );
}

#[test]
fn resource_plural_metadata_matches_the_pinned_icu_provider() {
    use cantos_localization::{Formatter, PluralCategory};
    for (tag, expected) in [
        ("vi-VN", vec![PluralCategory::Other]),
        ("en", vec![PluralCategory::One, PluralCategory::Other]),
        ("ja", vec![PluralCategory::Other]),
    ] {
        assert_eq!(
            Formatter::try_new(tag)
                .unwrap()
                .cardinal_categories()
                .collect::<Vec<_>>(),
            expected
        );
    }
}
