use super::*;
use crate::contexts::catalog::domain::*;
use crate::contexts::catalog::{content::wire, domain::content::*};
use crate::contexts::media::domain::lifecycle::*;
use crate::contexts::production::domain::brief::rights_digest;
use crate::contexts::production::domain::brief::ProductionBrief;
use proptest::prelude::*;
const WORK: &str = "10000000-0000-4000-8000-000000000001";
const UNIT: &str = "20000000-0000-4000-8000-000000000002";
const CONTENT: &str = "30000000-0000-4000-8000-000000000003";
const BRIEF: &str = "40000000-0000-4000-8000-000000000004";
const ID: &str = "50000000-0000-4000-8000-000000000005";
const OP: &str = "60000000-0000-4000-8000-000000000006";
fn brief() -> ProductionBrief {
    let content = ContentRevision::new(ContentRevisionFacts {
        id: ContentRevisionId::parse(CONTENT).unwrap(),
        work_id: WorkId::parse(WORK).unwrap(),
        unit_id: ContentUnitId::parse(UNIT).unwrap(),
        revision: 1,
        details: ContentDetails::new(
            "vi-VN".into(),
            ContentBody::Recording,
            vec![],
            "Synthetic recording fixture".into(),
        )
        .unwrap(),
        recorded_by: OwnerId::parse("alice").unwrap(),
        recorded_at: 1000,
    })
    .unwrap();
    let request: dto::CreateProductionBriefRequest = serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/briefs/v1/create-recording.json"
    ))
    .unwrap();
    ProductionBrief::admit(
        dto::ProductionBriefDocument {
            schema_version: "1".into(),
            id: BRIEF.into(),
            version: 1,
            work: dto::BriefWork {
                id: WORK.into(),
                owner_id: "alice".into(),
                kind: "podcast".into(),
                title: "Tập thử âm thanh".into(),
                rights_refs: vec!["synthetic-rights".into()],
                rights_digest: rights_digest(&["synthetic-rights".into()]).unwrap(),
            },
            content: wire::response(&content).unwrap(),
            instructions: dto::ProductionInstructions::Recording {},
            guidance: request.guidance,
            created_by: "alice".into(),
            created_at: 2000,
            digest: String::new(),
        },
        &content,
    )
    .unwrap()
}
fn request() -> dto::CreateAudioSubmissionRequest {
    dto::CreateAudioSubmissionRequest {
        operation_id: OP.into(),
        id: ID.into(),
        binding: Binding::from_brief(&brief()).unwrap().raw().clone(),
        expected_sha256: "a".repeat(64),
        expected_byte_length: 100,
        declarations: dto::ProducerDeclarations {
            producer: "Synthetic producer".into(),
            method: "recording".into(),
            provenance: "Original owned test signal".into(),
            rights_refs: vec!["synthetic-rights".into()],
            quality_report: "NOT_RUN listening".into(),
        },
        upload_token: "d".repeat(64),
        expires_at: 5000,
    }
}
fn submission() -> AudioSubmission {
    AudioSubmission::create(
        &request(),
        &brief(),
        &OwnerId::parse("alice").unwrap(),
        3000,
    )
    .unwrap()
}
fn rendition() -> AudioRendition {
    AudioRendition::create(
        &submission(),
        dto::AudioMediaMetadata {
            sha256: "a".repeat(64),
            byte_length: 100,
            container: "ogg".into(),
            codec: "opus".into(),
            channels: 1,
            sample_rate_hz: 48000,
            duration_ms: 1000,
            audio_bitrate_bps: 24000,
            profile_id: "opus-mono-24-v1".into(),
        },
        AudioRenditionId::parse(OP).unwrap(),
        4000,
    )
    .unwrap()
}
fn review(kind: dto::AudioReviewKind) -> dto::AudioReviewDocument {
    let r = rendition();
    record_review(
        dto::AudioReviewDocument {
            id: ID.into(),
            rendition_id: r.document().id.clone(),
            operation_id: OP.into(),
            sequence: 1,
            reviewer_id: "alice".into(),
            reviewed_at: 4500,
            request: dto::CreateAudioReviewRequest {
                operation_id: OP.into(),
                rendition_digest: r.document().digest.clone(),
                sha256: "a".repeat(64),
                kind,
                decision: dto::AudioReviewDecision::Approved,
                notes: "Synthetic approval; no listening claim".into(),
                evidence_refs: vec!["fixture-only".into()],
            },
            digest: String::new(),
        },
        &r,
    )
    .unwrap()
}
struct Fixture {
    content: ContentRevision,
    work: Work,
    unit: ContentUnit,
    r: AudioRendition,
    reviews: Vec<AudioReview>,
    candidate: ReleaseCandidate,
    rights: DistributionClearance,
    approval: PublicationApproval,
    object: ObjectObservation,
    delivery: DeliveryObservation,
    actor: OwnerId,
}
fn fixture() -> Fixture {
    let work = Work::new(NewWork {
        id: WorkId::parse(WORK).unwrap(),
        owner_id: OwnerId::parse("alice").unwrap(),
        kind: WorkKind::Podcast,
        details: WorkDetails {
            title: "Ánh đèn cuối sân khấu".into(),
            synopsis: "Thử nghiệm".into(),
            language: "vi-VN".into(),
            visibility: Visibility::Private,
            rights_refs: vec!["synthetic-rights".into()],
        },
        at: 1000,
    })
    .unwrap();
    let unit = ContentUnit::new(
        &work,
        NewContentUnit {
            id: ContentUnitId::parse(UNIT).unwrap(),
            kind: ContentUnitKind::Episode,
            details: UnitDetails {
                title: "Một lời hẹn".into(),
                description: "Tập thử".into(),
                source: None,
                script: None,
                content_digest: None,
            },
            position: 1,
            at: 1000,
        },
    )
    .unwrap();
    let r = rendition();
    let reviews: Vec<_> = [
        dto::AudioReviewKind::Qc,
        dto::AudioReviewKind::Rights,
        dto::AudioReviewKind::Editorial,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, k)| {
        let mut raw = review(k);
        raw.id = format!("70000000-0000-4000-8000-{:012}", i);
        raw.sequence = i as u64 + 1;
        raw.operation_id = raw.id.clone();
        raw.request.operation_id = raw.id.clone();
        AudioReview::record(raw, &r).unwrap()
    })
    .collect();
    let candidate = ReleaseCandidate::record(
        dto::ReleaseCandidateDocument {
            id: ID.into(),
            rendition_id: r.document().id.clone(),
            rendition_digest: r.document().digest.clone(),
            binding: r.document().binding.clone(),
            sha256: "a".repeat(64),
            work_metadata_revision: 1,
            unit_metadata_revision: 1,
            review_ids: reviews.iter().map(|r| r.document().id.clone()).collect(),
            created_by: "alice".into(),
            created_at: 4500,
            operation_id: OP.into(),
            digest: String::new(),
        },
        &r,
    )
    .unwrap();
    let rights = DistributionClearance::record(
        dto::DistributionClearanceDocument {
            id: CONTENT.into(),
            candidate_id: ID.into(),
            actor_id: "alice".into(),
            sequence: 1,
            recorded_at: 4600,
            request: dto::RecordDistributionRightsRequest {
                operation_id: CONTENT.into(),
                candidate_digest: candidate.document().digest.clone(),
                decision: dto::DistributionDecision::Cleared,
                rights_refs: vec!["synthetic-rights".into()],
                valid_until: 10000,
                evidence_refs: vec!["synthetic-owner-reviewed-permission".into()],
                notes: "Fixture permission for distribution".into(),
            },
            digest: String::new(),
        },
        &candidate,
    )
    .unwrap();
    let approval = PublicationApproval::record(
        dto::PublicationApprovalDocument {
            id: BRIEF.into(),
            candidate_id: ID.into(),
            actor_id: "alice".into(),
            sequence: 1,
            recorded_at: 4700,
            request: dto::RecordPublicationApprovalRequest {
                operation_id: BRIEF.into(),
                candidate_digest: candidate.document().digest.clone(),
                clearance_digest: rights.document().digest.clone(),
                decision: dto::PublicationApprovalDecision::Approved,
                notes: "Approve this exact synthetic candidate".into(),
            },
            digest: String::new(),
        },
        &candidate,
    )
    .unwrap();
    let content = brief().document().content.clone();
    let content = crate::contexts::catalog::restore_content(content).unwrap();
    Fixture {
        content,
        work,
        unit,
        r,
        reviews,
        candidate,
        rights,
        approval,
        object: ObjectObservation::Verified {
            sha256: Sha256::parse(&"a".repeat(64)).unwrap(),
            byte_length: 100,
        },
        delivery: DeliveryObservation::Verified(
            DeliveryEvidence::new(
                "publication-fixture".into(),
                Sha256::parse(&"a".repeat(64)).unwrap(),
                100,
                4800,
                9000,
            )
            .unwrap(),
        ),
        actor: OwnerId::parse("alice").unwrap(),
    }
}
impl Fixture {
    fn facts(&self) -> PublicationFacts<'_> {
        PublicationFacts {
            candidate: &self.candidate,
            rendition: &self.r,
            work: &self.work,
            unit: &self.unit,
            content: &self.content,
            upload_expires_at: 8000,
            actor: &self.actor,
            current_brief: true,
            upload_revoked: false,
            reviews: &self.reviews,
            rights: Some(&self.rights),
            approval: Some(&self.approval),
            object: &self.object,
            delivery: &self.delivery,
            now: 5000,
        }
    }
}
#[test]
fn genuine_admitted_values_create_an_immutable_exact_release() {
    let f = fixture();
    let witness = decide(&f.facts()).unwrap();
    let release = AudioRelease::publish(
        ReleaseId::parse(UNIT).unwrap(),
        &f.actor,
        5000,
        None,
        witness,
    )
    .unwrap();
    assert_eq!(release.document().manifest.media.sha256, "a".repeat(64));
    assert_eq!(release.document().manifest.binding, f.r.document().binding);
    assert_eq!(release.document().manifest.review_evidence.len(), 3);
    assert_eq!(
        AudioRelease::restore(release.document().clone(), &f.candidate, &f.r, &f.content).unwrap(),
        release
    );
    let mut corrupt = release.document().clone();
    corrupt.manifest.language = "en-US".into();
    assert_eq!(
        AudioRelease::restore(corrupt, &f.candidate, &f.r, &f.content),
        Err(PublicationError::BindingMismatch)
    );
}
#[test]
fn readiness_accumulates_independent_failures_and_never_trusts_producer_claims() {
    use dto::PublicationBlocker as B;
    let f = fixture();
    let mut facts = f.facts();
    facts.current_brief = false;
    facts.upload_revoked = true;
    facts.reviews = &[];
    facts.rights = None;
    facts.approval = None;
    facts.object = &ObjectObservation::Missing;
    facts.delivery = &DeliveryObservation::Unavailable;
    assert_eq!(
        evaluate(&facts),
        vec![
            B::StaleBrief,
            B::UploadPermissionRevoked,
            B::ObjectMissing,
            B::QcNotApproved,
            B::RightsReviewNotApproved,
            B::EditorialNotApproved,
            B::CandidateReviewsChanged,
            B::RightsUnrecorded,
            B::ApprovalMissing,
            B::DeliveryUnavailable
        ]
    );
}
#[test]
fn bounded_gate_truth_table_matches_an_independent_oracle() {
    use dto::PublicationBlocker as B;
    let f = fixture();
    for bits in 0..256 {
        let mut facts = f.facts();
        facts.current_brief = bits & 1 == 0;
        facts.upload_revoked = bits & 2 != 0;
        if bits & 4 != 0 {
            facts.reviews = &[];
        }
        if bits & 8 != 0 {
            facts.rights = None;
        }
        if bits & 16 != 0 {
            facts.approval = None;
        }
        if bits & 32 != 0 {
            facts.object = &ObjectObservation::Missing;
        }
        if bits & 64 != 0 {
            facts.delivery = &DeliveryObservation::Unavailable;
        }
        if bits & 128 != 0 {
            facts.now = 10000;
        }
        let mut expected = Vec::new();
        if bits & 1 != 0 {
            expected.push(B::StaleBrief);
        }
        if bits & 2 != 0 {
            expected.push(B::UploadPermissionRevoked);
        }
        if bits & 4 != 0 {
            expected.extend([
                B::QcNotApproved,
                B::RightsReviewNotApproved,
                B::EditorialNotApproved,
                B::CandidateReviewsChanged,
            ]);
        }
        if bits & 8 != 0 {
            expected.push(B::RightsUnrecorded);
            if bits & 16 == 0 {
                expected.push(B::ApprovalStale);
            }
        } else if bits & 128 != 0 {
            expected.push(B::RightsExpired);
        }
        if bits & 16 != 0 {
            expected.push(B::ApprovalMissing);
        }
        if bits & 32 != 0 {
            expected.push(B::ObjectMissing);
        }
        if bits & 64 != 0 {
            expected.push(B::DeliveryUnavailable);
        } else if bits & 128 != 0 {
            expected.push(B::DeliveryEvidenceExpired);
        }
        if bits & 128 != 0 {
            expected.push(B::UploadPermissionExpired);
        }
        expected.sort();
        assert_eq!(evaluate(&facts), expected, "bits={bits}");
        assert_eq!(decide(&facts).is_ok(), bits == 0);
    }
}
#[test]
fn later_qc_withdrawal_invalidates_candidate_and_publish() {
    let mut f = fixture();
    let mut raw = f.reviews[0].document().clone();
    raw.id = WORK.into();
    raw.sequence = 4;
    raw.request.decision = dto::AudioReviewDecision::Withdrawn;
    f.reviews.push(AudioReview::record(raw, &f.r).unwrap());
    assert_eq!(
        evaluate(&f.facts()),
        vec![
            dto::PublicationBlocker::QcNotApproved,
            dto::PublicationBlocker::CandidateReviewsChanged
        ]
    );
}
#[test]
fn changed_clearance_invalidates_exact_publication_approval() {
    let mut f = fixture();
    let mut raw = f.rights.document().clone();
    raw.sequence += 1;
    raw.request.decision = dto::DistributionDecision::Revoked;
    f.rights = DistributionClearance::record(raw, &f.candidate).unwrap();
    assert_eq!(
        evaluate(&f.facts()),
        vec![
            dto::PublicationBlocker::RightsRevoked,
            dto::PublicationBlocker::ApprovalStale
        ]
    );
}
#[test]
fn approval_withdrawal_and_owner_change_fail_closed() {
    let mut f = fixture();
    let mut raw = f.approval.document().clone();
    raw.request.decision = dto::PublicationApprovalDecision::Withdrawn;
    f.approval = PublicationApproval::record(raw, &f.candidate).unwrap();
    f.actor = OwnerId::parse("other").unwrap();
    assert_eq!(
        evaluate(&f.facts()),
        vec![
            dto::PublicationBlocker::OwnerScopeLost,
            dto::PublicationBlocker::ApprovalWithdrawn
        ]
    );
}
#[test]
fn wrong_work_expired_rights_and_missing_bytes_all_block() {
    let f = fixture();
    let mut facts = f.facts();
    facts.now = 10000;
    facts.object = &ObjectObservation::Missing;
    let mut other = f.work.clone();
    other = other
        .edit(
            1,
            WorkDetails {
                rights_refs: vec!["different-rights".into()],
                ..other.details().clone()
            },
            6000,
        )
        .unwrap();
    facts.work = &other;
    assert_eq!(
        evaluate(&facts),
        vec![
            dto::PublicationBlocker::MetadataChanged,
            dto::PublicationBlocker::UploadPermissionExpired,
            dto::PublicationBlocker::ObjectMissing,
            dto::PublicationBlocker::RightsExpired,
            dto::PublicationBlocker::RightsScopeMismatch,
            dto::PublicationBlocker::DeliveryEvidenceExpired
        ]
    );
}
#[test]
fn object_and_delivery_digest_mismatch_are_distinct() {
    let mut f = fixture();
    f.object = ObjectObservation::Verified {
        sha256: Sha256::parse(&"b".repeat(64)).unwrap(),
        byte_length: 100,
    };
    f.delivery = DeliveryObservation::IntegrityMismatch;
    assert_eq!(
        evaluate(&f.facts()),
        vec![
            dto::PublicationBlocker::ObjectIntegrityMismatch,
            dto::PublicationBlocker::DeliveryIntegrityMismatch
        ]
    );
}
#[test]
fn raw_records_cannot_forge_bound_actor_identity_or_digest() {
    let f = fixture();
    let mut raw = f.rights.document().clone();
    raw.actor_id = "other".into();
    assert_eq!(
        DistributionClearance::restore(raw, &f.candidate),
        Err(PublicationError::BindingMismatch)
    );
    let mut raw = f.approval.document().clone();
    raw.request.notes = "tampered".into();
    assert_eq!(
        PublicationApproval::restore(raw, &f.candidate),
        Err(PublicationError::InvalidDigest)
    );
    assert_eq!(
        Sha256::parse(&"A".repeat(64)),
        Err(PublicationError::InvalidDigest)
    );
}
#[test]
fn delivery_identity_never_admits_private_path_url_or_invalid_window() {
    for value in [
        "/private/objects/key",
        "https://cdn.test/key",
        "publication-../object",
        "publication-token?credential=secret",
    ] {
        assert_eq!(
            DeliveryEvidence::new(
                value.into(),
                Sha256::parse(&"a".repeat(64)).unwrap(),
                100,
                0,
                1
            ),
            Err(PublicationError::InvalidId)
        );
    }
    assert_eq!(
        DeliveryEvidence::new(
            "publication-fixture".into(),
            Sha256::parse(&"a".repeat(64)).unwrap(),
            100,
            1,
            1
        ),
        Err(PublicationError::InvalidTime)
    );
}
proptest! {
    #[test]
    fn rights_expiry_has_no_overflow_or_inclusive_expiry(start in any::<i64>(),end in any::<i64>(),now in any::<i64>()) {
        prop_assert_eq!(rights_window_valid(start,end,now),start>=0 && i128::from(now)>=i128::from(start) && i128::from(now)<i128::from(end));
    }
    #[test]
    fn pointer_never_wraps_or_accepts_a_stale_version(actual in any::<u64>(),expected in any::<u64>()){
        let result=advance_pointer(actual,expected);
        if actual!=expected{prop_assert_eq!(result,Err(PublicationError::PointerConflict));}
        else if actual>=i64::MAX as u64{prop_assert_eq!(result,Err(PublicationError::VersionExhausted));}
        else{prop_assert_eq!(result,Ok(actual+1));}
    }
}

#[test]
fn a_witness_cannot_be_stamped_before_approval_or_after_delivery_expiry() {
    let f = fixture();
    for at in [4600, 9500] {
        let witness = decide(&f.facts()).unwrap();
        assert_eq!(
            AudioRelease::publish(ReleaseId::parse(UNIT).unwrap(), &f.actor, at, None, witness),
            Err(PublicationError::InvalidTime)
        );
    }
    let mut raw = f.approval.document().clone();
    raw.recorded_at = 6000;
    let approval = PublicationApproval::record(raw, &f.candidate).unwrap();
    let mut facts = f.facts();
    facts.approval = Some(&approval);
    assert_eq!(
        evaluate(&facts),
        vec![dto::PublicationBlocker::ApprovalStale]
    );
}
#[test]
fn rehashed_manifest_cannot_forge_rejected_evidence_or_private_delivery() {
    let f = fixture();
    let r = AudioRelease::publish(
        ReleaseId::parse(UNIT).unwrap(),
        &f.actor,
        5000,
        None,
        decide(&f.facts()).unwrap(),
    )
    .unwrap();
    for index in 0..6 {
        let mut raw = r.document().clone();
        match index {
            0 => {
                raw.manifest.distribution_clearance.request.decision =
                    dto::DistributionDecision::Revoked
            }
            1 => {
                raw.manifest.publication_approval.request.decision =
                    dto::PublicationApprovalDecision::Withdrawn
            }
            2 => {
                raw.manifest.review_evidence[0].request.decision =
                    dto::AudioReviewDecision::Rejected
            }
            3 => raw.manifest.delivery_object_id = "/private/staging".into(),
            4 => raw.manifest.media.channels = 2,
            5 => raw.manifest.language = "en-US".into(),
            _ => unreachable!(),
        }
        raw.manifest.digest.clear();
        raw.manifest.digest = digest("rm-r1", &raw.manifest).unwrap();
        raw.digest.clear();
        raw.digest = digest("rl-r1", &raw).unwrap();
        assert!(matches!(
            AudioRelease::restore(raw, &f.candidate, &f.r, &f.content),
            Err(PublicationError::InvalidDigest
                | PublicationError::BindingMismatch
                | PublicationError::InvalidId)
        ));
    }
}

#[test]
fn each_storage_observation_and_archival_has_an_exact_blocker() {
    use dto::PublicationBlocker as B;
    let f = fixture();
    for (object, expected) in [
        (ObjectObservation::Missing, B::ObjectMissing),
        (ObjectObservation::Unverified, B::ObjectUnverified),
        (
            ObjectObservation::IntegrityMismatch,
            B::ObjectIntegrityMismatch,
        ),
    ] {
        let mut facts = f.facts();
        facts.object = &object;
        assert_eq!(evaluate(&facts), vec![expected]);
    }
    for (delivery, expected) in [
        (DeliveryObservation::Unavailable, B::DeliveryUnavailable),
        (DeliveryObservation::Missing, B::DeliveryMissing),
        (
            DeliveryObservation::IntegrityMismatch,
            B::DeliveryIntegrityMismatch,
        ),
    ] {
        let mut facts = f.facts();
        facts.delivery = &delivery;
        assert_eq!(evaluate(&facts), vec![expected]);
    }
    let archived = f.work.archive(1, 6000).unwrap();
    let mut facts = f.facts();
    facts.work = &archived;
    assert_eq!(evaluate(&facts), vec![B::Archived, B::MetadataChanged]);
    let mut facts = f.facts();
    facts.now = 8000;
    assert_eq!(evaluate(&facts), vec![B::UploadPermissionExpired]);
}

#[test]
fn duplicate_reviews_and_future_reviews_preserve_publish_restore_consistency() {
    let mut f = fixture();
    f.reviews.extend(f.reviews.clone());
    let release = AudioRelease::publish(
        ReleaseId::parse(UNIT).unwrap(),
        &f.actor,
        5000,
        None,
        decide(&f.facts()).unwrap(),
    )
    .unwrap();
    assert_eq!(release.document().manifest.review_evidence.len(), 3);
    assert_eq!(
        AudioRelease::restore(release.document().clone(), &f.candidate, &f.r, &f.content).unwrap(),
        release
    );
    let mut raw = f.reviews[0].document().clone();
    raw.reviewed_at = 6000;
    let future = AudioReview::record(raw, &f.r).unwrap();
    f.reviews[0] = future.clone();
    f.reviews[3] = future;
    assert_eq!(
        evaluate(&f.facts()),
        vec![dto::PublicationBlocker::QcNotApproved]
    );
}
