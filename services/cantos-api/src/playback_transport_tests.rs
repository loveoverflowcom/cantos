//! Mock integration of origin delivery (proposed ADR 0032), not public delivery acceptance.
//! Composes the real Theatre JSON handlers, ListenerService, OriginPlayback grant sealing, the
//! production `/media/{grant}` route and the shared HTTP byte transport. Publication/SQL
//! eligibility and publication-class object storage are named test doubles, never
//! PrivateStorage; the bytes are arbitrary transport bytes, not a claimed playable Opus file.
use crate::{
    adapters::inbound::http::playback_contract_router,
    contexts::media::{
        domain::{
            object_storage::{NormalizedByteRange, StorageError},
            playback_grant::GrantKey,
        },
        storage::port::{StorageFuture, StorageRead},
        ListenerService, MediaGrant, OriginPlayback, PlaybackDelivery, PlaybackDeliveryFuture,
        PlaybackFixtureEligibility, PlaybackPermit, PublicMediaOrigin, PublicationObjectRef,
        PublicationObjects,
    },
};
use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    response::Response,
    Router,
};
use bytes::Bytes;
use cantos_contracts::theatre::PlaybackAuthorization;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tower::ServiceExt;

const PUBLICATION: &str = "80000000-0000-4000-8000-000000000008";
const ORIGIN: &str = "https://listen.example.test";
const MEDIA_PREFIX: &str = "https://listen.example.test/api/v1/theatre";

type OpenHook = Box<dyn FnOnce() + Send>;
/// Object ID, SHA-256, byte length and the normalized range of one recorded open.
type RecordedOpen = (String, String, u64, Option<(u64, u64)>);

/// Named publication-class object double. It records each open and returns exactly the
/// requested slice, as an honest promoted-object adapter must; it performs no authorization.
struct FixtureObjects {
    bytes: Mutex<Bytes>,
    opens: Mutex<Vec<RecordedOpen>>,
    polls: Arc<AtomicUsize>,
    failure: Mutex<Option<StorageError>>,
    declared_length_offset: AtomicI64,
    during_open: Mutex<Option<OpenHook>>,
}
impl FixtureObjects {
    fn new(bytes: Bytes) -> Self {
        Self {
            bytes: Mutex::new(bytes),
            opens: Mutex::new(vec![]),
            polls: Arc::new(AtomicUsize::new(0)),
            failure: Mutex::new(None),
            declared_length_offset: AtomicI64::new(0),
            during_open: Mutex::new(None),
        }
    }
    fn opens(&self) -> usize {
        self.opens.lock().unwrap().len()
    }
}
impl PublicationObjects for FixtureObjects {
    fn open(
        &self,
        object: PublicationObjectRef,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        self.opens.lock().unwrap().push((
            object.object_id().into(),
            object.sha256().into(),
            object.byte_length(),
            range.map(|range| (range.first(), range.last())),
        ));
        // Models an eligibility or clock change that lands while the object is being opened.
        if let Some(hook) = self.during_open.lock().unwrap().take() {
            hook();
        }
        let failure = *self.failure.lock().unwrap();
        let bytes = self.bytes.lock().unwrap().clone();
        let selected = match range {
            Some(range) => bytes.slice(range.first() as usize..=range.last() as usize),
            None => bytes,
        };
        let declared =
            (selected.len() as i64 + self.declared_length_offset.load(Ordering::SeqCst)) as u64;
        let polls = self.polls.clone();
        Box::pin(async move {
            if let Some(error) = failure {
                return Err(error);
            }
            let stream = futures_util::stream::unfold((selected, 0_usize), move |(bytes, at)| {
                let polls = polls.clone();
                async move {
                    if at == bytes.len() {
                        return None;
                    }
                    let end = (at + 7).min(bytes.len());
                    polls.fetch_add(1, Ordering::SeqCst);
                    Some((Ok(bytes.slice(at..end)), (bytes, end)))
                }
            });
            Ok(StorageRead {
                stream: Box::pin(stream),
                byte_length: declared,
            })
        })
    }
}

struct Fixture {
    service: ListenerService,
    objects: Arc<FixtureObjects>,
    eligibility: Arc<Mutex<PlaybackFixtureEligibility>>,
    now: Arc<AtomicI64>,
}
impl Fixture {
    fn new() -> Self {
        Self::with(7, 4)
    }
    fn with(key: u8, lanes: usize) -> Self {
        let bytes = Bytes::from((0..100_u8).collect::<Vec<_>>());
        let objects = Arc::new(FixtureObjects::new(bytes.clone()));
        let eligibility = Arc::new(Mutex::new(PlaybackFixtureEligibility::Current));
        let now = Arc::new(AtomicI64::new(5000));
        let origin = OriginPlayback::new(
            PublicMediaOrigin::parse(ORIGIN).unwrap(),
            GrantKey::new(&[key; 32]).unwrap(),
            objects.clone(),
        );
        let service = ListenerService::playback_contract_fixture(
            origin,
            format!("{:x}", Sha256::digest(&bytes)),
            bytes.len() as u64,
            eligibility.clone(),
            now.clone(),
            lanes,
        );
        Self {
            service,
            objects,
            eligibility,
            now,
        }
    }
    fn router(&self) -> Router {
        playback_contract_router(self.service.clone())
    }
    fn set(&self, eligibility: PlaybackFixtureEligibility) {
        *self.eligibility.lock().unwrap() = eligibility;
    }
    async fn post(&self) -> Response {
        self.router()
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
            .unwrap()
    }
    async fn authorize(&self) -> PlaybackAuthorization {
        let response = self.post().await;
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&body(response).await).unwrap()
    }
    async fn fetch(&self, path: &str, method: Method, headers: &[(&str, &str)]) -> Response {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        self.router()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }
}

/// The contract router mounts Theatre routes without the host prefix.
fn media_path(grant: &PlaybackAuthorization) -> String {
    grant.url.strip_prefix(MEDIA_PREFIX).unwrap().to_owned()
}

async fn body(response: Response) -> Bytes {
    response.into_body().collect().await.unwrap().to_bytes()
}

fn assert_private(response: &Response) {
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

fn assert_transport_headers(response: &Response, length: u64) {
    assert_private(response);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/ogg");
    assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(
        response.headers()[header::CONTENT_LENGTH],
        length.to_string()
    );
}

async fn assert_not_found(response: Response, method: &Method) {
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_private(&response);
    let bytes = body(response).await;
    if *method == Method::HEAD {
        assert!(bytes.is_empty());
    } else {
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            serde_json::json!({"code": "not_found", "retryable": false})
        );
    }
}

#[tokio::test]
async fn json_authorization_issues_an_origin_grant_that_serves_exact_release_ranges() {
    let fixture = Fixture::new();
    let grant = fixture.authorize().await;
    assert!(grant
        .url
        .starts_with("https://listen.example.test/api/v1/theatre/media/pg1_"));
    assert!(!grant.url.contains("publication-fixture"));
    assert_eq!(grant.publication_id, PUBLICATION);
    assert_eq!(grant.content_type, "audio/ogg");
    assert!(grant.byte_range_supported);
    // Bounded by the 120 s TTL and the fixture's rights window (valid_until 10000).
    assert_eq!(grant.expires_at_ms, 10_000);
    let whole = (0..100_u8).collect::<Vec<_>>();
    assert_eq!(grant.sha256, format!("{:x}", Sha256::digest(&whole)));
    let path = media_path(&grant);
    for (range, first, last) in [
        ("bytes=0-6", 0, 6),
        ("bytes=41-53", 41, 53),
        ("bytes=-9", 91, 99),
        ("bytes=93-", 93, 99),
        ("bytes=97-999", 97, 99),
    ] {
        let response = fixture.fetch(&path, Method::GET, &[("range", range)]).await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_transport_headers(&response, (last - first + 1) as u64);
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes {first}-{last}/100")
        );
        assert_eq!(body(response).await.as_ref(), &whole[first..=last]);
    }
    let response = fixture
        .fetch(&path, Method::GET, &[("accept-encoding", "gzip, br")])
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_transport_headers(&response, 100);
    // Checksum of the delivered full representation equals the sealed manifest SHA-256.
    assert_eq!(
        format!("{:x}", Sha256::digest(body(response).await)),
        grant.sha256
    );
    let response = fixture
        .fetch(&path, Method::HEAD, &[("range", "bytes=7-9")])
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_transport_headers(&response, 100);
    assert!(body(response).await.is_empty());
    // Every byte request opened the exact sealed publication object, never another one.
    let opens = fixture.objects.opens.lock().unwrap().clone();
    assert_eq!(opens.len(), 7);
    for (object, sha256, length, _) in &opens {
        assert_eq!(
            (object.as_str(), sha256.as_str(), *length),
            ("publication-fixture", grant.sha256.as_str(), 100)
        );
    }
    assert_eq!(opens[1].3, Some((41, 53)));
    assert_eq!(opens[5].3, None);
}

#[tokio::test]
async fn conditional_and_unsatisfied_requests_open_nothing_and_never_change_identity() {
    let fixture = Fixture::new();
    let grant = fixture.authorize().await;
    let path = media_path(&grant);
    let etag = format!("\"{}\"", grant.sha256);
    for (validator, status, length) in [
        (etag.as_str(), StatusCode::PARTIAL_CONTENT, 3),
        ("W/\"different\"", StatusCode::OK, 100),
        ("\"different\"", StatusCode::OK, 100),
    ] {
        let response = fixture
            .fetch(
                &path,
                Method::GET,
                &[("range", "bytes=2-4"), ("if-range", validator)],
            )
            .await;
        assert_eq!(response.status(), status);
        assert_transport_headers(&response, length);
        assert_eq!(response.headers()[header::ETAG], etag.as_str());
        body(response).await;
    }
    let opened = fixture.objects.opens();
    let response = fixture
        .fetch(&path, Method::GET, &[("range", "bytes=100-")])
        .await;
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */100");
    assert_private(&response);
    let response = fixture
        .fetch(
            &path,
            Method::GET,
            &[("if-none-match", &etag), ("range", "bytes=100-")],
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert!(body(response).await.is_empty());
    let response = fixture
        .fetch(&path, Method::GET, &[("if-match", "\"different\"")])
        .await;
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    assert_eq!(fixture.objects.opens(), opened);
    let response = fixture
        .fetch(&path, Method::GET, &[("range", "bytes=0-1,4-5")])
        .await;
    assert_eq!(response.status(), StatusCode::OK); // Multipart unsupported: ignore Range.
    assert_transport_headers(&response, 100);
    body(response).await;
    for (name, value) in [("range", "bytes=9-2"), ("if-range", "not-a-validator")] {
        let response = fixture
            .fetch(&path, Method::GET, &[(name, value), ("range", "bytes=0-1")])
            .await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{name}: {value}"
        );
        assert_private(&response);
    }
}

#[tokio::test]
async fn every_fetch_rechecks_withdrawal_revocation_supersession_owner_and_expiry() {
    for eligibility in [
        PlaybackFixtureEligibility::Withdrawn,
        PlaybackFixtureEligibility::Revoked,
        PlaybackFixtureEligibility::Superseded,
        PlaybackFixtureEligibility::OwnerDisabled,
    ] {
        let fixture = Fixture::new();
        let path = media_path(&fixture.authorize().await);
        fixture.set(eligibility);
        for method in [Method::GET, Method::HEAD] {
            // A cached validator never bypasses the current-eligibility read.
            let response = fixture
                .fetch(&path, method.clone(), &[("if-none-match", "*")])
                .await;
            assert_not_found(response, &method).await;
        }
        assert_eq!(fixture.objects.opens(), 0);
    }
    let fixture = Fixture::new();
    let path = media_path(&fixture.authorize().await);
    fixture.now.store(9999, Ordering::SeqCst);
    let response = fixture.fetch(&path, Method::GET, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    body(response).await;
    fixture.now.store(10_000, Ordering::SeqCst);
    assert_not_found(fixture.fetch(&path, Method::GET, &[]).await, &Method::GET).await;
    assert_eq!(fixture.objects.opens(), 1);
}

#[tokio::test]
async fn withdrawal_or_expiry_while_the_object_opens_drops_the_stream_unread() {
    let withdrawn = Fixture::new();
    let path = media_path(&withdrawn.authorize().await);
    let eligibility = withdrawn.eligibility.clone();
    *withdrawn.objects.during_open.lock().unwrap() = Some(Box::new(move || {
        *eligibility.lock().unwrap() = PlaybackFixtureEligibility::Withdrawn;
    }));
    assert_not_found(withdrawn.fetch(&path, Method::GET, &[]).await, &Method::GET).await;
    assert_eq!(withdrawn.objects.opens(), 1);
    assert_eq!(withdrawn.objects.polls.load(Ordering::SeqCst), 0);

    let expired = Fixture::new();
    let path = media_path(&expired.authorize().await);
    let now = expired.now.clone();
    *expired.objects.during_open.lock().unwrap() = Some(Box::new(move || {
        now.store(10_000, Ordering::SeqCst);
    }));
    assert_not_found(expired.fetch(&path, Method::HEAD, &[]).await, &Method::HEAD).await;
    assert_eq!(expired.objects.polls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn forged_tampered_foreign_or_malformed_grants_are_an_indistinguishable_not_found() {
    let fixture = Fixture::new();
    let path = media_path(&fixture.authorize().await);
    let token = path.strip_prefix("/media/").unwrap();
    let flip = |index: usize| {
        let mut bytes = token.as_bytes().to_vec();
        bytes[index] = if bytes[index] == b'0' { b'1' } else { b'0' };
        format!("/media/{}", String::from_utf8(bytes).unwrap())
    };
    let foreign = Fixture::with(9, 4);
    let foreign_path = media_path(&foreign.authorize().await);
    for candidate in [
        flip(10),  // another publication ID
        flip(40),  // an unauthenticated longer expiry
        flip(115), // the authenticator itself
        foreign_path,
        "/media/pg1_short".into(),
        format!("/media/{}", token.to_ascii_uppercase()),
        "/media/%ZZ".into(),
    ] {
        assert_not_found(
            fixture.fetch(&candidate, Method::GET, &[]).await,
            &Method::GET,
        )
        .await;
    }
    assert_eq!(fixture.objects.opens(), 0);
}

#[tokio::test]
async fn refresh_reissues_the_same_pinned_release_until_withdrawal() {
    let fixture = Fixture::new();
    let first = fixture.authorize().await;
    fixture.now.store(9000, Ordering::SeqCst);
    let refreshed = fixture.authorize().await;
    assert_eq!(
        (
            &refreshed.publication_id,
            &refreshed.rendition_id,
            &refreshed.sha256
        ),
        (&first.publication_id, &first.rendition_id, &first.sha256)
    );
    assert_eq!(refreshed.expires_at_ms, 10_000);
    let response = fixture
        .fetch(&media_path(&refreshed), Method::GET, &[])
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    body(response).await;
    fixture.set(PlaybackFixtureEligibility::Withdrawn);
    assert_eq!(fixture.post().await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bounded_lanes_follow_the_body_and_caller_drop_cancels_further_reads() {
    let fixture = Fixture::with(7, 1);
    let path = media_path(&fixture.authorize().await);
    let response = fixture.fetch(&path, Method::GET, &[]).await;
    assert_eq!(fixture.objects.polls.load(Ordering::SeqCst), 0);
    let busy = fixture.fetch(&path, Method::GET, &[]).await;
    assert_eq!(busy.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private(&busy);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body(busy).await).unwrap(),
        serde_json::json!({"code": "unavailable", "retryable": true})
    );
    let mut stream = response.into_body();
    let first = stream.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(first.as_ref(), &[0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(fixture.objects.polls.load(Ordering::SeqCst), 1);
    drop(stream);
    assert_eq!(fixture.objects.polls.load(Ordering::SeqCst), 1);
    let response = fixture.fetch(&path, Method::GET, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    body(response).await;
}

#[tokio::test]
async fn object_failures_are_typed_and_never_substitute_other_bytes() {
    for (failure, status, code, retryable) in [
        (
            StorageError::NotFound,
            StatusCode::SERVICE_UNAVAILABLE,
            "rendition_unavailable",
            true,
        ),
        (
            StorageError::Timeout,
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            true,
        ),
        (
            StorageError::DigestMismatch,
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            false,
        ),
    ] {
        let fixture = Fixture::new();
        let path = media_path(&fixture.authorize().await);
        *fixture.objects.failure.lock().unwrap() = Some(failure);
        let response = fixture.fetch(&path, Method::GET, &[]).await;
        assert_eq!(response.status(), status, "{failure:?}");
        assert_private(&response);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body(response).await).unwrap(),
            serde_json::json!({"code": code, "retryable": retryable})
        );
    }
    // A reader declaring another length is rejected before a byte is sent.
    let fixture = Fixture::new();
    let path = media_path(&fixture.authorize().await);
    fixture
        .objects
        .declared_length_offset
        .store(-1, Ordering::SeqCst);
    let response = fixture.fetch(&path, Method::GET, &[]).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(fixture.objects.polls.load(Ordering::SeqCst), 0);
    // Shorter bytes than declared terminate the body with an error, never a short success.
    let fixture = Fixture::new();
    let path = media_path(&fixture.authorize().await);
    *fixture.objects.bytes.lock().unwrap() = Bytes::from((0..90_u8).collect::<Vec<_>>());
    fixture
        .objects
        .declared_length_offset
        .store(10, Ordering::SeqCst);
    let response = fixture.fetch(&path, Method::GET, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.into_body().collect().await.is_err());
}

#[derive(Default)]
struct GrantOnly(Mutex<Option<PlaybackPermit>>);
impl PlaybackDelivery for GrantOnly {
    fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture {
        *self.0.lock().unwrap() = Some(permit.clone());
        Box::pin(async move {
            MediaGrant::verified_for(&permit, format!("{ORIGIN}/elsewhere"), 9000, true)
        })
    }
}

#[tokio::test]
async fn grant_only_composition_serves_no_media_even_for_a_valid_origin_token() {
    let fixture = Fixture::new();
    let path = media_path(&fixture.authorize().await);
    let delivery = Arc::new(GrantOnly::default());
    let grant_only = fixture
        .service
        .clone()
        .with_playback_delivery(delivery.clone());
    let response = playback_contract_router(grant_only)
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_not_found(response, &Method::GET).await;
    assert_eq!(fixture.objects.opens(), 0);
}
