//! Fetch effects only. Studio cookies and producer bearer requests cannot share credentials.
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
    let path =
        crate::api_endpoint::endpoint(&command.path()?).map_err(|_| ClientError::InvalidInput)?;
    let deadline = Deadline::new(controller)?;
    let signal = deadline.controller.signal();
    let mut request = Request::get(&path)
        .method(
            command
                .method()
                .parse()
                .map_err(|_| ClientError::InvalidInput)?,
        )
        .credentials(match command.credentials() {
            Credentials::SameOrigin => RequestCredentials::Include,
            Credentials::Omit => RequestCredentials::Omit,
        })
        .header("Accept", "application/json")
        .abort_signal(Some(&signal));
    let expected_fence = match command.credentials() {
        Credentials::SameOrigin => Some(session_fence.ok_or(ClientError::SessionChanged)?),
        Credentials::Omit => None,
    };
    if let Some(fence) = expected_fence {
        request = request.header(crate::api::SESSION_HEADER, &fence.header());
    }
    if let Command::ProducerStatus { credential, .. } | Command::Upload { credential, .. } = command
    {
        request = request.header("Authorization", &credential.bearer());
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
