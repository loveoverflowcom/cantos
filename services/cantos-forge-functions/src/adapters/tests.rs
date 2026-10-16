//! The filesystem adapter against a real temporary directory.

use std::{fs, path::Path, thread};

use serde_json::Value;
use tempfile::TempDir;

use super::{
    bundle::{AUDIO_FILE, MANIFEST_FILE},
    FsRuntime,
};
use crate::{
    application::{
        export::{prepare_export, ExportRequest},
        ports::{
            BundleError, Corruption, InitOutcome, RecordedEvidence, Runtime, SourceError, Store,
            StoreError, MAX_SOURCE_BYTES,
        },
        register::{register_artifact, Placement, RegisterRequest, Registered, Target},
        validate::{validate_artifact, ValidateRequest},
    },
    domain::{
        artifact::{
            ArtifactKind, ArtifactPayload, ManuscriptOrigin, ReplayError, RevisionError,
            RightsClaim,
        },
        audio_profile::AudioProfile,
        checks::{Detail, DetailValue},
        digest::Sha256Digest,
        evaluation::{CheckOutcome, Evidence, Finding},
        identity::{ArtifactId, EvidenceId},
        label::{CheckName, ProducerLabel},
        ogg_opus::testing::Fixture,
    },
};

const STORY: ArtifactPayload = ArtifactPayload::Manuscript {
    origin: ManuscriptOrigin::Original,
};
const PROFILE: AudioProfile = AudioProfile::OpusMono24V1;

struct Lab {
    _dir: TempDir,
    root: std::path::PathBuf,
    runtime: FsRuntime,
}

impl Lab {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("workspace");
        let runtime = FsRuntime::new();
        assert_eq!(runtime.initialize(&root), Ok(InitOutcome::Created));
        Self {
            _dir: dir,
            root,
            runtime,
        }
    }

    fn store(&self) -> super::fs::FsStore {
        self.runtime.open(&self.root).unwrap()
    }

    fn register(&self, target: Target, payload: ArtifactPayload, bytes: &[u8]) -> Placement {
        let request = RegisterRequest {
            target,
            payload,
            producer: ProducerLabel::parse("AI host session").unwrap(),
            rights: RightsClaim::OriginalWork,
            inputs: Vec::new(),
            bytes: bytes.to_vec(),
        };
        match register_artifact(&self.store(), self.runtime.ids(), request) {
            Ok(Registered::Created(placement)) => placement,
            other => panic!("expected a created revision, found {other:?}"),
        }
    }

    fn revision_file(&self, artifact: ArtifactId, number: u32) -> std::path::PathBuf {
        self.root
            .join("artifacts")
            .join(artifact.to_string())
            .join(format!("r{number:08}.json"))
    }

    fn blob_file(&self, digest: Sha256Digest) -> std::path::PathBuf {
        self.root
            .join("blobs")
            .join(digest.to_string().replace(':', "-"))
    }
}

fn audio(seed: usize) -> Vec<u8> {
    Fixture::mono(vec![30 + seed; 50]).build()
}

fn corrupt(error: StoreError) -> Corruption {
    match error {
        StoreError::Corrupt(corruption) => corruption,
        other => panic!("expected corruption, found {other:?}"),
    }
}

#[test]
fn init_creates_once_refuses_foreign_directories_and_open_needs_a_workspace() {
    let dir = TempDir::new().unwrap();
    let runtime = FsRuntime::new();
    let root = dir.path().join("nested").join("workspace");
    assert_eq!(runtime.initialize(&root), Ok(InitOutcome::Created));
    assert_eq!(
        runtime.initialize(&root),
        Ok(InitOutcome::AlreadyInitialized)
    );
    assert!(runtime.open(&root).is_ok());

    let foreign = dir.path().join("foreign");
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("notes.txt"), "mine").unwrap();
    assert_eq!(
        runtime.initialize(&foreign),
        Err(StoreError::NotEmptyDirectory)
    );
    assert!(
        !foreign.join("forge-workspace.json").exists(),
        "nothing written in a foreign directory"
    );
    assert_eq!(
        runtime.open(&foreign).err(),
        Some(StoreError::NotInitialized)
    );
    assert_eq!(
        runtime.open(&dir.path().join("missing")).err(),
        Some(StoreError::NotInitialized)
    );

    let a_file = dir.path().join("file.txt");
    fs::write(&a_file, "x").unwrap();
    assert!(matches!(
        runtime.initialize(&a_file),
        Err(StoreError::Unavailable { .. })
    ));
}

#[test]
fn an_empty_existing_directory_can_become_a_workspace() {
    let dir = TempDir::new().unwrap();
    let runtime = FsRuntime::new();
    assert_eq!(runtime.initialize(dir.path()), Ok(InitOutcome::Created));
}

#[test]
fn an_unknown_workspace_format_is_refused_not_guessed() {
    let lab = Lab::new();
    fs::write(
        lab.root.join("forge-workspace.json"),
        "{\"schema\":\"cantos-forge.workspace/v9\"}",
    )
    .unwrap();
    assert_eq!(
        lab.runtime.open(&lab.root).err(),
        Some(StoreError::UnsupportedWorkspace {
            found: "cantos-forge.workspace/v9".to_owned()
        })
    );
    fs::write(lab.root.join("forge-workspace.json"), "not json").unwrap();
    assert!(matches!(
        lab.runtime.open(&lab.root).err(),
        Some(StoreError::Corrupt(Corruption::MalformedRecord { .. }))
    ));
}

#[test]
fn revisions_survive_reopening_and_keep_per_artifact_order() {
    let lab = Lab::new();
    let first = lab.register(Target::New, STORY, "Chương một".as_bytes());
    let second = lab.register(
        Target::Existing(first.artifact),
        STORY,
        "Chương một, sửa".as_bytes(),
    );
    let other = lab.register(Target::New, ArtifactPayload::Audio, &audio(0));
    let revisions = lab.runtime.open(&lab.root).unwrap().revisions().unwrap();
    let seen: Vec<_> = revisions
        .iter()
        .map(|revision| {
            (
                revision.artifact(),
                revision.revision().get(),
                revision.facts().content,
            )
        })
        .collect();
    let mut expected = vec![
        (first.artifact, 1, first.digest),
        (first.artifact, 2, second.digest),
        (other.artifact, 1, other.digest),
    ];
    expected.sort_by_key(|(artifact, number, _)| (*artifact, *number));
    assert_eq!(seen, expected);
}

#[test]
fn blobs_are_named_by_digest_stored_once_and_refuse_foreign_bytes() {
    let lab = Lab::new();
    let store = lab.store();
    let digest = Sha256Digest::of(b"exact bytes");
    store.put_blob(digest, b"exact bytes").unwrap();
    store.put_blob(digest, b"exact bytes").unwrap();
    assert_eq!(store.blob(digest).unwrap(), b"exact bytes");
    assert_eq!(fs::read_dir(lab.root.join("blobs")).unwrap().count(), 1);
    assert_eq!(
        store.put_blob(digest, b"other bytes"),
        Err(StoreError::Corrupt(Corruption::BlobDigestMismatch {
            digest
        }))
    );
    let absent = Sha256Digest::of(b"never stored");
    assert_eq!(
        corrupt(store.blob(absent).unwrap_err()),
        Corruption::MissingBlob { digest: absent }
    );
}

#[test]
fn a_blob_changed_on_disk_is_detected_when_read_and_when_stored_again() {
    let lab = Lab::new();
    let placed = lab.register(Target::New, STORY, b"original");
    fs::write(lab.blob_file(placed.digest), b"tampered").unwrap();
    let store = lab.store();
    let mismatch = Corruption::BlobDigestMismatch {
        digest: placed.digest,
    };
    assert_eq!(corrupt(store.blob(placed.digest).unwrap_err()), mismatch);
    // Storing the same digest again must not paper over the damage.
    assert_eq!(
        corrupt(store.put_blob(placed.digest, b"original").unwrap_err()),
        mismatch
    );
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_never_followed_inside_a_workspace() {
    use std::os::unix::fs::symlink;
    let lab = Lab::new();
    let placed = lab.register(Target::New, STORY, b"original");
    let outside = lab.root.parent().unwrap().join("outside.txt");
    fs::write(&outside, b"original").unwrap();
    let blob = lab.blob_file(placed.digest);
    fs::remove_file(&blob).unwrap();
    symlink(&outside, &blob).unwrap();
    assert!(matches!(
        corrupt(lab.store().blob(placed.digest).unwrap_err()),
        Corruption::NotARegularFile { .. }
    ));

    let record = lab.revision_file(placed.artifact, 1);
    let copy = lab.root.parent().unwrap().join("record.json");
    fs::copy(&record, &copy).unwrap();
    fs::remove_file(&record).unwrap();
    symlink(&copy, &record).unwrap();
    assert!(matches!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::NotARegularFile { .. }
    ));
}

#[test]
fn tampered_revision_records_fail_closed_with_the_exact_corruption() {
    let lab = Lab::new();
    let placed = lab.register(Target::New, STORY, b"v1");
    lab.register(Target::Existing(placed.artifact), STORY, b"v2");
    let path = lab.revision_file(placed.artifact, 1);
    let original = fs::read_to_string(&path).unwrap();

    // The file claims another revision number than its name.
    fs::write(&path, original.replace("\"revision\":1", "\"revision\":5")).unwrap();
    assert_eq!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::RevisionNumbering {
            artifact: placed.artifact
        }
    );
    // An unknown field is refused, not ignored.
    fs::write(
        &path,
        original.replace("{\"schema\"", "{\"extra\":1,\"schema\""),
    )
    .unwrap();
    assert!(matches!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::MalformedRecord { .. }
    ));
    // A digest that is not canonical is refused by the domain's own parser.
    fs::write(&path, original.replace("sha256:", "sha256:Z")).unwrap();
    assert!(matches!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::MalformedRecord { .. }
    ));
    // Truncation (a crash that bypassed the atomic publish) is malformed, not a panic.
    fs::write(&path, &original[..original.len() / 2]).unwrap();
    assert!(matches!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::MalformedRecord { .. }
    ));
    fs::write(&path, &original).unwrap();
    assert_eq!(lab.store().revisions().unwrap().len(), 2);
}

#[test]
fn a_recorded_history_that_registration_would_refuse_is_rejected_on_load() {
    let lab = Lab::new();
    let placed = lab.register(Target::New, STORY, b"v1");
    let path = lab.revision_file(placed.artifact, 1);
    let kind_change = fs::read_to_string(&path)
        .unwrap()
        .replace("\"revision\":1", "\"revision\":2")
        .replace(
            "{\"type\":\"manuscript\",\"origin\":\"original\"}",
            "{\"type\":\"audio\"}",
        );
    fs::write(lab.revision_file(placed.artifact, 2), kind_change).unwrap();
    assert_eq!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::HistoryRejected {
            artifact: placed.artifact,
            error: ReplayError::Refused {
                position: 1,
                error: RevisionError::KindChanged {
                    from: ArtifactKind::Manuscript,
                    to: ArtifactKind::Audio
                }
            }
        }
    );
}

#[test]
fn files_forge_did_not_write_are_reported_not_skipped_and_temporaries_are_ignored() {
    let lab = Lab::new();
    let placed = lab.register(Target::New, STORY, b"v1");
    let artifact_dir = lab.root.join("artifacts").join(placed.artifact.to_string());
    fs::write(artifact_dir.join(".tmp-1234-0"), b"half a record").unwrap();
    assert_eq!(lab.store().revisions().unwrap().len(), 1);
    fs::write(artifact_dir.join("notes.txt"), b"hello").unwrap();
    assert!(matches!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::UnexpectedEntry { .. }
    ));
    fs::remove_file(artifact_dir.join("notes.txt")).unwrap();
    fs::create_dir(lab.root.join("artifacts").join("not-an-id")).unwrap();
    assert!(matches!(
        corrupt(lab.store().revisions().unwrap_err()),
        Corruption::UnexpectedEntry { .. }
    ));
}

#[test]
fn appending_an_existing_revision_conflicts_and_leaves_the_original_untouched() {
    let lab = Lab::new();
    let placed = lab.register(Target::New, STORY, b"v1");
    let before = fs::read(lab.revision_file(placed.artifact, 1)).unwrap();
    let store = lab.store();
    let revision = store.revisions().unwrap().remove(0);
    assert_eq!(store.append_revision(&revision), Err(StoreError::Conflict));
    assert_eq!(
        fs::read(lab.revision_file(placed.artifact, 1)).unwrap(),
        before
    );
}

#[test]
fn concurrent_writers_of_the_same_revision_yield_exactly_one_winner() {
    let lab = Lab::new();
    let first = lab.register(Target::New, STORY, b"v1");
    let store = lab.store();
    // Two different revision 2 candidates for one artifact, written from two threads.
    let candidates: Vec<_> = [b"left".as_slice(), b"right".as_slice()]
        .into_iter()
        .map(|bytes| {
            let request = RegisterRequest {
                target: Target::Existing(first.artifact),
                payload: STORY,
                producer: ProducerLabel::parse("AI host session").unwrap(),
                rights: RightsClaim::OriginalWork,
                inputs: Vec::new(),
                bytes: bytes.to_vec(),
            };
            (request, bytes.to_vec())
        })
        .collect();
    let results: Vec<_> = thread::scope(|scope| {
        let handles: Vec<_> = candidates
            .into_iter()
            .map(|(request, _)| {
                let store = &store;
                let runtime = &lab.runtime;
                scope.spawn(move || register_artifact(store, runtime.ids(), request))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    let winners = results
        .iter()
        .filter(|result| matches!(result, Ok(Registered::Created(_))))
        .count();
    let conflicts = results
        .iter()
        .filter(|result| {
            matches!(
                result,
                Err(crate::application::register::RegisterError::Store(
                    StoreError::Conflict
                ))
            )
        })
        .count();
    // Both may succeed only if the second observed the first's revision and became revision 3.
    assert!(winners >= 1, "{results:?}");
    assert_eq!(winners + conflicts, 2, "{results:?}");
    let numbers: Vec<u32> = store
        .revisions()
        .unwrap()
        .iter()
        .filter(|revision| revision.artifact() == first.artifact)
        .map(|revision| revision.revision().get())
        .collect();
    let expected: Vec<u32> = (1..=u32::try_from(1 + winners).unwrap()).collect();
    assert_eq!(
        numbers, expected,
        "no revision number was lost or duplicated"
    );
}

#[test]
fn evidence_round_trips_and_a_misnamed_record_is_malformed() {
    let lab = Lab::new();
    let placed = lab.register(Target::New, ArtifactPayload::Audio, &audio(0));
    let revision = lab.store().revisions().unwrap().remove(0);
    let record = RecordedEvidence {
        evidence: Evidence {
            id: EvidenceId::from_bits(42),
            subject: revision.pin(),
            basis: Default::default(),
            finding: Finding::Deterministic {
                check: CheckName::parse("ogg-opus-profile-v1:opus-mono-24-v1").unwrap(),
                outcome: CheckOutcome::Passed,
            },
        },
        detail: Detail::new([("duration_ms".to_owned(), DetailValue::Count(1_000))]),
    };
    let store = lab.store();
    store.append_evidence(&record).unwrap();
    assert_eq!(store.append_evidence(&record), Err(StoreError::Conflict));
    assert_eq!(store.evidence().unwrap(), vec![record.clone()]);
    let _ = placed;

    let misnamed = lab
        .root
        .join("evidence")
        .join(format!("{}.json", EvidenceId::from_bits(43)));
    fs::copy(
        lab.root
            .join("evidence")
            .join(format!("{}.json", EvidenceId::from_bits(42))),
        &misnamed,
    )
    .unwrap();
    assert!(matches!(
        corrupt(store.evidence().unwrap_err()),
        Corruption::MalformedRecord { .. }
    ));
}

#[test]
fn sources_must_be_regular_bounded_files() {
    let lab = Lab::new();
    let runtime = &lab.runtime;
    let file = lab.root.parent().unwrap().join("script.txt");
    fs::write(&file, "Lời thoại").unwrap();
    assert_eq!(
        runtime.read_source(&file),
        Ok("Lời thoại".as_bytes().to_vec())
    );
    assert_eq!(
        runtime.read_source(&lab.root),
        Err(SourceError::NotARegularFile)
    );
    assert!(matches!(
        runtime.read_source(&lab.root.join("missing.txt")),
        Err(SourceError::Unavailable { .. })
    ));
    let big = lab.root.parent().unwrap().join("big.bin");
    fs::File::create(&big)
        .unwrap()
        .set_len(MAX_SOURCE_BYTES + 1)
        .unwrap();
    assert_eq!(
        runtime.read_source(&big),
        Err(SourceError::TooLarge {
            max_bytes: MAX_SOURCE_BYTES
        })
    );
}

fn validated_audio(lab: &Lab) -> Placement {
    let placed = lab.register(Target::New, ArtifactPayload::Audio, &audio(0));
    validate_artifact(
        &lab.store(),
        lab.runtime.ids(),
        ValidateRequest {
            artifact: placed.artifact,
            profile: Some(PROFILE),
        },
    )
    .unwrap();
    placed
}

#[test]
fn a_bundle_holds_the_exact_audio_bytes_and_a_manifest_that_claims_no_submission() {
    let lab = Lab::new();
    let placed = validated_audio(&lab);
    let bundle = prepare_export(
        &lab.store(),
        ExportRequest {
            artifact: placed.artifact,
            profile: PROFILE,
        },
    )
    .unwrap();
    let destination = lab.root.parent().unwrap().join("bundles").join("out");
    let receipt = lab.runtime.write_bundle(&destination, &bundle).unwrap();

    assert_eq!(fs::read(destination.join(AUDIO_FILE)).unwrap(), audio(0));
    let manifest_bytes = fs::read(destination.join(MANIFEST_FILE)).unwrap();
    assert_eq!(receipt.manifest_digest, Sha256Digest::of(&manifest_bytes));
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["schema"], "cantos-forge.handoff-bundle/v1");
    assert_eq!(manifest["artifact"], placed.artifact.to_string());
    assert_eq!(manifest["revision"], 1);
    assert_eq!(manifest["audio"]["sha256"], placed.digest.to_string());
    assert_eq!(manifest["audio"]["byte_length"], audio(0).len());
    assert_eq!(manifest["profile"], "opus-mono-24-v1");
    assert_eq!(manifest["measured"]["outcome"], "passed");
    assert_eq!(manifest["measured"]["detail"]["container"], "ogg");
    assert_eq!(manifest["rights"]["type"], "original_work");
    assert_eq!(manifest["provenance"]["producer"], "AI host session");
    assert_eq!(manifest["evaluation"]["deterministic"]["status"], "current");
    assert_eq!(manifest["evaluation"]["ai_review"]["status"], "not_run");
    assert_eq!(manifest["evaluation"]["human_review"]["status"], "not_run");
    assert_eq!(manifest["submission"]["status"], "not_run");
    assert!(manifest["submission"]["note"]
        .as_str()
        .unwrap()
        .contains("local files only"));
}

#[test]
fn a_bundle_is_never_merged_into_or_written_over_an_existing_destination() {
    let lab = Lab::new();
    let placed = validated_audio(&lab);
    let bundle = prepare_export(
        &lab.store(),
        ExportRequest {
            artifact: placed.artifact,
            profile: PROFILE,
        },
    )
    .unwrap();
    let destination = lab.root.parent().unwrap().join("occupied");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("keep.txt"), "mine").unwrap();
    assert_eq!(
        lab.runtime.write_bundle(&destination, &bundle),
        Err(BundleError::DestinationExists)
    );
    assert_eq!(
        fs::read_to_string(destination.join("keep.txt")).unwrap(),
        "mine"
    );
    assert!(!Path::new(&destination.join(MANIFEST_FILE)).exists());
}
