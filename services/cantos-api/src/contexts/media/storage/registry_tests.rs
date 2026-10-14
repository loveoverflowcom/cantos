//! These tests exercise the production registry with named deterministic port doubles.
//! They establish routing, input forwarding and error translation, not provider persistence,
//! actual-byte integrity, network cancellation or live-cloud acceptance.

use super::*;
use crate::contexts::media::domain::object_storage::{NormalizedByteRange, StorageCapability};
use crate::contexts::media::storage::port::{StorageFuture, StorageRead};
use std::{cell::RefCell, sync::Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
struct VerificationCall {
    adapter: &'static str,
    key: ObjectKey,
    integrity: ObjectIntegrity,
}

/// The double records facts supplied by the real registry and returns a selected port outcome.
/// It never reads environment variables, opens files or contacts an endpoint.
struct RecordingPrivateStorage {
    name: &'static str,
    calls: Arc<Mutex<Vec<VerificationCall>>>,
    outcome: Result<(), StorageError>,
}

impl PrivateStorage for RecordingPrivateStorage {
    fn put_if_absent(&self, _audio: VerifiedAudio) -> StorageFuture<()> {
        Box::pin(async {
            Err(StorageError::UnsupportedCapability(
                StorageCapability::ConditionalCreate,
            ))
        })
    }

    fn verify(&self, key: ObjectKey, integrity: ObjectIntegrity) -> StorageFuture<()> {
        self.calls.lock().unwrap().push(VerificationCall {
            adapter: self.name,
            key,
            integrity,
        });
        let outcome = self.outcome;
        Box::pin(async move { outcome })
    }

    fn open_verified(
        &self,
        _key: ObjectKey,
        _integrity: ObjectIntegrity,
        _range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        Box::pin(async {
            Err(StorageError::UnsupportedCapability(
                StorageCapability::RangeRead,
            ))
        })
    }
}

fn target(name: &str) -> StorageTargetId {
    StorageTargetId::parse(name).unwrap()
}

fn object(target_name: &str, bytes: &[u8]) -> PrivateObjectRef {
    PrivateObjectRef::new(
        target(target_name),
        ObjectKey::for_bytes(bytes),
        ObjectIntegrity::for_bytes(bytes).unwrap(),
    )
    .unwrap()
}

fn adapter(
    name: &'static str,
    calls: &Arc<Mutex<Vec<VerificationCall>>>,
    outcome: Result<(), StorageError>,
) -> Arc<dyn PrivateStorage> {
    Arc::new(RecordingPrivateStorage {
        name,
        calls: calls.clone(),
        outcome,
    })
}

fn expected_call(name: &'static str, object: &PrivateObjectRef) -> VerificationCall {
    VerificationCall {
        adapter: name,
        key: object.key().clone(),
        integrity: object.integrity().clone(),
    }
}

fn assert_storage_error(result: Result<(), StoreError>, expected: StorageError) {
    match result {
        Err(StoreError::Storage(observed)) => assert_eq!(observed, expected),
        other => panic!("expected exact Storage({expected:?}), observed {other:?}"),
    }
}

#[tokio::test]
async fn changing_active_write_target_keeps_historical_reads_on_the_recorded_target() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let origin = adapter("historical-origin-double", &calls, Ok(()));
    let replacement = adapter("new-write-target-double", &calls, Ok(()));
    let registry = StorageRegistry {
        active: target("origin-private"),
        targets: BTreeMap::from([
            (target("origin-private"), origin.clone()),
            (target("replacement-private"), replacement.clone()),
        ]),
    };
    let historical = object("origin-private", b"immutable historical audio");
    assert!(Arc::ptr_eq(
        &registry.resolve(&registry.active).unwrap(),
        &origin,
    ));
    registry.verify(&historical).await.unwrap();

    let mut switched = registry.clone();
    switched.active = target("replacement-private");
    assert!(Arc::ptr_eq(
        &switched.resolve(&switched.active).unwrap(),
        &replacement,
    ));
    switched.verify(&historical).await.unwrap();
    let new_object = object("replacement-private", b"a separate newly admitted asset");
    switched.verify(&new_object).await.unwrap();

    assert_eq!(historical.target(), &target("origin-private"));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            expected_call("historical-origin-double", &historical),
            expected_call("historical-origin-double", &historical),
            expected_call("new-write-target-double", &new_object),
        ]
    );
}

#[tokio::test]
async fn missing_historical_target_never_falls_back_to_a_healthy_active_target() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let registry = StorageRegistry {
        active: target("active-private"),
        targets: BTreeMap::from([(
            target("active-private"),
            adapter("healthy-active-double", &calls, Ok(())),
        )]),
    };
    assert_storage_error(
        registry
            .verify(&object("removed-historical-target", b"historical bytes"))
            .await,
        StorageError::Unavailable,
    );
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn registry_forwards_the_exact_immutable_key_and_integrity_to_its_selected_adapter() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let registry = StorageRegistry {
        active: target("private-one"),
        targets: BTreeMap::from([(
            target("private-one"),
            adapter("exact-binding-double", &calls, Ok(())),
        )]),
    };
    let first = object("private-one", b"exact revision one bytes");
    let second = object("private-one", b"different exact revision two bytes");
    registry.verify(&first).await.unwrap();
    registry.verify(&second).await.unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            expected_call("exact-binding-double", &first),
            expected_call("exact-binding-double", &second),
        ],
    );
    assert_ne!(first.key(), second.key());
}

#[tokio::test]
async fn registry_preserves_typed_port_failures_without_retry_or_target_fallback() {
    for failure in [
        StorageError::Timeout,
        StorageError::Forbidden,
        StorageError::Unavailable,
        StorageError::Cancelled,
        StorageError::Conflict,
        StorageError::InvalidTarget,
        StorageError::UnsupportedCapability(StorageCapability::ConditionalCreate),
        StorageError::UnsupportedCapability(StorageCapability::RangeRead),
        StorageError::UnsupportedCapability(StorageCapability::TemporaryRead),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let registry = StorageRegistry {
            active: target("healthy-alternative"),
            targets: BTreeMap::from([
                (
                    target("historical-private"),
                    adapter("failing-historical-double", &calls, Err(failure)),
                ),
                (
                    target("healthy-alternative"),
                    adapter("unused-alternative-double", &calls, Ok(())),
                ),
            ]),
        };
        let historical = object("historical-private", b"accepted immutable historical bytes");
        assert_storage_error(registry.verify(&historical).await, failure);
        assert_eq!(
            *calls.lock().unwrap(),
            vec![expected_call("failing-historical-double", &historical)],
        );
    }
}

#[tokio::test]
async fn missing_or_corrupt_accepted_objects_remain_corrupt_revision_failures() {
    for failure in [
        StorageError::NotFound,
        StorageError::InvalidKey,
        StorageError::InvalidIntegrity,
        StorageError::DigestMismatch,
        StorageError::LengthMismatch,
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let registry = StorageRegistry {
            active: target("private-one"),
            targets: BTreeMap::from([(
                target("private-one"),
                adapter("corrupt-object-double", &calls, Err(failure)),
            )]),
        };
        let accepted = object("private-one", b"accepted private bytes");
        assert!(matches!(
            registry.verify(&accepted).await,
            Err(StoreError::CorruptRevision)
        ));
        assert_eq!(
            *calls.lock().unwrap(),
            vec![expected_call("corrupt-object-double", &accepted)],
        );
    }
}

const S3_CONFIG: &str = r#"
version = 1
[storage]
active_write_target = "private-one"
[storage.targets.private-one]
driver = "s3"
endpoint = "https://storage.example.invalid"
region = "auto"
private_bucket = "private-audio"
force_path_style = true
access_key_env = "TEST_STORAGE_ACCESS"
secret_key_env = "TEST_STORAGE_SECRET"
session_token_env = "TEST_STORAGE_SESSION"
connect_timeout_ms = 1000
request_timeout_ms = 2000
conditional_put = true
range_reads = true
presigned_reads = false
"#;

fn fake_secret(name: &str) -> Option<String> {
    match name {
        "TEST_STORAGE_ACCESS" => Some("SYNTHETICACCESS".into()),
        "TEST_STORAGE_SECRET" => Some("synthetic-secret-sentinel".into()),
        "TEST_STORAGE_SESSION" => Some("synthetic-session-sentinel".into()),
        _ => None,
    }
}

#[test]
fn configuration_rejects_unknown_fields_inline_credentials_and_missing_targets() {
    for contents in [
        S3_CONFIG.replace("version = 1", "version = 1\nunknown_schema = true"),
        S3_CONFIG.replace("[storage]", "[storage]\nunknown_topology = true"),
        format!("{S3_CONFIG}\naccess_key = 'inline-sentinel'\n"),
        format!("{S3_CONFIG}\nsecret_key = 'inline-sentinel'\n"),
        format!("{S3_CONFIG}\npublic_bucket = 'not-implemented'\n"),
        S3_CONFIG.replace(
            "active_write_target = \"private-one\"",
            "active_write_target = \"missing-target\"",
        ),
    ] {
        assert!(matches!(
            StorageConfig::parse(&contents),
            Err(StorageError::InvalidTarget)
        ));
    }
}

#[tokio::test]
async fn composition_sources_only_the_declared_credential_names_from_fake_injection() {
    let requests = RefCell::new(Vec::new());
    let config = StorageConfig::parse(S3_CONFIG).unwrap();
    let registry = StorageRegistry::configured(config, |name| {
        requests.borrow_mut().push(name.to_owned());
        fake_secret(name)
    })
    .unwrap();
    assert_eq!(registry.active, target("private-one"));
    assert_eq!(registry.targets.len(), 1);
    assert!(registry.resolve(&target("private-one")).is_ok());
    assert_eq!(
        requests.into_inner(),
        vec![
            "TEST_STORAGE_ACCESS",
            "TEST_STORAGE_SECRET",
            "TEST_STORAGE_SESSION",
        ],
    );
}

#[tokio::test]
async fn missing_any_declared_secret_rejects_configuration_without_a_default_fallback() {
    for (missing, expected_requests) in [
        ("TEST_STORAGE_ACCESS", vec!["TEST_STORAGE_ACCESS"]),
        (
            "TEST_STORAGE_SECRET",
            vec!["TEST_STORAGE_ACCESS", "TEST_STORAGE_SECRET"],
        ),
        (
            "TEST_STORAGE_SESSION",
            vec![
                "TEST_STORAGE_ACCESS",
                "TEST_STORAGE_SECRET",
                "TEST_STORAGE_SESSION",
            ],
        ),
    ] {
        let requests = RefCell::new(Vec::new());
        let config = StorageConfig::parse(S3_CONFIG).unwrap();
        let result = StorageRegistry::configured(config, |name| {
            requests.borrow_mut().push(name.to_owned());
            if name == missing {
                None
            } else {
                fake_secret(name)
            }
        });
        assert!(matches!(result, Err(StoreError::InvalidRequest)));
        assert_eq!(requests.into_inner(), expected_requests);
    }
}

#[tokio::test]
async fn malformed_injected_credentials_fail_closed_before_any_object_operation() {
    for (invalid, value) in [
        ("TEST_STORAGE_ACCESS", ""),
        ("TEST_STORAGE_ACCESS", "access/with/path"),
        ("TEST_STORAGE_SECRET", "secret\nwith-newline"),
        ("TEST_STORAGE_SESSION", "session\nwith-newline"),
    ] {
        let config = StorageConfig::parse(S3_CONFIG).unwrap();
        let result = StorageRegistry::configured(config, |name| {
            if name == invalid {
                Some(value.into())
            } else {
                fake_secret(name)
            }
        });
        assert!(matches!(
            result,
            Err(StoreError::Storage(StorageError::InvalidTarget))
        ));
    }
}

#[test]
fn active_s3_target_without_atomic_create_capability_is_rejected_without_emulation() {
    let unsupported = S3_CONFIG.replace("conditional_put = true", "conditional_put = false");
    assert!(matches!(
        StorageConfig::parse(&unsupported),
        Err(StorageError::UnsupportedCapability(
            StorageCapability::ConditionalCreate
        ))
    ));
}
