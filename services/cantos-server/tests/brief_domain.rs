use cantos_contracts as dto;
use cantos_server::contexts::{
    catalog::{
        content::wire,
        domain::{content::*, ContentUnitId, OwnerId, WorkId},
    },
    production::domain::brief::*,
};

const WORK: &str = "10000000-0000-4000-8000-000000000001";
const UNIT: &str = "20000000-0000-4000-8000-000000000002";
const CONTENT: &str = "30000000-0000-4000-8000-000000000003";
const BRIEF: &str = "40000000-0000-4000-8000-000000000004";

fn content(recording: bool) -> ContentRevision {
    ContentRevision::new(ContentRevisionFacts {
        id: ContentRevisionId::parse(CONTENT).unwrap(),
        work_id: WorkId::parse(WORK).unwrap(),
        unit_id: ContentUnitId::parse(UNIT).unwrap(),
        revision: 1,
        details: ContentDetails::new(
            "vi-VN".into(),
            if recording {
                ContentBody::Recording
            } else {
                ContentBody::Text {
                    text: ManuscriptText::new("Người kể đã trở về.").unwrap(),
                    source: None,
                }
            },
            vec![],
            "Original synthetic test prose.".into(),
        )
        .unwrap(),
        recorded_by: OwnerId::parse("alice").unwrap(),
        recorded_at: 1000,
    })
    .unwrap()
}
fn guidance(recording: bool) -> dto::BriefGuidance {
    dto::BriefGuidance {
        editorial: dto::EditorialIntent {
            mode: if recording {
                dto::ProductionMode::Recording
            } else {
                dto::ProductionMode::Narration
            },
            notes: "Đọc rõ, giữ hơi thở tự nhiên.".into(),
        },
        voice: None,
        sound_design: None,
        required_capabilities: vec![
            if recording {
                dto::ProducerCapability::Recording
            } else {
                dto::ProducerCapability::Narration
            },
            dto::ProducerCapability::OggOpusMono,
        ],
        delivery: dto::DeliveryConstraints {
            profile_id: "opus-mono-24-v1".into(),
            permitted_use: "Private production review only.".into(),
            attribution: "Original synthetic fixture.".into(),
            naming: "chapter-01.ogg".into(),
            qa_instructions:
                "Compare every paragraph; record measured and listened evidence separately.".into(),
        },
    }
}
fn document(recording: bool) -> dto::ProductionBriefDocument {
    let content = content(recording);
    dto::ProductionBriefDocument {
        schema_version: "1".into(),
        id: BRIEF.into(),
        version: 1,
        work: dto::BriefWork {
            id: WORK.into(),
            owner_id: "alice".into(),
            kind: if recording { "podcast" } else { "novel" }.into(),
            title: "Ánh đèn cuối sân khấu".into(),
            rights_refs: vec![],
            rights_digest: rights_digest(&[]).unwrap(),
        },
        content: wire::response(&content).unwrap(),
        instructions: if recording {
            dto::ProductionInstructions::Recording {}
        } else {
            dto::ProductionInstructions::Text {
                text: "Người kể đã trở về.".into(),
            }
        },
        guidance: guidance(recording),
        created_by: "alice".into(),
        created_at: 2000,
        digest: String::new(),
    }
}
fn brief() -> ProductionBrief {
    ProductionBrief::admit(document(false), &content(false)).unwrap()
}
fn grant(brief: &ProductionBrief) -> dto::ProducerGrantResponse {
    dto::ProducerGrantResponse {
        id: "50000000-0000-4000-8000-000000000005".into(),
        work_id: WORK.into(),
        unit_id: UNIT.into(),
        brief_id: brief.id().as_str().into(),
        brief_digest: brief.digest().as_str().into(),
        permission: "read_instructions".into(),
        expires_at: 5000,
    }
}

#[test]
fn prose_and_recording_require_no_script_cast_or_provider() {
    for recording in [false, true] {
        let b = ProductionBrief::admit(document(recording), &content(recording)).unwrap();
        assert_eq!(b.document().guidance.voice, None);
        assert_eq!(b.document().guidance.sound_design, None);
        assert_eq!(b.document().version, 1);
    }
}

#[test]
fn canonical_document_matches_independent_python_literal_digest() {
    let expected: dto::ProductionBriefDocument = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/briefs/v1/narration-document.json"
    ))
    .unwrap();
    assert_eq!(brief().document(), &expected);
    assert_eq!(
        expected.digest,
        "pb-r1:sha256:c3b45fe14e00f32690e972d5bad2d83bcfea6870bea17c47ed2733049b8f5c9c"
    );
}
#[test]
fn embedded_content_cannot_bypass_the_admitted_revision() {
    for field in [
        "work_id",
        "unit_id",
        "id",
        "revision",
        "content_digest",
        "recorded_by",
        "recorded_at",
        "details",
    ] {
        let mut raw = document(false);
        match field {
            "work_id" => raw.content.work_id = UNIT.into(),
            "unit_id" => raw.content.unit_id = WORK.into(),
            "id" => raw.content.id = BRIEF.into(),
            "revision" => raw.content.revision = 2,
            "content_digest" => raw.content.content_digest = "cc-c1:sha256:wrong".into(),
            "recorded_by" => raw.content.recorded_by = "bob".into(),
            "recorded_at" => raw.content.recorded_at = 1001,
            "details" => raw.content.details.provenance_note = "Forged receipt".into(),
            _ => unreachable!(),
        }
        assert_eq!(
            ProductionBrief::admit(raw, &content(false)).unwrap_err(),
            BriefError::ContentMismatch,
            "{field}"
        );
    }
}
#[test]
fn explicit_profiles_versions_and_optional_guidance_are_admitted_without_defaults() {
    for profile in [
        "opus-mono-20-v1",
        "opus-mono-24-v1",
        "opus-mono-32-v1",
        "opus-mono-40-v1",
    ] {
        assert!(AudioProfile::parse(profile).is_ok());
    }
    for profile in ["", "aac-mono-40-v1", "opus-mono-24", "opus-mono-24-v2"] {
        assert_eq!(
            AudioProfile::parse(profile),
            Err(BriefError::UnsupportedProfile)
        );
    }
    assert_eq!(next_brief_version(1, 0), Err(BriefError::InvalidVersion));
    assert_eq!(
        next_brief_version(i64::MAX as u64, i64::MAX as u64),
        Err(BriefError::InvalidVersion)
    );
    assert_eq!(next_brief_version(1, 1), Ok(2));
    assert_eq!(
        ProductionBriefId::parse("not-a-uuid"),
        Err(BriefError::InvalidId)
    );
    assert_eq!(
        BriefDigest::parse("pb-r1:sha256:ABCD"),
        Err(BriefError::InvalidDigest)
    );
}
#[test]
fn required_controls_fail_explicitly_instead_of_silent_fallback() {
    let g = Guidance::admit(guidance(false)).unwrap();
    assert_eq!(
        g.check_capabilities(&[dto::ProducerCapability::OggOpusMono]),
        Err(BriefError::Incompatible(dto::ProducerCapability::Narration))
    );
    assert_eq!(
        g.check_capabilities(&[
            dto::ProducerCapability::Narration,
            dto::ProducerCapability::OggOpusMono
        ]),
        Ok(())
    );
    assert_eq!(
        g.check_capabilities(&[
            dto::ProducerCapability::Narration,
            dto::ProducerCapability::Narration,
            dto::ProducerCapability::OggOpusMono
        ]),
        Err(BriefError::InvalidGuidance)
    );
    let mut raw = guidance(false);
    raw.voice = Some(dto::VoiceGuidance {
        revision: 1,
        notes: String::new(),
        pronunciation: vec![dto::PronunciationDirection {
            term: "Cantos".into(),
            reading: "can-tôs".into(),
        }],
        casting: vec![],
    });
    assert_eq!(
        Guidance::admit(raw.clone()).unwrap_err(),
        BriefError::Incompatible(dto::ProducerCapability::Pronunciation)
    );
    raw.required_capabilities
        .push(dto::ProducerCapability::Pronunciation);
    assert!(Guidance::admit(raw.clone()).is_ok());
    let duplicate = raw.voice.as_ref().unwrap().pronunciation[0].clone();
    raw.voice.as_mut().unwrap().pronunciation.push(duplicate);
    assert_eq!(
        Guidance::admit(raw).unwrap_err(),
        BriefError::InvalidGuidance
    );
}
#[test]
fn every_instruction_input_changes_the_digest_and_nfc_is_stable() {
    let mut windows = guidance(false);
    windows.editorial.notes = "Người kể\r\nKhẽ thôi.\rĐọc chậm.".into();
    assert_eq!(
        Guidance::admit(windows).unwrap().as_raw().editorial.notes,
        "Người kể\nKhẽ thôi.\nĐọc chậm."
    );
    let mut hostile = guidance(false);
    hostile.editorial.notes = "Không đọc\u{0007}".into();
    assert_eq!(
        Guidance::admit(hostile).unwrap_err(),
        BriefError::InvalidGuidance
    );
    let base = brief();
    let mut nfd = document(false);
    nfd.guidance.editorial.notes = "Đọc rõ, giữ hơi thở tự nhiên.".into();
    assert_eq!(
        ProductionBrief::admit(nfd, &content(false))
            .unwrap()
            .digest(),
        base.digest()
    );
    for field in [
        "mode",
        "notes",
        "profile",
        "permitted_use",
        "attribution",
        "naming",
        "qa",
        "voice",
        "sound",
        "capabilities",
        "rights",
    ] {
        let mut raw = document(false);
        match field {
            "mode" => {
                raw.guidance.editorial.mode = dto::ProductionMode::Recording;
                raw.guidance
                    .required_capabilities
                    .push(dto::ProducerCapability::Recording)
            }
            "notes" => raw.guidance.editorial.notes.push('!'),
            "profile" => raw.guidance.delivery.profile_id = "opus-mono-32-v1".into(),
            "permitted_use" => raw.guidance.delivery.permitted_use.push('!'),
            "attribution" => raw.guidance.delivery.attribution.push('!'),
            "naming" => raw.guidance.delivery.naming.push('!'),
            "qa" => raw.guidance.delivery.qa_instructions.push('!'),
            "voice" => {
                raw.guidance.voice = Some(dto::VoiceGuidance {
                    revision: 2,
                    notes: "Ấm, bình thản".into(),
                    pronunciation: vec![],
                    casting: vec![],
                })
            }
            "sound" => {
                raw.guidance.sound_design = Some(dto::SoundDesignGuidance {
                    revision: 1,
                    notes: "Yên tĩnh".into(),
                });
                raw.guidance
                    .required_capabilities
                    .push(dto::ProducerCapability::SoundDesign)
            }
            "capabilities" => raw
                .guidance
                .required_capabilities
                .push(dto::ProducerCapability::Recording),
            "rights" => {
                raw.work.rights_refs.push("right-1".into());
                raw.work.rights_digest = rights_digest(&raw.work.rights_refs).unwrap()
            }
            _ => unreachable!(),
        }
        assert_ne!(
            ProductionBrief::admit(raw, &content(false))
                .unwrap()
                .digest(),
            base.digest(),
            "{field}"
        );
    }
}
#[test]
fn stale_comparison_never_retargets_or_changes_the_frozen_brief() {
    let b = brief();
    let original = b.document().clone();
    let c = content(false);
    assert_eq!(
        b.freshness(&c, &[], 1, false).unwrap(),
        dto::BriefFreshness::Current
    );
    assert_eq!(
        b.freshness(&content(true), &[], 1, false).unwrap(),
        dto::BriefFreshness::ContentChanged
    );
    assert_eq!(
        b.freshness(&c, &["right-1".into()], 1, false).unwrap(),
        dto::BriefFreshness::RightsChanged
    );
    assert_eq!(
        b.freshness(&c, &[], 2, false).unwrap(),
        dto::BriefFreshness::GuidanceChanged
    );
    assert_eq!(
        b.freshness(&c, &[], 1, true).unwrap(),
        dto::BriefFreshness::Archived
    );
    assert_eq!(b.document(), &original);
}
#[test]
fn producer_permission_is_exact_expiring_and_revocable() {
    let b = brief();
    let scope = ProducerScope::admit(&grant(&b)).unwrap();
    assert_eq!(scope.check(&b, 4999, false), Ok(()));
    assert_eq!(scope.check(&b, 5000, false), Err(BriefError::ExpiredGrant));
    assert_eq!(scope.check(&b, 2000, true), Err(BriefError::RevokedGrant));
    for field in ["work", "unit", "brief", "digest", "permission"] {
        let mut raw = grant(&b);
        match field {
            "work" => raw.work_id = UNIT.into(),
            "unit" => raw.unit_id = WORK.into(),
            "brief" => raw.brief_id = CONTENT.into(),
            "digest" => raw.brief_digest = format!("pb-r1:sha256:{}", "0".repeat(64)),
            "permission" => raw.permission = "upload".into(),
            _ => unreachable!(),
        }
        match ProducerScope::admit(&raw) {
            Ok(s) => assert_eq!(s.check(&b, 2000, false), Err(BriefError::ScopeMismatch)),
            Err(error) => assert_eq!(error, BriefError::ScopeMismatch),
        }
    }
    assert_eq!(
        validate_grant_expiry(1000, 1000),
        Err(BriefError::ExpiredGrant)
    );
    assert_eq!(validate_grant_expiry(1000, 1000 + MAX_GRANT_MS), Ok(()));
    assert_eq!(
        validate_grant_expiry(1000, 1001 + MAX_GRANT_MS),
        Err(BriefError::ExpiredGrant)
    );
}
