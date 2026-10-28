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
    SessionRefused,
    SessionRefusedHelp,
    Dashboard,
    Import,
    Back,
    Cancel,
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
        Entry::SessionRefused => Key::SessionRefused,
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
        Entry::SessionRefused => Some(Key::SessionRefusedHelp),
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
        Key::SessionRefused => resources::text(resources::Key::SessionRefused, locale),
        Key::SessionRefusedHelp => resources::text(resources::Key::SessionRefusedHelp, locale),
        Key::Dashboard => resources::text(resources::Key::Dashboard, locale),
        Key::Import => resources::text(resources::Key::Import, locale),
        Key::Back => resources::text(resources::Key::Back, locale),
        Key::Cancel => resources::text(resources::Key::Cancel, locale),
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
    fn the_entry_has_no_retry_action_and_keeps_its_other_recovery_paths() {
        // #565 AC-07: a state-less "Try again" beside the sign-in form is redundant. The form
        // itself, the guarded observation on page restore and reload stay the recovery; the
        // editor's own revision retry is a different action and is not touched.
        let view = include_str!("../view.rs");
        assert!(!view.contains("session-refresh"));
        assert!(!view.contains("EntryKey::Retry"));
        assert!(view.contains("id=\"session-logout\""));
        assert!(view.contains("SignInPanel"));
        assert!(
            !include_str!("../../../../../packages/localization/catalog/v1/catalog.json")
                .contains("\"studio.entry.retry\"")
        );
    }

    #[test]
    fn real_states_keep_their_help_in_both_locales() {
        for entry in [
            Entry::Denied,
            Entry::Unavailable,
            Entry::SignInUnavailable,
            Entry::RetainedOwner,
            Entry::SessionRefused,
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

    #[test]
    fn studio_topbar_keeps_registry_selection_and_the_shared_tonal_icon_action() {
        let view = include_str!("../view.rs");
        let topbar = view
            .split("<div class=\"toolbar studio-topbar\">")
            .nth(1)
            .and_then(|rest| rest.split("<AccountEntry").next())
            .expect("the topbar language and theme controls exist");
        assert!(topbar.contains("Locale::supported()"), "{topbar}");
        assert!(topbar.contains("option.autonym()"), "{topbar}");
        assert!(
            topbar.contains("save_preference(selected).err()"),
            "{topbar}"
        );
        let theme = topbar
            .lines()
            .find(|line| line.contains("<ThemeCycleButton"))
            .expect("the existing theme action is reused");
        assert!(theme.contains("variant=ButtonVariant::Tonal"), "{theme}");
        assert!(!theme.contains("show_label=true"), "{theme}");
        // Studio inherits the shared tonal states and focus/forced-colour rules. The v6
        // account-control overrides would bring back the superseded high-emphasis pills.
        let css = include_str!("../../studio.css");
        assert!(!css.contains("--cantos-color-theatre-account-control"));
        assert!(!css.contains("--cantos-color-theatre-on-account-control"));
    }
}
