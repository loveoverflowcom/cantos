//! Theatre signup transport for Identity v1. Both routes are anonymous (`credentials: omit`, no
//! session fence): a receipt grants no account, session or Studio access, and completion never
//! sets a cookie. The browser adds `Origin`; no code here forges it.
#[cfg(any(target_arch = "wasm32", test))]
use crate::application::signup::{CompletionFailure, SignupFailure};
#[cfg(any(target_arch = "wasm32", test))]
use cantos_contracts::identity_auth::{SignupAccepted, SignupCompleted, SignupStatus};
#[cfg(any(target_arch = "wasm32", test))]
use cantos_web_auth::core::retry_after;

pub const SIGNUP_PATH: &str = "/api/v1/theatre/identity/signup";
pub const COMPLETE_PATH: &str = "/api/v1/theatre/identity/signup/complete";
#[cfg(any(target_arch = "wasm32", test))]
const MAX_BODY: usize = 8192;

#[cfg(any(target_arch = "wasm32", test))]
fn json_body(content_type: Option<&str>, body: &[u8]) -> bool {
    body.len() <= MAX_BODY
        && content_type.is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        })
}

/// `202 {accepted:true, proof_ttl_seconds}` is the only receipt, and it says nothing about the
/// address: the reply is identical for a registered mailbox.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn accepted(
    status: u16,
    content_type: Option<&str>,
    retry_after_header: Option<&str>,
    body: &[u8],
) -> Result<(), SignupFailure> {
    match status {
        202 => {
            if !json_body(content_type, body) {
                return Err(SignupFailure::Protocol);
            }
            match serde_json::from_slice::<SignupAccepted>(body) {
                Ok(SignupAccepted {
                    accepted: true,
                    proof_ttl_seconds,
                }) if proof_ttl_seconds > 0 => Ok(()),
                _ => Err(SignupFailure::Protocol),
            }
        }
        400 => Err(SignupFailure::InvalidEmail),
        403 | 404 => Err(SignupFailure::NotConfigured),
        429 => Err(SignupFailure::RateLimited(retry_after(retry_after_header))),
        503 => Err(SignupFailure::Unavailable),
        _ => Err(SignupFailure::UnknownOutcome),
    }
}

/// Only an exact `{status}` receipt is success. Definite refusals (400/401/403/404) had no
/// effect; 429, 503, other statuses and an unreadable 200 may have committed, so the caller
/// repeats only the identical request.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn completed(
    status: u16,
    content_type: Option<&str>,
    retry_after_header: Option<&str>,
    body: &[u8],
) -> Result<SignupStatus, CompletionFailure> {
    match status {
        200 => {
            if !json_body(content_type, body) {
                return Err(CompletionFailure::UnknownOutcome);
            }
            serde_json::from_slice::<SignupCompleted>(body)
                .map(|receipt| receipt.status)
                .map_err(|_| CompletionFailure::UnknownOutcome)
        }
        400 => Err(CompletionFailure::Invalid),
        401 => Err(CompletionFailure::Refused),
        403 => Err(CompletionFailure::Forbidden),
        404 => Err(CompletionFailure::NotConfigured),
        429 => Err(CompletionFailure::RateLimited(retry_after(
            retry_after_header,
        ))),
        503 => Err(CompletionFailure::Unavailable),
        _ => Err(CompletionFailure::UnknownOutcome),
    }
}

#[cfg(target_arch = "wasm32")]
type Answer = (u16, Option<String>, Option<String>, Vec<u8>);

/// Network loss and deadlines are unknown outcomes: the request may have been processed.
#[cfg(target_arch = "wasm32")]
async fn post(path: &'static str, body: String) -> Option<Answer> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller = AbortController::new().ok()?;
    let deadline = super::browser::deadline(controller).ok()?;
    let signal = deadline.controller.signal();
    let request = Request::post(&crate::api_endpoint::endpoint(path).ok()?)
        .credentials(RequestCredentials::Omit)
        .redirect(web_sys::RequestRedirect::Error)
        .abort_signal(Some(&signal))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .body(body)
        .ok()?;
    let response = request.send().await.ok()?;
    let status = response.status();
    let content_type = response.headers().get("content-type");
    let retry = response.headers().get("retry-after");
    let bytes = response.binary().await.ok()?;
    Some((status, content_type, retry, bytes))
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> u64 {
    web_sys::js_sys::Date::now() as u64
}

/// Re-render the panel when the 429 hold ends, so the disabled start button re-enables itself.
#[cfg(target_arch = "wasm32")]
fn release_after(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    seconds: u32,
) {
    use leptos::prelude::*;
    use web_sys::wasm_bindgen::{closure::Closure, JsCast};
    let Some(window) = web_sys::window() else {
        return;
    };
    let release = Closure::once_into_js(move || {
        let _ = state.try_update(|_| ());
    });
    let millis = seconds.saturating_mul(1000).min(i32::MAX as u32) as i32;
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(release.unchecked_ref(), millis);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn submit(state: leptos::prelude::RwSignal<crate::application::signup::SignupState>) {
    use cantos_contracts::identity_auth::SignupStart;
    use leptos::prelude::*;

    let Some((issued, email)) = state.write().begin_submit(now_ms()) else {
        return;
    };
    leptos::task::spawn_local(async move {
        let result = match serde_json::to_string(&SignupStart { email }) {
            Ok(body) => match post(SIGNUP_PATH, body).await {
                Some((status, content_type, retry, body)) => {
                    accepted(status, content_type.as_deref(), retry.as_deref(), &body)
                }
                None => Err(SignupFailure::UnknownOutcome),
            },
            Err(_) => Err(SignupFailure::Protocol),
        };
        let now = now_ms();
        let held = match result {
            Err(SignupFailure::RateLimited(seconds)) => Some(seconds),
            _ => None,
        };
        let _ = state.try_update(|current| {
            current.finish_submit(issued, result, now);
        });
        if let Some(seconds) = held {
            release_after(state, seconds);
        }
    });
}

/// Send the explicit completion. `on_completed` runs only for a current, successful answer and
/// moves to sign-in; no session exists yet and none is adopted.
#[cfg(target_arch = "wasm32")]
pub(crate) fn complete(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    on_completed: leptos::prelude::Callback<()>,
) {
    use leptos::prelude::*;
    let Some((issued, body)) = state.write().begin_complete() else {
        return;
    };
    send(state, issued, body, on_completed);
}

/// Repeat the identical retained request after an unknown outcome.
#[cfg(target_arch = "wasm32")]
pub(crate) fn retry(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    on_completed: leptos::prelude::Callback<()>,
) {
    use leptos::prelude::*;
    let Some((issued, body)) = state.write().begin_retry(now_ms()) else {
        return;
    };
    send(state, issued, body, on_completed);
}

#[cfg(target_arch = "wasm32")]
fn send(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    issued: u64,
    body: String,
    on_completed: leptos::prelude::Callback<()>,
) {
    use crate::application::signup::SignupPhase;
    use leptos::prelude::*;
    leptos::task::spawn_local(async move {
        let result = match post(COMPLETE_PATH, body).await {
            Some((status, content_type, retry, body)) => {
                completed(status, content_type.as_deref(), retry.as_deref(), &body)
            }
            None => Err(CompletionFailure::UnknownOutcome),
        };
        let now = now_ms();
        let finished = state
            .try_update(|current| {
                current.finish_complete(issued, result, now)
                    && matches!(current.phase(), SignupPhase::Completed(_))
            })
            .unwrap_or(false);
        if finished {
            on_completed.run(());
        }
    });
}

/// Close local intent. A close cannot undo an account a pending request may already have
/// created; the owner can still sign in with the chosen password.
pub(crate) fn cancel(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    preview: bool,
) {
    use leptos::prelude::*;
    let _ = preview;
    state.write().clear();
}

/// Invalidate private inputs before leaving without waking the subtree which navigation is
/// about to dispose. A tracked reset could remount its conditional form just before disposal.
pub(crate) fn leave(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    preview: bool,
) {
    use leptos::prelude::*;
    let _ = preview;
    state.update_untracked(crate::application::signup::SignupState::clear);
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: Option<&str> = Some("application/json; charset=utf-8");

    #[test]
    fn the_start_receipt_is_exactly_accepted_and_never_names_the_address() {
        let body = br#"{"accepted":true,"proof_ttl_seconds":900}"#;
        assert_eq!(accepted(202, JSON, None, body), Ok(()));
        for body in [
            br#"{"accepted":false,"proof_ttl_seconds":900}"#.as_slice(),
            br#"{"accepted":true,"proof_ttl_seconds":0}"#,
            br#"{"accepted":true}"#,
            br#"{"accepted":true,"proof_ttl_seconds":900,"exists":true}"#,
            br#"{"status":"accepted"}"#,
            &[b' '; 8193],
        ] {
            assert_eq!(
                accepted(202, JSON, None, body),
                Err(SignupFailure::Protocol)
            );
        }
        for mime in [None, Some("text/html"), Some("application/jsonp")] {
            assert_eq!(
                accepted(
                    202,
                    mime,
                    None,
                    br#"{"accepted":true,"proof_ttl_seconds":900}"#
                ),
                Err(SignupFailure::Protocol)
            );
        }
    }

    #[test]
    fn start_failures_distinguish_invalid_budget_outage_and_unknown_effect() {
        assert_eq!(
            accepted(400, JSON, None, b"{}"),
            Err(SignupFailure::InvalidEmail)
        );
        assert_eq!(
            accepted(429, JSON, Some("12"), b"{}"),
            Err(SignupFailure::RateLimited(12))
        );
        assert_eq!(
            accepted(429, JSON, None, b"{}"),
            Err(SignupFailure::RateLimited(30))
        );
        assert_eq!(
            accepted(503, JSON, None, b"{}"),
            Err(SignupFailure::Unavailable)
        );
        for status in [403, 404] {
            assert_eq!(
                accepted(status, JSON, None, b"{}"),
                Err(SignupFailure::NotConfigured)
            );
        }
        for status in [401, 500, 502, 504] {
            assert_eq!(
                accepted(status, JSON, None, b"{}"),
                Err(SignupFailure::UnknownOutcome)
            );
        }
    }

    #[test]
    fn completion_receipts_are_history_never_a_session() {
        for (body, expected) in [
            (r#"{"status":"completed"}"#, SignupStatus::Completed),
            (r#"{"status":"replayed"}"#, SignupStatus::Replayed),
        ] {
            assert_eq!(completed(200, JSON, None, body.as_bytes()), Ok(expected));
        }
        // Anything that carries credentials or a session is not the contract's answer, and an
        // unreadable 200 may have committed: it is an unknown outcome, retried identically.
        for body in [
            br#"{"status":"completed","session_id":"forged"}"#.as_slice(),
            br#"{"status":"completed","access_token":"x"}"#,
            br#"{"status":"session"}"#,
            br#"{"actor_id":"a","session_id":"s"}"#,
            b"",
            &[b' '; 8193],
        ] {
            assert_eq!(
                completed(200, JSON, None, body),
                Err(CompletionFailure::UnknownOutcome)
            );
        }
        assert_eq!(
            completed(200, Some("text/html"), None, br#"{"status":"completed"}"#),
            Err(CompletionFailure::UnknownOutcome)
        );
    }

    #[test]
    fn only_definite_refusals_are_terminal_and_everything_else_may_have_committed() {
        for (status, expected) in [
            (400, CompletionFailure::Invalid),
            (401, CompletionFailure::Refused),
            (403, CompletionFailure::Forbidden),
            (404, CompletionFailure::NotConfigured),
            (503, CompletionFailure::Unavailable),
            (429, CompletionFailure::RateLimited(30)),
        ] {
            assert_eq!(
                completed(status, JSON, None, b"{}"),
                Err(expected),
                "{status}"
            );
        }
        assert_eq!(
            completed(429, JSON, Some("5"), b"{}"),
            Err(CompletionFailure::RateLimited(5))
        );
        for status in [409, 500, 502, 504, 0] {
            assert_eq!(
                completed(status, JSON, None, b"{}"),
                Err(CompletionFailure::UnknownOutcome),
                "{status}"
            );
        }
    }

    #[test]
    fn signup_routes_are_the_two_anonymous_contract_paths() {
        assert_eq!(SIGNUP_PATH, "/api/v1/theatre/identity/signup");
        assert_eq!(COMPLETE_PATH, "/api/v1/theatre/identity/signup/complete");
        for path in [SIGNUP_PATH, COMPLETE_PATH] {
            assert!(crate::live::transport::is_anonymous_identity(path));
            assert!(!crate::live::transport::is_protected(path));
        }
    }
}
