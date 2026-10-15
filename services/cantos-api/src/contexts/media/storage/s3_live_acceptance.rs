//! Operator-run acceptance of a real S3-compatible private bucket (Long Vân, R2, B2, ...).
//!
//! This is evidence acquisition, not a gate: the test is `#[ignore]`d, no xtask target runs it
//! and it refuses to start without an explicit acknowledgement. It exercises the production
//! [`S3Storage`] adapter (signing, addressing, conditional create, pinned reads, ranges,
//! grants) plus a few raw requests the adapter intentionally never issues, and prints a
//! redacted ledger. Credentials, endpoint, bucket and signed URLs are never printed. Every
//! unrun or unobservable capability stays `NOT_RUN`/`UNSUPPORTED`; a mocked or skipped check is
//! never reported as a pass. The only effects are random probe objects under the adapter's fixed
//! private prefix and, only while ownership and integrity remain certain, conditional deletion
//! of this run's unversioned probes. Uncertain or versioned objects are retained and reported.
//!
//! See `docs/evidence/longvan-s3-acceptance.md` for the command, the inputs and the criteria.

use futures_util::TryStreamExt;

use super::*;
use crate::contexts::media::domain::object_storage::ByteRange;

#[path = "s3_live_acceptance/privacy.rs"]
mod privacy;

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

    fn unsafe_to_mutate(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, Outcome::Fail(_) | Outcome::Review(_)))
            || ["anonymous_object_denied", "anonymous_listing_denied"]
                .iter()
                .any(|name| self.outcome(name) != Some(Outcome::Pass))
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

/// A successful seed HTTP response plus verified actual bytes and opaque read validators.
/// Random keys alone, a successful adapter duplicate, and a failed PUT prove no ownership.
struct OwnedProbe {
    key: ObjectKey,
    integrity: ObjectIntegrity,
    location: VerifiedLocation,
}

#[derive(Default)]
struct ProbeJournal {
    attempted: Vec<ObjectKey>,
    owned: Vec<OwnedProbe>,
    deleted: Vec<ObjectKey>,
    stopped: Option<&'static str>,
}

impl ProbeJournal {
    fn stop(&mut self, reason: &'static str) -> &'static str {
        self.stopped.get_or_insert(reason)
    }

    fn retained(&self, ledger: &mut Ledger) {
        for key in &self.attempted {
            if self.deleted.iter().any(|deleted| deleted == key) {
                continue;
            }
            let kind = if self.owned.iter().any(|probe| probe.key == *key) {
                "retained_owned_probe_key"
            } else {
                "orphaned_unproven_probe_key"
            };
            ledger.observe(kind, key.as_str());
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
    conditional_delete_verified: bool,
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
            conditional_delete_verified: flag("CANTOS_LIVE_S3_CONDITIONAL_DELETE_VERIFIED", false),
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
    probes: &mut ProbeJournal,
    bytes: &[u8],
) -> Result<(ObjectKey, ObjectIntegrity), &'static str> {
    if probes.stopped.is_some() {
        return Err("mutations_stopped");
    }
    let (key, integrity) = expected(bytes);
    // Only an explicit authenticated 404 is absence. Never turn denied/unavailable into absent.
    match raw_signed(storage, Method::GET, &key).await {
        Ok(StatusCode::NOT_FOUND) => {}
        Ok(StatusCode::OK) => return Err(probes.stop("probe_key_preexisting")),
        Ok(_) => return Err(probes.stop("probe_absence_unproven")),
        Err(_) => return Err(probes.stop("probe_absence_unavailable")),
    }
    probes.attempted.push(key.clone());
    // The adapter deliberately accepts 412 for idempotent duplicates. A fresh seed instead
    // requires a distinct successful create response; a racing 412 can never become ours.
    match raw_put(storage, &key, bytes, true).await {
        Ok(StatusCode::OK) => {}
        Ok(StatusCode::PRECONDITION_FAILED) => {
            return Err(probes.stop("seed_412_ownership_unproven"))
        }
        Ok(StatusCode::PAYLOAD_TOO_LARGE) => {
            return Err(probes.stop("http_413_gateway_body_limit_below_object_envelope"))
        }
        Ok(StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT) => {
            return Err(probes.stop("timeout_raise_CANTOS_LIVE_S3_REQUEST_TIMEOUT_MS"))
        }
        Ok(_) => return Err(probes.stop("seed_put_status_uncertain")),
        Err(_) => return Err(probes.stop("seed_put_outcome_uncertain")),
    }
    let location = storage
        .verify_location(&key, &integrity)
        .await
        .map_err(|_| probes.stop("seed_integrity_unproven"))?;
    if location.version_id.is_none() && location.etag.is_none() {
        return Err(probes.stop("seed_read_pin_unavailable"));
    }
    probes.owned.push(OwnedProbe {
        key: key.clone(),
        integrity: integrity.clone(),
        location,
    });
    Ok((key, integrity))
}

async fn verify_owned(storage: &S3Storage, probe: &OwnedProbe) -> Result<(), &'static str> {
    let current = storage
        .verify_location(&probe.key, &probe.integrity)
        .await
        .map_err(|_| "owned_probe_integrity_unproven")?;
    if current.version_id != probe.location.version_id || current.etag != probe.location.etag {
        return Err("owned_probe_pin_changed");
    }
    Ok(())
}

async fn conditional_create(storage: &S3Storage, probes: &mut ProbeJournal) -> Outcome {
    if probes.stopped.is_some() {
        return Outcome::NotRun("safety_stop");
    }
    let original = probe_bytes(2048);
    let (key, _) = match put_and_verify(storage, probes, &original).await {
        Ok(object) => object,
        Err(reason) => return Outcome::Fail(reason),
    };
    let owned = probes
        .owned
        .last()
        .expect("successful seed records ownership");
    if let Err(reason) = verify_owned(storage, owned).await {
        probes.stop(reason);
        return Outcome::Fail(reason);
    }
    // Different bytes at the same key, which the adapter can never send, isolate whether the
    // provider enforces `If-None-Match: *` rather than silently overwriting an immutable object.
    let intruder = probe_bytes(2048);
    let outcome = match raw_put(storage, &key, &intruder, true).await {
        Ok(StatusCode::PRECONDITION_FAILED) => match verify_owned(storage, owned).await {
            Ok(_) => Outcome::Pass,
            Err(_) => Outcome::Fail("object_changed_after_412"),
        },
        Ok(StatusCode::OK) => Outcome::Fail("if_none_match_ignored: duplicate create accepted"),
        Ok(
            StatusCode::BAD_REQUEST | StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED,
        ) => Outcome::Unsupported("conditional_create_header_rejected"),
        Ok(StatusCode::CONFLICT) => Outcome::Review("http_409_concurrent_create_semantics"),
        Ok(StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED) => Outcome::Fail("forbidden"),
        Ok(_) => Outcome::Fail("unexpected_status"),
        Err(error) => Outcome::Fail(error.code()),
    };
    if outcome != Outcome::Pass {
        probes.stop("conditional_create_failed_or_uncertain");
    }
    outcome
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
    integrity: &ObjectIntegrity,
    ledger: &mut Ledger,
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
    let diagnostics = privacy::anonymous_object_check(object.clone(), integrity).await;
    let object_outcome = diagnostics.outcome;
    for (name, value) in diagnostics.observations {
        ledger.observe(name, value);
    }
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
    probes: &mut ProbeJournal,
) -> Outcome {
    if probes.stopped.is_some() {
        return Outcome::NotRun("safety_stop");
    }
    let mut sizes = vec![MIB + 1, 8 * MIB];
    if environment.full_envelope {
        sizes.push(MAX_OBJECT_BYTES as usize);
    }
    for size in sizes {
        let bytes = probe_bytes(size);
        if let Err(reason) = put_and_verify(storage, probes, &bytes).await {
            return Outcome::Fail(reason);
        }
    }
    Outcome::Pass
}

/// No production delete port is added. Validate the entire owned set before the first DELETE,
/// then require the verified opaque validator in each conditional DELETE. Atomic protection
/// still depends on the provider honoring If-Match; fake tests do not establish that. Never retry.
async fn cleanup(
    storage: &S3Storage,
    probes: &mut ProbeJournal,
    conditional_delete_verified: bool,
    ledger: &mut Ledger,
) {
    if probes.stopped.is_some() {
        ledger.record("cleanup_current_object", Outcome::NotRun("safety_stop"));
        probes.retained(ledger);
        return;
    }
    if probes.owned.is_empty() {
        ledger.record(
            "cleanup_current_object",
            Outcome::NotRun("no_proven_owned_probe"),
        );
        probes.retained(ledger);
        return;
    }
    if !conditional_delete_verified {
        ledger.record(
            "cleanup_current_object",
            Outcome::NotRun("conditional_delete_not_verified"),
        );
        probes.retained(ledger);
        return;
    }
    // Deleting a version by ID would permanently purge it and widens the current-object
    // runbook scope; deleting the current version could leave old versions behind. Retain it.
    if probes
        .owned
        .iter()
        .any(|probe| probe.location.version_id.is_some())
    {
        ledger.record(
            "cleanup_current_object",
            Outcome::NotRun("versioned_probe_retained"),
        );
        probes.retained(ledger);
        return;
    }
    for probe in &probes.owned {
        if probe.location.etag.is_none() {
            probes.stop("cleanup_strong_etag_unavailable");
            ledger.record(
                "cleanup_current_object",
                Outcome::NotRun("cleanup_strong_etag_unavailable"),
            );
            probes.retained(ledger);
            return;
        }
        if let Err(reason) = verify_owned(storage, probe).await {
            probes.stop(reason);
            ledger.record("cleanup_current_object", Outcome::Fail(reason));
            probes.retained(ledger);
            return;
        }
    }
    for probe in &probes.owned {
        let etag = probe
            .location
            .etag
            .as_ref()
            .expect("all cleanup validators were checked before mutation");
        let response = storage
            .request(
                Method::DELETE,
                storage.object_url(&probe.key).expect("seeded key is valid"),
                EMPTY_PAYLOAD_SHA256,
                &[("if-match", etag.clone())],
                None,
            )
            .await;
        let outcome = match response {
            Ok(response)
                if matches!(response.status(), StatusCode::OK | StatusCode::NO_CONTENT) =>
            {
                match raw_signed(storage, Method::GET, &probe.key).await {
                    Ok(StatusCode::NOT_FOUND) => Outcome::Pass,
                    _ => Outcome::Fail("delete_absence_unconfirmed"),
                }
            }
            Ok(response)
                if matches!(
                    response.status(),
                    StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED
                ) =>
            {
                Outcome::UnsupportedByPermission(
                    "delete_forbidden: retained probes need operator review",
                )
            }
            Ok(_) => Outcome::Fail("conditional_delete_failed_or_object_changed"),
            Err(_) => Outcome::Fail("delete_outcome_uncertain"),
        };
        if outcome != Outcome::Pass {
            probes.stop("cleanup_failed_or_uncertain");
            ledger.record("cleanup_current_object", outcome);
            probes.retained(ledger);
            return;
        }
        probes.deleted.push(probe.key.clone());
    }
    ledger.record("cleanup_current_object", Outcome::Pass);
}

fn complete_ledger(mut ledger: Ledger) -> Ledger {
    for name in [
        "put_head_get_integrity",
        "duplicate_put_idempotent",
        "read_pin_available",
        "full_read_actual_bytes",
        "range_206_first_middle_final_suffix",
        "anonymous_object_denied",
        "anonymous_listing_denied",
        "presigned_get_range_206",
        "presigned_unsigned_denied",
        "presigned_expired_denied",
        "conditional_create_enforced",
        "size_ladder_up_to_envelope",
        "cleanup_current_object",
    ] {
        if ledger.outcome(name).is_none() {
            ledger.record(name, Outcome::NotRun("stopped_before_check"));
        }
    }
    ledger
}

/// Read-only checks may collect diagnostics after a failure. No new mutation or cleanup can
/// follow a failed/uncertain observation; attempted but unproven keys remain operator evidence.
async fn run(environment: &Environment) -> Ledger {
    let mut ledger = Ledger::default();
    let preflight = ledger.record(
        "tls_dns_preflight",
        tls_preflight(&environment.plain_client(), &environment.endpoint).await,
    );
    if matches!(preflight, Outcome::Fail(_)) {
        // Nothing was written; the remaining capabilities were not exercised.
        return complete_ledger(ledger);
    }
    let storage = match environment.storage() {
        Ok(storage) => storage,
        Err(error) => {
            ledger.record("adapter_configuration", Outcome::Fail(error.code()));
            return complete_ledger(ledger);
        }
    };
    let mut probes = ProbeJournal::default();

    let probe = probe_bytes(4096);
    let seeded = put_and_verify(&storage, &mut probes, &probe).await;
    ledger.record(
        "put_head_get_integrity",
        match &seeded {
            Ok(_) => Outcome::Pass,
            Err(reason) => Outcome::Fail(reason),
        },
    );
    if let Ok((key, integrity)) = &seeded {
        ledger.record(
            "duplicate_put_idempotent",
            match verify_owned(&storage, &probes.owned[0]).await {
                Ok(()) => match storage
                    .put_bytes(
                        key.clone(),
                        integrity.clone(),
                        Bytes::copy_from_slice(&probe),
                        "audio/ogg",
                    )
                    .await
                {
                    Ok(()) => match verify_owned(&storage, &probes.owned[0]).await {
                        Ok(()) => Outcome::Pass,
                        Err(reason) => Outcome::Fail(reason),
                    },
                    Err(error) => Outcome::Fail(error.code()),
                },
                Err(reason) => Outcome::Fail(reason),
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
        let (object, listing) =
            anonymous_denial(environment, &storage, key, integrity, &mut ledger).await;
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
    if ledger.unsafe_to_mutate() {
        probes.stop("acceptance_check_failed_or_uncertain");
    }
    ledger.record(
        "conditional_create_enforced",
        conditional_create(&storage, &mut probes).await,
    );
    ledger.record(
        "size_ladder_up_to_envelope",
        size_ladder(environment, &storage, &mut probes).await,
    );
    if !environment.full_envelope
        && ledger.outcome("size_ladder_up_to_envelope") == Some(Outcome::Pass)
    {
        ledger.observe(
            "size_ladder_max",
            "8 MiB; set CANTOS_LIVE_S3_FULL_ENVELOPE=1 for 32 MiB",
        );
    }

    cleanup(
        &storage,
        &mut probes,
        environment.conditional_delete_verified,
        &mut ledger,
    )
    .await;
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
    complete_ledger(ledger)
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
#[ignore = "operator-run live provider acceptance; writes probes with separately verified opt-in cleanup; never part of a gate"]
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

#[test]
fn further_mutations_require_both_private_access_checks_to_have_passed() {
    for outcome in [
        Outcome::NotRun("request_not_sent"),
        Outcome::Unsupported("capability_missing"),
        Outcome::UnsupportedByPermission("permission_missing"),
        Outcome::Review("unusual_response"),
        Outcome::Fail("bytes_exposed"),
    ] {
        for blocked_name in ["anonymous_object_denied", "anonymous_listing_denied"] {
            let mut ledger = Ledger::default();
            for name in ["anonymous_object_denied", "anonymous_listing_denied"] {
                ledger.record(
                    name,
                    if name == blocked_name {
                        outcome
                    } else {
                        Outcome::Pass
                    },
                );
            }
            assert!(ledger.unsafe_to_mutate(), "{blocked_name}: {outcome:?}");
        }
    }
    let mut ledger = Ledger::default();
    assert!(ledger.unsafe_to_mutate());
    ledger.record("anonymous_object_denied", Outcome::Pass);
    ledger.record("anonymous_listing_denied", Outcome::Pass);
    assert!(!ledger.unsafe_to_mutate());
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
    enum SeedFault {
        Racing412,
        Failed,
        StoredThenFailed,
    }

    #[derive(Clone, Copy)]
    struct Behavior {
        honor_if_none_match: bool,
        max_body_bytes: usize,
        public_read: bool,
        missing_get_status: StatusCode,
        seed_fault: Option<(usize, SeedFault)>,
        replace_before_delete: bool,
        versioned: bool,
    }

    impl Behavior {
        const WELL_BEHAVED: Self = Self {
            honor_if_none_match: true,
            max_body_bytes: MAX_OBJECT_BYTES as usize,
            public_read: false,
            missing_get_status: StatusCode::NOT_FOUND,
            seed_fault: None,
            replace_before_delete: false,
            versioned: false,
        };
    }

    struct ObservedRequest {
        method: Method,
        key: String,
        if_match: Option<String>,
        if_none_match: Option<String>,
        query: Option<String>,
    }

    #[derive(Clone)]
    struct Provider {
        behavior: Behavior,
        objects: Arc<Mutex<Objects>>,
        requests: Arc<Mutex<Vec<ObservedRequest>>>,
        seed_count: Arc<Mutex<usize>>,
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
        provider.requests.lock().unwrap().push(ObservedRequest {
            method: parts.method.clone(),
            key: key.clone(),
            if_match: parts
                .headers
                .get("if-match")
                .map(|value| value.to_str().unwrap().to_owned()),
            if_none_match: parts
                .headers
                .get("if-none-match")
                .map(|value| value.to_str().unwrap().to_owned()),
            query: parts.uri.query().map(str::to_owned),
        });
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
                if let std::collections::hash_map::Entry::Vacant(entry) = objects.entry(key.clone())
                {
                    let mut count = provider.seed_count.lock().unwrap();
                    *count += 1;
                    if let Some((at, fault)) = provider.behavior.seed_fault {
                        if *count == at {
                            return match fault {
                                SeedFault::Racing412 => {
                                    // A different writer wins after absence, even with identical bytes.
                                    entry.insert((body, etag));
                                    reply(StatusCode::PRECONDITION_FAILED, vec![], vec![])
                                }
                                SeedFault::Failed => {
                                    reply(StatusCode::SERVICE_UNAVAILABLE, vec![], vec![])
                                }
                                SeedFault::StoredThenFailed => {
                                    entry.insert((body, etag));
                                    reply(StatusCode::SERVICE_UNAVAILABLE, vec![], vec![])
                                }
                            };
                        }
                    }
                }
                objects.insert(key, (body, etag.clone()));
                reply(StatusCode::OK, vec![("etag", etag)], vec![])
            }
            Method::DELETE if !authorized => reply(StatusCode::FORBIDDEN, vec![], vec![]),
            Method::DELETE => {
                if provider.behavior.replace_before_delete {
                    objects.insert(
                        key.clone(),
                        (b"replacement".to_vec(), "\"replacement\"".to_owned()),
                    );
                }
                let Some((_, current_etag)) = objects.get(&key) else {
                    return reply(StatusCode::NOT_FOUND, vec![], vec![]);
                };
                let pin = parts
                    .headers
                    .get("if-match")
                    .and_then(|value| value.to_str().ok());
                if pin != Some(current_etag.as_str()) {
                    return reply(StatusCode::PRECONDITION_FAILED, vec![], vec![]);
                }
                objects.remove(&key);
                reply(StatusCode::NO_CONTENT, vec![], vec![])
            }
            Method::GET => {
                if !authorized && !provider.behavior.public_read {
                    return reply(StatusCode::FORBIDDEN, vec![], vec![]);
                }
                let Some((bytes, etag)) = objects.get(&key) else {
                    return reply(provider.behavior.missing_get_status, vec![], vec![]);
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
                let mut response = match range {
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
                };
                if provider.behavior.versioned {
                    response
                        .headers_mut()
                        .insert("x-amz-version-id", "owned-version".parse().unwrap());
                }
                response
            }
            _ => reply(StatusCode::METHOD_NOT_ALLOWED, vec![], vec![]),
        }
    }

    async fn serve(behavior: Behavior) -> Running {
        let provider = Provider {
            behavior,
            objects: Arc::new(Mutex::new(HashMap::new())),
            requests: Arc::new(Mutex::new(Vec::new())),
            seed_count: Arc::new(Mutex::new(0)),
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
            // This in-process fake enforces If-Match; no real-provider claim is implied.
            conditional_delete_verified: true,
            request_timeout_ms: 20_000,
            allow_loopback: true,
        }
    }

    fn mutation_count(provider: &Provider, method: Method) -> usize {
        provider
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.method == method)
            .count()
    }

    #[tokio::test]
    async fn even_identical_preexisting_bytes_never_authorize_put_or_cleanup() {
        let running = serve(Behavior::WELL_BEHAVED).await;
        let storage = environment(&running.endpoint).storage().unwrap();
        let bytes = b"preexisting matching bytes";
        let (key, _) = expected(bytes);
        running.provider.objects.lock().unwrap().insert(
            key.as_str().to_owned(),
            (bytes.to_vec(), "\"preexisting\"".to_owned()),
        );
        let mut probes = ProbeJournal::default();
        assert_eq!(
            put_and_verify(&storage, &mut probes, bytes).await.err(),
            Some("probe_key_preexisting")
        );
        assert_eq!(
            put_and_verify(&storage, &mut probes, b"another probe")
                .await
                .err(),
            Some("mutations_stopped")
        );
        let mut ledger = Ledger::default();
        cleanup(&storage, &mut probes, true, &mut ledger).await;
        assert!(probes.attempted.is_empty());
        assert!(probes.owned.is_empty());
        assert_eq!(mutation_count(&running.provider, Method::PUT), 0);
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
        assert_eq!(
            running.provider.objects.lock().unwrap()[key.as_str()].0,
            bytes
        );
    }

    #[tokio::test]
    async fn denied_or_failed_absence_never_becomes_absent() {
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let running = serve(Behavior {
                missing_get_status: status,
                ..Behavior::WELL_BEHAVED
            })
            .await;
            let ledger = run(&environment(&running.endpoint)).await;
            assert_eq!(
                ledger.outcome("put_head_get_integrity"),
                Some(Outcome::Fail("probe_absence_unproven"))
            );
            assert_eq!(
                ledger.outcome("conditional_create_enforced"),
                Some(Outcome::NotRun("safety_stop"))
            );
            assert_eq!(
                ledger.outcome("cleanup_current_object"),
                Some(Outcome::NotRun("safety_stop"))
            );
            assert_eq!(mutation_count(&running.provider, Method::PUT), 0);
            assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
        }
    }

    #[tokio::test]
    async fn seed_412_or_failed_put_never_claims_ownership_or_deletes_earlier_probes() {
        for at in [1, 2] {
            for fault in [
                SeedFault::Racing412,
                SeedFault::Failed,
                SeedFault::StoredThenFailed,
            ] {
                let running = serve(Behavior {
                    seed_fault: Some((at, fault)),
                    ..Behavior::WELL_BEHAVED
                })
                .await;
                let ledger = run(&environment(&running.endpoint)).await;
                let reason = match fault {
                    SeedFault::Racing412 => "seed_412_ownership_unproven",
                    SeedFault::Failed | SeedFault::StoredThenFailed => "seed_put_status_uncertain",
                };
                let check = if at == 1 {
                    "put_head_get_integrity"
                } else {
                    "conditional_create_enforced"
                };
                assert_eq!(ledger.outcome(check), Some(Outcome::Fail(reason)));
                assert_eq!(
                    ledger.outcome("size_ladder_up_to_envelope"),
                    Some(Outcome::NotRun("safety_stop"))
                );
                assert_eq!(
                    ledger.outcome("cleanup_current_object"),
                    Some(Outcome::NotRun("safety_stop"))
                );
                // A later seed fails after the first probe's deliberate adapter duplicate.
                assert_eq!(
                    mutation_count(&running.provider, Method::PUT),
                    if at == 1 { 1 } else { 3 }
                );
                assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
                let fault_stored = !matches!(fault, SeedFault::Failed);
                assert_eq!(
                    running.provider.objects.lock().unwrap().len(),
                    at - 1 + usize::from(fault_stored)
                );
                assert!(ledger
                    .observed
                    .iter()
                    .any(|(name, _)| *name == "orphaned_unproven_probe_key"));
            }
        }
    }

    #[tokio::test]
    async fn any_changed_probe_blocks_the_entire_cleanup_before_the_first_delete() {
        for change_bytes in [false, true] {
            let running = serve(Behavior::WELL_BEHAVED).await;
            let storage = environment(&running.endpoint).storage().unwrap();
            let mut probes = ProbeJournal::default();
            put_and_verify(&storage, &mut probes, b"first owned probe")
                .await
                .unwrap();
            let (key, _) = put_and_verify(&storage, &mut probes, b"second owned probe")
                .await
                .unwrap();
            let changed = if change_bytes {
                b"replacement bytes".to_vec()
            } else {
                b"second owned probe".to_vec()
            };
            running.provider.objects.lock().unwrap().insert(
                key.as_str().to_owned(),
                (changed.clone(), "\"changed\"".to_owned()),
            );
            let mut ledger = Ledger::default();
            cleanup(&storage, &mut probes, true, &mut ledger).await;
            let reason = if change_bytes {
                "owned_probe_integrity_unproven"
            } else {
                "owned_probe_pin_changed"
            };
            assert_eq!(
                ledger.outcome("cleanup_current_object"),
                Some(Outcome::Fail(reason))
            );
            assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
            let objects = running.provider.objects.lock().unwrap();
            assert_eq!(objects.len(), 2);
            assert_eq!(objects[key.as_str()].0, changed);
        }
    }

    #[tokio::test]
    async fn conditional_delete_race_retains_replacement_and_stops_remaining_deletes() {
        let running = serve(Behavior {
            replace_before_delete: true,
            ..Behavior::WELL_BEHAVED
        })
        .await;
        let storage = environment(&running.endpoint).storage().unwrap();
        let mut probes = ProbeJournal::default();
        let (first, _) = put_and_verify(&storage, &mut probes, b"first owned probe")
            .await
            .unwrap();
        let (second, _) = put_and_verify(&storage, &mut probes, b"second owned probe")
            .await
            .unwrap();
        let original_etag = probes.owned[0].location.etag.clone();
        let mut ledger = Ledger::default();
        cleanup(&storage, &mut probes, true, &mut ledger).await;
        assert_eq!(
            ledger.outcome("cleanup_current_object"),
            Some(Outcome::Fail("conditional_delete_failed_or_object_changed"))
        );
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 1);
        let requests = running.provider.requests.lock().unwrap();
        let delete = requests
            .iter()
            .find(|request| request.method == Method::DELETE)
            .unwrap();
        assert_eq!(delete.if_match, original_etag);
        assert!(delete.query.is_none());
        let objects = running.provider.objects.lock().unwrap();
        assert_eq!(objects[first.as_str()].0, b"replacement");
        assert_eq!(objects[second.as_str()].0, b"second owned probe");
        assert!(probes.deleted.is_empty());
    }

    #[tokio::test]
    async fn versioned_probe_is_retained_without_current_delete_or_version_purge() {
        let running = serve(Behavior {
            versioned: true,
            ..Behavior::WELL_BEHAVED
        })
        .await;
        let storage = environment(&running.endpoint).storage().unwrap();
        let mut probes = ProbeJournal::default();
        put_and_verify(&storage, &mut probes, b"versioned owned probe")
            .await
            .unwrap();
        let mut ledger = Ledger::default();
        cleanup(&storage, &mut probes, true, &mut ledger).await;
        assert_eq!(
            ledger.outcome("cleanup_current_object"),
            Some(Outcome::NotRun("versioned_probe_retained"))
        );
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
        assert_eq!(running.provider.objects.lock().unwrap().len(), 1);
        assert!(ledger
            .observed
            .iter()
            .any(|(name, _)| *name == "retained_owned_probe_key"));
    }

    #[tokio::test]
    async fn unverified_conditional_delete_retains_every_probe_without_any_delete() {
        let running = serve(Behavior::WELL_BEHAVED).await;
        let mut environment = environment(&running.endpoint);
        environment.conditional_delete_verified = false;
        let ledger = run(&environment).await;
        assert_eq!(
            ledger.outcome("cleanup_current_object"),
            Some(Outcome::NotRun("conditional_delete_not_verified"))
        );
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
        assert_eq!(running.provider.objects.lock().unwrap().len(), 4);
        assert_eq!(
            ledger
                .observed
                .iter()
                .filter(|(name, _)| *name == "retained_owned_probe_key")
                .count(),
            4
        );
        assert!(!ledger.has_failure());
    }

    #[tokio::test]
    async fn well_behaved_provider_passes_every_runnable_check_and_cleans_up_only_its_probes() {
        let running = serve(Behavior::WELL_BEHAVED).await;
        running.provider.objects.lock().unwrap().insert(
            "unrelated-existing-key".to_owned(),
            (b"untouched".to_vec(), "\"unrelated\"".to_owned()),
        );
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
        let objects = running.provider.objects.lock().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects["unrelated-existing-key"].0, b"untouched");
        drop(objects);
        let requests = running.provider.requests.lock().unwrap();
        let deletes: Vec<_> = requests
            .iter()
            .filter(|request| request.method == Method::DELETE)
            .collect();
        assert_eq!(deletes.len(), 4);
        let mut deleted = std::collections::HashSet::new();
        for request in deletes {
            assert!(request
                .if_match
                .as_deref()
                .is_some_and(|etag| etag.starts_with('"')));
            assert!(
                request.query.is_none(),
                "version purges are outside cleanup scope"
            );
            assert!(deleted.insert(request.key.clone()), "never retry a DELETE");
        }
        for request in requests
            .iter()
            .filter(|request| request.method == Method::PUT)
        {
            assert_eq!(request.if_none_match.as_deref(), Some("*"));
        }
    }

    #[tokio::test]
    async fn provider_that_ignores_if_none_match_stops_mutations_and_retains_probes() {
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
        assert_eq!(
            ledger.outcome("size_ladder_up_to_envelope"),
            Some(Outcome::NotRun("safety_stop"))
        );
        assert_eq!(
            ledger.outcome("cleanup_current_object"),
            Some(Outcome::NotRun("safety_stop"))
        );
        assert!(!running.provider.objects.lock().unwrap().is_empty());
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
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
        // Earlier evidence remains, but no cleanup mutation may follow the failed PUT.
        assert_eq!(
            ledger.outcome("put_head_get_integrity"),
            Some(Outcome::Pass)
        );
        assert!(!running.provider.objects.lock().unwrap().is_empty());
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
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
            Some(Outcome::Fail("anonymous_object_bytes_exposed"))
        );
        assert_eq!(
            ledger.outcome("anonymous_listing_denied"),
            Some(Outcome::Fail("anonymous_read_allowed"))
        );
        assert_eq!(
            ledger.outcome("conditional_create_enforced"),
            Some(Outcome::NotRun("safety_stop"))
        );
        assert_eq!(mutation_count(&running.provider, Method::DELETE), 0);
    }

    #[tokio::test]
    async fn unreachable_endpoint_writes_nothing_and_reports_a_fixed_reason() {
        // Port 9 on loopback refuses connections; no request carrying credentials is sent.
        let ledger = run(&environment("http://127.0.0.1:9")).await;
        assert!(matches!(
            ledger.outcome("tls_dns_preflight"),
            Some(Outcome::Fail(_))
        ));
        assert_eq!(
            ledger.outcome("put_head_get_integrity"),
            Some(Outcome::NotRun("stopped_before_check"))
        );
        assert_eq!(
            ledger.outcome("cleanup_current_object"),
            Some(Outcome::NotRun("stopped_before_check"))
        );
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
        assert!(
            !Environment::from_lookup(&complete)
                .unwrap()
                .conditional_delete_verified
        );
        for (value, expected) in [
            ("1", true),
            ("true", true),
            ("0", false),
            ("false", false),
            ("yes", false),
        ] {
            let environment = Environment::from_lookup(&|name| {
                if name == "CANTOS_LIVE_S3_CONDITIONAL_DELETE_VERIFIED" {
                    Some(value.to_owned())
                } else {
                    complete(name)
                }
            })
            .unwrap();
            assert_eq!(environment.conditional_delete_verified, expected);
        }
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
