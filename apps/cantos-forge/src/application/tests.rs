//! Use cases against the in-memory fakes: deterministic, no filesystem.

use super::{
    export::{prepare_export, ExportError, ExportRequest},
    memory::{CountingIds, MemoryStore},
    ports::{Corruption, Store, StoreError},
    register::{
        register_artifact, InputRef, Placement, RegisterError, RegisterRequest, Registered, Target,
    },
    validate::{validate_artifact, ValidateError, ValidateRequest, Write},
    view::{list_artifacts, show_artifact},
};
use crate::domain::{
    artifact::{
        ArtifactKind, ArtifactPayload, InputPinsError, ManuscriptOrigin, RevisionError,
        RevisionNumber, RightsClaim,
    },
    audio_profile::AudioProfile,
    checks::DetailValue,
    digest::Sha256Digest,
    evaluation::{CheckOutcome, LayerStatus},
    handoff::ExportRefusal,
    identity::{ArtifactId, EvidenceId},
    label::ProducerLabel,
    lineage::Freshness,
    ogg_opus::testing::Fixture,
};

const STORY: ArtifactPayload = ArtifactPayload::Manuscript {
    origin: ManuscriptOrigin::Original,
};
const PROFILE: AudioProfile = AudioProfile::OpusMono24V1;

fn request(target: Target, payload: ArtifactPayload, bytes: &[u8]) -> RegisterRequest {
    RegisterRequest {
        target,
        payload,
        producer: ProducerLabel::parse("AI host session").unwrap(),
        rights: RightsClaim::OriginalWork,
        inputs: Vec::new(),
        bytes: bytes.to_vec(),
    }
}

/// One second of valid mono Opus at about 12 kbit/s: inside every profile.
fn audio_bytes(seed: u8) -> Vec<u8> {
    Fixture::mono(vec![30 + usize::from(seed); 50]).build()
}

struct Lab {
    store: MemoryStore,
    ids: CountingIds,
}

impl Lab {
    fn new() -> Self {
        Self {
            store: MemoryStore::default(),
            ids: CountingIds::default(),
        }
    }

    fn register(&self, request: RegisterRequest) -> Result<Registered, RegisterError> {
        register_artifact(&self.store, &self.ids, request)
    }

    fn created(&self, request: RegisterRequest) -> Placement {
        match self.register(request) {
            Ok(Registered::Created(placement)) => placement,
            other => panic!("expected a created revision, found {other:?}"),
        }
    }

    fn validate(
        &self,
        artifact: ArtifactId,
        profile: Option<AudioProfile>,
    ) -> Result<super::validate::Validated, ValidateError> {
        validate_artifact(
            &self.store,
            &self.ids,
            ValidateRequest { artifact, profile },
        )
    }

    /// A story and an audio artifact pinned to it, both registered.
    fn story_and_audio(&self) -> (Placement, Placement) {
        let story = self.created(request(
            Target::New,
            STORY,
            "Lời hẹn cuối mùa mưa.".as_bytes(),
        ));
        let mut audio = request(Target::New, ArtifactPayload::Audio, &audio_bytes(0));
        audio.inputs = vec![InputRef {
            artifact: story.artifact,
            revision: story.revision,
        }];
        let audio = self.created(audio);
        (story, audio)
    }
}

fn rev(n: u32) -> RevisionNumber {
    RevisionNumber::new(n).unwrap()
}

#[test]
fn a_new_artifact_gets_a_fresh_identity_and_its_bytes_are_stored_under_their_digest() {
    let lab = Lab::new();
    let placed = lab.created(request(Target::New, STORY, b"chuong mot"));
    assert_eq!(placed.artifact, ArtifactId::from_bits(1));
    assert_eq!(placed.revision, rev(1));
    assert_eq!(placed.digest, Sha256Digest::of(b"chuong mot"));
    assert_eq!(lab.store.blob(placed.digest).unwrap(), b"chuong mot");
    let second = lab.created(request(Target::New, STORY, b"chuong mot"));
    assert_eq!(
        second.artifact,
        ArtifactId::from_bits(2),
        "same bytes, separate artifact"
    );
}

#[test]
fn replaying_a_registration_creates_nothing_and_a_change_is_a_successor() {
    let lab = Lab::new();
    let first = lab.created(request(Target::New, STORY, b"v1"));
    let again = lab.register(request(Target::Existing(first.artifact), STORY, b"v1"));
    assert_eq!(again, Ok(Registered::AlreadyCurrent(first)));
    assert_eq!(lab.store.revisions().unwrap().len(), 1);
    let second = lab.created(request(Target::Existing(first.artifact), STORY, b"v2"));
    assert_eq!(second.revision, rev(2));
    assert_eq!(lab.store.revisions().unwrap().len(), 2);
}

#[test]
fn naming_an_unknown_artifact_never_creates_it_with_a_chosen_identity() {
    let lab = Lab::new();
    let ghost = ArtifactId::from_bits(99);
    assert_eq!(
        lab.register(request(Target::Existing(ghost), STORY, b"x")),
        Err(RegisterError::UnknownArtifact(ghost))
    );
    assert!(lab.store.revisions().unwrap().is_empty());
}

#[test]
fn an_input_pin_takes_its_digest_from_history_and_unknown_inputs_are_refused() {
    let lab = Lab::new();
    let (story, audio) = lab.story_and_audio();
    let revisions = lab.store.revisions().unwrap();
    let recorded = revisions
        .iter()
        .find(|revision| revision.artifact() == audio.artifact)
        .unwrap();
    let pins: Vec<_> = recorded.facts().inputs.iter().collect();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].digest, story.digest);
    assert_eq!(pins[0].revision, story.revision);

    let missing_revision = InputRef {
        artifact: story.artifact,
        revision: rev(7),
    };
    let mut bad = request(Target::New, ArtifactPayload::Audio, &audio_bytes(1));
    bad.inputs = vec![missing_revision];
    assert_eq!(
        lab.register(bad),
        Err(RegisterError::UnknownInput(missing_revision))
    );

    let once = InputRef {
        artifact: story.artifact,
        revision: story.revision,
    };
    let mut twice = request(Target::New, ArtifactPayload::Audio, &audio_bytes(2));
    twice.inputs = vec![once, once];
    assert_eq!(
        lab.register(twice),
        Err(RegisterError::RepeatedInput(
            InputPinsError::RepeatedArtifact {
                artifact: story.artifact
            }
        ))
    );
}

#[test]
fn a_kind_change_or_a_self_input_is_refused_and_writes_nothing() {
    let lab = Lab::new();
    let story = lab.created(request(Target::New, STORY, b"v1"));
    let as_audio = request(
        Target::Existing(story.artifact),
        ArtifactPayload::Audio,
        b"v2",
    );
    assert_eq!(
        lab.register(as_audio),
        Err(RegisterError::Refused(RevisionError::KindChanged {
            from: ArtifactKind::Manuscript,
            to: ArtifactKind::Audio
        }))
    );
    let mut self_pinned = request(Target::Existing(story.artifact), STORY, b"v2");
    self_pinned.inputs = vec![InputRef {
        artifact: story.artifact,
        revision: story.revision,
    }];
    assert_eq!(
        lab.register(self_pinned),
        Err(RegisterError::Refused(RevisionError::SelfInput))
    );
    assert_eq!(lab.store.revisions().unwrap().len(), 1);
}

#[test]
fn losing_a_race_for_the_same_revision_is_a_conflict_not_a_silent_overwrite() {
    let lab = Lab::new();
    *lab.store.lose_next_revision_race.borrow_mut() = true;
    assert_eq!(
        lab.register(request(Target::New, STORY, b"x")),
        Err(RegisterError::Store(StoreError::Conflict))
    );
    assert!(lab.store.revisions().unwrap().is_empty());
    // The command is safe to repeat afterwards.
    assert!(matches!(
        lab.register(request(Target::New, STORY, b"x")),
        Ok(Registered::Created(_))
    ));
}

#[test]
fn validation_needs_the_right_profile_arguments_for_each_kind() {
    let lab = Lab::new();
    let (story, audio) = lab.story_and_audio();
    let report = lab.created(request(Target::New, ArtifactPayload::Report, b"{}"));
    let script = lab.created(request(Target::New, ArtifactPayload::ScriptIr, b"{}"));
    assert_eq!(
        lab.validate(audio.artifact, None).unwrap_err(),
        ValidateError::ProfileRequired
    );
    assert_eq!(
        lab.validate(story.artifact, Some(PROFILE)).unwrap_err(),
        ValidateError::ProfileNotApplicable {
            kind: ArtifactKind::Manuscript
        }
    );
    for (placed, kind) in [
        (report, ArtifactKind::Report),
        (script, ArtifactKind::ScriptIr),
    ] {
        assert_eq!(
            lab.validate(placed.artifact, None).unwrap_err(),
            ValidateError::NoCheckFor { kind }
        );
    }
    let ghost = ArtifactId::from_bits(404);
    assert_eq!(
        lab.validate(ghost, None).unwrap_err(),
        ValidateError::UnknownArtifact(ghost)
    );
    assert!(
        lab.store.evidence().unwrap().is_empty(),
        "refusals record no evidence"
    );
}

#[test]
fn validating_audio_records_evidence_once_and_a_replay_returns_the_same_record() {
    let lab = Lab::new();
    let (_, audio) = lab.story_and_audio();
    let first = lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    assert_eq!(first.write, Write::Recorded);
    assert_eq!(first.result.outcome, CheckOutcome::Passed);
    assert_eq!(first.evidence, EvidenceId::from_bits(1));
    let replay = lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    assert_eq!(replay.write, Write::AlreadyRecorded);
    assert_eq!(replay.evidence, first.evidence);
    assert_eq!(lab.store.evidence().unwrap().len(), 1);
    // Another profile is another check, hence another record.
    let wider = lab
        .validate(audio.artifact, Some(AudioProfile::OpusMono40V1))
        .unwrap();
    assert_eq!(wider.write, Write::Recorded);
    assert_eq!(lab.store.evidence().unwrap().len(), 2);
}

#[test]
fn failing_audio_is_recorded_as_failed_evidence_with_its_reason() {
    let lab = Lab::new();
    let not_audio = lab.created(request(
        Target::New,
        ArtifactPayload::Audio,
        b"definitely not an Ogg stream",
    ));
    let validated = lab.validate(not_audio.artifact, Some(PROFILE)).unwrap();
    assert_eq!(validated.result.outcome, CheckOutcome::Failed);
    let records = lab.store.evidence().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].detail.get("refusal"),
        Some(&DetailValue::Text("not_ogg".to_owned()))
    );
}

#[test]
fn a_new_revision_needs_its_own_evidence() {
    let lab = Lab::new();
    let (_, audio) = lab.story_and_audio();
    let v1 = lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    let v2 = lab.created(request(
        Target::Existing(audio.artifact),
        ArtifactPayload::Audio,
        &audio_bytes(1),
    ));
    assert_eq!(v2.revision, rev(2));
    let after = lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    assert_eq!(after.write, Write::Recorded);
    assert_ne!(after.evidence, v1.evidence);
    assert_eq!(after.subject.revision, rev(2));
}

#[test]
fn validating_tampered_stored_bytes_fails_closed_instead_of_measuring_them() {
    let lab = Lab::new();
    let (_, audio) = lab.story_and_audio();
    lab.store
        .blobs
        .borrow_mut()
        .insert(audio.digest, audio_bytes(9));
    assert_eq!(
        lab.validate(audio.artifact, Some(PROFILE)).unwrap_err(),
        ValidateError::Store(StoreError::Corrupt(Corruption::BlobDigestMismatch {
            digest: audio.digest
        }))
    );
}

#[test]
fn export_is_refused_until_the_profile_check_has_run_and_then_prepares_a_bundle() {
    let lab = Lab::new();
    let (story, audio) = lab.story_and_audio();
    let export = ExportRequest {
        artifact: audio.artifact,
        profile: PROFILE,
    };
    assert_eq!(
        prepare_export(&lab.store, export).unwrap_err(),
        ExportError::Refused(vec![ExportRefusal::CheckNotRun])
    );
    lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    let bundle = prepare_export(&lab.store, export).unwrap();
    assert_eq!(bundle.audio, audio_bytes(0));
    assert_eq!(bundle.measured.outcome, CheckOutcome::Passed);
    assert_eq!(bundle.plan.profile, PROFILE);
    assert_eq!(bundle.plan.inputs.len(), 1);
    assert_eq!(bundle.plan.inputs[0].artifact, story.artifact);
    assert_eq!(bundle.plan.layers.ai_review, LayerStatus::NotRun);
    assert_eq!(bundle.plan.layers.human_review, LayerStatus::NotRun);
}

#[test]
fn export_reports_every_reason_when_rights_are_unstated_and_the_check_is_missing() {
    let lab = Lab::new();
    let mut unstated = request(Target::New, ArtifactPayload::Audio, &audio_bytes(0));
    unstated.rights = RightsClaim::Unstated;
    let placed = lab.created(unstated);
    assert_eq!(
        prepare_export(
            &lab.store,
            ExportRequest {
                artifact: placed.artifact,
                profile: PROFILE
            }
        )
        .unwrap_err(),
        ExportError::Refused(vec![
            ExportRefusal::RightsUnstated,
            ExportRefusal::CheckNotRun
        ])
    );
}

#[test]
fn a_changed_script_makes_validated_audio_stale_and_blocks_export() {
    let lab = Lab::new();
    let (story, audio) = lab.story_and_audio();
    lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    lab.created(request(
        Target::Existing(story.artifact),
        STORY,
        "Lời hẹn đã đổi.".as_bytes(),
    ));
    let error = prepare_export(
        &lab.store,
        ExportRequest {
            artifact: audio.artifact,
            profile: PROFILE,
        },
    )
    .unwrap_err();
    let ExportError::Refused(reasons) = error else {
        panic!("expected a refusal, found {error:?}");
    };
    assert!(
        matches!(reasons.as_slice(), [ExportRefusal::InputsStale(stale)] if stale.len() == 1),
        "{reasons:?}"
    );
    // The audio itself is untouched and its evidence is still about its own revision.
    let detail = show_artifact(&lab.store, audio.artifact).unwrap();
    assert!(matches!(detail.inputs, Ok(Freshness::Stale(_))));
    assert_eq!(
        detail.evaluation.unwrap().deterministic,
        LayerStatus::Current(CheckOutcome::Passed)
    );
}

#[test]
fn export_never_trusts_stored_bytes_that_no_longer_match_their_digest() {
    let lab = Lab::new();
    let (_, audio) = lab.story_and_audio();
    lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    lab.store
        .blobs
        .borrow_mut()
        .insert(audio.digest, audio_bytes(9));
    assert_eq!(
        prepare_export(
            &lab.store,
            ExportRequest {
                artifact: audio.artifact,
                profile: PROFILE
            }
        )
        .unwrap_err(),
        ExportError::Store(StoreError::Corrupt(Corruption::BlobDigestMismatch {
            digest: audio.digest
        }))
    );
}

#[test]
fn list_and_show_report_heads_and_independent_layers() {
    let lab = Lab::new();
    let (story, audio) = lab.story_and_audio();
    let listed = list_artifacts(&lab.store).unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|summary| (summary.artifact, summary.kind))
            .collect::<Vec<_>>(),
        [
            (story.artifact, ArtifactKind::Manuscript),
            (audio.artifact, ArtifactKind::Audio)
        ]
    );
    let before = show_artifact(&lab.store, audio.artifact).unwrap();
    assert_eq!(
        before.evaluation.unwrap().deterministic,
        LayerStatus::NotRun
    );
    assert!(matches!(before.inputs, Ok(Freshness::Current)));
    lab.validate(audio.artifact, Some(PROFILE)).unwrap();
    let after = show_artifact(&lab.store, audio.artifact).unwrap();
    let layers = after.evaluation.unwrap();
    assert_eq!(
        layers.deterministic,
        LayerStatus::Current(CheckOutcome::Passed)
    );
    assert_eq!(
        layers.ai_review,
        LayerStatus::NotRun,
        "a technical pass is not an AI review"
    );
    assert_eq!(
        layers.human_review,
        LayerStatus::NotRun,
        "and not a listening"
    );
    assert_eq!(after.evidence.len(), 1);
    assert_eq!(
        show_artifact(&lab.store, ArtifactId::from_bits(404)).unwrap_err(),
        super::view::ViewError::UnknownArtifact(ArtifactId::from_bits(404))
    );
}
