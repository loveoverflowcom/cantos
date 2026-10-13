//! Same-origin HTTP transport. Session credential becomes an HttpOnly cookie.
use cantos_contracts::protobuf::ProtobufDto;
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationReviewResponse, ApiError, ApproveProductionRequest,
    ErrorCode, FreezeProductionRequest, HistoryResponse, ImportRequest, ImportResponse,
    ProductionApprovalResponse, ProductionCatalogResponse, ProductionPreviewResponse,
    ProductionRightsClaimResponse, ProductionSettingsResponse, ProductionSnapshotResponse,
    ProductionStateResponse, ReviewRequest, ReviewResponse, RevisionResponse,
    SaveProductionRightsRequest, SaveProductionSettingsRequest, SaveRevisionRequest,
    ScriptValidationResponse, SessionRequest, SessionResponse, SourceResponse,
    StudioAuthenticationConfiguration, ValidateScriptRequest,
};
use gloo_net::http::{Request, Response};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{AbortController, Window};

use crate::session::{current_attempt, SessionFence};
use crate::wire::{self, REVISION_CODEC};
use leptos::prelude::Callback;
use std::cell::Cell;

pub const SESSION_HEADER: &str = "x-cantos-studio-session";
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

#[derive(Clone, Copy)]
pub struct StudioHttp {
    session: Option<ClientSession>,
    on_denial: Option<Callback<ApiError>>,
}

pub struct ObservedSession {
    pub response: SessionResponse,
    pub fence: SessionFence,
}

impl StudioHttp {
    pub const fn new() -> Self {
        Self {
            session: None,
            on_denial: None,
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
    pub fn notify_denial<T>(self, result: Result<T, ApiError>) -> Result<T, ApiError> {
        if let Err(error) = &result {
            if error.code == ErrorCode::Unauthenticated {
                self.invalidate_projection(error);
            }
        }
        result
    }
    pub(crate) fn invalidate_projection(self, error: &ApiError) {
        let matches = CURRENT_FENCE.with(|current| {
            let current = current.get();
            current_attempt(
                self.fence(),
                self.session.map_or(0, |s| s.epoch),
                current.map(|s| s.fence),
                current.map_or(0, |s| s.epoch),
            )
        });
        if matches {
            if let Some(callback) = self.on_denial {
                callback.run(error.clone());
            }
        }
    }
    fn required_fence(self) -> Result<SessionFence, ApiError> {
        self.fence().ok_or_else(unauthenticated)
    }
}

fn unauthenticated() -> ApiError {
    ApiError {
        code: ErrorCode::Unauthenticated,
        current_revision: None,
        issues: vec![],
    }
}

fn session_changed() -> ApiError {
    let mut error = unauthenticated();
    error.issues.push(cantos_contracts::FieldIssue {
        path: "session".into(),
        rule: "session_changed".into(),
    });
    error
}
pub fn is_session_changed(error: &ApiError) -> bool {
    error.code == ErrorCode::Unauthenticated
        && error
            .issues
            .iter()
            .any(|issue| issue.path == "session" && issue.rule == "session_changed")
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
        Err(
            if status == 409 && error.code == ErrorCode::Unauthenticated {
                session_changed()
            } else {
                error
            },
        )
    }
}

async fn execute<T: DeserializeOwned>(
    api: StudioHttp,
    request: Request,
    deadline: RequestDeadline,
    fence: Option<SessionFence>,
) -> Result<T, ApiError> {
    let response = request.send().await.map_err(|_| unavailable())?;
    let result = match check_echo(&response, fence) {
        Ok(()) => decode(response).await,
        Err(error) => {
            api.invalidate_projection(&error);
            Err(error)
        }
    };
    // A timeout while decoding the body remains an ambiguous write outcome. The caller
    // reconciles its immutable operation snapshot; this transport never retries a request.
    drop(deadline);
    api.notify_denial(result)
}

async fn get<T: DeserializeOwned>(api: StudioHttp, path: &str) -> Result<T, ApiError> {
    let fence = if path == "/api/v1/studio/auth/configuration" {
        None
    } else {
        Some(api.required_fence()?)
    };
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let mut request = Request::get(path).abort_signal(Some(&signal));
    if let Some(fence) = fence {
        request = request.header(SESSION_HEADER, &fence.header());
    }
    let request = request.build().map_err(|_| unavailable())?;
    execute(api, request, deadline, fence).await
}

async fn post<T: DeserializeOwned, P: Serialize>(
    api: StudioHttp,
    path: &str,
    body: &P,
) -> Result<T, ApiError> {
    let fence = Some(api.required_fence()?);
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = Request::post(path)
        .header(SESSION_HEADER, &api.required_fence()?.header())
        .abort_signal(Some(&signal))
        .json(body)
        .map_err(|_| unavailable())?;
    execute(api, request, deadline, fence).await
}

/// The negotiated routes (head, pinned revision, save, validation) declare `REVISION_CODEC` and
/// decode each response by its own `Content-Type`; every other route keeps `get`/`post` above.
async fn negotiate<T: DeserializeOwned + ProtobufDto>(
    api: StudioHttp,
    request: Request,
    deadline: RequestDeadline,
    fence: SessionFence,
) -> Result<T, ApiError> {
    let response = request.send().await.map_err(|_| unavailable())?;
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
    api.notify_denial(
        wire::decode_response(REVISION_CODEC, ok, content_type.as_deref(), &body?).map_err(
            |error| {
                if status == 409 && error.code == ErrorCode::Unauthenticated {
                    session_changed()
                } else {
                    error
                }
            },
        ),
    )
}

async fn negotiated_get<T: DeserializeOwned + ProtobufDto>(
    api: StudioHttp,
    path: &str,
) -> Result<T, ApiError> {
    let fence = api.required_fence()?;
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = Request::get(path)
        .header(SESSION_HEADER, &fence.header())
        .header("Accept", wire::accept(REVISION_CODEC))
        .abort_signal(Some(&signal))
        .build()
        .map_err(|_| unavailable())?;
    negotiate(api, request, deadline, fence).await
}

async fn negotiated_post<T: DeserializeOwned + ProtobufDto, P: Serialize + ProtobufDto>(
    api: StudioHttp,
    path: &str,
    body: &P,
) -> Result<T, ApiError> {
    let fence = api.required_fence()?;
    let bytes = wire::encode_body(REVISION_CODEC, body)?;
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = Request::post(path)
        .header(SESSION_HEADER, &fence.header())
        .header("Content-Type", REVISION_CODEC.media_type())
        .header("Accept", wire::accept(REVISION_CODEC))
        .abort_signal(Some(&signal))
        .body(js_sys::Uint8Array::from(bytes.as_slice()))
        .map_err(|_| unavailable())?;
    negotiate(api, request, deadline, fence).await
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

    pub fn original_import_path(&self, id: &str) -> Result<String, ApiError> {
        Ok(format!(
            "{}/original?studio_session_id={}",
            import_path(id)?,
            self.required_fence()?.header()
        ))
    }
    pub async fn login(&self, token: String) -> Result<ObservedSession, ApiError> {
        let deadline = RequestDeadline::new()?;
        let signal = deadline.controller.signal();
        let mut request = Request::post("/api/v1/studio/session").abort_signal(Some(&signal));
        if let Some(fence) = self.fence() {
            request = request.header(SESSION_HEADER, &fence.header());
        }
        let response = request
            .json(&SessionRequest { token })
            .map_err(|_| unavailable())?
            .send()
            .await
            .map_err(|_| unavailable())?;
        let issued_fence = response
            .headers()
            .get(SESSION_HEADER)
            .as_deref()
            .and_then(SessionFence::new);
        let exchanged: SessionResponse = decode(response).await?;
        let observed = self.current_session().await?.ok_or_else(unauthenticated)?;
        if issued_fence != Some(observed.fence) {
            return Err(session_changed());
        }
        crate::session::confirm_exchange(exchanged, observed.response.clone())?;
        drop(deadline);
        Ok(observed)
    }
    pub async fn authentication_configuration(
        &self,
    ) -> Result<StudioAuthenticationConfiguration, ApiError> {
        get(*self, "/api/v1/studio/auth/configuration").await
    }
    pub async fn current_session(&self) -> Result<Option<ObservedSession>, ApiError> {
        let deadline = RequestDeadline::new()?;
        let signal = deadline.controller.signal();
        let response = Request::get("/api/v1/studio/session")
            .abort_signal(Some(&signal))
            .send()
            .await
            .map_err(|_| unavailable())?;
        let fence = response
            .headers()
            .get(SESSION_HEADER)
            .as_deref()
            .and_then(SessionFence::new);
        let result = match decode::<SessionResponse>(response).await {
            Ok(response) if !response.actor_id.is_empty() => Ok(Some(ObservedSession {
                response,
                fence: fence.ok_or_else(unauthenticated)?,
            })),
            Ok(_) => Err(unavailable()),
            Err(error) if error.code == ErrorCode::Unauthenticated => Ok(None),
            Err(error) => Err(error),
        };
        drop(deadline);
        result
    }
    pub async fn logout(&self) -> Result<(), ApiError> {
        let deadline = RequestDeadline::new()?;
        let signal = deadline.controller.signal();
        let mut request = Request::delete("/api/v1/studio/session").abort_signal(Some(&signal));
        request = if let Some(fence) = self.fence() {
            request.header(SESSION_HEADER, &fence.header())
        } else {
            request.header("x-cantos-studio-stale-only", "1")
        };
        let response = request.send().await.map_err(|_| unavailable())?;
        let result = if response.status() == 204 {
            Ok(())
        } else {
            let status = response.status();
            let error: ApiError = response.json().await.unwrap_or_else(|_| unavailable());
            Err(
                if status == 409 && error.code == ErrorCode::Unauthenticated {
                    session_changed()
                } else {
                    error
                },
            )
        };
        drop(deadline);
        result
    }
    pub fn navigate_to_oidc(&self) -> Result<(), ApiError> {
        let path = match self.fence() {
            Some(fence) => format!(
                "/api/v1/studio/auth/login?studio_session_id={}",
                fence.header()
            ),
            None => "/api/v1/studio/auth/login".into(),
        };
        web_sys::window()
            .ok_or_else(unavailable)?
            .location()
            .set_href(&path)
            .map_err(|_| unavailable())
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
