//! Prototype against contract fixtures, not public delivery or codec acceptance.
//! Composes the real Theatre JSON handler, ListenerService and shared HTTP byte transport.
//! Publication/SQL/object authority is replaced by named test-only doubles, never PrivateStorage.
use crate::{
    contexts::media::{
        storage::port::StorageRead, ListenerError, ListenerService, MediaGrant, PlaybackDelivery,
        PlaybackDeliveryFuture, PlaybackFixtureEligibility, PlaybackPermit,
    },
    interfaces::http::{audio_transport, playback_contract_router},
};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use bytes::Bytes;
use cantos_contracts::theatre::{ListenerErrorCode, PlaybackAuthorization, PublicationManifest};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::Semaphore;
use tower::ServiceExt;

const PUBLICATION: &str = "80000000-0000-4000-8000-000000000008";
const GRANT_URL: &str = "https://synthetic.invalid/fixture-bytes";

#[derive(Default)]
struct CapturingPlaybackDelivery(Mutex<Option<PlaybackPermit>>);
impl PlaybackDelivery for CapturingPlaybackDelivery {
    fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture {
        *self.0.lock().unwrap() = Some(permit.clone());
        Box::pin(async move { MediaGrant::verified_for(&permit, GRANT_URL.into(), 9000, true) })
    }
}

#[derive(Clone)]
struct PublicationBytesFixture {
    service: ListenerService,
    delivery: Arc<CapturingPlaybackDelivery>,
    bytes: Arc<Mutex<Bytes>>,
    eligibility: Arc<Mutex<PlaybackFixtureEligibility>>,
    now: Arc<AtomicI64>,
    lanes: Arc<Semaphore>,
    source_polls: Arc<AtomicUsize>,
}
impl PublicationBytesFixture {
    fn new() -> Self {
        // Arbitrary locally owned transport bytes, deliberately NOT a claimed playable Opus file.
        // The fake publication authority pins their actual SHA/length; codec admission is mocked.
        let bytes = Bytes::from((0..100_u8).collect::<Vec<_>>());
        let delivery = Arc::new(CapturingPlaybackDelivery::default());
        let eligibility = Arc::new(Mutex::new(PlaybackFixtureEligibility::Current));
        let now = Arc::new(AtomicI64::new(5000));
        let service = ListenerService::playback_contract_fixture(
            delivery.clone(),
            format!("{:x}", Sha256::digest(&bytes)),
            bytes.len() as u64,
            eligibility.clone(),
            now.clone(),
        );
        Self {
            service,
            delivery,
            bytes: Arc::new(Mutex::new(bytes)),
            eligibility,
            now,
            lanes: Arc::new(Semaphore::new(1)),
            source_polls: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn router(&self) -> Router {
        playback_contract_router(self.service.clone()).merge(
            Router::new()
                .route("/fixture-bytes", get(fixture_bytes))
                .with_state(self.clone()),
        )
    }
    async fn authorize(&self) -> PlaybackAuthorization {
        let response = self
            .router()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!(
                        "/publications/{PUBLICATION}/playback-authorizations"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }
    async fn fetch(&self, method: Method, headers: &[(&str, &str)]) -> Response {
        let mut request = Request::builder().method(method).uri("/fixture-bytes");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        self.router()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }
}

async fn fixture_bytes(
    State(fixture): State<PublicationBytesFixture>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let Some(permit) = fixture.delivery.0.lock().unwrap().clone() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    // A grant can be captured before authorize_playback's final read rejects it. Re-resolve on
    // EACH GET/HEAD before Range/conditional planning so a cached validator never grants access.
    let manifest = match fixture.service.manifest(permit.publication_id()).await {
        Ok(manifest) => manifest,
        Err(ListenerError::NotFound) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let now = fixture.now.load(Ordering::SeqCst);
    if now >= 9000 || now >= permit.valid_until_ms() || !same_identity(&permit, &manifest) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let plan =
        match audio_transport::plan(&headers, &method, &manifest.sha256, manifest.byte_length) {
            Ok(plan) => plan,
            Err(error) => return fixture_transport_error_response(error, &method),
        };
    let lane = fixture.lanes.clone().try_acquire_owned().unwrap();
    if !plan.requires_storage() {
        let rejection = fixture_rejection(plan.status(), ListenerErrorCode::InvalidRequest);
        return audio_transport::metadata_response(plan, &method, lane, rejection).unwrap();
    }
    let bytes = fixture.bytes.lock().unwrap().clone();
    if bytes.len() as u64 != permit.byte_length()
        || format!("{:x}", Sha256::digest(&bytes)) != permit.sha256()
    {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let selected = match plan.range() {
        Some(range) => bytes.slice(range.first() as usize..=range.last() as usize),
        None => bytes,
    };
    let byte_length = selected.len() as u64;
    let polls = fixture.source_polls.clone();
    let stream = futures_util::stream::unfold((selected, 0_usize), move |(bytes, offset)| {
        let polls = polls.clone();
        async move {
            if offset == bytes.len() {
                return None;
            }
            let end = (offset + 7).min(bytes.len());
            polls.fetch_add(1, Ordering::SeqCst);
            Some((Ok(bytes.slice(offset..end)), (bytes, end)))
        }
    });
    // No await/effect between last eligibility read and deterministic source construction.
    // TODO(issue120, #10/#11): a real publication-class adapter must pin actual object version,
    // verify full bytes outside this fixture, and repeat authorization AFTER object-open I/O.
    audio_transport::response(
        StorageRead {
            stream: Box::pin(stream),
            byte_length,
        },
        plan,
        &method,
        lane,
    )
    .unwrap()
}

fn fixture_rejection(status: StatusCode, code: ListenerErrorCode) -> Response {
    audio_transport::private_response(
        (
            status,
            Json(cantos_contracts::theatre::ListenerError {
                code,
                retryable: false,
            }),
        )
            .into_response(),
    )
}
fn fixture_transport_error_response(
    error: audio_transport::AudioTransportError,
    method: &Method,
) -> Response {
    use audio_transport::AudioTransportError;
    let (status, code) = match error {
        AudioTransportError::MalformedRange
        | AudioTransportError::MalformedIfRange
        | AudioTransportError::MalformedIfMatch
        | AudioTransportError::MalformedIfNoneMatch
        | AudioTransportError::UnsupportedMethod => {
            (StatusCode::BAD_REQUEST, ListenerErrorCode::InvalidRequest)
        }
        AudioTransportError::InvalidRepresentation
        | AudioTransportError::InconsistentPlan
        | AudioTransportError::DeclaredLengthMismatch => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ListenerErrorCode::Unavailable,
        ),
    };
    let mut response = fixture_rejection(status, code);
    if *method == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    response
}

fn same_identity(permit: &PlaybackPermit, manifest: &PublicationManifest) -> bool {
    permit.publication_id() == manifest.publication_id
        && permit.rendition_id() == manifest.rendition_id
        && permit.manifest_digest() == manifest.manifest_digest
        && permit.sha256() == manifest.sha256
        && permit.byte_length() == manifest.byte_length
}

fn assert_transport_headers(response: &Response, length: u64) {
    assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/ogg");
    assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(
        response.headers()[header::CONTENT_LENGTH],
        length.to_string()
    );
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, no-store, no-transform"
    );
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
}

#[tokio::test]
async fn theatre_json_authorization_consumes_exact_release_then_native_range_contract() {
    let fixture = PublicationBytesFixture::new();
    let grant = fixture.authorize().await;
    assert_eq!(grant.url, GRANT_URL);
    assert_eq!(grant.publication_id, PUBLICATION);
    assert_eq!(grant.content_type, "audio/ogg");
    assert!(grant.byte_range_supported);
    assert_eq!(
        grant.sha256,
        format!("{:x}", Sha256::digest((0..100_u8).collect::<Vec<_>>()))
    );
    for (range, first, last) in [
        ("bytes=0-6", 0, 6),
        ("bytes=41-53", 41, 53),
        ("bytes=-9", 91, 99),
        ("bytes=93-", 93, 99),
        ("bytes=97-999", 97, 99),
    ] {
        let response = fixture.fetch(Method::GET, &[("range", range)]).await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_transport_headers(&response, (last - first + 1) as u64);
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes {first}-{last}/100")
        );
        assert_eq!(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .as_ref(),
            &(first..=last).collect::<Vec<u8>>()
        );
    }
    let response = fixture
        .fetch(Method::GET, &[("accept-encoding", "gzip, br")])
        .await;
    assert_transport_headers(&response, 100);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        &(0..100_u8).collect::<Vec<_>>()
    );
    let response = fixture.fetch(Method::HEAD, &[("range", "bytes=7-9")]).await;
    assert_transport_headers(&response, 100);
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .is_empty());
}

#[tokio::test]
async fn conditional_ranges_and_unsatisfied_requests_never_change_release_or_bytes() {
    let fixture = PublicationBytesFixture::new();
    let grant = fixture.authorize().await;
    let etag = format!("\"{}\"", grant.sha256);
    for (validator, status, length) in [
        (etag.as_str(), StatusCode::PARTIAL_CONTENT, 3),
        ("W/\"different\"", StatusCode::OK, 100),
        ("\"different\"", StatusCode::OK, 100),
    ] {
        let response = fixture
            .fetch(
                Method::GET,
                &[("range", "bytes=2-4"), ("if-range", validator)],
            )
            .await;
        assert_eq!(response.status(), status);
        assert_transport_headers(&response, length);
        response.into_body().collect().await.unwrap();
    }
    let response = fixture.fetch(Method::GET, &[("range", "bytes=100-")]).await;
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */100");
    drop(response);
    let response = fixture
        .fetch(Method::GET, &[("range", "bytes=0-1,4-5")])
        .await;
    assert_eq!(response.status(), StatusCode::OK); // Multipart unsupported: ignore entire Range.
    assert_transport_headers(&response, 100);
    response.into_body().collect().await.unwrap();
    let response = fixture
        .fetch(
            Method::GET,
            &[("if-none-match", &etag), ("range", "bytes=100-")],
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED); // Conditions before Range.
    assert!(response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .is_empty());
}

#[tokio::test]
async fn every_fixture_fetch_rechecks_revocation_withdrawal_supersession_owner_and_expiry() {
    for eligibility in [
        PlaybackFixtureEligibility::Withdrawn,
        PlaybackFixtureEligibility::Revoked,
        PlaybackFixtureEligibility::Superseded,
        PlaybackFixtureEligibility::OwnerDisabled,
    ] {
        let fixture = PublicationBytesFixture::new();
        fixture.authorize().await;
        *fixture.eligibility.lock().unwrap() = eligibility;
        for method in [Method::GET, Method::HEAD] {
            let response = fixture.fetch(method, &[("if-none-match", "*")]).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(fixture.source_polls.load(Ordering::SeqCst), 0);
        }
    }
    for now in [9000, 10000] {
        let fixture = PublicationBytesFixture::new();
        fixture.authorize().await;
        fixture.now.store(now, Ordering::SeqCst);
        assert_eq!(
            fixture.fetch(Method::GET, &[]).await.status(),
            if now == 9000 {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::NOT_FOUND
            }
        );
        assert_eq!(fixture.source_polls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn transport_body_is_demand_driven_and_caller_drop_cancels_further_source_polls() {
    let fixture = PublicationBytesFixture::new();
    fixture.authorize().await;
    let response = fixture.fetch(Method::GET, &[]).await;
    assert_eq!(fixture.source_polls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.lanes.available_permits(), 0);
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(first.as_ref(), &[0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(fixture.source_polls.load(Ordering::SeqCst), 1);
    drop(body);
    assert_eq!(fixture.source_polls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.lanes.available_permits(), 1);
    *fixture.bytes.lock().unwrap() = Bytes::from(vec![255; 100]);
    assert_eq!(
        fixture.fetch(Method::GET, &[]).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(fixture.source_polls.load(Ordering::SeqCst), 1);
}
