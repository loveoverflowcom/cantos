//! Studio email/password sign-in resources. vi-VN and en are exhaustive. No message names the
//! failing factor of a login: the API never says, and the UI does not guess. A 403 is an
//! Origin refusal or a roster denial and the response cannot tell which, so its message never
//! claims that the password was right.
use crate::localization::generated::signin as resources;
use crate::signin::SignInFailure;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Email,
    Password,
    ShowPassword,
    HidePassword,
    Submit,
    Submitting,
    ReauthenticateHelp,
    Incomplete,
    InvalidInput,
    InvalidCredentials,
    NotPermitted,
    NotAvailable,
    RateLimited,
    Unavailable,
    AccountMismatch,
    CleanupPending,
    Superseded,
    SessionNotStored,
}

pub const ALL: [Key; 18] = [
    Key::Email,
    Key::Password,
    Key::ShowPassword,
    Key::HidePassword,
    Key::Submit,
    Key::Submitting,
    Key::ReauthenticateHelp,
    Key::Incomplete,
    Key::InvalidInput,
    Key::InvalidCredentials,
    Key::NotPermitted,
    Key::NotAvailable,
    Key::RateLimited,
    Key::Unavailable,
    Key::AccountMismatch,
    Key::CleanupPending,
    Key::Superseded,
    Key::SessionNotStored,
];

pub fn text(key: Key, locale: crate::localization::Locale) -> &'static str {
    match key {
        Key::Email => resources::text(resources::Key::Email, locale),
        Key::Password => resources::text(resources::Key::Password, locale),
        Key::ShowPassword => resources::text(resources::Key::ShowPassword, locale),
        Key::HidePassword => resources::text(resources::Key::HidePassword, locale),
        Key::Submit => resources::text(resources::Key::Submit, locale),
        Key::Submitting => resources::text(resources::Key::Submitting, locale),
        Key::ReauthenticateHelp => resources::text(resources::Key::ReauthenticateHelp, locale),
        Key::Incomplete => resources::text(resources::Key::Incomplete, locale),
        Key::InvalidInput => resources::text(resources::Key::InvalidInput, locale),
        Key::InvalidCredentials => resources::text(resources::Key::InvalidCredentials, locale),
        Key::NotPermitted => resources::text(resources::Key::NotPermitted, locale),
        Key::NotAvailable => resources::text(resources::Key::NotAvailable, locale),
        Key::RateLimited => resources::text(resources::Key::RateLimitedSummary, locale),
        Key::Unavailable => resources::text(resources::Key::Unavailable, locale),
        Key::AccountMismatch => resources::text(resources::Key::AccountMismatch, locale),
        Key::CleanupPending => resources::text(resources::Key::CleanupPending, locale),
        Key::Superseded => resources::text(resources::Key::Superseded, locale),
        Key::SessionNotStored => resources::text(resources::Key::SessionNotStored, locale),
    }
}

pub fn failure_key(failure: SignInFailure) -> Key {
    match failure {
        SignInFailure::Incomplete => Key::Incomplete,
        SignInFailure::InvalidInput => Key::InvalidInput,
        SignInFailure::InvalidCredentials => Key::InvalidCredentials,
        SignInFailure::NotPermitted => Key::NotPermitted,
        SignInFailure::NotAvailable => Key::NotAvailable,
        SignInFailure::RateLimited(_) => Key::RateLimited,
        SignInFailure::Unavailable => Key::Unavailable,
        SignInFailure::AccountMismatch => Key::AccountMismatch,
        SignInFailure::CleanupPending => Key::CleanupPending,
        SignInFailure::Superseded => Key::Superseded,
        SignInFailure::SessionNotStored => Key::SessionNotStored,
    }
}

/// The message for a failed attempt, with the bounded Retry-After seconds where there is one.
pub fn failure_text(failure: SignInFailure, locale: crate::localization::Locale) -> String {
    match failure {
        SignInFailure::RateLimited(seconds) => crate::localization::present(
            crate::localization::generated::Message::StudioSigninRateLimited {
                arg_seconds: u64::from(seconds),
            }
            .render(locale),
            locale,
        ),
        _ => text(failure_key(failure), locale).to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_resource_is_nonblank_and_distinct_failures_read_differently() {
        for key in ALL {
            for locale in crate::localization::Locale::supported() {
                assert!(!text(key, locale).trim().is_empty(), "{key:?}");
            }
        }
        let failures = [
            SignInFailure::Incomplete,
            SignInFailure::InvalidInput,
            SignInFailure::InvalidCredentials,
            SignInFailure::NotPermitted,
            SignInFailure::NotAvailable,
            SignInFailure::RateLimited(30),
            SignInFailure::Unavailable,
            SignInFailure::AccountMismatch,
            SignInFailure::CleanupPending,
            SignInFailure::Superseded,
            SignInFailure::SessionNotStored,
        ];
        for locale in crate::localization::Locale::supported() {
            let messages: Vec<_> = failures.iter().map(|f| failure_text(*f, locale)).collect();
            for (index, message) in messages.iter().enumerate() {
                assert!(!messages[..index].contains(message), "{message}");
            }
        }
    }

    #[test]
    fn no_message_claims_the_password_was_right_or_names_the_failing_factor() {
        for failure in [
            SignInFailure::Incomplete,
            SignInFailure::InvalidCredentials,
            SignInFailure::InvalidInput,
            SignInFailure::NotPermitted,
            SignInFailure::Unavailable,
            SignInFailure::NotAvailable,
            SignInFailure::AccountMismatch,
            SignInFailure::SessionNotStored,
        ] {
            for locale in crate::localization::Locale::supported() {
                let message = failure_text(failure, locale).to_lowercase();
                // A 403 cannot prove the credential was verified (it may be an Origin refusal).
                assert!(!message.contains("mật khẩu đúng"), "{failure:?}");
                assert!(!message.contains("password is correct"), "{failure:?}");
                assert!(!message.contains("password is right"), "{failure:?}");
                assert!(!message.contains("không tồn tại"), "{failure:?}");
                assert!(!message.contains("wrong password"), "{failure:?}");
                assert!(!message.contains("no such account"), "{failure:?}");
            }
        }
        // The 403 message says the request was refused and the account may not enter Studio.
        assert!(failure_text(
            SignInFailure::NotPermitted,
            crate::localization::Locale::English
        )
        .contains("may not access"));
        assert!(failure_text(
            SignInFailure::NotPermitted,
            crate::localization::Locale::Vietnamese
        )
        .contains("không được phép"));
    }

    #[test]
    fn retry_after_seconds_are_shown_and_the_placeholder_never_leaks() {
        assert_eq!(
            failure_text(
                SignInFailure::RateLimited(42),
                crate::localization::Locale::English
            ),
            "Too many attempts. Wait 42 seconds and try again."
        );
        assert!(failure_text(
            SignInFailure::RateLimited(42),
            crate::localization::Locale::Vietnamese
        )
        .contains("42 giây"));
        for key in ALL {
            if key != Key::RateLimited {
                assert!(!text(key, crate::localization::Locale::English).contains("{seconds}"));
                assert!(!text(key, crate::localization::Locale::Vietnamese).contains("{seconds}"));
            }
        }
    }
}
