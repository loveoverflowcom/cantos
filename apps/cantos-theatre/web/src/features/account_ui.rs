//! Small, immutable presentation facts; the mounted account remains the only state owner.
use crate::application::account_view::{
    AccountFailure, AccountViewState, LoginRefusal, SessionPhase,
};
use cantos_ui::StatusTone;
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccountUi {
    pub phase: SessionPhase,
    pub can_authenticate: bool,
    pub can_retry_logout: bool,
    pub cleanup_in_flight: bool,
    pub login_refusal: Option<LoginRefusal>,
}

impl AccountUi {
    fn of(state: &AccountViewState) -> Self {
        Self {
            phase: state.phase(),
            can_authenticate: state.can_authenticate(),
            can_retry_logout: state.can_retry_logout(),
            cleanup_in_flight: state.cleanup_in_flight(),
            login_refusal: state.login_refusal(),
        }
    }
}

pub(crate) fn account_ui(state: ReadSignal<AccountViewState>) -> Memo<AccountUi> {
    // Borrowing avoids copying Library/sync data; equality stops unrelated notifications
    // before they reach profile/status/control closures. This is not another writable owner.
    Memo::new(move |_| state.with(AccountUi::of))
}

/// Presentation only: a missing session is normal, and an unresolved cleanup is not a
/// definite sign-out failure. Admission and recovery remain with AccountViewState.
pub(crate) fn session_tone(phase: SessionPhase) -> StatusTone {
    match phase {
        SessionPhase::Idle | SessionPhase::Checking => StatusTone::Info,
        SessionPhase::Guest | SessionPhase::SignedIn => StatusTone::Neutral,
        SessionPhase::LogoutPending => StatusTone::Warning,
        SessionPhase::Failed(
            AccountFailure::RateLimited(_)
            | AccountFailure::SessionChanged
            | AccountFailure::LogoutPending,
        ) => StatusTone::Warning,
        SessionPhase::Failed(AccountFailure::SignedOut) => StatusTone::Neutral,
        SessionPhase::Failed(
            AccountFailure::Diagnostic(_)
            | AccountFailure::Unavailable
            | AccountFailure::Protocol
            | AccountFailure::NotFound
            | AccountFailure::CursorExpired
            | AccountFailure::LoginRefused(_),
        ) => StatusTone::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_feedback_uses_the_canonical_pairs_available_in_both_themes() {
        let css = include_str!("../styles/theatre.css");
        let tokens: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../packages/ui-system/shared/tokens.json"
        ))
        .unwrap();
        for (tone, background, foreground) in [
            ("info", "primaryContainer", "onPrimaryContainer"),
            ("warning", "warningContainer", "warning"),
            ("error", "errorContainer", "error"),
        ] {
            let rule = css
                .split(&format!(".theatre-auth-feedback[data-tone=\"{tone}\"] {{"))
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            for role in [background, foreground] {
                let variable = role.chars().fold(String::new(), |mut name, c| {
                    if c.is_ascii_uppercase() {
                        name.push('-');
                    }
                    name.push(c.to_ascii_lowercase());
                    name
                });
                assert!(rule.contains(&format!("var(--cantos-color-{variable})")));
                for theme in ["light", "dark"] {
                    assert!(tokens["themes"][theme][format!("color.{role}")]["value"].is_string());
                }
            }
            assert!(!rule.contains('#') && !rule.contains("rgb"));
        }
        let neutral = css
            .split(".theatre-auth-feedback[data-tone=\"neutral\"] {")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(neutral.contains("color: inherit"));
        assert!(!neutral.contains("error"));
    }

    #[test]
    fn session_feedback_distinguishes_normal_loading_warning_and_error() {
        for (phase, expected) in [
            (SessionPhase::Idle, StatusTone::Info),
            (SessionPhase::Checking, StatusTone::Info),
            (SessionPhase::Guest, StatusTone::Neutral),
            (SessionPhase::SignedIn, StatusTone::Neutral),
            (SessionPhase::LogoutPending, StatusTone::Warning),
            (
                SessionPhase::Failed(AccountFailure::SignedOut),
                StatusTone::Neutral,
            ),
            (
                SessionPhase::Failed(AccountFailure::RateLimited(5)),
                StatusTone::Warning,
            ),
            (
                SessionPhase::Failed(AccountFailure::SessionChanged),
                StatusTone::Warning,
            ),
            (
                SessionPhase::Failed(AccountFailure::LogoutPending),
                StatusTone::Warning,
            ),
            (
                SessionPhase::Failed(AccountFailure::Unavailable),
                StatusTone::Error,
            ),
            (
                SessionPhase::Failed(AccountFailure::NotFound),
                StatusTone::Error,
            ),
            (
                SessionPhase::Failed(AccountFailure::Protocol),
                StatusTone::Error,
            ),
            (
                SessionPhase::Failed(AccountFailure::CursorExpired),
                StatusTone::Error,
            ),
            (
                SessionPhase::Failed(AccountFailure::LoginRefused(LoginRefusal::Credentials)),
                StatusTone::Error,
            ),
            (
                SessionPhase::Failed(AccountFailure::LoginRefused(LoginRefusal::Invalid)),
                StatusTone::Error,
            ),
        ] {
            assert_eq!(session_tone(phase), expected, "{phase:?}");
        }
    }
}
