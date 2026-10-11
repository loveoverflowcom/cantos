//! Same-origin HTTP transport. Session credential becomes an HttpOnly cookie.
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationReviewResponse, ApiError, ApproveProductionRequest,
    ErrorCode, FreezeProductionRequest, HistoryResponse, ImportRequest, ImportResponse,
    ProductionApprovalResponse, ProductionCatalogResponse, ProductionPreviewResponse,
    ProductionRightsClaimResponse, ProductionSettingsResponse, ProductionSnapshotResponse,
    ProductionStateResponse, ReviewRequest, ReviewResponse, RevisionResponse,
    SaveProductionRightsRequest, SaveProductionSettingsRequest, SaveRevisionRequest,
    ScriptValidationResponse, SessionRequest, SessionResponse, SourceResponse,
    ValidateScriptRequest,
};
use gloo_net::http::{Request, Response};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{AbortController, Window};

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
pub struct StudioHttp;

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
        Err(response.json().await.unwrap_or_else(|_| unavailable()))
    }
}

async fn execute<T: DeserializeOwned>(
    request: Request,
    deadline: RequestDeadline,
) -> Result<T, ApiError> {
    let response = request.send().await.map_err(|_| unavailable())?;
    let result = decode(response).await;
    // A timeout while decoding the body remains an ambiguous write outcome. The caller
    // reconciles its immutable operation snapshot; this transport never retries a request.
    drop(deadline);
    result
}

async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = Request::get(path)
        .abort_signal(Some(&signal))
        .build()
        .map_err(|_| unavailable())?;
    execute(request, deadline).await
}

async fn post<T: DeserializeOwned, P: Serialize>(path: &str, body: &P) -> Result<T, ApiError> {
    let deadline = RequestDeadline::new()?;
    let signal = deadline.controller.signal();
    let request = Request::post(path)
        .abort_signal(Some(&signal))
        .json(body)
        .map_err(|_| unavailable())?;
    execute(request, deadline).await
}

impl StudioHttp {
    pub async fn production_catalog(&self) -> Result<ProductionCatalogResponse, ApiError> {
        get("/api/v1/production/catalog").await
    }

    pub async fn production_state(
        &self,
        script: &str,
    ) -> Result<ProductionStateResponse, ApiError> {
        get(&format!("{}/production", script_path(script)?)).await
    }

    pub async fn production_preview(
        &self,
        script: &str,
    ) -> Result<ProductionPreviewResponse, ApiError> {
        get(&format!("{}/production/review", script_path(script)?)).await
    }

    pub async fn save_production_settings(
        &self,
        script: &str,
        request: &SaveProductionSettingsRequest,
    ) -> Result<ProductionSettingsResponse, ApiError> {
        post(
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
        get(&production_snapshot_path(script, snapshot)?).await
    }

    pub async fn approve_production(
        &self,
        script: &str,
        snapshot: &str,
        request: &ApproveProductionRequest,
    ) -> Result<ProductionApprovalResponse, ApiError> {
        post(
            &format!("{}/approvals", production_snapshot_path(script, snapshot)?),
            request,
        )
        .await
    }
    pub async fn validate(
        &self,
        request: &ValidateScriptRequest,
    ) -> Result<ScriptValidationResponse, ApiError> {
        post("/api/v1/validation", request).await
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
        get(&format!(
            "{}/history?after_revision={after_revision}&limit={limit}",
            script_path(script)?
        ))
        .await
    }

    pub async fn review(
        &self,
        script: &str,
        request: &ReviewRequest,
    ) -> Result<ReviewResponse, ApiError> {
        post(&format!("{}/reviews", script_path(script)?), request).await
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
        get(&format!("{}/sources/{source}", script_path(script)?)).await
    }

    pub async fn read_adaptation(&self, id: &str) -> Result<AdaptationReviewResponse, ApiError> {
        get(&format!("{}/review", adaptation_path(id)?)).await
    }

    pub async fn accept_adaptation(
        &self,
        id: &str,
        request: &AcceptAdaptationRequest,
    ) -> Result<RevisionResponse, ApiError> {
        post(&format!("{}/accept", adaptation_path(id)?), request).await
    }

    pub async fn read_revision(
        &self,
        script: &str,
        revision: u64,
    ) -> Result<RevisionResponse, ApiError> {
        if revision == 0 || revision > i64::MAX as u64 {
            return Err(invalid_request());
        }
        get(&format!("{}/revisions/{revision}", script_path(script)?)).await
    }
    pub async fn import(&self, request: &ImportRequest) -> Result<ImportResponse, ApiError> {
        post("/api/v1/imports", request).await
    }

    pub async fn read_import(&self, id: &str) -> Result<ImportResponse, ApiError> {
        get(&import_path(id)?).await
    }

    pub fn original_import_path(&self, id: &str) -> Result<String, ApiError> {
        Ok(format!("{}/original", import_path(id)?))
    }
    pub async fn login(&self, token: String) -> Result<SessionResponse, ApiError> {
        post("/api/v1/session", &SessionRequest { token }).await
    }
    pub async fn read(&self, script: &str) -> Result<RevisionResponse, ApiError> {
        get(&format!("{}/head", script_path(script)?)).await
    }
    pub async fn save(
        &self,
        script: &str,
        request: &SaveRevisionRequest,
    ) -> Result<RevisionResponse, ApiError> {
        post(&format!("{}/revisions", script_path(script)?), request).await
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
    Ok(format!("/api/v1/scripts/{id}"))
}

fn adaptation_path(id: &str) -> Result<String, ApiError> {
    if !crate::adaptation::is_run_id(id) {
        return Err(ApiError {
            code: ErrorCode::InvalidRequest,
            current_revision: None,
            issues: vec![],
        });
    }
    Ok(format!("/api/v1/adaptations/{id}"))
}

fn import_path(id: &str) -> Result<String, ApiError> {
    if !crate::import::is_import_source_id(id) {
        return Err(ApiError {
            code: ErrorCode::InvalidRequest,
            current_revision: None,
            issues: vec![],
        });
    }
    Ok(format!("/api/v1/imports/{id}"))
}
