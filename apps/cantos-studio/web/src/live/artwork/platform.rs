//! Browser I/O only: original file bytes, transient object URLs, fenced private reads and XHR progress.
//! Every owner request here carries the HttpOnly cookie session and the session fence, and only
//! ever to this surface of the API origin (`credentialed_url`).
use super::*;
use crate::{
    live::{ClientError, Command, Reply},
    session::SessionFence,
};
use leptos::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, Event, ProgressEvent, XmlHttpRequest};

/// The one gate for sending the ambient cookie from this module: the configured API origin's
/// Studio surface, never an arbitrary or foreign URL.
fn credentialed_url(path: &str) -> Result<String, ClientError> {
    let path = crate::api_endpoint::endpoint(path).map_err(|_| ClientError::InvalidInput)?;
    let origin = web_sys::window()
        .ok_or(ClientError::Transport)?
        .location()
        .origin()
        .map_err(|_| ClientError::Transport)?;
    cantos_web_auth::core::credentialed_target(
        &path,
        &origin,
        crate::api_endpoint::api_origin(),
        cantos_web_auth::Surface::Studio,
    )
    .map_err(|_| ClientError::InvalidInput)
}

pub fn selected(event: Event) -> Option<web_sys::File> {
    event
        .target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()?
        .files()?
        .get(0)
}
pub fn reset_file() {
    if let Some(input) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("artwork-file"))
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        input.set_value("");
    }
}
pub fn revoke(url: &str) {
    let _ = web_sys::Url::revoke_object_url(url);
}
pub fn focus_status() {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("artwork-status"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    });
}
async fn hash(bytes: &[u8]) -> Result<String, ClientError> {
    let crypto = web_sys::window()
        .ok_or(ClientError::Transport)?
        .crypto()
        .map_err(|_| ClientError::Transport)?;
    let promise = crypto
        .subtle()
        .digest_with_str_and_u8_array("SHA-256", bytes)
        .map_err(|_| ClientError::Transport)?;
    let raw = JsFuture::from(promise)
        .await
        .map_err(|_| ClientError::Transport)?;
    Ok(js_sys::Uint8Array::new(&raw)
        .to_vec()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub async fn read(file: web_sys::File) -> Result<LocalImage, Problem> {
    let mime = file_bounds(&file.type_(), file.size() as u64)?;
    let url = web_sys::Url::create_object_url_with_blob(&file).map_err(|_| Problem::Decode)?;
    let result = async {
        let image = web_sys::HtmlImageElement::new().map_err(|_| Problem::Decode)?;
        image.set_src(&url);
        JsFuture::from(image.decode())
            .await
            .map_err(|_| Problem::Decode)?;
        let (width, height) = (image.natural_width(), image.natural_height());
        dimensions(width, height)?;
        let buffer = JsFuture::from(file.array_buffer())
            .await
            .map_err(|_| Problem::Decode)?;
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        file_bounds(&file.type_(), bytes.len() as u64)?;
        let sha256 = hash(&bytes).await.map_err(|_| Problem::Decode)?;
        Ok(LocalImage {
            name: file.name(),
            bytes: Arc::new(bytes),
            sha256,
            mime,
            width,
            height,
            url: url.clone(),
        })
    }
    .await;
    if result.is_err() {
        revoke(&url);
    }
    result
}

#[derive(Clone, Copy, Debug)]
pub struct Transfer {
    pub bytes: u64,
    pub complete: bool,
}

/// Closures live until completion/cancellation; dropping this guard detaches every listener.
struct Upload {
    xhr: XmlHttpRequest,
    signal: web_sys::AbortSignal,
    _done: Closure<dyn FnMut(Event)>,
    _failed: Closure<dyn FnMut(Event)>,
    _progress: Closure<dyn FnMut(ProgressEvent)>,
    _abort: Closure<dyn FnMut(Event)>,
}
impl Drop for Upload {
    fn drop(&mut self) {
        self.xhr.set_onload(None);
        self.xhr.set_onerror(None);
        self.xhr.set_onabort(None);
        self.xhr.set_ontimeout(None);
        if let Ok(upload) = self.xhr.upload() {
            upload.set_onprogress(None);
        }
        self.signal.set_onabort(None);
        let _ = self.xhr.abort();
    }
}
pub async fn upload(
    command: &Command,
    controller: AbortController,
    fence: Option<SessionFence>,
    progress: Option<Callback<Transfer>>,
) -> Result<Reply, ClientError> {
    let Command::UploadImage { intent, bytes, .. } = command else {
        return Err(ClientError::InvalidInput);
    };
    let fence = fence.ok_or(ClientError::SessionChanged)?;
    let credential = crate::api_endpoint::auth()
        .map_err(|_| ClientError::Transport)?
        .access(Some(&fence.header()))
        .await
        .map_err(crate::live::browser::auth_error)?;
    let target = credentialed_url(&command.path()?)?;
    let Ok(xhr) = XmlHttpRequest::new() else {
        // XHR unavailable: the fetch fallback has the same credential policy, minus progress.
        return upload_fetch(command, controller, fence, credential, progress).await;
    };
    xhr.open_with_async("PUT", &target, true)
        .map_err(|_| ClientError::Transport)?;
    xhr.set_timeout(60_000);
    // Ambient cookie for the API origin only: `target` was validated above. The browser adds
    // Origin itself; the session id is a nonsecret fence, never a credential.
    xhr.set_with_credentials(true);
    xhr.set_response_type(web_sys::XmlHttpRequestResponseType::Arraybuffer);
    let (fence_name, fence_value) = credential.fence();
    for (name, value) in [
        ("Accept", "application/json"),
        ("Content-Type", mime_name(intent.request.content_type)),
        (fence_name, fence_value),
    ] {
        xhr.set_request_header(name, value)
            .map_err(|_| ClientError::Transport)?;
    }
    let mut resolve_done = None;
    let mut resolve_failed = None;
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        resolve_done = Some(resolve.clone());
        resolve_failed = Some(resolve);
    });
    let done = Closure::wrap(Box::new(move |_: Event| {
        if let Some(resolve) = &resolve_done {
            let _ = resolve.call1(&JsValue::NULL, &JsValue::TRUE);
        }
    }) as Box<dyn FnMut(Event)>);
    let failed = Closure::wrap(Box::new(move |_: Event| {
        if let Some(resolve) = &resolve_failed {
            let _ = resolve.call1(&JsValue::NULL, &JsValue::FALSE);
        }
    }) as Box<dyn FnMut(Event)>);
    let total = bytes.len() as u64;
    let progress_credential = credential.clone();
    let transferred = Closure::wrap(Box::new(move |e: ProgressEvent| {
        if progress_credential.check().is_err() {
            return;
        }
        if e.length_computable() && e.total() as u64 == total {
            if let Some(callback) = progress {
                callback.run(Transfer {
                    bytes: e.loaded() as u64,
                    complete: e.loaded() >= e.total(),
                });
            }
        }
    }) as Box<dyn FnMut(ProgressEvent)>);
    let abort_xhr = xhr.clone();
    let abort = Closure::wrap(Box::new(move |_: Event| {
        let _ = abort_xhr.abort();
    }) as Box<dyn FnMut(Event)>);
    let signal = controller.signal();
    signal.set_onabort(Some(abort.as_ref().unchecked_ref()));
    xhr.set_onload(Some(done.as_ref().unchecked_ref()));
    xhr.set_onerror(Some(failed.as_ref().unchecked_ref()));
    xhr.set_onabort(Some(failed.as_ref().unchecked_ref()));
    xhr.set_ontimeout(Some(failed.as_ref().unchecked_ref()));
    xhr.upload()
        .map_err(|_| ClientError::Transport)?
        .set_onprogress(Some(transferred.as_ref().unchecked_ref()));
    let guard = Upload {
        xhr,
        signal,
        _done: done,
        _failed: failed,
        _progress: transferred,
        _abort: abort,
    };
    if guard.signal.aborted() {
        return Err(ClientError::Cancelled);
    }
    guard
        .xhr
        .send_with_opt_u8_array(Some(bytes.as_slice()))
        .map_err(|_| ClientError::Transport)?;
    let success = JsFuture::from(promise)
        .await
        .map_err(|_| ClientError::Transport)?;
    if success != JsValue::TRUE {
        return Err(ClientError::Transport);
    }
    if guard.xhr.response_url() != target {
        return Err(ClientError::ReplyMismatch);
    }
    let status = guard.xhr.status().map_err(|_| ClientError::Transport)?;
    let header = |name: &str| guard.xhr.get_response_header(name).ok().flatten();
    credential
        .response(
            status,
            header("Retry-After").as_deref(),
            header(crate::api::SESSION_HEADER).as_deref(),
        )
        .map_err(crate::live::browser::auth_error)?;
    if (200..300).contains(&status)
        && !fence.accepts(
            guard
                .xhr
                .get_response_header(crate::api::SESSION_HEADER)
                .ok()
                .flatten()
                .as_deref(),
        )
    {
        return Err(ClientError::SessionChanged);
    }
    if !guard
        .xhr
        .get_response_header("Content-Type")
        .ok()
        .flatten()
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(ClientError::Protocol { status });
    }
    let response = guard.xhr.response().map_err(|_| ClientError::Transport)?;
    let result = command.decode(status, &js_sys::Uint8Array::new(&response).to_vec());
    settle(&credential, result)
}

/// A 409 `unauthenticated` is a session-fence mismatch (another tab changed account): drop the
/// local session so nothing else is dispatched on it. Otherwise the lease must still be current.
fn settle(
    credential: &cantos_web_auth::browser::Lease,
    result: Result<Reply, ClientError>,
) -> Result<Reply, ClientError> {
    if result
        .as_ref()
        .err()
        .is_some_and(ClientError::is_session_fence_mismatch)
    {
        let _ = credential.fence_mismatch();
        return result;
    }
    credential
        .check()
        .map_err(crate::live::browser::auth_error)?;
    result
}

/// Fetch fallback with the same credential policy as the XHR path (`Lease::apply`: cookie,
/// session fence, no redirects). It reports no invented upload percentage and only acknowledges
/// verified response bytes.
async fn upload_fetch(
    command: &Command,
    controller: AbortController,
    fence: SessionFence,
    credential: cantos_web_auth::browser::Lease,
    progress: Option<Callback<Transfer>>,
) -> Result<Reply, ClientError> {
    let Command::UploadImage { intent, bytes, .. } = command else {
        return Err(ClientError::InvalidInput);
    };
    let target = credentialed_url(&command.path()?)?;
    let _deadline = crate::live::browser::Deadline::new(controller.clone())?;
    let response = credential
        .apply(
            gloo_net::http::Request::put(&target)
                .header("Accept", "application/json")
                .header("Content-Type", mime_name(intent.request.content_type))
                .abort_signal(Some(&controller.signal())),
        )
        .body(js_sys::Uint8Array::from(bytes.as_slice()))
        .map_err(|_| ClientError::Transport)?
        .send()
        .await
        .map_err(|_| ClientError::Transport)?;
    credential
        .receive(&response)
        .map_err(crate::live::browser::auth_error)?;
    let status = response.status();
    if response.ok()
        && !fence.accepts(
            response
                .headers()
                .get(crate::api::SESSION_HEADER)
                .as_deref(),
        )
    {
        return Err(ClientError::SessionChanged);
    }
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
    let result = settle(&credential, command.decode(status, &body));
    if result.is_ok() {
        if let Some(progress) = progress {
            progress.run(Transfer {
                bytes: bytes.len() as u64,
                complete: true,
            });
        }
    }
    result
}

/// Fetch exact asset + bytes using the current Studio fence. Never put a session in an image URL.
pub async fn preview(
    work: &str,
    selection: &WorkArtworkSelection,
    controller: AbortController,
    fence: SessionFence,
) -> Result<(String, ImageVariantMetadata), ClientError> {
    use gloo_net::http::Request;
    if !crate::adaptation::is_run_id(work)
        || !crate::adaptation::is_run_id(&selection.image_asset_id)
    {
        return Err(ClientError::InvalidInput);
    }
    let credential = crate::api_endpoint::auth()
        .map_err(|_| ClientError::Transport)?
        .access(Some(&fence.header()))
        .await
        .map_err(crate::live::browser::auth_error)?;
    let _deadline = crate::live::browser::Deadline::new(controller.clone())?;
    let base = credentialed_url(&format!(
        "{}/works/{work}/image-assets/{}",
        routes::STUDIO_BASE,
        selection.image_asset_id
    ))?;
    let get = |url: &str, accept: &str| {
        credential.apply(
            Request::get(url)
                .header("Accept", accept)
                .abort_signal(Some(&controller.signal())),
        )
    };
    let response = get(&base, "application/json")
        .send()
        .await
        .map_err(|_| ClientError::Transport)?;
    let status = response.status();
    credential
        .receive(&response)
        .map_err(crate::live::browser::auth_error)?;
    if response.ok()
        && !fence.accepts(
            response
                .headers()
                .get(crate::api::SESSION_HEADER)
                .as_deref(),
        )
    {
        return Err(ClientError::SessionChanged);
    }
    if !response.ok() {
        let error = ClientError::http(
            status,
            &response
                .binary()
                .await
                .map_err(|_| ClientError::Transport)?,
        );
        if error.is_session_fence_mismatch() {
            let _ = credential.fence_mismatch();
        }
        return Err(error);
    }
    if status != 200
        || !response.headers().get("content-type").is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(ClientError::Protocol { status });
    }
    let asset: ImageAssetDocument = response
        .json()
        .await
        .map_err(|_| ClientError::Protocol { status })?;
    credential
        .check()
        .map_err(crate::live::browser::auth_error)?;
    if asset.work_id != work
        || asset.id != selection.image_asset_id
        || asset.digest != selection.image_asset_digest
    {
        return Err(ClientError::ReplyMismatch);
    }
    let response = get(
        &format!("{base}/bytes"),
        mime_name(asset.verified_media.content_type),
    )
    .send()
    .await
    .map_err(|_| ClientError::Transport)?;
    let status = response.status();
    credential
        .receive(&response)
        .map_err(crate::live::browser::auth_error)?;
    if response.ok()
        && !fence.accepts(
            response
                .headers()
                .get(crate::api::SESSION_HEADER)
                .as_deref(),
        )
    {
        return Err(ClientError::SessionChanged);
    }
    if !response.ok() {
        let error = ClientError::http(
            status,
            &response
                .binary()
                .await
                .map_err(|_| ClientError::Transport)?,
        );
        if error.is_session_fence_mismatch() {
            let _ = credential.fence_mismatch();
        }
        return Err(error);
    }
    if status != 200 {
        return Err(ClientError::Protocol { status });
    }
    if response.headers().get("content-type").as_deref()
        != Some(mime_name(asset.verified_media.content_type))
    {
        return Err(ClientError::ReplyMismatch);
    }
    let bytes = response
        .binary()
        .await
        .map_err(|_| ClientError::Transport)?;
    if bytes.len() as u64 != asset.verified_media.byte_length
        || hash(&bytes).await? != asset.verified_media.sha256
    {
        return Err(ClientError::ReplyMismatch);
    }
    credential
        .check()
        .map_err(crate::live::browser::auth_error)?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime_name(asset.verified_media.content_type));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|_| ClientError::Transport)?;
    let url =
        web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| ClientError::Transport)?;
    Ok((url, asset.verified_media))
}
