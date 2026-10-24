//! Listener-facing names for a released Work's language.
//!
//! Discovery no longer offers a content-language filter (#424); this only reads a release's tag as
//! the listener would. The two named languages are the ones this app names, not a statement about
//! which languages the catalog holds; any other tag is shown verbatim.
use crate::messages::Locale;

/// A language this app names. The tag is the normalized (lower-case) form.
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

    #[test]
    fn released_tags_read_as_names_and_unknown_tags_stay_verbatim() {
        assert_eq!(display_name("vi-VN"), "Tiếng Việt");
        assert_eq!(display_name("en"), "English");
        assert_eq!(display_name("en-GB"), "en-GB");
        assert_eq!(display_name(""), "");
    }
}
