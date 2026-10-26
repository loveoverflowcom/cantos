//! Bilingual resources for the app-owned auth/account compositions.
#[cfg(test)]
use crate::messages::Locale;

pub(crate) use crate::localization::generated::auth::{text, Key as CopyKey};
#[cfg(test)]
const ALL: &[CopyKey] = CopyKey::ALL;

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_ui::{AccessibleName, DisabledReason};

    #[test]
    fn every_auth_resource_is_nonblank_in_both_languages_and_valid_for_named_controls() {
        for key in ALL {
            for locale in [Locale::Vietnamese, Locale::English] {
                let copy = text(*key, locale);
                assert!(AccessibleName::new(copy).is_ok(), "{key:?}");
                assert!(DisabledReason::new(copy).is_ok(), "{key:?}");
                assert!(!copy.contains("Cantos Theatre"));
            }
        }
    }

    /// ADR 0048 stage 0: a signup started in the app finishes by returning to it, never by
    /// treating this Web page as the app's session. The copy names that path in both locales.
    #[test]
    fn account_created_notice_names_the_return_to_the_app() {
        let vi = text(CopyKey::AccountCreatedNotice, Locale::Vietnamese);
        let en = text(CopyKey::AccountCreatedNotice, Locale::English);
        assert!(
            vi.contains("ứng dụng Cantos") && vi.contains("quay lại ứng dụng"),
            "{vi}"
        );
        assert!(
            en.contains("Cantos app") && en.contains("go back to the app"),
            "{en}"
        );
    }
}
