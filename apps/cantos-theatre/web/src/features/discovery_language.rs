//! Listener-facing names for the audio-language filter.
//!
//! The request still carries the release's language tag; only the choice is made by name. The two
//! named languages are the choices this app offers, not a statement about which languages the
//! catalog holds. A tag that arrives from a link and is not named here stays selectable under its
//! own tag, so applying a link never silently drops its filter.
use crate::messages::{text, Key, Locale};
use cantos_ui::SelectOption;

/// A language this app names. The tag is the normalized (lower-case) form `DiscoveryQuery` sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Named {
    Vietnamese,
    English,
}

impl Named {
    const ALL: [Self; 2] = [Self::Vietnamese, Self::English];

    const fn tag(self) -> &'static str {
        match self {
            Self::Vietnamese => "vi-vn",
            Self::English => "en",
        }
    }

    const fn locale(self) -> Locale {
        match self {
            Self::Vietnamese => Locale::Vietnamese,
            Self::English => Locale::English,
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|named| named.tag().eq_ignore_ascii_case(tag))
    }
}

/// The select's options for the language currently applied (`""` = every language).
pub(crate) fn options(selected: &str, locale: Locale) -> Vec<SelectOption> {
    let mut options = vec![SelectOption::new(
        "",
        text(Key::DiscoveryLanguageAny, locale),
    )];
    options.extend(Named::ALL.into_iter().map(|named| SelectOption {
        lang: Some(named.locale().lang().to_owned()),
        ..SelectOption::new(named.tag(), named.locale().autonym())
    }));
    if !selected.is_empty() && Named::from_tag(selected).is_none() {
        options.push(SelectOption::new(selected, selected));
    }
    options
}

/// A released language tag as the listener reads it: its own name when named, else the tag.
pub(crate) fn display_name(tag: &str) -> String {
    Named::from_tag(tag).map_or_else(
        || tag.to_owned(),
        |named| named.locale().autonym().to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(options: &[SelectOption]) -> Vec<(&str, &str)> {
        options
            .iter()
            .map(|option| (option.value.as_str(), option.label.as_str()))
            .collect()
    }

    #[test]
    fn named_languages_are_chosen_by_name_and_sent_as_their_tag() {
        assert_eq!(
            pairs(&options("", Locale::Vietnamese)),
            [
                ("", "Mọi ngôn ngữ"),
                ("vi-vn", "Tiếng Việt"),
                ("en", "English")
            ]
        );
        // The names are the languages' own, so the list does not translate with the UI.
        assert_eq!(
            pairs(&options("en", Locale::English)),
            [
                ("", "All languages"),
                ("vi-vn", "Tiếng Việt"),
                ("en", "English")
            ]
        );
    }

    #[test]
    fn a_tag_from_a_link_that_is_not_named_stays_selectable_under_its_own_tag() {
        let options = options("fr-ca", Locale::English);
        assert_eq!(options.len(), 4);
        assert_eq!(pairs(&options)[3], ("fr-ca", "fr-ca"));
        // A named tag, in any case, never adds a duplicate.
        assert_eq!(super::options("VI-VN", Locale::English).len(), 3);
    }

    #[test]
    fn released_tags_read_as_names_and_unknown_tags_stay_verbatim() {
        assert_eq!(display_name("vi-VN"), "Tiếng Việt");
        assert_eq!(display_name("en"), "English");
        assert_eq!(display_name("en-GB"), "en-GB");
        assert_eq!(display_name(""), "");
    }
}
