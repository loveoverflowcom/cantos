//! Pure projection of the shared session owner's failures onto Theatre's account vocabulary.
//! 401 is signed out, 409 a session change, 429 a bounded wait, 503/network an unknown effect that
//! never reads as signed out, and 404 a clear fault with no provider or mock fallback.
#[cfg(any(target_arch = "wasm32", test))]
use crate::application::account_view::{AccountFailure as Failure, LoginRefusal};
#[cfg(any(target_arch = "wasm32", test))]
use crate::application::errors::ErrorKind;
#[cfg(any(target_arch = "wasm32", test))]
use cantos_web_auth::Failure as Auth;

/// Status projection shared by every Theatre session and protected call.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn auth_error(error: Auth) -> Failure {
    match error {
        Auth::SignedOut => Failure::SignedOut,
        Auth::Changed | Auth::AccountMismatch => Failure::SessionChanged,
        // A 400 or a credential refusal is not a possible answer of a session read.
        Auth::Protocol | Auth::Invalid | Auth::InvalidCredentials => Failure::Protocol,
        Auth::Unavailable => Failure::Unavailable,
        Auth::RateLimited(seconds) => Failure::RateLimited(seconds),
        // A wrong Origin is a deployment fault, not a lost session.
        Auth::Forbidden => Failure::Diagnostic(ErrorKind::Forbidden),
        Auth::NotFound => Failure::NotFound,
        Auth::CleanupPending => Failure::SignedOut,
    }
}

/// Sign-in answers: a refusal leaves the listener a guest who may try again. A fence change
/// means the attempt was cancelled and its late session was already queued for revocation.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn login_error(error: Auth) -> Failure {
    match error {
        Auth::InvalidCredentials => Failure::LoginRefused(LoginRefusal::Credentials),
        Auth::Invalid => Failure::LoginRefused(LoginRefusal::Invalid),
        Auth::Changed => Failure::SignedOut,
        other => auth_error(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_failures_keep_unknown_effect_distinct_from_signed_out() {
        assert_eq!(auth_error(Auth::SignedOut), Failure::SignedOut);
        assert_eq!(auth_error(Auth::Changed), Failure::SessionChanged);
        assert_eq!(auth_error(Auth::AccountMismatch), Failure::SessionChanged);
        assert_eq!(auth_error(Auth::Unavailable), Failure::Unavailable);
        assert_eq!(auth_error(Auth::Protocol), Failure::Protocol);
        assert_eq!(auth_error(Auth::RateLimited(7)), Failure::RateLimited(7));
        assert_eq!(auth_error(Auth::NotFound), Failure::NotFound);
        assert_eq!(
            auth_error(Auth::Forbidden),
            Failure::Diagnostic(ErrorKind::Forbidden)
        );
        // A session read can never be answered by a credential refusal or a 400.
        assert_eq!(auth_error(Auth::InvalidCredentials), Failure::Protocol);
        assert_eq!(auth_error(Auth::Invalid), Failure::Protocol);
        assert_eq!(auth_error(Auth::CleanupPending), Failure::SignedOut);
    }

    #[test]
    fn sign_in_refusals_return_to_guest_and_a_cancelled_attempt_is_signed_out() {
        assert_eq!(
            login_error(Auth::InvalidCredentials),
            Failure::LoginRefused(LoginRefusal::Credentials)
        );
        assert_eq!(
            login_error(Auth::Invalid),
            Failure::LoginRefused(LoginRefusal::Invalid)
        );
        assert_eq!(login_error(Auth::Changed), Failure::SignedOut);
        // Everything else keeps the shared projection: 503 is unknown, never a refusal.
        for other in [
            Auth::Unavailable,
            Auth::RateLimited(30),
            Auth::NotFound,
            Auth::Forbidden,
            Auth::Protocol,
            Auth::CleanupPending,
        ] {
            assert_eq!(login_error(other), auth_error(other));
        }
    }
}
