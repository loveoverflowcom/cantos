//! Studio owner transport: the API-issued HttpOnly cookie session (`credentials: include`) plus
//! the session fence on every owner request. The cookie is never read, stored or forwarded here.
use crate::contract::protobuf::ProtobufDto;
use crate::contract::{
    AcceptAdaptationRequest, AdaptationReviewResponse, ApiError, ApproveProductionRequest,
    ErrorCode, FreezeProductionRequest, HistoryResponse, ImportRequest, ImportResponse,
    ProductionApprovalResponse, ProductionCatalogResponse, ProductionPreviewResponse,
    ProductionRightsClaimResponse, ProductionSettingsResponse, ProductionSnapshotResponse,
    ProductionStateResponse, ReviewRequest, ReviewResponse, RevisionResponse,
    SaveProductionRightsRequest, SaveProductionSettingsRequest, SaveRevisionRequest,
    ScriptValidationResponse, SessionResponse, SourceResponse, ValidateScriptRequest,
};
use gloo_net::http::{Request, Response};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{AbortController, Window};

use crate::session::{current_attempt, SessionFence};
use crate::wire::{self, REVISION_CODEC};
use leptos::prelude::{Callable, Callback};
use std::cell::Cell;

/// The fence header, from its one owner: the client that sets it and the checks that read it
/// can no longer disagree on the spelling.
pub const SESSION_HEADER: &str =
    cantos_web_auth::core::session_header(cantos_web_auth::Surface::Studio);
#[derive(Clone, Copy)]
struct ClientSession {
    fence: SessionFence,
    epoch: u64,
}
thread_local! { static CURRENT_FENCE: Cell<Option<ClientSession>> = const {Cell::new(None)}; }

const HTTP_DEADLINE_MS: i32 = 15_000;

/// Keep the callback and signal alive until headers and the complete response body settle.
/// Dropping an interrupted future also cancels its fetch and releases its browser timer.
struct RequestDeadline {
    controller: AbortController,
    window: Window,
    timer: i32,
    _callback: Closure<dyn FnMut()>,
}

impl RequestDeadline {
    fn new() -> Result<Self, ApiError> {
        let window = web_sys::window().ok_or_else(unavailable)?;
        let controller = AbortController::new().map_err(|_| unavailable())?;
        let timeout_controller = controller.clone();
        let callback =
            Closure::wrap(Box::new(move || timeout_controller.abort()) as Box<dyn FnMut()>);
        let timer = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                HTTP_DEADLINE_MS,
            )
            .map_err(|_| unavailable())?;
        Ok(Self {
            controller,
            window,
            timer,
            _callback: callback,
        })
    }
}

impl Drop for RequestDeadline {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}

#[derive(Clone, Copy, Default)]
pub struct StudioHttp {
    session: Option<ClientSession>,
    on_denial: Option<Callback<ApiError>>,
    on_admitted: Option<Callback<()>>,
}

pub struct ObservedSession {
    pub response: SessionResponse,
    pub fence: SessionFence,
}

/// Only an API-confirmed session establishes an actor and a fence; its id must be a canonical UUID.
fn observed(view: cantos_web_auth::core::SessionView) -> Result<ObservedSession, ApiError> {
    Ok(ObservedSession {
        response: SessionResponse {
            actor_id: view.actor,
        },
        fence: SessionFence::new(&view.session).ok_or_else(unauthenticated)?,
    })
}

impl StudioHttp {
    pub const fn new() -> Self {
        Self {
            session: None,
            on_denial: None,
            on_admitted: None,
        }
    }
    /// Capture synchronously before scheduling a request; a queued A attempt never adopts B.
    pub fn snapshot(self) -> Self {
        Self {
            session: self.session.or_else(|| CURRENT_FENCE.with(Cell::get)),
            ..self
        }
    }
    pub fn fence(self) -> Option<SessionFence> {
        self.session.map(|session| session.fence)
    }
    pub fn accept_fence(fence: Option<SessionFence>, epoch: u64) {
        CURRENT_FENCE
            .with(|current| current.set(fence.map(|fence| ClientSession { fence, epoch })));
    }
    pub fn with_denial(self, on_denial: Callback<ApiError>) -> Self {
        Self {
            on_denial: Some(on_denial),
            ..self
        }
    }
    /// Report each protected answer the API admitted for the current presentation, so the shell
    /// can tell a session that works from one it only keeps confirming (#566).
    pub fn with_admitted(self, on_admitted: Callback<()>) -> Self {
        Self {
            on_admitted: Some(on_admitted),
            ..self
        }
    }
    pub fn notify_denial<T>(self, result: Result<T, ApiError>) -> Result<T, ApiError> {
        if let Err(error) = &result {
            if error.code == ErrorCode::Unauthenticated {
                self.invalidate_projection(error);
            }
        }
        result
    }
    /// Whether this request still belongs to the presentation currently shown.
    fn is_current(self) -> bool {
        CURRENT_FENCE.with(|current| {
            let current = current.get();
            current_attempt(
                self.fence(),
                self.session.map_or(0, |s| s.epoch),
                current.map(|s| s.fence),
                current.map_or(0, |s| s.epoch),
            )
        })
    }
    pub(crate) fn invalidate_projection(self, error: &ApiError) {
        if self.is_current() {
            if let Some(callback) = self.on_denial {
                callback.run(error.clone());
            }
        }
    }
    fn admitted(self) {
        if self.is_current() {
            if let Some(callback) = self.on_admitted {
                callback.run(());
            }
        }
    }
    fn required_fence(self) -> Result<SessionFence, ApiError> {
        self.fence().ok_or_else(unauthenticated)
    }
}

/// The Studio projection of an identity failure (see `session::failure_error`).
pub(crate) fn auth_error(error: cantos_web_auth::Failure) -> ApiError {
    crate::session::failure_error(error)
}
impl StudioHttp {
    /// Admit the owner session for one request. A session that is gone or was switched before
    /// the request leaves is an observed denial: it re-reads the session instead of staying stale.
    pub(crate) async fn credential(self) -> Result<cantos_web_auth::browser::Lease, ApiError> {
        let fence = self.required_fence()?.header();
        match crate::api_endpoint::auth()
            .map_err(auth_error)?
            .access(Some(&fence))
            .await
        {
            Ok(lease) => Ok(lease),
            Err(
                failure @ (cantos_web_auth::Failure::SignedOut | cantos_web_auth::Failure::Changed),
            ) => {
                let error = if failure == cantos_web_auth::Failure::Changed {
                    session_changed()
                } else {
                    unauthenticated()
                };
                self.invalidate_projection(&error);
                Err(error)
            }
            Err(failure) => Err(auth_error(failure)),
        }
    }
}

pub use crate::session::is_session_changed;
use crate::session::{session_changed, unauthenticated};

/// Seconds the shared owner still holds this surface after a 429, for a view that shows a wait.
/// Nothing is dispatched before it ends: `credential()` refuses first.
pub fn rate_wait() -> Option<u32> {
    match crate::api_endpoint::auth().ok()?.gate() {
        Err(cantos_web_auth::Failure::RateLimited(seconds)) => Some(seconds),
        _ => None,
    }
}

/// The shared owner judges an owner answer before its body: 429 hold, stale lease, a real 401 and
/// an echo of another session (which drops the local session). A `Changed` 409 is the API's fence
/// refusal, so nothing was applied and it is an observed session change; any other `Changed`
/// leaves an unknown outcome. Either way the current projection re-reads its session.
fn received(
    api: StudioHttp,
    credential: &cantos_web_auth::browser::Lease,
    response: &Response,
) -> Result<(), ApiError> {
    match credential.receive(response) {
        Err(cantos_web_auth::Failure::Changed) => {
            let error = if response.status() == 409 {
                session_changed()
            } else {
                crate::session::unconfirmed_response()
            };
            api.invalidate_projection(&error);
            Err(error)
        }
        other => {
            let result = api.notify_denial(other.map_err(auth_error));
            if result.is_ok() {
                api.admitted();
            }
            result
        }
    }
}

fn check_echo(response: &Response, fence: Option<SessionFence>) -> Result<(), ApiError> {
    if response.ok()
        && fence
            .is_some_and(|fence| !fence.accepts(response.headers().get(SESSION_HEADER).as_deref()))
    {
        return Err(crate::session::unconfirmed_response());
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub struct StudioContext {
    pub api: StudioHttp,
}

fn unavailable() -> ApiError {
    ApiError {
        code: ErrorCode::Unavailable,
        current_revision: None,
        issues: vec![],
    }
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    if response.ok() {
        response.json().await.map_err(|_| unavailable())
    } else {
        let status = response.status();
        let error: ApiError = response.json().await.unwrap_or_else(|_| unavailable());
        Err(if crate::session::is_fence_mismatch(status, &error) {
            session_changed()
        } else {
            error
        })
    }
}

async fn execute<T: DeserializeOwned>(
    api: StudioHttp,
    request: Request,
    deadline: RequestDeadline,
    fence: SessionFence,
    credential: cantos_web_auth::browser::Lease,
) -> Result<T, ApiError> {
    let response = request.send().await.map_err(|_| unavailable())?;
    received(api, &credential, &response)?;
    let result = match check_echo(&response, Some(fence)) {
        Ok(()) => decode(response).await,
        Err(error) => {
            api.invalidate_projection(&error);
            Err(error)
        }
    };
    // A timeout while decoding the body remains an ambiguous write outcome. The caller
    // reconciles its immutable operation snapshot; this transport never retries a request.
    drop(deadline);
    // A 409 `unauthenticated` is a session-fence mismatch (another tab changed account): drop
    // the local session so nothing else is dispatched on it. The denial then re-reads the
    // session before any other data is accepted.
    if result.as_ref().err().is_some_and(is_session_changed) {
        let _ = credential.fence_mismatch();
        return api.notify_denial(result);
    }
    credential.check().map_err(auth_error)?;
    api.notify_denial(result)
}

async fn get<T: DeserializeOwned>(api: StudioHttp, path: &str) -> Result<T, ApiError> {
    let fence = api.required_fence()?;
    let credential = api.credential().await?;
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = credential
        .apply(
            Request::get(&crate::api_endpoint::endpoint(path).map_err(|_| unavailable())?)
                .abort_signal(Some(&signal)),
        )
        .build()
        .map_err(|_| unavailable())?;
    execute(api, request, deadline, fence, credential).await
}

async fn post<T: DeserializeOwned, P: Serialize>(
    api: StudioHttp,
    path: &str,
    body: &P,
) -> Result<T, ApiError> {
    let fence = api.required_fence()?;
    let credential = api.credential().await?;
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = credential
        .apply(
            Request::post(&crate::api_endpoint::endpoint(path).map_err(|_| unavailable())?)
                .abort_signal(Some(&signal)),
        )
        .json(body)
        .map_err(|_| unavailable())?;
    execute(api, request, deadline, fence, credential).await
}

/// The negotiated routes (head, pinned revision, save, validation) declare `REVISION_CODEC` and
/// decode each response by its own `Content-Type`; every other route keeps `get`/`post` above.
async fn negotiate<T: DeserializeOwned + ProtobufDto>(
    api: StudioHttp,
    request: Request,
    deadline: RequestDeadline,
    fence: SessionFence,
    credential: cantos_web_auth::browser::Lease,
) -> Result<T, ApiError> {
    let response = request.send().await.map_err(|_| unavailable())?;
    received(api, &credential, &response)?;
    if let Err(error) = check_echo(&response, Some(fence)) {
        api.invalidate_projection(&error);
        return Err(error);
    }
    let content_type = response.headers().get("content-type");
    let ok = response.ok();
    let status = response.status();
    // As in `execute`, a lost body after the headers is ambiguous, and nothing is retried.
    let body = response.binary().await.map_err(|_| unavailable());
    drop(deadline);
    let decoded = wire::decode_response(REVISION_CODEC, ok, content_type.as_deref(), &body?);
    if let Err(error) = &decoded {
        if crate::session::is_fence_mismatch(status, error) {
            let _ = credential.fence_mismatch();
            return api.notify_denial(Err(session_changed()));
        }
    }
    credential.check().map_err(auth_error)?;
    api.notify_denial(decoded)
}

async fn negotiated_get<T: DeserializeOwned + ProtobufDto>(
    api: StudioHttp,
    path: &str,
) -> Result<T, ApiError> {
    let fence = api.required_fence()?;
    let credential = api.credential().await?;
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = credential
        .apply(
            Request::get(&crate::api_endpoint::endpoint(path).map_err(|_| unavailable())?)
                .header("Accept", wire::accept(REVISION_CODEC))
                .abort_signal(Some(&signal)),
        )
        .build()
        .map_err(|_| unavailable())?;
    negotiate(api, request, deadline, fence, credential).await
}

async fn negotiated_post<T: DeserializeOwned + ProtobufDto, P: Serialize + ProtobufDto>(
    api: StudioHttp,
    path: &str,
    body: &P,
) -> Result<T, ApiError> {
    let fence = api.required_fence()?;
    let credential = api.credential().await?;
    let bytes = wire::encode_body(REVISION_CODEC, body)?;
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = credential
        .apply(
            Request::post(&crate::api_endpoint::endpoint(path).map_err(|_| unavailable())?)
                .header("Content-Type", REVISION_CODEC.media_type())
                .header("Accept", wire::accept(REVISION_CODEC))
                .abort_signal(Some(&signal)),
        )
        .body(js_sys::Uint8Array::from(bytes.as_slice()))
        .map_err(|_| unavailable())?;
    negotiate(api, request, deadline, fence, credential).await
}

impl StudioHttp {
    pub async fn production_catalog(&self) -> Result<ProductionCatalogResponse, ApiError> {
        get(*self, "/api/v1/studio/production/catalog").await
    }

    pub async fn production_state(
        &self,
        script: &str,
    ) -> Result<ProductionStateResponse, ApiError> {
        get(*self, &format!("{}/production", script_path(script)?)).await
    }

    pub async fn production_preview(
        &self,
        script: &str,
    ) -> Result<ProductionPreviewResponse, ApiError> {
        get(
            *self,
            &format!("{}/production/review", script_path(script)?),
        )
        .await
    }

    pub async fn save_production_settings(
        &self,
        script: &str,
        request: &SaveProductionSettingsRequest,
    ) -> Result<ProductionSettingsResponse, ApiError> {
        post(
            *self,
            &format!("{}/production/settings", script_path(script)?),
            request,
        )
        .await
    }

    pub async fn save_production_rights(
        &self,
        script: &str,
        request: &SaveProductionRightsRequest,
    ) -> Result<ProductionRightsClaimResponse, ApiError> {
        post(
            *self,
            &format!("{}/production/rights", script_path(script)?),
            request,
        )
        .await
    }

    pub async fn freeze_production(
        &self,
        script: &str,
        request: &FreezeProductionRequest,
    ) -> Result<ProductionSnapshotResponse, ApiError> {
        post(
            *self,
            &format!("{}/production/snapshots", script_path(script)?),
            request,
        )
        .await
    }

    pub async fn production_snapshot(
        &self,
        script: &str,
        snapshot: &str,
    ) -> Result<ProductionSnapshotResponse, ApiError> {
        get(*self, &production_snapshot_path(script, snapshot)?).await
    }

    pub async fn approve_production(
        &self,
        script: &str,
        snapshot: &str,
        request: &ApproveProductionRequest,
    ) -> Result<ProductionApprovalResponse, ApiError> {
        post(
            *self,
            &format!("{}/approvals", production_snapshot_path(script, snapshot)?),
            request,
        )
        .await
    }
    pub async fn validate(
        &self,
        request: &ValidateScriptRequest,
    ) -> Result<ScriptValidationResponse, ApiError> {
        negotiated_post(*self, "/api/v1/studio/validation", request).await
    }

    pub async fn history(
        &self,
        script: &str,
        after_revision: u64,
        limit: u64,
    ) -> Result<HistoryResponse, ApiError> {
        if after_revision > i64::MAX as u64 || !(1..=50).contains(&limit) {
            return Err(invalid_request());
        }
        get(
            *self,
            &format!(
                "{}/history?after_revision={after_revision}&limit={limit}",
                script_path(script)?
            ),
        )
        .await
    }

    pub async fn review(
        &self,
        script: &str,
        request: &ReviewRequest,
    ) -> Result<ReviewResponse, ApiError> {
        post(*self, &format!("{}/reviews", script_path(script)?), request).await
    }

    pub async fn read_source(
        &self,
        script: &str,
        source: &str,
    ) -> Result<SourceResponse, ApiError> {
        // Legacy evidence IDs remain valid; only import downloads require the src_ format.
        if source.is_empty()
            || source.len() > 64
            || !source.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
        {
            return Err(invalid_request());
        }
        get(*self, &format!("{}/sources/{source}", script_path(script)?)).await
    }

    pub async fn read_adaptation(&self, id: &str) -> Result<AdaptationReviewResponse, ApiError> {
        get(*self, &format!("{}/review", adaptation_path(id)?)).await
    }

    pub async fn accept_adaptation(
        &self,
        id: &str,
        request: &AcceptAdaptationRequest,
    ) -> Result<RevisionResponse, ApiError> {
        post(*self, &format!("{}/accept", adaptation_path(id)?), request).await
    }

    pub async fn read_revision(
        &self,
        script: &str,
        revision: u64,
    ) -> Result<RevisionResponse, ApiError> {
        if revision == 0 || revision > i64::MAX as u64 {
            return Err(invalid_request());
        }
        negotiated_get(
            *self,
            &format!("{}/revisions/{revision}", script_path(script)?),
        )
        .await
    }
    pub async fn import(&self, request: &ImportRequest) -> Result<ImportResponse, ApiError> {
        post(*self, "/api/v1/studio/imports", request).await
    }

    pub async fn read_import(&self, id: &str) -> Result<ImportResponse, ApiError> {
        get(*self, &import_path(id)?).await
    }

    pub async fn download_original(
        &self,
        id: &str,
    ) -> Result<(Vec<u8>, cantos_web_auth::browser::Lease), ApiError> {
        let fence = self.required_fence()?;
        let credential = self.credential().await?;
        let path = format!("{}/original", import_path(id)?);
        let deadline = RequestDeadline::new()?;
        let response = credential
            .apply(
                Request::get(&crate::api_endpoint::endpoint(&path).map_err(|_| unavailable())?)
                    .abort_signal(Some(&deadline.controller.signal())),
            )
            .send()
            .await
            .map_err(|_| unavailable())?;
        received(*self, &credential, &response)?;
        check_echo(&response, Some(fence))?;
        if !response.ok() {
            let status = response.status();
            let error: ApiError = response.json().await.unwrap_or_else(|_| unavailable());
            if crate::session::is_fence_mismatch(status, &error) {
                let _ = credential.fence_mismatch();
                return Err(session_changed());
            }
            return Err(error);
        }
        let bytes = response.binary().await.map_err(|_| unavailable())?;
        credential.check().map_err(auth_error)?;
        if bytes.len() > crate::import::MAX_FILE_BYTES {
            return Err(unavailable());
        }
        Ok((bytes, credential))
    }
    /// Authoritative read of the cookie session (`GET identity/session`). `Ok(None)` is a
    /// genuine 401; every other failure is an unknown state, never "signed out". A pending
    /// logout cleanup is retried first so a failed revoke is never silently forgotten.
    pub async fn current_session(&self) -> Result<Option<ObservedSession>, ApiError> {
        let client = crate::api_endpoint::auth().map_err(auth_error)?;
        client.cleanup().await.map_err(auth_error)?;
        client
            .read_session()
            .await
            .map_err(auth_error)?
            .map(observed)
            .transpose()
    }
    /// Email/password sign-in as one typed intent. The login is confirmed by an authoritative
    /// read: a reply whose cookie the browser did not keep, or another session than the one
    /// just issued (another tab won the race), is never admitted.
    pub async fn sign_in(
        &self,
        intent: crate::signin::SignInIntent,
    ) -> Result<ObservedSession, cantos_web_auth::Failure> {
        use cantos_web_auth::Failure;
        let client = crate::api_endpoint::auth()?;
        let crate::signin::SignInIntent {
            email,
            password,
            expected_actor,
        } = intent;
        let issued = client.login(email, password, expected_actor).await?;
        let view = client.read_session().await?.ok_or(Failure::SignedOut)?;
        if view.session != issued.session || view.actor != issued.actor {
            return Err(Failure::Changed);
        }
        observed(view).map_err(|_| Failure::Protocol)
    }
    pub async fn logout(&self) -> Result<(), ApiError> {
        let expected = self.fence().map(|f| f.header());
        crate::api_endpoint::auth()
            .map_err(auth_error)?
            .logout(expected.as_deref())
            .await
            .map_err(auth_error)
    }
    pub fn begin_logout(&self) -> Result<(), ApiError> {
        let expected = self.fence().map(|f| f.header());
        crate::api_endpoint::auth()
            .map_err(auth_error)?
            .begin_logout(expected.as_deref())
            .map_err(auth_error)
    }
    pub async fn read(&self, script: &str) -> Result<RevisionResponse, ApiError> {
        negotiated_get(*self, &format!("{}/head", script_path(script)?)).await
    }
    pub async fn save(
        &self,
        script: &str,
        request: &SaveRevisionRequest,
    ) -> Result<RevisionResponse, ApiError> {
        negotiated_post(
            *self,
            &format!("{}/revisions", script_path(script)?),
            request,
        )
        .await
    }
}

fn production_snapshot_path(script: &str, snapshot: &str) -> Result<String, ApiError> {
    if !crate::adaptation::is_run_id(snapshot) {
        return Err(invalid_request());
    }
    Ok(format!(
        "{}/production/snapshots/{snapshot}",
        script_path(script)?
    ))
}

fn invalid_request() -> ApiError {
    ApiError {
        code: ErrorCode::InvalidRequest,
        current_revision: None,
        issues: vec![],
    }
}

fn script_path(id: &str) -> Result<String, ApiError> {
    // Paths only accept canonical UUIDs; arbitrary user input is never interpreted as a URL.
    if !crate::adaptation::is_run_id(id) {
        return Err(invalid_request());
    }
    Ok(format!("/api/v1/studio/scripts/{id}"))
}

fn adaptation_path(id: &str) -> Result<String, ApiError> {
    if !crate::adaptation::is_run_id(id) {
        return Err(ApiError {
            code: ErrorCode::InvalidRequest,
            current_revision: None,
            issues: vec![],
        });
    }
    Ok(format!("/api/v1/studio/adaptations/{id}"))
}

fn import_path(id: &str) -> Result<String, ApiError> {
    if !crate::import::is_import_source_id(id) {
        return Err(ApiError {
            code: ErrorCode::InvalidRequest,
            current_revision: None,
            issues: vec![],
        });
    }
    Ok(format!("/api/v1/studio/imports/{id}"))
}
