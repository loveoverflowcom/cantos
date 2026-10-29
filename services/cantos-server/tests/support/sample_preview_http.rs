//! Opt-in local preview of the private sample dataset (issue #427, Part of; refs #539).
//!
//! Composed only by the explicit `--sample-serve` host and its tests: the canonical
//! `api_router` mounts none of this, so no production binary contains the capability. Every
//! answer is read through the real Studio routes of the same Store with the host's private
//! development token, held server-side and never sent to a browser. The handlers therefore
//! inherit current-revision reads, owner scoping and verified private-object delivery, and
//! add only: dataset membership, a draft-only projection and a loopback `Host` guard.
//!
//! Nothing here is a publication, release, rights or QC claim. The projection carries no
//! publication ID, timestamp, duration, playback URL, storage key, provenance or rights text.
use std::{collections::BTreeSet, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    extract::{Path, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use super::input;

pub const NAMESPACE: &str = "/api/v1/sample-preview";
pub const SCHEMA: &str = "cantos.sample-preview.v1";
/// The Studio origin the in-process reads present, as the sample host already configures.
const STUDIO_ORIGIN: &str = "http://127.0.0.1:8080";
/// The API listener and the Trunk dev server (whose `/api/` proxy sends the target host without port).
const HOSTS: [&str; 6] = [
    "127.0.0.1",
    "localhost",
    "127.0.0.1:8080",
    "localhost:8080",
    "127.0.0.1:8082",
    "localhost:8082",
];
const ORIGINS: [&str; 4] = [
    "127.0.0.1:8080",
    "localhost:8080",
    "127.0.0.1:8082",
    "localhost:8082",
];
/// The largest private cover the contract admits (8 MiB) plus framing slack.
const MAX_STUDIO_BODY: usize = 9 * 1024 * 1024;

/// One Work of the validated dataset and exactly the chapters it owns, in sample order.
#[derive(Clone, Debug)]
pub struct Member {
    pub work_id: String,
    pub unit_ids: Vec<String>,
}

#[derive(Clone)]
pub struct Preview {
    studio: Router,
    token: Arc<str>,
    members: Arc<Vec<Member>>,
    gate: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    InvalidRequest,
    NotFound,
    Unavailable,
    /// The Store does not hold exactly the sample dataset, or an object failed verification.
    Mismatch,
}

struct Cover {
    asset_id: String,
    sha256: String,
    content_type: String,
    width_px: u64,
    height_px: u64,
    byte_length: u64,
    alt_text: String,
}

struct View {
    summary: Value,
    chapters: Vec<Value>,
}

pub fn members(spec: &input::Input) -> Vec<Member> {
    spec.works
        .iter()
        .map(|work| Member {
            work_id: input::identity(&work.sample_key, "work"),
            unit_ids: work
                .chapters
                .iter()
                .map(|chapter| input::identity(&chapter.sample_key, "chapter"))
                .collect(),
        })
        .collect()
}

fn failure(kind: Failure) -> Response {
    let (status, code, retryable) = match kind {
        Failure::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request", false),
        Failure::NotFound => (StatusCode::NOT_FOUND, "not_found", false),
        Failure::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable", true),
        Failure::Mismatch => (StatusCode::INTERNAL_SERVER_ERROR, "sample_mismatch", false),
    };
    json_response(status, &json!({"code": code, "retryable": retryable}))
}

fn json_response(status: StatusCode, value: &Value) -> Response {
    let mut response = (status, serde_json::to_vec(value).unwrap_or_default()).into_response();
    hardened(response.headers_mut());
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn hardened(headers: &mut HeaderMap) {
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}

/// DNS-rebinding and foreign-page guard: only the loopback API and dev-server authorities.
async fn loopback_only(request: Request, next: Next) -> Response {
    let host_ok = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| HOSTS.contains(&host));
    let origin_ok = request.headers().get(header::ORIGIN).is_none_or(|origin| {
        origin
            .to_str()
            .ok()
            .and_then(|origin| origin.strip_prefix("http://"))
            .is_some_and(|authority| ORIGINS.contains(&authority))
    });
    if !host_ok || !origin_ok || !matches!(request.method(), &Method::GET | &Method::HEAD) {
        let mut response = json_response(
            StatusCode::FORBIDDEN,
            &json!({"code": "forbidden", "retryable": false}),
        );
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static("Origin"));
        return response;
    }
    if request.uri().query().is_some() {
        return failure(Failure::InvalidRequest);
    }
    next.run(request).await
}

impl Preview {
    fn member(&self, work: &str) -> Result<&Member, Failure> {
        self.members
            .iter()
            .find(|member| member.work_id == work)
            .ok_or(Failure::NotFound)
    }

    async fn studio(&self, path: &str) -> Result<(HeaderMap, Vec<u8>), Failure> {
        let _guard = self.gate.lock().await;
        let request = Request::builder()
            .method(Method::GET)
            .uri(path)
            .header(header::COOKIE, format!("cantos_session={}", self.token))
            .header(header::ORIGIN, STUDIO_ORIGIN)
            .body(Body::empty())
            .map_err(|_| Failure::Unavailable)?;
        let response = match self.studio.clone().oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        };
        let (parts, body) = response.into_parts();
        let bytes = to_bytes(body, MAX_STUDIO_BODY)
            .await
            .map_err(|_| Failure::Mismatch)?;
        match parts.status {
            StatusCode::OK => Ok((parts.headers, bytes.to_vec())),
            StatusCode::NOT_FOUND => Err(Failure::NotFound),
            StatusCode::INTERNAL_SERVER_ERROR => Err(Failure::Mismatch),
            _ => Err(Failure::Unavailable),
        }
    }

    async fn studio_json(&self, path: &str) -> Result<Value, Failure> {
        let (_, bytes) = self.studio(path).await?;
        serde_json::from_slice(&bytes).map_err(|_| Failure::Mismatch)
    }

    /// The current draft Work, its exact dataset chapters and its selected cover, all read from
    /// the Store now. A foreign, missing or extra chapter is a mismatch, never a quiet filter.
    async fn view(&self, member: &Member) -> Result<View, Failure> {
        let id = &member.work_id;
        let work = self
            .studio_json(&format!("/api/v1/studio/works/{id}"))
            .await?;
        let details = &work["details"];
        let text = |value: &Value| value.as_str().map(str::to_owned).ok_or(Failure::Mismatch);
        let kind = text(&work["kind"])?;
        if work["id"] != *id || !matches!(kind.as_str(), "novel" | "story" | "podcast") {
            return Err(Failure::Mismatch);
        }
        let units = self
            .studio_json(&format!("/api/v1/studio/works/{id}/content-units"))
            .await?;
        let items = units["items"].as_array().ok_or(Failure::Mismatch)?;
        if !units["next_cursor"].is_null() {
            return Err(Failure::Mismatch);
        }
        let found: BTreeSet<&str> = items
            .iter()
            .filter_map(|unit| unit["id"].as_str())
            .collect();
        let expected: BTreeSet<&str> = member.unit_ids.iter().map(String::as_str).collect();
        if found != expected || items.len() != expected.len() {
            return Err(Failure::Mismatch);
        }
        let mut chapters = Vec::new();
        for (index, unit) in items.iter().enumerate() {
            chapters.push(json!({
                "unit_id": text(&unit["id"])?,
                "kind": "chapter",
                "position": index + 1,
                "title": text(&unit["details"]["title"])?,
                "description": text(&unit["details"]["description"])?,
                "audio": "unavailable",
            }));
        }
        let cover = self.cover_of(id).await?;
        let summary = json!({
            "work_id": id,
            "kind": kind,
            "title": text(&details["title"])?,
            "description": text(&details["synopsis"])?,
            "language": text(&details["language"])?,
            "chapter_count": chapters.len(),
            "cover": cover.as_ref().map(|cover| json!({
                "path": format!("{NAMESPACE}/works/{id}/cover/{}", cover.sha256),
                "sha256": cover.sha256,
                "content_type": cover.content_type,
                "width_px": cover.width_px,
                "height_px": cover.height_px,
                "byte_length": cover.byte_length,
                "alt_text": cover.alt_text,
            })),
        });
        Ok(View { summary, chapters })
    }

    async fn cover_of(&self, work: &str) -> Result<Option<Cover>, Failure> {
        let artwork = self
            .studio_json(&format!("/api/v1/studio/works/{work}/artwork"))
            .await?;
        if artwork["work_id"] != *work {
            return Err(Failure::Mismatch);
        }
        let selection = &artwork["artwork"];
        if selection.is_null() {
            return Ok(None);
        }
        let asset_id = selection["image_asset_id"]
            .as_str()
            .ok_or(Failure::Mismatch)?
            .to_owned();
        let asset = self
            .studio_json(&format!(
                "/api/v1/studio/works/{work}/image-assets/{asset_id}"
            ))
            .await?;
        let media = &asset["verified_media"];
        // The selection pins the exact asset digest; the Work and asset must agree.
        if asset["work_id"] != *work
            || asset["id"] != asset_id.as_str()
            || asset["digest"] != selection["image_asset_digest"]
        {
            return Err(Failure::Mismatch);
        }
        let number = |value: &Value| value.as_u64().ok_or(Failure::Mismatch);
        let content_type = media["content_type"].as_str().ok_or(Failure::Mismatch)?;
        if content_type != "image/avif" {
            return Err(Failure::Mismatch);
        }
        Ok(Some(Cover {
            asset_id,
            sha256: media["sha256"]
                .as_str()
                .ok_or(Failure::Mismatch)?
                .to_owned(),
            content_type: content_type.to_owned(),
            width_px: number(&media["width_px"])?,
            height_px: number(&media["height_px"])?,
            byte_length: number(&media["byte_length"])?,
            alt_text: selection["alt_text"]
                .as_str()
                .ok_or(Failure::Mismatch)?
                .to_owned(),
        }))
    }
}

async fn list(State(preview): State<Preview>) -> Response {
    let mut items = Vec::new();
    for member in preview.members.iter() {
        match preview.view(member).await {
            Ok(view) => items.push(view.summary),
            Err(kind) => return failure(kind),
        }
    }
    json_response(
        StatusCode::OK,
        &json!({"schema": SCHEMA, "dataset_id": input::DATASET, "items": items}),
    )
}

async fn detail(State(preview): State<Preview>, Path(work): Path<String>) -> Response {
    let member = match preview.member(&work) {
        Ok(member) => member,
        Err(kind) => return failure(kind),
    };
    match preview.view(member).await {
        Ok(view) => json_response(
            StatusCode::OK,
            &json!({"schema": SCHEMA, "work": view.summary, "chapters": view.chapters}),
        ),
        Err(kind) => failure(kind),
    }
}

async fn cover(
    State(preview): State<Preview>,
    Path((work, sha256)): Path<(String, String)>,
    method: Method,
) -> Response {
    let member = match preview.member(&work) {
        Ok(member) => member,
        Err(kind) => return failure(kind),
    };
    let selected = match preview.cover_of(&member.work_id).await {
        Ok(Some(cover)) if cover.sha256 == sha256 => cover,
        // No cover, or a digest the Work's current selection does not pin.
        Ok(_) => return failure(Failure::NotFound),
        Err(kind) => return failure(kind),
    };
    let path = format!(
        "/api/v1/studio/works/{}/image-assets/{}/bytes",
        member.work_id, selected.asset_id
    );
    let bytes = match preview.studio(&path).await {
        Ok((_, bytes)) => bytes,
        Err(kind) => return failure(kind),
    };
    // The Studio read verifies the object; the preview re-proves the exact selected identity.
    if bytes.len() as u64 != selected.byte_length
        || format!("{:x}", Sha256::digest(&bytes)) != sha256
    {
        return failure(Failure::Mismatch);
    }
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(bytes)
    };
    let mut response = (StatusCode::OK, body).into_response();
    hardened(response.headers_mut());
    if let Ok(value) = HeaderValue::from_str(&selected.content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&selected.byte_length.to_string()) {
        response.headers_mut().insert(header::CONTENT_LENGTH, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

impl Preview {
    /// `studio` is the canonical API router over the sample's own Store; `token` its private
    /// development session, used only for these in-process reads.
    pub fn new(studio: Router, token: &str, members: Vec<Member>) -> Self {
        Self {
            studio,
            token: token.into(),
            members: Arc::new(members),
            gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Startup proof, before any port is bound: the Store's current reads must cover exactly the
    /// declared dataset. A failure names the Work and stops the host rather than serving a
    /// partial or foreign catalog.
    pub async fn verify(&self) -> Result<(), String> {
        for member in self.members.iter() {
            self.view(member).await.map_err(|kind| {
                format!("sample work {} is not served: {kind:?}", member.work_id)
            })?;
        }
        Ok(())
    }

    /// The preview routes in front of the canonical API router, which stays the fallback: every
    /// other path keeps its production meaning, including the anonymous absence of the sample.
    pub fn router(self) -> Router {
        let studio = self.studio.clone();
        Router::new()
            .route(&format!("{NAMESPACE}/works"), get(list))
            .route(&format!("{NAMESPACE}/works/{{work}}"), get(detail))
            .route(
                &format!("{NAMESPACE}/works/{{work}}/cover/{{sha256}}"),
                get(cover),
            )
            .route_layer(middleware::from_fn(loopback_only))
            .with_state(self)
            .fallback_service(studio)
    }
}
