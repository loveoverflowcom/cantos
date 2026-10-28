//! Login-only Studio resources; independent of Theatre authentication and membership copy.
use crate::entry::Entry;
use crate::localization::generated::entry as resources;

#[derive(Clone, Copy)]
pub enum Key {
    LoginTitle,
    Welcome,
    WelcomeHelp,
    Checking,
    Denied,
    DeniedHelp,
    Unavailable,
    UnavailableHelp,
    SignInUnavailable,
    SignInUnavailableHelp,
    Retained,
    RetainedHelp,
    Dashboard,
    Import,
    Back,
    Cancel,
    Retry,
    Session,
    CloseAccount,
    MemoryHelp,
    Reauthenticate,
    CleanupPending,
}

pub fn title(entry: Entry) -> Key {
    match entry {
        Entry::Checking => Key::Checking,
        Entry::Denied => Key::Denied,
        Entry::Unavailable => Key::Unavailable,
        Entry::SignInUnavailable => Key::SignInUnavailable,
        Entry::RetainedOwner => Key::Retained,
        Entry::Login => Key::LoginTitle,
        Entry::Workspace => Key::Dashboard,
    }
}

/// Supporting copy under the entry title. A healthy sign-in (or its session check) has none:
/// access policy is enforced by the backend, so the form never restates it.
pub fn help(entry: Entry) -> Option<Key> {
    match entry {
        Entry::Checking | Entry::Login => None,
        Entry::Denied => Some(Key::DeniedHelp),
        Entry::Unavailable => Some(Key::UnavailableHelp),
        Entry::SignInUnavailable => Some(Key::SignInUnavailableHelp),
        Entry::RetainedOwner => Some(Key::RetainedHelp),
        Entry::Workspace => Some(Key::WelcomeHelp),
    }
}

pub fn text(key: Key, locale: crate::localization::Locale) -> &'static str {
    match key {
        Key::CleanupPending => resources::text(resources::Key::CleanupPending, locale),
        Key::MemoryHelp => resources::text(resources::Key::MemoryHelp, locale),
        Key::SignInUnavailable => resources::text(resources::Key::SignInUnavailable, locale),
        Key::SignInUnavailableHelp => {
            resources::text(resources::Key::SignInUnavailableHelp, locale)
        }
        Key::Reauthenticate => resources::text(resources::Key::Reauthenticate, locale),
        Key::LoginTitle => resources::text(resources::Key::LoginTitle, locale),
        Key::Welcome => resources::text(resources::Key::Welcome, locale),
        Key::WelcomeHelp => resources::text(resources::Key::WelcomeHelp, locale),
        Key::Checking => resources::text(resources::Key::Checking, locale),
        Key::Denied => resources::text(resources::Key::Denied, locale),
        Key::DeniedHelp => resources::text(resources::Key::DeniedHelp, locale),
        Key::Unavailable => resources::text(resources::Key::Unavailable, locale),
        Key::UnavailableHelp => resources::text(resources::Key::UnavailableHelp, locale),
        Key::Retained => resources::text(resources::Key::Retained, locale),
        Key::RetainedHelp => resources::text(resources::Key::RetainedHelp, locale),
        Key::Dashboard => resources::text(resources::Key::Dashboard, locale),
        Key::Import => resources::text(resources::Key::Import, locale),
        Key::Back => resources::text(resources::Key::Back, locale),
        Key::Cancel => resources::text(resources::Key::Cancel, locale),
        Key::Retry => resources::text(resources::Key::Retry, locale),
        Key::Session => resources::text(resources::Key::Session, locale),
        Key::CloseAccount => resources::text(resources::Key::CloseAccount, locale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localization::Locale;

    #[test]
    fn healthy_sign_in_and_session_check_carry_no_access_policy_line() {
        assert!(help(Entry::Login).is_none());
        assert!(help(Entry::Checking).is_none());
    }

    #[test]
    fn real_states_keep_their_help_in_both_locales() {
        for entry in [
            Entry::Denied,
            Entry::Unavailable,
            Entry::SignInUnavailable,
            Entry::RetainedOwner,
            Entry::Workspace,
        ] {
            let key = help(entry).expect("a real entry state keeps its supporting copy");
            for locale in [Locale::Vietnamese, Locale::English] {
                assert!(!text(key, locale).is_empty());
            }
        }
        assert_eq!(
            text(Key::LoginTitle, Locale::Vietnamese),
            "Đăng nhập Cantos Studio"
        );
    }

    #[test]
    fn studio_css_frames_operable_focus_targets_but_never_a_programmatic_heading() {
        let css = include_str!("../../studio.css");
        let headings = ":is(h1, h2, h3, h4, h5, h6)";
        // Every generic `[tabindex="-1"]` ring excludes headings; headings have an explicit none.
        let rings: Vec<&str> = css
            .lines()
            .filter(|line| line.contains("[tabindex=\"-1\"]") && line.contains("solid"))
            .collect();
        assert!(!rings.is_empty());
        for line in rings {
            assert!(line.contains(&format!(":not({headings})")), "{line}");
        }
        assert!(css.lines().any(|line| line.contains(&format!(
            ".studio-shell {headings}[tabindex=\"-1\"]:focus-visible"
        )) && line.contains("outline: none")));
        // Interactive controls keep the shared flush focus indicator.
        assert!(css.lines().any(
            |line| line.contains("summary, button, input, textarea, select")
                && line.contains(":focus-visible")
                && line.contains("outline: var(--cantos-size-focus-width) solid")
        ));
    }

    #[test]
    fn studio_css_hides_only_the_language_label_visually() {
        let css = include_str!("../../studio.css");
        let rule = css
            .lines()
            .find(|line| line.contains(".studio-language-field > .cui-field-label"))
            .expect("the language label has a visual-hide rule");
        assert!(rule.contains("clip-path: inset(50%)"), "{rule}");
        assert!(!rule.contains("display: none"), "{rule}");
        assert!(!rule.contains("visibility: hidden"), "{rule}");
    }
}
