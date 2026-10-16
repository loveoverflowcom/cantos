//! Same-origin enrollment transport. A receipt grants no account, session or Studio access.
#[cfg(any(target_arch = "wasm32", test))]
use crate::application::signup::SignupFailure;
#[cfg(any(target_arch = "wasm32", test))]
use cantos_contracts::theatre_registration::{Accepted, VerificationResponse, VerificationStatus};

#[cfg(any(target_arch = "wasm32", test))]
fn json_body(content_type: Option<&str>, body: &[u8]) -> Result<(), SignupFailure> {
    if body.len() > 8192
        || !content_type.is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(SignupFailure::Protocol);
    }
    Ok(())
}

#[cfg(any(target_arch = "wasm32", test))]
fn accepted(status: u16, content_type: Option<&str>, body: &[u8]) -> Result<(), SignupFailure> {
    json_body(content_type, body)?;
    match status {
        202 => serde_json::from_slice::<Accepted>(body)
            .map(|_| ())
            .map_err(|_| SignupFailure::Protocol),
        400 if serde_json::from_slice::<cantos_contracts::ApiError>(body)
            .is_ok_and(|error| error.code == cantos_contracts::ErrorCode::InvalidRequest) =>
        {
            Err(SignupFailure::InvalidEmail)
        }
        429 | 503 => Err(SignupFailure::Unavailable),
        _ => Err(SignupFailure::UnknownOutcome),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn verified(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<VerificationStatus, SignupFailure> {
    json_body(content_type, body)?;
    match status {
        200 => serde_json::from_slice::<VerificationResponse>(body)
            .map(|reply| reply.status)
            .map_err(|_| SignupFailure::Protocol),
        429 | 503 => Err(SignupFailure::Unavailable),
        _ => Err(SignupFailure::UnknownOutcome),
    }
}

#[cfg(target_arch = "wasm32")]
async fn post(
    path: &'static str,
    body: String,
) -> Result<(u16, Option<String>, Vec<u8>), SignupFailure> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller = AbortController::new().map_err(|_| SignupFailure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| SignupFailure::Unavailable)?;
    let signal = deadline.controller.signal();
    let request = Request::post(path)
        .credentials(RequestCredentials::SameOrigin)
        .abort_signal(Some(&signal))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|_| SignupFailure::Unavailable)?;
    let response = request
        .send()
        .await
        .map_err(|_| SignupFailure::UnknownOutcome)?;
    let status = response.status();
    let content_type = response.headers().get("content-type");
    let bytes = response
        .binary()
        .await
        .map_err(|_| SignupFailure::UnknownOutcome)?;
    Ok((status, content_type, bytes))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn submit(state: leptos::prelude::RwSignal<crate::application::signup::SignupState>) {
    use cantos_contracts::theatre_registration::RegistrationRequest;
    use leptos::prelude::*;

    let Some((issued, email)) = state.write().begin_submit() else {
        return;
    };
    leptos::task::spawn_local(async move {
        let result = match serde_json::to_string(&RegistrationRequest { email }) {
            Ok(body) => match post("/api/v1/theatre/auth/registration", body).await {
                Ok((status, content_type, body)) => {
                    accepted(status, content_type.as_deref(), &body)
                }
                Err(error) => Err(error),
            },
            Err(_) => Err(SignupFailure::Protocol),
        };
        let _ = state.try_update(|current| {
            current.finish_submit(issued, result);
        });
    });
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn verify(state: leptos::prelude::RwSignal<crate::application::signup::SignupState>) {
    use cantos_contracts::theatre_registration::TokenRequest;
    use leptos::prelude::*;

    let Some((issued, token)) = state.write().begin_verify() else {
        return;
    };
    leptos::task::spawn_local(async move {
        let result = match serde_json::to_string(&TokenRequest {
            token: token.into_secret(),
        }) {
            Ok(body) => match post("/api/v1/theatre/auth/registration/verify", body).await {
                Ok((status, content_type, body)) => {
                    verified(status, content_type.as_deref(), &body)
                }
                Err(error) => Err(error),
            },
            Err(_) => Err(SignupFailure::Protocol),
        };
        let _ = state.try_update(|current| {
            current.finish_verify(issued, result);
        });
    });
}

/// Close local intent first. Even a failed cancellation response cannot revive a discarded link.
pub(crate) fn cancel(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    preview: bool,
) {
    cancel_local(state, preview, true);
}

/// Invalidate private inputs before leaving without waking the subtree which navigation is
/// about to dispose. A tracked reset could remount its conditional form just before disposal.
pub(crate) fn leave(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    preview: bool,
) {
    cancel_local(state, preview, false);
}

fn cancel_local(
    state: leptos::prelude::RwSignal<crate::application::signup::SignupState>,
    preview: bool,
    notify_current_page: bool,
) {
    use leptos::prelude::*;
    let token = if notify_current_page {
        state.write().clear()
    } else {
        state.update_untracked(crate::application::signup::SignupState::clear)
    };
    #[cfg(target_arch = "wasm32")]
    if !preview {
        if let Some(token) = token {
            leptos::task::spawn_local(async move {
                if let Ok(body) =
                    serde_json::to_string(&cantos_contracts::theatre_registration::TokenRequest {
                        token: token.into_secret(),
                    })
                {
                    let _ = post("/api/v1/theatre/auth/registration/cancel", body).await;
                }
            });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (token, preview);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_close_invalidates_without_notifying_the_form_which_route_will_dispose() {
        use crate::application::signup::{SignupPhase, SignupState, VerificationToken};
        use leptos::prelude::*;

        Owner::new().with(|| {
            let state = RwSignal::new(SignupState::default());
            state.update(|current| current.set_email("listener@example.invalid".to_owned()));
            let (issued, _) = state.write().begin_submit().unwrap();
            state.update(|current| {
                current.finish_submit(issued, Ok(()));
            });
            let old_route = Owner::new();
            let old_form = old_route.with(|| Memo::new(move |_| state.get().phase()));
            assert_eq!(old_form.get(), SignupPhase::EmailAccepted);

            leave(state, true);
            assert_eq!(state.get_untracked().phase(), SignupPhase::Editing);
            assert_eq!(state.get_untracked().email(), "");
            // The old conditional must stay hidden until its route owner is dropped.
            assert_eq!(old_form.get(), SignupPhase::EmailAccepted);
            old_route.cleanup();
            let new_form = Memo::new(move |_| state.get().phase());
            assert_eq!(new_form.get(), SignupPhase::Editing);

            // Same-page NewRequest is different: its reset must notify the existing form.
            state.update(|current| {
                current.capture(
                    VerificationToken::from_fragment(&format!(
                        "#register-verify?token={}",
                        "af".repeat(32)
                    ))
                    .unwrap(),
                )
            });
            assert_eq!(new_form.get(), SignupPhase::AwaitingConfirmation);
            cancel(state, true);
            assert_eq!(new_form.get(), SignupPhase::Editing);
            assert_eq!(state.write().begin_verify(), None);
            assert!(!state.write().finish_submit(issued, Ok(())));
        });
    }

    #[test]
    fn signup_receipts_require_exact_bounded_json_without_credentials_or_completion() {
        let accepted_body = br#"{"status":"accepted"}"#;
        assert_eq!(
            accepted(202, Some("application/json; charset=utf-8"), accepted_body),
            Ok(())
        );
        assert_eq!(
            verified(
                200,
                Some("application/json"),
                br#"{"status":"setup_link_sent"}"#
            ),
            Ok(VerificationStatus::SetupLinkSent)
        );
        assert_eq!(
            verified(
                200,
                Some("application/json"),
                br#"{"status":"unavailable"}"#
            ),
            Ok(VerificationStatus::Unavailable)
        );
        for body in [
            br#"{"status":"complete"}"#.as_slice(),
            br#"{"status":"accepted","actor_id":"forged"}"#,
            br#"{"status":"accepted","token":"secret"}"#,
            &[b' '; 8193],
        ] {
            assert_eq!(
                accepted(202, Some("application/json"), body),
                Err(SignupFailure::Protocol)
            );
        }
        for mime in [None, Some("text/html"), Some("application/jsonp")] {
            assert_eq!(
                accepted(202, mime, accepted_body),
                Err(SignupFailure::Protocol)
            );
        }
    }

    #[test]
    fn invalid_email_outage_and_ambiguous_writes_remain_different() {
        let invalid = br#"{"code":"invalid_request","current_revision":null,"issues":[]}"#;
        assert_eq!(
            accepted(400, Some("application/json"), invalid),
            Err(SignupFailure::InvalidEmail)
        );
        for status in [429, 503] {
            assert_eq!(
                accepted(status, Some("application/json"), invalid),
                Err(SignupFailure::Unavailable)
            );
            assert_eq!(
                verified(status, Some("application/json"), invalid),
                Err(SignupFailure::Unavailable)
            );
        }
        for status in [401, 403, 500] {
            assert_eq!(
                verified(status, Some("application/json"), invalid),
                Err(SignupFailure::UnknownOutcome)
            );
        }
    }
}
