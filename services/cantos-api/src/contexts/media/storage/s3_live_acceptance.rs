//! Operator-run acceptance of a real S3-compatible private bucket (Long Vân, R2, B2, ...).
//!
//! This is evidence acquisition, not a gate: the test is `#[ignore]`d, no xtask target runs it
//! and it refuses to start without an explicit acknowledgement. It exercises the production
//! [`S3Storage`] adapter (signing, addressing, conditional create, pinned reads, ranges,
//! grants) plus a few raw requests the adapter intentionally never issues, and prints a
//! redacted ledger. Credentials, endpoint, bucket and signed URLs are never printed. Every
//! unrun or unobservable capability stays `NOT_RUN`/`UNSUPPORTED`; a mocked or skipped check is
//! never reported as a pass. The only effects are random probe objects under the adapter's fixed
//! private prefix and, at the end, deletion of exactly those objects.
//!
//! See `docs/evidence/longvan-s3-acceptance.md` for the command, the inputs and the criteria.

use futures_util::TryStreamExt;

use super::*;
use crate::contexts::media::domain::object_storage::ByteRange;

const ACKNOWLEDGE_VALUE: &str = "write-and-delete-private-probe-objects";
const PRESIGN_EXPIRY_WAIT: Duration = Duration::from_secs(6);
const MIB: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Pass,
    Fail(&'static str),
    NotRun(&'static str),
    Unsupported(&'static str),
    UnsupportedByPermission(&'static str),
    /// Observed behavior the runbook says a human must interpret; never counted as a pass.
    Review(&'static str),
}

#[derive(Default)]
struct Ledger {
    entries: Vec<(&'static str, Outcome)>,
    observed: Vec<(&'static str, String)>,
}

impl Ledger {
    fn record(&mut self, name: &'static str, outcome: Outcome) -> Outcome {
        self.entries.push((name, outcome));
        outcome
    }

    fn observe(&mut self, name: &'static str, value: impl Into<String>) {
        self.observed.push((name, value.into()));
    }

    fn outcome(&self, name: &str) -> Option<Outcome> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == name)
            .map(|(_, outcome)| *outcome)
    }

    fn has_failure(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, Outcome::Fail(_)))
    }

    fn print(&self) {
        println!("live_s3_acceptance:");
        for (name, outcome) in &self.entries {
            let (status, note) = match outcome {
                Outcome::Pass => ("PASS", ""),
                Outcome::Fail(note) => ("FAIL", *note),
                Outcome::NotRun(note) => ("NOT_RUN", *note),
                Outcome::Unsupported(note) => ("UNSUPPORTED", *note),
                Outcome::UnsupportedByPermission(note) => ("UNSUPPORTED_BY_PERMISSION", *note),
                Outcome::Review(note) => ("REVIEW", *note),
            };
            if note.is_empty() {
                println!("  {name}: {status}");
            } else {
                println!("  {name}: {status}  # {note}");
            }
        }
        println!("  observed:");
        for (name, value) in &self.observed {
            println!("    {name}: {value}");
        }
    }
}

struct Environment {
    endpoint: String,
    region: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    force_path_style: bool,
    presigned_reads: bool,
    full_envelope: bool,
    request_timeout_ms: u64,
    /// Only the in-process fake provider tests permit plain-HTTP loopback.
    allow_loopback: bool,
}

impl Environment {
    /// Names only are ever reported; values are credentials or private topology.
    fn load() -> Self {
        Self::from_lookup(&|name| std::env::var(name).ok())
            .unwrap_or_else(|message| panic!("{message}"))
    }

    fn from_lookup(get: &dyn Fn(&str) -> Option<String>) -> Result<Self, String> {
        const REQUIRED: [&str; 6] = [
            "CANTOS_LIVE_S3_ACKNOWLEDGE",
            "CANTOS_LIVE_S3_ENDPOINT",
            "CANTOS_LIVE_S3_REGION",
            "CANTOS_LIVE_S3_BUCKET",
            "CANTOS_LIVE_S3_ACCESS_KEY_ID",
            "CANTOS_LIVE_S3_SECRET_ACCESS_KEY",
        ];
        let value = |name: &str| get(name).filter(|value| !value.is_empty());
        let missing: Vec<_> = REQUIRED
            .iter()
            .filter(|name| value(name).is_none())
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "NOT_RUN: refusing live acceptance; set {missing:?} (see docs/evidence/longvan-s3-acceptance.md)"
            ));
        }
        let text = |name: &str| value(name).unwrap_or_default();
        if text("CANTOS_LIVE_S3_ACKNOWLEDGE") != ACKNOWLEDGE_VALUE {
            return Err(
                "NOT_RUN: CANTOS_LIVE_S3_ACKNOWLEDGE must authorize writing and deleting probe objects"
                    .to_owned(),
            );
        }
        let flag = |name: &str, default: bool| match value(name).as_deref() {
            Some("1") | Some("true") => true,
            Some("0") | Some("false") => false,
            _ => default,
        };
        Ok(Self {
            endpoint: text("CANTOS_LIVE_S3_ENDPOINT"),
            region: text("CANTOS_LIVE_S3_REGION"),
            bucket: text("CANTOS_LIVE_S3_BUCKET"),
            access_key: text("CANTOS_LIVE_S3_ACCESS_KEY_ID"),
            secret_key: text("CANTOS_LIVE_S3_SECRET_ACCESS_KEY"),
            force_path_style: flag("CANTOS_LIVE_S3_PATH_STYLE", true),
            presigned_reads: flag("CANTOS_LIVE_S3_PRESIGNED_READS", false),
            full_envelope: flag("CANTOS_LIVE_S3_FULL_ENVELOPE", false),
            request_timeout_ms: value("CANTOS_LIVE_S3_REQUEST_TIMEOUT_MS")
                .and_then(|value| value.parse().ok())
                .unwrap_or(60_000),
            allow_loopback: false,
        })
    }

    fn settings(&self) -> S3Settings {
        S3Settings {
            endpoint: self.endpoint.clone(),
            region: self.region.clone(),
            bucket: self.bucket.clone(),
            force_path_style: self.force_path_style,
            connect_timeout_ms: 5_000,
            request_timeout_ms: self.request_timeout_ms,
            conditional_put: true,
            range_reads: true,
            presigned_reads: self.presigned_reads,
        }
    }

    fn storage(&self) -> Result<S3Storage, StorageError> {
        S3Storage::build(
            self.settings(),
            self.access_key.clone(),
            self.secret_key.clone(),
            None,
            self.allow_loopback,
        )
    }

    fn plain_client(&self) -> Client {
        https_client(self.allow_loopback)
    }
}

/// Same transport policy as the adapter (rustls, no redirects, no environment proxy), without
/// credentials. Plain-HTTP loopback is allowed only for the in-process fake provider tests.
fn https_client(allow_loopback: bool) -> Client {
    Client::builder()
        .https_only(!allow_loopback)
        .redirect(redirect::Policy::none())
        .no_proxy()
        .no_gzip()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("static client options are valid")
}

fn probe_bytes(length: usize) -> Vec<u8> {
    let mut bytes = vec![0_u8; length];
    getrandom::fill(&mut bytes).expect("operating system randomness");
    bytes
}

fn expected(bytes: &[u8]) -> (ObjectKey, ObjectIntegrity) {
    (
        ObjectKey::for_bytes(bytes),
        ObjectIntegrity::for_bytes(bytes).expect("probe sizes are within the object envelope"),
    )
}

async fn collect(read: StorageRead) -> Result<Vec<u8>, StorageError> {
    let chunks: Vec<_> = read.stream.try_collect().await?;
    Ok(chunks.into_iter().flatten().collect())
}

/// The reason is inferred from the error chain only; the chain itself (which can embed a URL)
/// is never printed.
fn transport_reason(error: &reqwest::Error) -> &'static str {
    let mut chain = String::new();
    let mut source: Option<&dyn std::error::Error> = Some(error);
    while let Some(error) = source {
        chain.push_str(&error.to_string().to_ascii_lowercase());
        chain.push('|');
        source = error.source();
    }
    if chain.contains("certificate") || chain.contains("unknownissuer") {
        "tls_certificate_not_trusted: check TLS inspection or a private CA; never disable verification"
    } else if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect_failed: DNS or network"
    } else {
        "transport_error"
    }
}

async fn raw_put(
    storage: &S3Storage,
    key: &ObjectKey,
    body: &[u8],
    conditional: bool,
) -> Result<StatusCode, StorageError> {
    let digest = format!("{:x}", Sha256::digest(body));
    let mut headers = vec![
        ("content-type", "audio/ogg".to_owned()),
        ("content-length", body.len().to_string()),
    ];
    if conditional {
        headers.insert(0, ("if-none-match", "*".to_owned()));
    }
    let response = storage
        .request(
            Method::PUT,
            storage.object_url(key)?,
            &digest,
            &headers,
            Some(Bytes::copy_from_slice(body)),
        )
        .await?;
    Ok(response.status())
}

async fn raw_signed(
    storage: &S3Storage,
    method: Method,
    key: &ObjectKey,
) -> Result<StatusCode, StorageError> {
    let response = storage
        .request(
            method,
            storage.object_url(key)?,
            EMPTY_PAYLOAD_SHA256,
            &[],
            None,
        )
        .await?;
    Ok(response.status())
}

async fn tls_preflight(client: &Client, endpoint: &str) -> Outcome {
    match client.get(endpoint).send().await {
        // Any HTTP answer proves DNS, TCP and TLS verification; authorization is checked later.
        Ok(_) => Outcome::Pass,
        Err(error) => Outcome::Fail(transport_reason(&error)),
    }
}

async fn put_and_verify(
    storage: &S3Storage,
    created: &mut Vec<ObjectKey>,
    bytes: &[u8],
) -> Result<(ObjectKey, ObjectIntegrity), StorageError> {
    let (key, integrity) = expected(bytes);
    created.push(key.clone());
    storage
        .put_bytes(
            key.clone(),
            integrity.clone(),
            Bytes::copy_from_slice(bytes),
            "audio/ogg",
        )
        .await?;
    Ok((key, integrity))
}

async fn conditional_create(storage: &S3Storage, created: &mut Vec<ObjectKey>) -> Outcome {
    let original = probe_bytes(2048);
    let (key, integrity) = match put_and_verify(storage, created, &original).await {
        Ok(object) => object,
        Err(_) => return Outcome::NotRun("seed_put_failed"),
    };
    // Different bytes at the same key, which the adapter can never send, isolate whether the
    // provider enforces `If-None-Match: *` rather than silently overwriting an immutable object.
    let intruder = probe_bytes(2048);
    match raw_put(storage, &key, &intruder, true).await {
        Ok(StatusCode::PRECONDITION_FAILED) => {
            match storage.verify_location(&key, &integrity).await {
                Ok(_) => Outcome::Pass,
                Err(_) => Outcome::Fail("object_changed_after_412"),
            }
        }
        Ok(StatusCode::OK) => Outcome::Fail("if_none_match_ignored: duplicate create accepted"),
        Ok(
            StatusCode::BAD_REQUEST | StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED,
        ) => Outcome::Unsupported("conditional_create_header_rejected"),
        Ok(StatusCode::CONFLICT) => Outcome::Review("http_409_concurrent_create_semantics"),
        Ok(StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED) => Outcome::Fail("forbidden"),
        Ok(_) => Outcome::Fail("unexpected_status"),
        Err(error) => Outcome::Fail(error.code()),
    }
}

async fn ranges(storage: &S3Storage, key: &ObjectKey, bytes: &[u8]) -> Outcome {
    let integrity = ObjectIntegrity::for_bytes(bytes).expect("probe sizes are valid");
    let total = bytes.len() as u64;
    for specification in [
        "bytes=0-1023",
        "bytes=1500-2999",
        "bytes=3072-",
        "bytes=-100",
    ] {
        let range = ByteRange::parse(specification)
            .and_then(|range| range.normalize(total))
            .expect("fixed probe ranges are satisfiable");
        let read = match storage
            .read_verified(key.clone(), integrity.clone(), Some(range))
            .await
        {
            Ok(read) => read,
            Err(error) => return Outcome::Fail(error.code()),
        };
        match collect(read).await {
            Ok(body) if body == bytes[range.first() as usize..=range.last() as usize] => {}
            Ok(_) => return Outcome::Fail("range_bytes_differ"),
            Err(error) => return Outcome::Fail(error.code()),
        }
    }
    Outcome::Pass
}

async fn anonymous_denial(
    environment: &Environment,
    storage: &S3Storage,
    key: &ObjectKey,
) -> (Outcome, Outcome) {
    let client = environment.plain_client();
    let object = match storage.object_url(key) {
        Ok(url) => url,
        Err(error) => return (Outcome::Fail(error.code()), Outcome::Fail(error.code())),
    };
    let denial = |status: StatusCode| match status {
        StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => Outcome::Pass,
        StatusCode::OK | StatusCode::PARTIAL_CONTENT => Outcome::Fail("anonymous_read_allowed"),
        StatusCode::NOT_FOUND => Outcome::Review("provider_masks_with_404"),
        _ => Outcome::Review("unexpected_anonymous_status"),
    };
    let object_outcome = match client.get(object.clone()).send().await {
        Ok(response) => denial(response.status()),
        Err(error) => Outcome::Fail(transport_reason(&error)),
    };
    let mut listing = object;
    if storage.settings.force_path_style {
        listing.set_path(&format!("/{}", storage.settings.bucket));
    } else {
        listing.set_path("/");
    }
    listing.set_query(Some("list-type=2&prefix=private%2Faudio%2F&max-keys=1"));
    let listing_outcome = match client.get(listing).send().await {
        Ok(response) => denial(response.status()),
        Err(error) => Outcome::Fail(transport_reason(&error)),
    };
    (object_outcome, listing_outcome)
}

async fn presigned(
    environment: &Environment,
    storage: &S3Storage,
    key: &ObjectKey,
    integrity: &ObjectIntegrity,
    bytes: &[u8],
) -> [Outcome; 3] {
    let client = environment.plain_client();
    let grant = match storage
        .temporary_read(key.clone(), integrity.clone(), 60)
        .await
    {
        Ok(Some(grant)) => grant,
        Ok(None) => {
            let reason = "no_usable_version_id: protected API stream stays the delivery path";
            return [Outcome::Unsupported(reason); 3];
        }
        Err(error) => return [Outcome::Fail(error.code()); 3],
    };
    let (url, _) = grant.into_parts();
    let range_get = match client
        .get(&url)
        .header(header::RANGE, "bytes=0-1023")
        .send()
        .await
    {
        Ok(response) if response.status() == StatusCode::PARTIAL_CONTENT => {
            match response.bytes().await {
                Ok(body) if body[..] == bytes[..1024] => Outcome::Pass,
                _ => Outcome::Fail("presigned_range_bytes_differ"),
            }
        }
        Ok(response) if response.status() == StatusCode::OK => {
            Outcome::Fail("presigned_range_ignored_http_200")
        }
        Ok(_) => Outcome::Fail("presigned_get_rejected"),
        Err(error) => Outcome::Fail(transport_reason(&error)),
    };
    let unsigned = match Url::parse(&url) {
        Ok(mut url) => {
            url.set_query(None);
            match client.get(url).send().await {
                Ok(response) if response.status() == StatusCode::FORBIDDEN => Outcome::Pass,
                Ok(_) => Outcome::Fail("unsigned_url_not_denied"),
                Err(error) => Outcome::Fail(transport_reason(&error)),
            }
        }
        Err(_) => Outcome::Fail("grant_url_unparsable"),
    };
    let expiry = match storage
        .temporary_read(key.clone(), integrity.clone(), 1)
        .await
    {
        Ok(Some(short)) => {
            let (url, _) = short.into_parts();
            tokio::time::sleep(PRESIGN_EXPIRY_WAIT).await;
            // A new request after the TTL (with skew), not a transfer begun before it.
            match client.get(&url).send().await {
                Ok(response) if response.status() == StatusCode::FORBIDDEN => Outcome::Pass,
                Ok(response) if response.status().is_success() => {
                    Outcome::Fail("expired_grant_still_valid")
                }
                Ok(_) => Outcome::Review("expired_grant_unusual_status"),
                Err(error) => Outcome::Fail(transport_reason(&error)),
            }
        }
        Ok(None) => Outcome::NotRun("short_grant_not_issued"),
        Err(error) => Outcome::Fail(error.code()),
    };
    [range_get, unsigned, expiry]
}

async fn size_ladder(
    environment: &Environment,
    storage: &S3Storage,
    created: &mut Vec<ObjectKey>,
) -> Outcome {
    let mut sizes = vec![MIB + 1, 8 * MIB];
    if environment.full_envelope {
        sizes.push(MAX_OBJECT_BYTES as usize);
    }
    for size in sizes {
        let bytes = probe_bytes(size);
        let (key, integrity) = expected(&bytes);
        created.push(key.clone());
        match raw_put(storage, &key, &bytes, true).await {
            Ok(StatusCode::OK) => {}
            Ok(StatusCode::PAYLOAD_TOO_LARGE) => {
                return Outcome::Fail("http_413_gateway_body_limit_below_object_envelope")
            }
            Ok(StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT) => {
                return Outcome::Fail("timeout_raise_CANTOS_LIVE_S3_REQUEST_TIMEOUT_MS")
            }
            Ok(_) => return Outcome::Fail("unexpected_put_status"),
            Err(error) => return Outcome::Fail(error.code()),
        }
        if let Err(error) = storage.verify_location(&key, &integrity).await {
            return Outcome::Fail(error.code());
        }
    }
    Outcome::Pass
}

/// Deletes exactly the random probe objects this run created. The production port has no
/// delete; this test-only effect never targets an object it did not just generate.
async fn cleanup(storage: &S3Storage, created: &[ObjectKey], ledger: &mut Ledger) {
    let mut denied = false;
    let mut failed = false;
    for key in created {
        match raw_signed(storage, Method::DELETE, key).await {
            Ok(StatusCode::NO_CONTENT | StatusCode::OK | StatusCode::NOT_FOUND) => {}
            Ok(StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED) => denied = true,
            _ => failed = true,
        }
    }
    let outcome = if denied {
        for key in created {
            ledger.observe("orphaned_probe_key", key.as_str());
        }
        Outcome::UnsupportedByPermission("delete_forbidden: probe objects remain, remove manually")
    } else if failed {
        Outcome::Fail("delete_failed_probe_objects_may_remain")
    } else {
        let mut gone = true;
        for key in created {
            gone &= matches!(
                raw_signed(storage, Method::GET, key).await,
                Ok(StatusCode::NOT_FOUND)
            );
        }
        if gone {
            Outcome::Pass
        } else {
            Outcome::Fail("object_still_readable_after_delete")
        }
    };
    ledger.record("cleanup_current_object", outcome);
}

/// Runs every check, records each outcome and always attempts cleanup of its own probe objects.
/// A failed check never skips a later independent check or the cleanup.
async fn run(environment: &Environment) -> Ledger {
    let mut ledger = Ledger::default();
    let preflight = ledger.record(
        "tls_dns_preflight",
        tls_preflight(&environment.plain_client(), &environment.endpoint).await,
    );
    if matches!(preflight, Outcome::Fail(_)) {
        // Nothing was written; the remaining capabilities were not exercised.
        return ledger;
    }
    let storage = match environment.storage() {
        Ok(storage) => storage,
        Err(error) => {
            ledger.record("adapter_configuration", Outcome::Fail(error.code()));
            return ledger;
        }
    };
    let mut created = Vec::new();

    let probe = probe_bytes(4096);
    let seeded = put_and_verify(&storage, &mut created, &probe).await;
    ledger.record(
        "put_head_get_integrity",
        match &seeded {
            Ok(_) => Outcome::Pass,
            Err(error) => Outcome::Fail(error.code()),
        },
    );
    if let Ok((key, integrity)) = &seeded {
        ledger.record(
            "duplicate_put_idempotent",
            match put_and_verify(&storage, &mut Vec::new(), &probe).await {
                Ok(_) => Outcome::Pass,
                Err(error) => Outcome::Fail(error.code()),
            },
        );
        match storage.verify_location(key, integrity).await {
            Ok(location) => {
                let pin = match (&location.version_id, &location.etag) {
                    (Some(_), _) => "version_id",
                    (None, Some(_)) => "strong_etag",
                    (None, None) => "none",
                };
                ledger.observe("read_pin", pin);
                ledger.observe(
                    "bucket_versioning_observed",
                    location.version_id.is_some().to_string(),
                );
                ledger.record(
                    "read_pin_available",
                    if pin == "none" {
                        Outcome::Fail("no_version_or_strong_etag: reads fail closed")
                    } else {
                        Outcome::Pass
                    },
                );
            }
            Err(error) => {
                ledger.record("read_pin_available", Outcome::Fail(error.code()));
            }
        }
        ledger.record(
            "full_read_actual_bytes",
            match storage
                .read_verified(key.clone(), integrity.clone(), None)
                .await
            {
                Ok(read) => match collect(read).await {
                    Ok(body) if body == probe => Outcome::Pass,
                    Ok(_) => Outcome::Fail("full_read_bytes_differ"),
                    Err(error) => Outcome::Fail(error.code()),
                },
                Err(error) => Outcome::Fail(error.code()),
            },
        );
        ledger.record(
            "range_206_first_middle_final_suffix",
            ranges(&storage, key, &probe).await,
        );
        let (object, listing) = anonymous_denial(environment, &storage, key).await;
        ledger.record("anonymous_object_denied", object);
        ledger.record("anonymous_listing_denied", listing);
        if environment.presigned_reads {
            let [range_get, unsigned, expiry] =
                presigned(environment, &storage, key, integrity, &probe).await;
            ledger.record("presigned_get_range_206", range_get);
            ledger.record("presigned_unsigned_denied", unsigned);
            ledger.record("presigned_expired_denied", expiry);
        } else {
            for name in [
                "presigned_get_range_206",
                "presigned_unsigned_denied",
                "presigned_expired_denied",
            ] {
                ledger.record(
                    name,
                    Outcome::NotRun("CANTOS_LIVE_S3_PRESIGNED_READS not set"),
                );
            }
        }
    }
    ledger.record(
        "conditional_create_enforced",
        conditional_create(&storage, &mut created).await,
    );
    ledger.record(
        "size_ladder_up_to_envelope",
        size_ladder(environment, &storage, &mut created).await,
    );
    if !environment.full_envelope {
        ledger.observe(
            "size_ladder_max",
            "8 MiB; set CANTOS_LIVE_S3_FULL_ENVELOPE=1 for 32 MiB",
        );
    }

    cleanup(&storage, &created, &mut ledger).await;
    for (name, reason) in [
        (
            "cors_web",
            "browser/DevTools check from the real Studio origin",
        ),
        (
            "multipart_abort",
            "no multipart consumer exists in this adapter",
        ),
        (
            "opus_play_seek_web",
            "no Theatre/Studio player consumes these objects yet",
        ),
        (
            "opus_play_seek_cmp",
            "no Theatre/Studio player consumes these objects yet",
        ),
        (
            "performance_measurement",
            "deferred; probe requests are not a performance test",
        ),
    ] {
        ledger.record(name, Outcome::NotRun(reason));
    }
    ledger
}

/// First, credential-free step: can this machine verify the endpoint's TLS chain with the
/// adapter's own transport policy? It sends one unauthenticated GET of the endpoint root and
/// writes nothing.
#[tokio::test]
#[ignore = "operator-run: one unauthenticated GET of the endpoint root; no credentials, no writes"]
async fn live_endpoint_tls_preflight() {
    let endpoint = std::env::var("CANTOS_LIVE_S3_ENDPOINT")
        .expect("NOT_RUN: set CANTOS_LIVE_S3_ENDPOINT to the HTTPS endpoint under test");
    let outcome = tls_preflight(&https_client(false), &endpoint).await;
    println!("tls_dns_preflight: {outcome:?}");
    assert_eq!(outcome, Outcome::Pass);
}

#[tokio::test]
#[ignore = "operator-run live provider acceptance; writes then deletes private probe objects; never part of a gate"]
async fn live_private_bucket_acceptance() {
    let environment = Environment::load();
    let ledger = run(&environment).await;
    ledger.print();
    assert!(
        !ledger.has_failure(),
        "live acceptance recorded FAIL entries; the target is not accepted"
    );
}

#[test]
fn ledger_never_reports_a_review_or_not_run_entry_as_a_failure_or_pass() {
    let mut ledger = Ledger::default();
    ledger.record("a", Outcome::Review("x"));
    ledger.record("b", Outcome::NotRun("x"));
    ledger.record("c", Outcome::Unsupported("x"));
    ledger.record("d", Outcome::UnsupportedByPermission("x"));
    assert!(!ledger.has_failure());
    ledger.record("e", Outcome::Fail("x"));
    assert!(ledger.has_failure());
}

#[tokio::test]
async fn transport_reason_never_echoes_the_error_chain() {
    // A refused loopback connection: the reason is a fixed phrase, never the URL or query.
    let error = Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .build()
        .unwrap()
        .get("http://127.0.0.1:9/secret-path?X-Amz-Signature=sentinel")
        .send()
        .await
        .unwrap_err();
    let reason = transport_reason(&error);
    assert!(!reason.contains("sentinel") && !reason.contains("127.0.0.1"));
}

/// An in-process, in-memory provider with switchable misbehavior. It validates no signature
/// (the signer has its own independent vectors); it only decides what a provider would answer,
/// so these tests prove the harness classifies behaviors correctly before an operator spends
/// time and a real account on it. Passing here is mocked evidence, never provider evidence.
mod fake_provider_tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use axum::{
        body::{to_bytes, Body},
        extract::State,
        http::{Request, Response as HttpResponse},
        routing::any,
        Router,
    };
    use tokio::{net::TcpListener, task::JoinHandle};

    use super::*;

    const BUCKET: &str = "probe-bucket";

    /// Key to (bytes, opaque strong ETag).
    type Objects = HashMap<String, (Vec<u8>, String)>;

    #[derive(Clone, Copy)]
    struct Behavior {
        honor_if_none_match: bool,
        max_body_bytes: usize,
        public_read: bool,
    }

    impl Behavior {
        const WELL_BEHAVED: Self = Self {
            honor_if_none_match: true,
            max_body_bytes: MAX_OBJECT_BYTES as usize,
            public_read: false,
        };
    }

    #[derive(Clone)]
    struct Provider {
        behavior: Behavior,
        objects: Arc<Mutex<Objects>>,
    }

    struct Running {
        endpoint: String,
        provider: Provider,
        handle: JoinHandle<()>,
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    fn reply(
        status: StatusCode,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    ) -> HttpResponse<Body> {
        let mut response = HttpResponse::builder().status(status);
        for (name, value) in headers {
            response = response.header(name, value);
        }
        response.body(Body::from(body)).unwrap()
    }

    /// `bytes=a-b`, `bytes=a-` and `bytes=-n` against an object of `length` bytes.
    fn parse_range(raw: &str, length: usize) -> Option<(usize, usize)> {
        let (first, last) = raw.strip_prefix("bytes=")?.split_once('-')?;
        let last_index = length.checked_sub(1)?;
        let (first, last) = match (first.is_empty(), last.is_empty()) {
            (false, false) => (
                first.parse().ok()?,
                last.parse::<usize>().ok()?.min(last_index),
            ),
            (false, true) => (first.parse().ok()?, last_index),
            (true, false) => (length.saturating_sub(last.parse().ok()?), last_index),
            (true, true) => return None,
        };
        (first <= last).then_some((first, last))
    }

    async fn handle(
        State(provider): State<Provider>,
        request: Request<Body>,
    ) -> HttpResponse<Body> {
        let (parts, body) = request.into_parts();
        let authorized = parts.headers.contains_key(header::AUTHORIZATION);
        let body = to_bytes(body, 64 * MIB).await.unwrap().to_vec();
        let path = parts.uri.path();
        let prefix = format!("/{BUCKET}/private/audio/");
        let Some(key) = path.strip_prefix(&prefix) else {
            // Bucket listing and the endpoint root.
            let listing = path == format!("/{BUCKET}");
            return if listing && !authorized && !provider.behavior.public_read {
                reply(StatusCode::FORBIDDEN, vec![], vec![])
            } else {
                reply(StatusCode::OK, vec![], b"<ListBucketResult/>".to_vec())
            };
        };
        let key = key.to_owned();
        let mut objects = provider.objects.lock().unwrap();
        match parts.method {
            Method::PUT if !authorized => reply(StatusCode::FORBIDDEN, vec![], vec![]),
            Method::PUT if body.len() > provider.behavior.max_body_bytes => {
                reply(StatusCode::PAYLOAD_TOO_LARGE, vec![], vec![])
            }
            Method::PUT => {
                let duplicate_guard = parts
                    .headers
                    .get("if-none-match")
                    .is_some_and(|value| value == "*");
                if duplicate_guard
                    && provider.behavior.honor_if_none_match
                    && objects.contains_key(&key)
                {
                    return reply(StatusCode::PRECONDITION_FAILED, vec![], vec![]);
                }
                let etag = format!("\"{}\"", &format!("{:x}", Sha256::digest(&body))[..32]);
                objects.insert(key, (body, etag.clone()));
                reply(StatusCode::OK, vec![("etag", etag)], vec![])
            }
            Method::DELETE if !authorized => reply(StatusCode::FORBIDDEN, vec![], vec![]),
            Method::DELETE => {
                objects.remove(&key);
                reply(StatusCode::NO_CONTENT, vec![], vec![])
            }
            Method::GET => {
                if !authorized && !provider.behavior.public_read {
                    return reply(StatusCode::FORBIDDEN, vec![], vec![]);
                }
                let Some((bytes, etag)) = objects.get(&key) else {
                    return reply(StatusCode::NOT_FOUND, vec![], vec![]);
                };
                let pin = parts
                    .headers
                    .get("if-match")
                    .and_then(|value| value.to_str().ok());
                if pin.is_some_and(|pin| pin != etag) {
                    return reply(StatusCode::PRECONDITION_FAILED, vec![], vec![]);
                }
                let range = parts
                    .headers
                    .get("range")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|raw| parse_range(raw, bytes.len()));
                match range {
                    Some((first, last)) => reply(
                        StatusCode::PARTIAL_CONTENT,
                        vec![
                            ("etag", etag.clone()),
                            ("content-length", (last - first + 1).to_string()),
                            (
                                "content-range",
                                format!("bytes {first}-{last}/{}", bytes.len()),
                            ),
                        ],
                        bytes[first..=last].to_vec(),
                    ),
                    None => reply(
                        StatusCode::OK,
                        vec![
                            ("etag", etag.clone()),
                            ("content-length", bytes.len().to_string()),
                        ],
                        bytes.clone(),
                    ),
                }
            }
            _ => reply(StatusCode::METHOD_NOT_ALLOWED, vec![], vec![]),
        }
    }

    async fn serve(behavior: Behavior) -> Running {
        let provider = Provider {
            behavior,
            objects: Arc::new(Mutex::new(HashMap::new())),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new()
            .route("/", any(handle))
            .route("/{*path}", any(handle))
            .with_state(provider.clone());
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Running {
            endpoint,
            provider,
            handle,
        }
    }

    fn environment(endpoint: &str) -> Environment {
        Environment {
            endpoint: endpoint.to_owned(),
            region: "us-east-1".to_owned(),
            bucket: BUCKET.to_owned(),
            access_key: "EXAMPLE".to_owned(),
            secret_key: "secret-sentinel".to_owned(),
            force_path_style: true,
            presigned_reads: false,
            full_envelope: false,
            request_timeout_ms: 20_000,
            allow_loopback: true,
        }
    }

    #[tokio::test]
    async fn well_behaved_provider_passes_every_runnable_check_and_cleans_up_only_its_probes() {
        let running = serve(Behavior::WELL_BEHAVED).await;
        let ledger = run(&environment(&running.endpoint)).await;
        ledger.print();
        for name in [
            "tls_dns_preflight",
            "put_head_get_integrity",
            "duplicate_put_idempotent",
            "read_pin_available",
            "full_read_actual_bytes",
            "range_206_first_middle_final_suffix",
            "anonymous_object_denied",
            "anonymous_listing_denied",
            "conditional_create_enforced",
            "size_ladder_up_to_envelope",
            "cleanup_current_object",
        ] {
            assert_eq!(ledger.outcome(name), Some(Outcome::Pass), "{name}");
        }
        // Capabilities this run cannot exercise are never promoted to a pass.
        for name in [
            "presigned_get_range_206",
            "cors_web",
            "multipart_abort",
            "performance_measurement",
        ] {
            assert!(
                matches!(ledger.outcome(name), Some(Outcome::NotRun(_))),
                "{name}"
            );
        }
        assert!(!ledger.has_failure());
        assert!(running.provider.objects.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn provider_that_ignores_if_none_match_fails_instead_of_passing_and_still_cleans_up() {
        let running = serve(Behavior {
            honor_if_none_match: false,
            ..Behavior::WELL_BEHAVED
        })
        .await;
        let ledger = run(&environment(&running.endpoint)).await;
        assert!(matches!(
            ledger.outcome("conditional_create_enforced"),
            Some(Outcome::Fail(_))
        ));
        assert!(ledger.has_failure());
        assert!(running.provider.objects.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn gateway_body_limit_below_the_object_envelope_fails_the_size_ladder() {
        let running = serve(Behavior {
            max_body_bytes: MIB,
            ..Behavior::WELL_BEHAVED
        })
        .await;
        let ledger = run(&environment(&running.endpoint)).await;
        assert_eq!(
            ledger.outcome("size_ladder_up_to_envelope"),
            Some(Outcome::Fail(
                "http_413_gateway_body_limit_below_object_envelope"
            ))
        );
        // Independent checks are not skipped by that failure.
        assert_eq!(
            ledger.outcome("put_head_get_integrity"),
            Some(Outcome::Pass)
        );
        assert!(running.provider.objects.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn publicly_readable_object_or_listing_is_a_failure() {
        let running = serve(Behavior {
            public_read: true,
            ..Behavior::WELL_BEHAVED
        })
        .await;
        let ledger = run(&environment(&running.endpoint)).await;
        assert_eq!(
            ledger.outcome("anonymous_object_denied"),
            Some(Outcome::Fail("anonymous_read_allowed"))
        );
        assert_eq!(
            ledger.outcome("anonymous_listing_denied"),
            Some(Outcome::Fail("anonymous_read_allowed"))
        );
    }

    #[tokio::test]
    async fn unreachable_endpoint_writes_nothing_and_reports_a_fixed_reason() {
        // Port 9 on loopback refuses connections; no request carrying credentials is sent.
        let ledger = run(&environment("http://127.0.0.1:9")).await;
        assert!(matches!(
            ledger.outcome("tls_dns_preflight"),
            Some(Outcome::Fail(_))
        ));
        assert_eq!(ledger.entries.len(), 1);
    }

    #[test]
    fn environment_refuses_without_every_input_and_the_exact_acknowledgement_and_never_echoes_values(
    ) {
        let complete = |name: &str| match name {
            "CANTOS_LIVE_S3_ACKNOWLEDGE" => Some(ACKNOWLEDGE_VALUE.to_owned()),
            "CANTOS_LIVE_S3_SECRET_ACCESS_KEY" => Some("secret-sentinel".to_owned()),
            "CANTOS_LIVE_S3_REQUEST_TIMEOUT_MS" | "CANTOS_LIVE_S3_PRESIGNED_READS" => None,
            _ => Some("value".to_owned()),
        };
        assert!(Environment::from_lookup(&complete).is_ok());
        let missing_secret = Environment::from_lookup(&|name| {
            (name != "CANTOS_LIVE_S3_SECRET_ACCESS_KEY")
                .then(|| complete(name))
                .flatten()
        })
        .err()
        .unwrap();
        assert!(missing_secret.contains("CANTOS_LIVE_S3_SECRET_ACCESS_KEY"));
        let wrong_ack = Environment::from_lookup(&|name| {
            if name == "CANTOS_LIVE_S3_ACKNOWLEDGE" {
                Some("yes".to_owned())
            } else {
                complete(name)
            }
        })
        .err()
        .unwrap();
        assert!(wrong_ack.starts_with("NOT_RUN"));
        for message in [missing_secret, wrong_ack] {
            assert!(!message.contains("secret-sentinel"));
        }
    }
}
