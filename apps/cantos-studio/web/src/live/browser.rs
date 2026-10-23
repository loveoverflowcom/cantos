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
        .header(
            "Accept",
            if matches!(command, Command::PreviewAudio { .. }) {
                "audio/ogg, application/json"
            } else {
                "application/json"
            },
        )
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
        credential.receive(&response).map_err(auth_error)?;
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
    if let Command::PreviewAudio { subject } = command {
        if response.ok() {
            let result = preview_response(&response, subject).await;
            drop(deadline);
            if let Some(credential) = &credential {
                credential.check().map_err(auth_error)?;
            }
            return result;
        }
    }
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
    let hash = sha256(&bytes).await?;
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

/// Identity failures as a command outcome (see `ClientError::from_identity`).
pub(crate) fn auth_error(error: cantos_web_auth::Failure) -> ClientError {
    ClientError::from_identity(error)
}

async fn sha256(bytes: &[u8]) -> Result<String, ClientError> {
    let crypto = web_sys::window()
        .ok_or(ClientError::Transport)?
        .crypto()
        .map_err(|_| ClientError::Transport)?;
    let promise = crypto
        .subtle()
        .digest_with_str_and_u8_array("SHA-256", bytes)
        .map_err(|_| ClientError::Transport)?;
    let digest = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| ClientError::Transport)?;
    Ok(js_sys::Uint8Array::new(&digest)
        .to_vec()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

// ReadableStream is already provided by gloo-net. Fallible calls to the standard reader API
// avoid adding a new web-sys feature to the shared Cargo graph for this bounded owner read.
fn reader_call(
    reader: &wasm_bindgen::JsValue,
    method: &str,
) -> Result<wasm_bindgen::JsValue, ClientError> {
    let function = js_sys::Reflect::get(reader, &wasm_bindgen::JsValue::from_str(method))
        .map_err(|_| ClientError::Transport)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| ClientError::Transport)?;
    function.call0(reader).map_err(|_| ClientError::Transport)
}
struct AudioReader {
    reader: js_sys::Object,
    finished: bool,
}
impl Drop for AudioReader {
    fn drop(&mut self) {
        if !self.finished {
            let _ = reader_call(self.reader.as_ref(), "cancel");
        }
        let _ = reader_call(self.reader.as_ref(), "releaseLock");
    }
}
async fn preview_response(
    response: &gloo_net::http::Response,
    subject: &audio_preview::Subject,
) -> Result<Reply, ClientError> {
    let status = response.status();
    if status != 200
        || !subject.valid()
        || response.headers().get("content-type").as_deref() != Some("audio/ogg")
        || response
            .headers()
            .get("content-length")
            .is_some_and(|length| length.parse::<u64>().ok() != Some(subject.byte_length))
    {
        return Err(ClientError::Protocol { status });
    }
    let stream = response.body().ok_or(ClientError::Protocol { status })?;
    let mut reader = AudioReader {
        reader: stream.get_reader(),
        finished: false,
    };
    let mut bytes = Vec::with_capacity(subject.byte_length as usize);
    loop {
        let promise = reader_call(reader.reader.as_ref(), "read")?
            .dyn_into::<js_sys::Promise>()
            .map_err(|_| ClientError::Transport)?;
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|_| ClientError::Transport)?;
        let done = js_sys::Reflect::get(&result, &wasm_bindgen::JsValue::from_str("done"))
            .map_err(|_| ClientError::Transport)?
            .as_bool()
            .ok_or(ClientError::Protocol { status })?;
        if done {
            reader.finished = true;
            break;
        }
        let chunk = js_sys::Reflect::get(&result, &wasm_bindgen::JsValue::from_str("value"))
            .map_err(|_| ClientError::Transport)?
            .dyn_into::<js_sys::Uint8Array>()
            .map_err(|_| ClientError::Protocol { status })?;
        if !subject.accepts_chunk(bytes.len(), chunk.length() as usize) {
            return Err(ClientError::ReplyMismatch);
        }
        bytes.extend_from_slice(&chunk.to_vec());
    }
    let observed = sha256(&bytes).await?;
    audio_preview::VerifiedAudio::admit(subject.clone(), std::sync::Arc::new(bytes), &observed)
        .map(Reply::AudioPreview)
}
