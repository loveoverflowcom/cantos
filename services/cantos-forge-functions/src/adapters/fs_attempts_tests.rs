use std::{
    cell::RefCell,
    sync::{Arc, Barrier},
};

use tempfile::TempDir;

use super::*;
use crate::{
    adapters::mock_provider::MockProvider,
    application::{
        memory::MemoryStore,
        ports::ProviderResult,
        produce::{ProduceError, Produced, Producer, Terms},
    },
    domain::{
        attempt::{AttemptState, Policy},
        generation::testing::request,
    },
};

struct SyncFault {
    directory: PathBuf,
    successes: usize,
    failures: usize,
}

#[derive(Clone)]
struct CreationGates {
    before: Arc<Barrier>,
    after: Arc<Barrier>,
}

thread_local! {
    static SYNC_FAULT: RefCell<Option<SyncFault>> = const { RefCell::new(None) };
    static CREATION_GATES: RefCell<Option<CreationGates>> = const { RefCell::new(None) };
}

pub(super) fn operation_creation_barrier(created: bool) {
    let barrier = CREATION_GATES.with(|gates| {
        gates.borrow().as_ref().map(|gates| {
            if created {
                gates.after.clone()
            } else {
                gates.before.clone()
            }
        })
    });
    if let Some(barrier) = barrier {
        barrier.wait();
    }
}

fn arm_sync_fault(directory: PathBuf, successes: usize, failures: usize) {
    SYNC_FAULT.with(|fault| {
        *fault.borrow_mut() = Some(SyncFault {
            directory,
            successes,
            failures,
        });
    });
}

pub(super) fn before_directory_sync(path: &Path) -> Result<(), AttemptStoreError> {
    SYNC_FAULT.with(|fault| {
        let mut fault = fault.borrow_mut();
        if let Some(planned) = fault.as_mut().filter(|planned| planned.directory == path) {
            if planned.successes > 0 {
                planned.successes -= 1;
            } else if planned.failures > 0 {
                planned.failures -= 1;
                return Err(unavailable(
                    "sync attempt directory",
                    "injected directory sync failure",
                ));
            }
        }
        Ok(())
    })
}

fn lab() -> (TempDir, FsAttemptStore) {
    let temporary = tempfile::tempdir().unwrap();
    let store = FsAttemptStore::initialize(&temporary.path().join("attempts")).unwrap();
    (temporary, store)
}

fn key() -> OperationKey {
    OperationKey::parse("Chương Một/../../draft").unwrap()
}

fn opened() -> Event {
    Event::Opened {
        key: key(),
        fingerprint: request().fingerprint(),
        cost: Cost::new(1).unwrap(),
    }
}

fn settled(settlement: Settlement) -> Event {
    Event::Settled {
        attempt: AttemptNumber::FIRST,
        settlement,
    }
}

fn terms() -> Terms {
    Terms {
        policy: Policy::new(3, 3).unwrap(),
        cost: Cost::new(1).unwrap(),
    }
}

fn first_file(store: &FsAttemptStore) -> PathBuf {
    store.directory(&key()).join("e00000000.json")
}

fn assert_corrupt<T: std::fmt::Debug>(result: Result<T, AttemptStoreError>, detail: &'static str) {
    assert_eq!(result.unwrap_err(), corrupt(detail));
}

fn reseal(path: &Path, change: impl FnOnce(&mut EventRecord)) {
    let mut stored: StoredEvent = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    change(&mut stored.record);
    stored.digest = stored.record.digest().unwrap();
    fs::write(path, serde_json::to_vec(&stored).unwrap()).unwrap();
}

#[test]
fn restart_replays_exact_events_and_keys_never_become_paths() {
    let (_temporary, store) = lab();
    assert_eq!(store.events(&key()), Ok(vec![]));
    let events = [
        opened(),
        settled(Settlement::Uncertain(UncertainReason::TimedOut)),
        Event::Regenerated {
            attempt: AttemptNumber::new(2).unwrap(),
            cost: Cost::new(2).unwrap(),
            acknowledged: Some(AttemptNumber::FIRST),
        },
        Event::Settled {
            attempt: AttemptNumber::new(2).unwrap(),
            settlement: Settlement::Succeeded(ProviderAudio {
                digest: Sha256Digest::of(b"provider bytes"),
                bytes: NonZeroU64::new(14).unwrap(),
            }),
        },
    ];
    for (index, event) in events.iter().enumerate() {
        store.append(&key(), index, event).unwrap();
    }
    let root = store.root.clone();
    drop(store);
    let reopened = FsAttemptStore::open(&root).unwrap();
    assert_eq!(reopened.events(&key()), Ok(events.to_vec()));
    assert_eq!(Operation::replay(&events).unwrap().ledger().spent(), 3);
    let names: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        names.len(),
        2,
        "only marker and one hashed operation directory"
    );
    assert!(reopened
        .directory(&key())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("op-"));
}

#[test]
fn every_provider_outcome_round_trips_without_collapsing_uncertainty_into_rejection() {
    let settlements = [
        Settlement::Rejected(Rejection::NotAttempted),
        Settlement::Rejected(Rejection::Refused),
        Settlement::Rejected(Rejection::RateLimited),
        Settlement::Rejected(Rejection::Unavailable),
        Settlement::Rejected(Rejection::InvalidAudio),
        Settlement::Uncertain(UncertainReason::TimedOut),
        Settlement::Uncertain(UncertainReason::ConnectionLost),
        Settlement::Uncertain(UncertainReason::ResultNotRecorded),
        Settlement::Uncertain(UncertainReason::Abandoned),
        Settlement::Succeeded(ProviderAudio {
            digest: Sha256Digest::of(b"audio"),
            bytes: NonZeroU64::new(5).unwrap(),
        }),
    ];
    for settlement in settlements {
        let (_temporary, store) = lab();
        store.append(&key(), 0, &opened()).unwrap();
        store.append(&key(), 1, &settled(settlement)).unwrap();
        let reopened = FsAttemptStore::open(&store.root).unwrap();
        assert_eq!(
            reopened.events(&key()),
            Ok(vec![opened(), settled(settlement)])
        );
    }
}

#[test]
fn an_existing_position_or_non_next_index_conflicts_without_overwriting() {
    let (_temporary, store) = lab();
    assert_eq!(
        store.append(&key(), 1, &opened()),
        Err(AttemptStoreError::Conflict)
    );
    assert!(!store.directory(&key()).exists());
    store.append(&key(), 0, &opened()).unwrap();
    let original = fs::read(first_file(&store)).unwrap();
    assert_eq!(
        store.append(&key(), 0, &opened()),
        Err(AttemptStoreError::Conflict)
    );
    assert_eq!(
        store.append(
            &key(),
            2,
            &settled(Settlement::Rejected(Rejection::Refused))
        ),
        Err(AttemptStoreError::Conflict)
    );
    assert_eq!(fs::read(first_file(&store)).unwrap(), original);
    assert_eq!(store.events(&key()), Ok(vec![opened()]));
}

#[test]
fn same_or_different_requests_racing_one_key_have_exactly_one_winner() {
    for different in [false, true] {
        for _ in 0..6 {
            let (_temporary, store) = lab();
            let gate = Arc::new(Barrier::new(2));
            let events = [
                opened(),
                Event::Opened {
                    key: key(),
                    fingerprint: if different {
                        RequestFingerprint::from_digest(Sha256Digest::of(b"different request"))
                    } else {
                        request().fingerprint()
                    },
                    cost: Cost::new(1).unwrap(),
                },
            ];
            let writers: Vec<_> = events
                .iter()
                .cloned()
                .map(|event| {
                    let gate = gate.clone();
                    let store = store.clone();
                    std::thread::spawn(move || {
                        gate.wait();
                        let result = store.append(&key(), 0, &event);
                        (event, result)
                    })
                })
                .collect();
            let results: Vec<_> = writers
                .into_iter()
                .map(|writer| writer.join().unwrap())
                .collect();
            assert_eq!(
                results.iter().filter(|(_, result)| result.is_ok()).count(),
                1,
                "{results:?}"
            );
            assert_eq!(
                results
                    .iter()
                    .filter(|(_, result)| *result == Err(AttemptStoreError::Conflict))
                    .count(),
                1,
                "{results:?}"
            );
            let winner = results
                .into_iter()
                .find(|(_, result)| result.is_ok())
                .unwrap()
                .0;
            assert_eq!(store.events(&key()), Ok(vec![winner]));
        }
    }
}

#[test]
fn impossible_or_foreign_events_are_rejected_before_any_directory_is_created() {
    let (_temporary, store) = lab();
    assert_corrupt(
        store.append(
            &key(),
            0,
            &settled(Settlement::Rejected(Rejection::Refused)),
        ),
        "attempt history fails domain replay",
    );
    let foreign = Event::Opened {
        key: OperationKey::parse("another operation").unwrap(),
        fingerprint: request().fingerprint(),
        cost: Cost::new(1).unwrap(),
    };
    assert_corrupt(
        store.append(&key(), 0, &foreign),
        "attempt history belongs to another operation",
    );
    assert!(!store.directory(&key()).exists());
    store.append(&key(), 0, &opened()).unwrap();
    assert_corrupt(
        store.append(&key(), 1, &opened()),
        "attempt history fails domain replay",
    );
    assert_eq!(store.events(&key()), Ok(vec![opened()]));
}

#[test]
fn corrupt_misbound_or_out_of_order_records_never_look_like_an_absent_operation() {
    type DamageCase = (&'static str, Box<dyn Fn(&Path)>);
    let cases: Vec<DamageCase> = vec![
        (
            "malformed attempt event",
            Box::new(|path| fs::write(path, b"{\"record\":").unwrap()),
        ),
        (
            "attempt event checksum mismatch",
            Box::new(|path| {
                let text = fs::read_to_string(path)
                    .unwrap()
                    .replace("\"cost\":1", "\"cost\":2");
                fs::write(path, text).unwrap();
            }),
        ),
        (
            "attempt event belongs to another key or position",
            Box::new(|path| reseal(path, |record| record.key = "foreign".to_owned())),
        ),
        (
            "attempt event belongs to another key or position",
            Box::new(|path| reseal(path, |record| record.index = 1)),
        ),
        (
            "unsupported attempt-event schema",
            Box::new(|path| reseal(path, |record| record.schema = "future/v2".to_owned())),
        ),
        (
            "invalid attempt-event values",
            Box::new(|path| {
                reseal(path, |record| {
                    record.event = EventWire::Opened {
                        fingerprint: request().fingerprint().to_string(),
                        cost: 0,
                    }
                })
            }),
        ),
        (
            "attempt history fails domain replay",
            Box::new(|path| {
                reseal(path, |record| {
                    record.event = EventWire::Settled {
                        attempt: 1,
                        settlement: SettlementWire::Rejected {
                            reason: RejectionWire::Refused,
                        },
                    }
                })
            }),
        ),
    ];
    for (detail, damage) in cases {
        let (_temporary, store) = lab();
        store.append(&key(), 0, &opened()).unwrap();
        damage(&first_file(&store));
        assert_corrupt(store.events(&key()), detail);
        assert_corrupt(store.append(&key(), 0, &opened()), detail);
    }
    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    fs::rename(
        first_file(&store),
        store.directory(&key()).join("e00000001.json"),
    )
    .unwrap();
    assert_corrupt(store.events(&key()), "attempt history is not contiguous");
}

#[test]
fn unexpected_fields_entries_marker_versions_and_non_regular_records_fail_closed() {
    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(first_file(&store)).unwrap()).unwrap();
    value["record"]["unexpected"] = true.into();
    fs::write(first_file(&store), serde_json::to_vec(&value).unwrap()).unwrap();
    assert_corrupt(store.events(&key()), "malformed attempt event");

    let (_temporary, store) = lab();
    fs::write(store.root.join(MARKER), b"{\"schema\":\"future/v2\"}").unwrap();
    assert_corrupt(store.events(&key()), "unsupported attempt-store schema");
    assert_corrupt(
        FsAttemptStore::open(&store.root),
        "unsupported attempt-store schema",
    );

    let (_temporary, store) = lab();
    fs::write(store.root.join("foreign"), b"unrelated operator file").unwrap();
    assert_corrupt(store.events(&key()), "unexpected attempt-root entry");

    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    fs::write(store.directory(&key()).join("foreign"), b"unrelated").unwrap();
    assert_corrupt(store.events(&key()), "unexpected attempt entry");

    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    fs::remove_file(first_file(&store)).unwrap();
    fs::create_dir(first_file(&store)).unwrap();
    assert_corrupt(store.events(&key()), "attempt record is not a regular file");
}

#[test]
fn event_size_event_count_total_read_bytes_and_temporary_count_are_bounded() {
    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    File::create(first_file(&store))
        .unwrap()
        .set_len(MAX_EVENT_BYTES + 1)
        .unwrap();
    assert_corrupt(store.events(&key()), "attempt record exceeds byte limit");

    let (_temporary, store) = lab();
    fs::create_dir(store.directory(&key())).unwrap();
    for index in 0..=MAX_EVENTS {
        File::create(store.directory(&key()).join(event_name(index))).unwrap();
    }
    assert_corrupt(store.events(&key()), "attempt history has too many records");

    let (_temporary, store) = lab();
    fs::create_dir(store.directory(&key())).unwrap();
    for index in 0..=MAX_HISTORY_BYTES / MAX_EVENT_BYTES {
        let index = index as usize;
        let event = if index == 0 {
            opened()
        } else if index % 2 == 1 {
            Event::Settled {
                attempt: AttemptNumber::new((index / 2 + 1) as u32).unwrap(),
                settlement: Settlement::Rejected(Rejection::Refused),
            }
        } else {
            Event::Regenerated {
                attempt: AttemptNumber::new((index / 2 + 1) as u32).unwrap(),
                cost: Cost::new(1).unwrap(),
                acknowledged: None,
            }
        };
        let mut bytes = StoredEvent::encode(&key(), index, &event).unwrap();
        bytes.resize(MAX_EVENT_BYTES as usize, b' ');
        fs::write(store.directory(&key()).join(event_name(index)), bytes).unwrap();
    }
    assert_corrupt(store.events(&key()), "attempt history exceeds byte limit");

    let (_temporary, store) = lab();
    for index in 0..=MAX_TEMPORARIES {
        fs::write(
            store.root.join(format!(".tmp-{index:032x}")),
            b"crash scratch",
        )
        .unwrap();
    }
    assert_corrupt(
        store.events(&key()),
        "too many attempt temporary files; operator cleanup required",
    );
}

#[test]
fn crash_temporaries_are_ignored_without_promotion_or_deleting_another_writers_file() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("interrupted-initialization");
    fs::create_dir(&root).unwrap();
    let stale_marker = root.join(".tmp-00000000000000000000000000000001");
    fs::write(&stale_marker, b"{\"schema\":").unwrap();
    let store = FsAttemptStore::initialize(&root).unwrap();
    fs::create_dir(store.directory(&key())).unwrap();
    let stale_event = store
        .directory(&key())
        .join(".tmp-00000000000000000000000000000002");
    fs::write(&stale_event, b"partial event").unwrap();
    assert_eq!(store.events(&key()), Ok(vec![]));
    store.append(&key(), 0, &opened()).unwrap();
    assert_eq!(store.events(&key()), Ok(vec![opened()]));
    assert_eq!(fs::read(stale_marker).unwrap(), b"{\"schema\":");
    assert_eq!(fs::read(stale_event).unwrap(), b"partial event");
}

#[test]
fn a_failed_directory_sync_never_grants_dispatch_even_when_the_reservation_is_visible() {
    let (_temporary, store) = lab();
    arm_sync_fault(store.directory(&key()), 0, 1);
    let provider = MockProvider::default();
    let blobs = MemoryStore::default();
    let producer = Producer {
        attempts: &store,
        blobs: &blobs,
        provider: &provider,
    };
    assert_eq!(
        producer.generate(&key(), &request(), terms()),
        Err(ProduceError::NotReserved(unavailable(
            "sync attempt directory",
            "injected directory sync failure"
        )))
    );
    assert!(
        first_file(&store).exists(),
        "the hard link happened before the failed sync"
    );
    assert!(provider.calls().is_empty());
    let Produced::Recorded(status) = producer.generate(&key(), &request(), terms()).unwrap() else {
        panic!("must replay the unowned reservation")
    };
    assert_eq!(status.latest.state(), AttemptState::Open);
    assert!(provider.calls().is_empty());
}

struct FailOutcomeSync<'a>(&'a FsAttemptStore);

impl AttemptStore for FailOutcomeSync<'_> {
    fn events(&self, key: &OperationKey) -> Result<Vec<Event>, AttemptStoreError> {
        self.0.events(key)
    }
    fn append(
        &self,
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<(), AttemptStoreError> {
        if matches!(event, Event::Settled { .. }) {
            // Its prefix read succeeds; publication sync and reconciliation readback sync fail.
            arm_sync_fault(self.0.directory(key), 1, 2);
        }
        self.0.append(key, index, event)
    }
}

#[test]
fn outcome_readback_cannot_heal_a_sync_failure_until_durability_is_confirmed() {
    let (_temporary, store) = lab();
    let attempts = FailOutcomeSync(&store);
    let provider = MockProvider::scripted([ProviderResult::Rejected(Rejection::Refused)]);
    let blobs = MemoryStore::default();
    let producer = Producer {
        attempts: &attempts,
        blobs: &blobs,
        provider: &provider,
    };
    assert_eq!(
        producer.generate(&key(), &request(), terms()),
        Err(ProduceError::OutcomeNotRecorded {
            attempt: AttemptNumber::FIRST,
            error: unavailable("sync attempt directory", "injected directory sync failure"),
        })
    );
    assert_eq!(provider.calls(), vec![AttemptNumber::FIRST]);
    let Produced::Recorded(status) = producer.generate(&key(), &request(), terms()).unwrap() else {
        panic!("must replay the outcome")
    };
    assert_eq!(
        status.latest.state(),
        AttemptState::Rejected(Rejection::Refused)
    );
    assert_eq!(provider.calls(), vec![AttemptNumber::FIRST]);
}

#[test]
fn a_process_exit_after_reservation_is_open_after_restart_and_never_calls_a_provider() {
    const CHILD_ROOT: &str = "CANTOS_FS_ATTEMPT_CRASH_CHILD_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let store = FsAttemptStore::open(Path::new(&root)).unwrap();
        store.append(&key(), 0, &opened()).unwrap();
        // Named crash point: reservation durably published; provider dispatch has not happened.
        std::process::exit(73);
    }
    let (_temporary, store) = lab();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "adapters::fs_attempts::tests::a_process_exit_after_reservation_is_open_after_restart_and_never_calls_a_provider"])
        .env(CHILD_ROOT, &store.root).output().unwrap().status;
    assert_eq!(status.code(), Some(73));
    let reopened = FsAttemptStore::open(&store.root).unwrap();
    let provider = MockProvider::default();
    let blobs = MemoryStore::default();
    let producer = Producer {
        attempts: &reopened,
        blobs: &blobs,
        provider: &provider,
    };
    let Produced::Recorded(status) = producer.generate(&key(), &request(), terms()).unwrap() else {
        panic!("must replay after process exit")
    };
    assert_eq!(status.latest.state(), AttemptState::Open);
    assert!(provider.calls().is_empty());
}

#[cfg(unix)]
#[test]
fn root_operation_record_marker_and_crash_temporary_symlinks_are_refused() {
    use std::os::unix::fs::symlink;
    let (temporary, store) = lab();
    let linked_root = temporary.path().join("linked-root");
    symlink(&store.root, &linked_root).unwrap();
    assert_corrupt(
        FsAttemptStore::open(&linked_root),
        "attempt root is not a real directory",
    );
    let linked_operation = store.directory(&key());
    symlink(temporary.path(), &linked_operation).unwrap();
    assert_corrupt(store.events(&key()), "operation is not a real directory");

    let (temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    let target = temporary.path().join("real-event");
    fs::rename(first_file(&store), &target).unwrap();
    symlink(&target, first_file(&store)).unwrap();
    assert_corrupt(store.events(&key()), "attempt record is not a regular file");

    let (temporary, store) = lab();
    fs::remove_file(store.root.join(MARKER)).unwrap();
    symlink(
        temporary.path().join("missing-marker"),
        store.root.join(MARKER),
    )
    .unwrap();
    assert_corrupt(
        FsAttemptStore::initialize(&store.root),
        "attempt record is not a regular file",
    );

    let (temporary, store) = lab();
    symlink(
        temporary.path().join("missing-temp"),
        store.root.join(".tmp-00000000000000000000000000000000"),
    )
    .unwrap();
    assert_corrupt(
        store.events(&key()),
        "attempt temporary is not a regular file",
    );
}

#[test]
fn initialization_never_adopts_foreign_data_or_creates_missing_parent_trees() {
    assert_eq!(
        FsAttemptStore::open(Path::new("relative")).unwrap_err(),
        unavailable(
            "configure local attempt store",
            "root must be an absolute operator directory"
        )
    );
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("operator-data"), b"keep me").unwrap();
    assert_corrupt(
        FsAttemptStore::initialize(temporary.path()),
        "unexpected attempt-root entry",
    );
    assert_eq!(
        fs::read(temporary.path().join("operator-data")).unwrap(),
        b"keep me"
    );
    assert!(matches!(
        FsAttemptStore::initialize(&temporary.path().join("missing-parent/ledger")),
        Err(AttemptStoreError::Unavailable {
            operation: "create attempt directory",
            ..
        })
    ));
    assert!(!temporary.path().join("missing-parent").exists());
}

#[test]
fn refusing_an_operation_at_capacity_preserves_existing_history_and_leaves_no_directory() {
    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    for index in 1..MAX_OPERATIONS {
        let filler = OperationKey::parse(&format!("capacity-fixture-{index}")).unwrap();
        fs::create_dir(store.directory(&filler)).unwrap();
    }
    let next = OperationKey::parse("over-capacity").unwrap();
    let event = Event::Opened {
        key: next.clone(),
        fingerprint: request().fingerprint(),
        cost: Cost::new(1).unwrap(),
    };
    assert_eq!(store.append(&next, 0, &event), Err(capacity_reached()));
    assert_eq!(store.events(&key()), Ok(vec![opened()]));
    assert!(!store.directory(&next).exists());
    let outcome = settled(Settlement::Rejected(Rejection::Refused));
    store.append(&key(), 1, &outcome).unwrap();
    assert_eq!(store.events(&key()), Ok(vec![opened(), outcome]));
}

#[test]
fn competing_creations_for_the_last_slot_do_not_poison_existing_operations() {
    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    for index in 1..MAX_OPERATIONS - 1 {
        let filler = OperationKey::parse(&format!("capacity-fixture-{index}")).unwrap();
        fs::create_dir(store.directory(&filler)).unwrap();
    }
    let gates = CreationGates {
        before: Arc::new(Barrier::new(2)),
        after: Arc::new(Barrier::new(2)),
    };
    let writers: Vec<_> = ["last-slot-a", "last-slot-b"]
        .into_iter()
        .map(|name| {
            let store = store.clone();
            let gates = gates.clone();
            std::thread::spawn(move || {
                CREATION_GATES.with(|configured| *configured.borrow_mut() = Some(gates));
                let key = OperationKey::parse(name).unwrap();
                let event = Event::Opened {
                    key: key.clone(),
                    fingerprint: request().fingerprint(),
                    cost: Cost::new(1).unwrap(),
                };
                let result = store.append(&key, 0, &event);
                (key, event, result)
            })
        })
        .collect();
    let results: Vec<_> = writers
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect();
    assert!(
        results
            .iter()
            .filter(|(_, _, result)| result.is_ok())
            .count()
            <= 1,
        "{results:?}"
    );
    assert_eq!(store.events(&key()), Ok(vec![opened()]));
    assert!(check_root(&store.root, true).unwrap() <= MAX_OPERATIONS);
    for (key, event, result) in results {
        match result {
            Ok(()) => assert_eq!(store.events(&key), Ok(vec![event])),
            Err(AttemptStoreError::Unavailable { .. }) => assert!(!store.directory(&key).exists()),
            other => panic!("expected a capacity refusal or a single winner, found {other:?}"),
        }
    }
}

#[test]
fn capacity_cleanup_never_deletes_another_writers_record() {
    let (_temporary, store) = lab();
    store.append(&key(), 0, &opened()).unwrap();
    let before = fs::read(first_file(&store)).unwrap();
    remove_unused_directory(&store.directory(&key())).unwrap();
    assert_eq!(fs::read(first_file(&store)).unwrap(), before);
    assert_eq!(store.events(&key()), Ok(vec![opened()]));
}
