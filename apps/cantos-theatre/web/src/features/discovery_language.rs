//! Listener-facing names for a released Work's language.
//!
//! Discovery no longer offers a content-language filter (#424); this only reads a release's tag as
//! the listener would. The two named languages are the ones this app names, not a statement about
//! which languages the catalog holds; any other tag is shown verbatim.
use crate::messages::Locale;

/// Content tags remain opaque. Only an exact resource tag receives its declared autonym;
/// regional/unknown content tags are never resolved through UI fallback policy.
pub(crate) fn display_name(tag: &str) -> String {
    Locale::supported()
        .find(|locale| locale.lang().eq_ignore_ascii_case(tag))
        .map_or_else(|| tag.to_owned(), |locale| locale.autonym().to_owned())
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
        assert_eq!(display_name("zh-Hant"), "zh-Hant");
    }
}
