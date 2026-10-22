//! Fetch effects only. Owner requests ride the HttpOnly cookie session (`credentials: include` plus
//! the session fence); producer capability requests carry only their own Bearer with credentials
//! omitted. The two never mix on one request.
use super::*;
use gloo_net::http::Request;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{AbortController, RequestCredentials, Window};

pub(crate) struct Deadline {
    window: Window,
    timer: i32,
    controller: AbortController,
    _callback: Closure<dyn FnMut()>,
}
impl Drop for Deadline {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}
impl Deadline {
    pub(crate) fn new(controller: AbortController) -> Result<Self, ClientError> {
        let window = web_sys::window().ok_or(ClientError::Transport)?;
        let abort = controller.clone();
        let callback = Closure::wrap(Box::new(move || abort.abort()) as Box<dyn FnMut()>);
        let timer = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                60_000,
            )
            .map_err(|_| ClientError::Transport)?;
        Ok(Self {
            window,
            timer,
            controller,
            _callback: callback,
        })
    }
}
pub async fn execute(
    command: &Command,
    controller: AbortController,
    session_fence: Option<crate::session::SessionFence>,
    progress: Option<leptos::prelude::Callback<artwork::platform::Transfer>>,
) -> Result<Reply, ClientError> {
    if matches!(command, Command::UploadImage { .. }) {
        return artwork::platform::upload(command, controller, session_fence, progress).await;
    }
    let owner = command.credentials().includes_cookie();
    let credential = if owner {
        let expected = session_fence.ok_or(ClientError::SessionChanged)?.header();
        Some(
            crate::api_endpoint::auth()
                .map_err(|_| ClientError::Transport)?
                .access(Some(&expected))
                .await
                .map_err(auth_error)?,
        )
    } else {
        None
    };
    let path =
        crate::api_endpoint::endpoint(&command.path()?).map_err(|_| ClientError::InvalidInput)?;
    let deadline = Deadline::new(controller)?;
    let signal = deadline.controller.signal();
    let request = Request::get(&path)
        .method(
            command
                .method()
                .parse()
                .map_err(|_| ClientError::InvalidInput)?,
        )
        .header("Accept", "application/json")
        .abort_signal(Some(&signal));
    let mut request = match &credential {
        Some(credential) => credential.apply(request),
        None => request
            .credentials(RequestCredentials::Omit)
            .redirect(web_sys::RequestRedirect::Error),
    };
    let expected_fence = owner
        .then(|| session_fence.ok_or(ClientError::SessionChanged))
        .transpose()?;
    if let Some(producer) = command.producer_credential() {
        request = request.header("Authorization", &producer.bearer());
    }
    let request = match command {
        Command::Upload { bytes, .. } => request
            .header("Content-Type", "application/octet-stream")
            .body(js_sys::Uint8Array::from(bytes.as_slice())),
        _ => match command.json()? {
            Some(body) => request
                .header("Content-Type", "application/json")
                .body(body),
            None => request.build(),
        },
    }
    .map_err(|_| ClientError::Transport)?;
    let response = request.send().await.map_err(|_| ClientError::Transport)?;
    if let Some(credential) = &credential {
        credential.response(response.status()).map_err(auth_error)?;
    }
    if response.ok()
        && expected_fence.is_some_and(|fence| {
            !fence.accepts(
                response
                    .headers()
                    .get(crate::api::SESSION_HEADER)
                    .as_deref(),
            )
        })
    {
        return Err(ClientError::SessionChanged);
    }
    let status = response.status();
    if status == 204 && command.expects_no_content() {
        return Ok(Reply::Revoked);
    }
    // A successful status with a proxy/HTML/malformed body never acknowledges a mutation.
    if !response.headers().get("content-type").is_some_and(|v| {
        v.split(';')
            .next()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(ClientError::Protocol { status });
    }
    let body = response
        .binary()
        .await
        .map_err(|_| ClientError::Transport)?;
    let result = command.decode(status, &body);
    drop(deadline);
    if let Some(credential) = &credential {
        // A 409 `unauthenticated` is a fence mismatch (another tab changed account): drop the
        // local session so nothing else is dispatched on it, and report the unknown outcome.
        if result
            .as_ref()
            .err()
            .is_some_and(ClientError::is_session_fence_mismatch)
        {
            let _ = credential.fence_mismatch();
            return result;
        }
        credential.check().map_err(auth_error)?;
    }
    result
}

pub async fn read_audio(
    file: web_sys::File,
) -> Result<(std::sync::Arc<Vec<u8>>, String), ClientError> {
    admit_audio_size(file.size() as u64)?;
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| ClientError::InvalidInput)?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    admit_audio_size(bytes.len() as u64)?;
    let crypto = web_sys::window()
        .ok_or(ClientError::Transport)?
        .crypto()
        .map_err(|_| ClientError::Transport)?;
    let digest = crypto
        .subtle()
        .digest_with_str_and_u8_array("SHA-256", &bytes)
        .map_err(|_| ClientError::Transport)?;
    let digest = wasm_bindgen_futures::JsFuture::from(digest)
        .await
        .map_err(|_| ClientError::Transport)?;
    let hash = js_sys::Uint8Array::new(&digest)
        .to_vec()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((std::sync::Arc::new(bytes), hash))
}
pub fn upload_token() -> Result<String, ClientError> {
    let crypto = web_sys::window()
        .ok_or(ClientError::Transport)?
        .crypto()
        .map_err(|_| ClientError::Transport)?;
    let mut bytes = [0u8; 32];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| ClientError::Transport)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Identity failures as a command outcome. A request that never left (or whose session moved)
/// is `Transport`/`SessionChanged`, both unknown outcomes for a write; only a real 401/403
/// carries an API error. Nothing here retries.
pub(crate) fn auth_error(error: cantos_web_auth::Failure) -> ClientError {
    use cantos_web_auth::Failure;
    match error {
        Failure::SignedOut | Failure::InvalidCredentials => ClientError::Http {
            status: 401,
            error: crate::api::auth_error(error),
        },
        Failure::Forbidden => ClientError::Http {
            status: 403,
            error: crate::api::auth_error(error),
        },
        Failure::Changed | Failure::AccountMismatch => ClientError::SessionChanged,
        Failure::NotFound
        | Failure::Invalid
        | Failure::RateLimited(_)
        | Failure::Unavailable
        | Failure::Protocol
        | Failure::CleanupPending => ClientError::Transport,
    }
}
