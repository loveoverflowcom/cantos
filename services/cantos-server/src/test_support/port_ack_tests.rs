//! Unknown COMMIT acknowledgement at real command boundaries. The clock/ack seam is
//! synthetic; transactions, schema, SQL, documents and replay receipts are production code.
use super::{
    editorial::{fixture, request, ALICE, SCRIPT},
    ServerFixture,
};
use crate::contract::studio::*;
use crate::platform::{
    store::{Store, StoreError},
    turso::Acknowledgement,
};
use serde_json::json;
use uuid::Uuid;
const ORIGINAL:&str="Đêm xuống, đèn Vọng Đài vẫn sáng.\r\n\r\nMai: Chúng mình sẽ chờ ở đây.\r\n\r\n— Ai đã mở cửa?\r\n";
fn document() -> String {
    json!({
        "contract_version":"cantos-adaptation-1","title":"Ngọn đèn Vọng Đài",
        "characters":[{"name":"Mai","personality":"Chưa xác định"}],
        "scenes":[{
            "title":"Bên cánh cửa","lines":[
                {"speaker":"Người dẫn chuyện","text":"Đêm xuống, đèn Vọng Đài vẫn sáng.","source_blocks":[0],"delivery":{"emotion":"calm","intensity_permille":300}},
                {"speaker":"Mai","text":"Chúng mình sẽ chờ ở đây.","source_blocks":[1],"delivery":{"emotion":"neutral","intensity_permille":300}},
                {"speaker":null,"text":"Ai đã mở cửa?","source_blocks":[2],"delivery":{"emotion":"neutral","intensity_permille":450}}
            ],"cues":[
                {"kind":"ambience","description":"Không khí đêm yên tĩnh","line_index":0,"edge":"start","source_blocks":[0]},
                {"kind":"music","description":"Nhạc nền cần tác giả duyệt","line_index":1,"edge":"start","source_blocks":[1]},
                {"kind":"sfx","description":"Tiếng cửa cần xác minh","line_index":2,"edge":"end","source_blocks":[2]}
            ],"pacing_note":"Nhịp chậm; khoảng dừng cần biên tập viên quyết định"
        }],"omitted_blocks":[],"review_notes":["Chưa xác định người đặt câu hỏi; không suy đoán danh tính"]
    }).to_string()
}
fn operation() -> String {
    Uuid::new_v4().to_string()
}
fn context_request(
    source: &ImportResponse,
    script: &str,
    revision: u64,
) -> AdaptationContextRequest {
    let ImportOutcome::Parsed { extraction } = &source.outcome else {
        panic!("parsed fixture required")
    };
    AdaptationContextRequest {
        operation_id: operation(),
        source_id: source.id.clone(),
        source_sha256: source.sha256.clone(),
        extractor_version: extraction.extractor_version.clone(),
        script_id: script.into(),
        expected_revision: revision,
        rights_authorization: true,
    }
}
fn submission(
    context: &AdaptationContextResponse,
    proposal_json: String,
) -> SubmitAdaptationProposalRequest {
    SubmitAdaptationProposalRequest {
        operation_id: operation(),
        context_digest: context.context_digest.clone(),
        proposal_json,
        generation: CallerGenerationMetadata {
            host_tool: "synthetic-integration-caller".into(),
            provider: None,
            model: None,
            configuration_json: None,
            prompt_version: context.prompt_version.clone(),
            usage: None,
            cost: None,
        },
    }
}
fn accept(context: &AdaptationContextResponse) -> AcceptAdaptationRequest {
    AcceptAdaptationRequest {
        operation_id: operation(),
        expected_revision: context.request.expected_revision,
        script_json: context.proposal.as_ref().unwrap().script_json.clone(),
        reviewed_findings: true,
    }
}
fn fixture_settings() -> ProductionSettings {
    ProductionSettings {
        bindings: ["narrator", "an", "minh"]
            .into_iter()
            .map(|id| CharacterCasting {
                character_id: id.into(),
                provider_id: "reference-only".into(),
                model_id: "reference-v1".into(),
                voice_id: "synthetic-character".into(),
                language: "vi-VN".into(),
                voice_rights_record_id: "voice_fixture".into(),
                performance: SynthesisPerformance {
                    rate_permille: 1_000,
                    pitch_semitones: 0,
                    emotion: None,
                    intensity_permille: None,
                },
                pronunciation: vec![],
            })
            .collect(),
        budget: ProductionBudget {
            currency: ProductionCurrency::VND,
            limit_minor: 10_000,
            rate: Some(ProductionRate {
                reference: "Synthetic planning rate, not a provider quote".into(),
                version: "fixture-v1".into(),
                units_per_charge: 100,
                amount_minor: 1,
            }),
            scope: "private integration planning only".into(),
            territory: "private-planning".into(),
        },
    }
}
fn fixture_claim(id: &str, subject: ProductionRightsSubject) -> ProductionRightsDeclaration {
    ProductionRightsDeclaration {
        record_id: id.into(),
        subject,
        scope: ProductionRightsScope::ProductionSynthesis,
        rights_holder: "Repository synthetic fixture author".into(),
        languages: vec!["vi-VN".into()],
        territory: "private-planning".into(),
        attribution: "Original synthetic fixtures".into(),
        restrictions: "private-planning-only".into(),
        permitted_scope: "private integration planning only".into(),
        status: ProductionRightsStatus::Granted,
        reference: "Recorded fixture assertion; not legal verification".into(),
        valid_from_unix: 0,
        valid_until_unix: None,
    }
}

fn uncertain(f: &ServerFixture, ack: Acknowledgement) -> Store {
    let mut s = f.store().clone();
    s.acknowledgement = ack;
    s
}
#[tokio::test]
async fn adaptation_receipts_reconcile_real_unknown_context_submission_acceptance_and_cancel_commits(
) {
    for ack in [
        Acknowledgement::LostAfterCommit,
        Acknowledgement::LostAfterRollback,
    ] {
        let mut f = fixture().await;
        let mut import = super::editorial::import();
        import.original_bytes = ORIGINAL.as_bytes().to_vec();
        import.metadata.rights_holder = Some("Synthetic author".into());
        import.metadata.permission_evidence = Some("Private local test".into());
        import.metadata.usage_scope = Some("Private adaptation".into());
        let source = f.store().import_manuscript(ALICE, import).await.unwrap();
        let context_request = context_request(&source, &operation(), 0);
        let s = uncertain(&f, ack);
        let result = s
            .create_adaptation_context(ALICE, context_request.clone())
            .await;
        assert!(matches!(result, Err(StoreError::Unavailable)), "{result:?}");
        let context = f
            .store()
            .create_adaptation_context(ALICE, context_request.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .create_adaptation_context(ALICE, context_request)
                .await
                .unwrap(),
            context
        );
        assert_eq!(
            f.count("SELECT count(*) FROM adaptation_runs", &[]).await,
            1
        );
        let submitted = submission(&context, document());
        assert!(matches!(
            s.submit_adaptation_proposal(ALICE, &context.id, submitted.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let receipt = f
            .store()
            .submit_adaptation_proposal(ALICE, &context.id, submitted.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .submit_adaptation_proposal(ALICE, &context.id, submitted)
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(
            f.count("SELECT count(*) FROM adaptation_submissions", &[])
                .await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM adaptation_proposals", &[])
                .await,
            1
        );
        let context = f
            .store()
            .load_adaptation_context(ALICE, &context.id)
            .await
            .unwrap();
        let acceptance = accept(&context);
        assert!(matches!(
            s.accept_adaptation(ALICE, &context.id, acceptance.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let saved = f
            .store()
            .accept_adaptation(ALICE, &context.id, acceptance.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .accept_adaptation(ALICE, &context.id, acceptance.clone())
                .await
                .unwrap(),
            saved
        );
        assert_eq!(
            f.count("SELECT count(*) FROM adaptation_acceptances", &[])
                .await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM script_revisions", &[]).await,
            1
        );
        drop(s);
        f.restart().await;
        assert_eq!(
            f.store()
                .accept_adaptation(ALICE, &context.id, acceptance)
                .await
                .unwrap(),
            saved
        );
        let context = f
            .store()
            .create_adaptation_context(ALICE, context_request_for_cancel(&source))
            .await
            .unwrap();
        let cancellation = CancelAdaptationRequest {
            operation_id: operation(),
        };
        let s = uncertain(&f, ack);
        assert!(matches!(
            s.cancel_adaptation_context(ALICE, &context.id, cancellation.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let canceled = f
            .store()
            .cancel_adaptation_context(ALICE, &context.id, cancellation.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .cancel_adaptation_context(ALICE, &context.id, cancellation)
                .await
                .unwrap(),
            canceled
        );
        assert_eq!(canceled.status, AdaptationStatus::Cancelled);
    }
}
fn context_request_for_cancel(source: &ImportResponse) -> AdaptationContextRequest {
    context_request(source, &operation(), 0)
}
#[tokio::test]
async fn production_settings_rights_snapshots_and_approvals_reconcile_real_unknown_commits() {
    for ack in [
        Acknowledgement::LostAfterCommit,
        Acknowledgement::LostAfterRollback,
    ] {
        let mut f = fixture().await;
        f.store().save(ALICE, SCRIPT, request(0)).await.unwrap();
        f.store()
            .review(
                ALICE,
                SCRIPT,
                ReviewRequest {
                    operation_id: operation(),
                    revision: 1,
                },
            )
            .await
            .unwrap();
        let s = uncertain(&f, ack);
        let req = SaveProductionSettingsRequest {
            operation_id: operation(),
            expected_revision: 1,
            expected_settings_version: 0,
            settings: fixture_settings(),
        };
        assert!(matches!(
            s.save_production_settings(ALICE, SCRIPT, req.clone()).await,
            Err(StoreError::Unavailable)
        ));
        let settings = f
            .store()
            .save_production_settings(ALICE, SCRIPT, req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .save_production_settings(ALICE, SCRIPT, req)
                .await
                .unwrap(),
            settings
        );
        let mut claims: Vec<_> =
            crate::script_ir::read_script(super::editorial::FIXTURE.as_bytes())
                .unwrap()
                .evidence_refs()
                .into_iter()
                .filter(|(kind, _)| *kind == "rights")
                .map(|(_, id)| {
                    (
                        id.to_owned(),
                        ProductionRightsSubject::Evidence {
                            record_id: id.to_owned(),
                        },
                    )
                })
                .collect();
        claims.push((
            "voice_fixture".into(),
            ProductionRightsSubject::Voice {
                provider_id: "reference-only".into(),
                model_id: "reference-v1".into(),
                voice_id: "synthetic-character".into(),
            },
        ));
        let claim_count = i64::try_from(claims.len()).unwrap();
        for (id, subject) in claims {
            let req = SaveProductionRightsRequest {
                operation_id: operation(),
                expected_version: 0,
                claim: fixture_claim(&id, subject),
            };
            assert!(matches!(
                s.save_production_rights(ALICE, SCRIPT, req.clone()).await,
                Err(StoreError::Unavailable)
            ));
            let saved = f
                .store()
                .save_production_rights(ALICE, SCRIPT, req.clone())
                .await
                .unwrap();
            assert_eq!(
                f.store()
                    .save_production_rights(ALICE, SCRIPT, req)
                    .await
                    .unwrap(),
                saved
            );
        }
        let preview = f.store().production_preview(ALICE, SCRIPT).await.unwrap();
        let req = FreezeProductionRequest {
            operation_id: operation(),
            expected_revision: 1,
            settings_version: 1,
            input_digest: preview.input_digest,
        };
        assert!(matches!(
            s.freeze_production(ALICE, SCRIPT, req.clone()).await,
            Err(StoreError::Unavailable)
        ));
        let snapshot = f
            .store()
            .freeze_production(ALICE, SCRIPT, req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .freeze_production(ALICE, SCRIPT, req)
                .await
                .unwrap(),
            snapshot
        );
        let req = ApproveProductionRequest {
            operation_id: operation(),
            input_digest: snapshot.document.input_digest.clone(),
        };
        assert!(matches!(
            s.approve_production(ALICE, SCRIPT, &snapshot.id, req.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let approval = f
            .store()
            .approve_production(ALICE, SCRIPT, &snapshot.id, req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .approve_production(ALICE, SCRIPT, &snapshot.id, req.clone())
                .await
                .unwrap(),
            approval
        );
        assert_eq!(
            f.count("SELECT count(*) FROM production_settings", &[])
                .await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM production_rights_claims", &[])
                .await,
            claim_count
        );
        assert_eq!(
            f.count("SELECT count(*) FROM production_snapshots", &[])
                .await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM production_approvals", &[])
                .await,
            1
        );
        drop(s);
        f.restart().await;
        assert_eq!(
            f.store()
                .approve_production(ALICE, SCRIPT, &snapshot.id, req)
                .await
                .unwrap(),
            approval
        );
    }
}
#[tokio::test]
async fn catalog_content_delegation_brief_and_grant_receipts_reconcile_unknown_commits() {
    use crate::contexts::catalog::{
        content::wire as content_wire,
        delegation::wire as delegation_wire,
        domain::{ContentUnitId, UnitDetails, Visibility, WorkDetails, WorkId, WorkKind},
        CatalogCommand, CatalogStoreError,
    };
    for ack in [
        Acknowledgement::LostAfterCommit,
        Acknowledgement::LostAfterRollback,
    ] {
        let mut f = ServerFixture::new().await;
        for (actor, token) in [("alice", ALICE), ("bob", super::editorial::BOB)] {
            f.studio_caller(actor, token).await;
        }
        let now_ms = f.clock_reading().unwrap() / 1000;
        let private = std::fs::canonicalize(f.dir())
            .unwrap()
            .join("private-audio");
        f.store = Some(f.store().clone().with_audio_storage(&private).unwrap());
        f.exec("INSERT INTO script_evidence VALUES('alice','rights','fixture-rights','Private synthetic declaration')",&[]).await.unwrap();
        let s = uncertain(&f, ack);
        let work_id = WorkId::parse(&operation()).unwrap();
        let op = operation();
        let command = CatalogCommand::CreateWork {
            id: work_id.clone(),
            kind: WorkKind::Novel,
            details: WorkDetails {
                title: "Đêm có đèn".into(),
                synopsis: "Tổng hợp riêng tư".into(),
                language: "vi-VN".into(),
                visibility: Visibility::Private,
                rights_refs: vec!["fixture-rights".into()],
            },
        };
        assert!(matches!(
            s.catalog_execute(ALICE, &op, command.clone()).await,
            Err(CatalogStoreError::Storage(StoreError::Unavailable))
        ));
        let work = f
            .store()
            .catalog_execute(ALICE, &op, command.clone())
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(
                f.store()
                    .catalog_execute(ALICE, &op, command)
                    .await
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(work).unwrap()
        );
        let unit_id = ContentUnitId::parse(&operation()).unwrap();
        let op = operation();
        let command = CatalogCommand::CreateUnit {
            work_id: work_id.clone(),
            expected_work_revision: 1,
            id: unit_id.clone(),
            kind: crate::contexts::catalog::domain::ContentUnitKind::Chapter,
            details: UnitDetails {
                title: "Chương một".into(),
                description: "Thử nghiệm".into(),
                source: None,
                script: None,
                content_digest: None,
            },
        };
        assert!(matches!(
            s.catalog_execute(ALICE, &op, command.clone()).await,
            Err(CatalogStoreError::Storage(StoreError::Unavailable))
        ));
        let unit = f
            .store()
            .catalog_execute(ALICE, &op, command.clone())
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(
                f.store()
                    .catalog_execute(ALICE, &op, command)
                    .await
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(unit).unwrap()
        );
        let req = AppendContentRequest {
            operation_id: operation(),
            id: operation(),
            expected_revision: 0,
            expected_work_revision: 2,
            details: ContentDetails {
                language: "vi-VN".into(),
                body: ContentBody::Text {
                    text: "Người bạn đã trở về.".into(),
                    source: None,
                },
                rights_refs: vec!["fixture-rights".into()],
                provenance_note: "Original synthetic text".into(),
            },
        };
        let (op, command) = content_wire::admit_append(unit_id.as_str(), req).unwrap();
        assert!(matches!(
            s.append_content(ALICE, &op, command.clone()).await,
            Err(CatalogStoreError::Storage(StoreError::Unavailable))
        ));
        let content = f
            .store()
            .append_content(ALICE, &op, command.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store().append_content(ALICE, &op, command).await.unwrap(),
            content
        );
        let raw = IssueDelegationRequest {
            operation_id: operation(),
            id: operation(),
            delegate_actor_id: "bob".into(),
            actions: vec![DelegationAction::ContentWrite],
            expires_at: now_ms + 60_000,
        };
        let (op, grant) = delegation_wire::admit_issue(work_id.as_str(), raw).unwrap();
        assert!(matches!(
            s.issue_delegation(ALICE, &op, grant.clone()).await,
            Err(CatalogStoreError::Storage(StoreError::Unavailable))
        ));
        let issued = f
            .store()
            .issue_delegation(ALICE, &op, grant.clone())
            .await
            .unwrap();
        let replay = f.store().issue_delegation(ALICE, &op, grant).await.unwrap();
        assert_eq!(replay.view.grant, issued.view.grant);
        assert!(replay.secret.is_none());
        assert_eq!(
            f.count("SELECT count(*) FROM catalog_delegation_grants", &[])
                .await,
            1
        );
        assert!(matches!(
            s.revoke_delegation(ALICE, &work_id, issued.view.grant.id())
                .await,
            Err(CatalogStoreError::Storage(StoreError::Unavailable))
        ));
        f.store()
            .revoke_delegation(ALICE, &work_id, issued.view.grant.id())
            .await
            .unwrap();
        f.store()
            .revoke_delegation(ALICE, &work_id, issued.view.grant.id())
            .await
            .unwrap();
        assert_eq!(
            f.count("SELECT count(*) FROM catalog_delegation_revocations", &[])
                .await,
            1
        );
        let mut req: CreateProductionBriefRequest = serde_json::from_str(include_str!(
            "../../contracts/studio/briefs/v1/fixtures/create-narration.json"
        ))
        .unwrap();
        req.id = operation();
        req.operation_id = operation();
        req.content_revision_id = content.content.id.clone();
        req.content_revision_digest = content.content.revision_digest.clone();
        assert!(matches!(
            s.create_production_brief(ALICE, unit_id.as_str(), req.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let brief = f
            .store()
            .create_production_brief(ALICE, unit_id.as_str(), req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .create_production_brief(ALICE, unit_id.as_str(), req)
                .await
                .unwrap(),
            brief
        );
        let req = CreateProducerGrantRequest {
            operation_id: operation(),
            id: operation(),
            brief_digest: brief.brief.digest.clone(),
            token: "d".repeat(64),
            expires_at: now_ms + 60_000,
            capabilities: vec![
                ProducerCapability::Narration,
                ProducerCapability::OggOpusMono,
            ],
        };
        assert!(matches!(
            s.create_producer_grant(ALICE, unit_id.as_str(), 1, req.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let grant = f
            .store()
            .create_producer_grant(ALICE, unit_id.as_str(), 1, req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .create_producer_grant(ALICE, unit_id.as_str(), 1, req)
                .await
                .unwrap(),
            grant
        );
        assert!(matches!(
            s.revoke_producer_grant(ALICE, unit_id.as_str(), &grant.id)
                .await,
            Err(StoreError::Unavailable)
        ));
        f.store()
            .revoke_producer_grant(ALICE, unit_id.as_str(), &grant.id)
            .await
            .unwrap();
        f.store()
            .revoke_producer_grant(ALICE, unit_id.as_str(), &grant.id)
            .await
            .unwrap();

        // Real encoded bytes and actual private/public filesystem adapters; no provider or CDN.
        use sha2::{Digest, Sha256};
        let encoded = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-ac",
                "1",
                "-c:a",
                "libopus",
                "-b:a",
                "24k",
                "-vbr",
                "off",
                "-f",
                "ogg",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(encoded.status.success());
        let bytes = encoded.stdout;
        let upload_token = "e".repeat(64);
        let b = serde_json::to_value(&brief.brief).unwrap();
        let req:CreateAudioSubmissionRequest=serde_json::from_value(json!({"operation_id":operation(),"id":operation(),"binding":{"work_id":work_id.as_str(),"unit_id":unit_id.as_str(),"content_revision_id":b["content"]["id"],"content_revision_digest":b["content"]["revision_digest"],"brief_id":b["id"],"brief_version":b["version"],"brief_digest":b["digest"],"profile_id":"opus-mono-24-v1"},"expected_sha256":format!("{:x}",Sha256::digest(&bytes)),"expected_byte_length":bytes.len(),"declarations":{"producer":"Synthetic external producer","method":"recorded test signal","provenance":"Original generated sine","rights_refs":["fixture-rights"],"quality_report":"NOT_RUN listening"},"upload_token":upload_token,"expires_at":now_ms+60_000})).unwrap();
        assert!(matches!(
            s.create_audio_submission(ALICE, unit_id.as_str(), req.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let submitted = f
            .store()
            .create_audio_submission(ALICE, unit_id.as_str(), req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .create_audio_submission(ALICE, unit_id.as_str(), req.clone())
                .await
                .unwrap(),
            submitted
        );
        assert!(matches!(
            s.upload_audio_submission(&upload_token, &req.id, bytes.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let rendition = f
            .store()
            .upload_audio_submission(&upload_token, &req.id, bytes.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .upload_audio_submission(&upload_token, &req.id, bytes)
                .await
                .unwrap(),
            rendition
        );
        for kind in [
            AudioReviewKind::Qc,
            AudioReviewKind::Rights,
            AudioReviewKind::Editorial,
        ] {
            let review = CreateAudioReviewRequest {
                operation_id: operation(),
                rendition_digest: rendition.digest.clone(),
                sha256: rendition.verified_media.sha256.clone(),
                kind,
                decision: AudioReviewDecision::Approved,
                notes: "Named synthetic test review, not listened acceptance".into(),
                evidence_refs: vec!["fixture-rights".into()],
            };
            assert!(matches!(
                s.review_audio_submission(ALICE, unit_id.as_str(), &req.id, review.clone())
                    .await,
                Err(StoreError::Unavailable)
            ));
            let saved = f
                .store()
                .review_audio_submission(ALICE, unit_id.as_str(), &req.id, review.clone())
                .await
                .unwrap();
            assert_eq!(
                f.store()
                    .review_audio_submission(ALICE, unit_id.as_str(), &req.id, review)
                    .await
                    .unwrap(),
                saved
            );
        }
        let candidate_req = CreateReleaseCandidateRequest {
            operation_id: operation(),
            rendition_digest: rendition.digest.clone(),
        };
        assert!(matches!(
            s.create_audio_release_candidate(
                ALICE,
                unit_id.as_str(),
                &req.id,
                candidate_req.clone()
            )
            .await,
            Err(StoreError::Unavailable)
        ));
        let candidate = f
            .store()
            .create_audio_release_candidate(ALICE, unit_id.as_str(), &req.id, candidate_req.clone())
            .await
            .unwrap();
        assert_eq!(
            f.store()
                .create_audio_release_candidate(ALICE, unit_id.as_str(), &req.id, candidate_req)
                .await
                .unwrap(),
            candidate
        );
        let c = &candidate.candidate;
        use crate::contexts::media::{
            FilesystemPublicationStorage, PublicationService, PublicationStoreError,
        };
        let public = std::sync::Arc::new(
            FilesystemPublicationStorage::new(
                &std::fs::canonicalize(f.dir()).unwrap().join("public-audio"),
            )
            .unwrap(),
        );
        let publication =
            PublicationService::new(f.store().clone()).with_origin_publication(public.clone());
        let faulty = PublicationService::new(s.clone()).with_origin_publication(public);
        let clearance:RecordDistributionRightsRequest=serde_json::from_value(json!({"operation_id":operation(),"candidate_digest":c.digest,"decision":"cleared","rights_refs":["fixture-rights"],"valid_until":now_ms+3_600_000,"evidence_refs":["fixture-rights"],"notes":"Synthetic owner decision"})).unwrap();
        assert!(matches!(
            faulty
                .record_audio_distribution_rights(ALICE, unit_id.as_str(), &c.id, clearance.clone())
                .await,
            Err(PublicationStoreError::Storage(StoreError::Unavailable))
        ));
        let rights = publication
            .record_audio_distribution_rights(ALICE, unit_id.as_str(), &c.id, clearance.clone())
            .await
            .unwrap();
        assert_eq!(
            publication
                .record_audio_distribution_rights(ALICE, unit_id.as_str(), &c.id, clearance)
                .await
                .unwrap(),
            rights
        );
        let approval:RecordPublicationApprovalRequest=serde_json::from_value(json!({"operation_id":operation(),"candidate_digest":c.digest,"clearance_digest":rights.digest,"decision":"approved","notes":"Synthetic approval only"})).unwrap();
        assert!(matches!(
            faulty
                .record_audio_publication_approval(ALICE, unit_id.as_str(), &c.id, approval.clone())
                .await,
            Err(PublicationStoreError::Storage(StoreError::Unavailable))
        ));
        let approved = publication
            .record_audio_publication_approval(ALICE, unit_id.as_str(), &c.id, approval.clone())
            .await
            .unwrap();
        assert_eq!(
            publication
                .record_audio_publication_approval(ALICE, unit_id.as_str(), &c.id, approval)
                .await
                .unwrap(),
            approved
        );
        let publish = PublishAudioReleaseRequest {
            operation_id: operation(),
            candidate_digest: c.digest.clone(),
            expected_pointer_version: 0,
            expected_active_release_id: None,
        };
        let outcome = faulty
            .publish_audio_release(ALICE, unit_id.as_str(), &c.id, publish.clone())
            .await;
        assert!(
            matches!(
                outcome,
                Err(PublicationStoreError::Storage(StoreError::Unavailable))
            ),
            "{outcome:?}"
        );
        let release = publication
            .publish_audio_release(ALICE, unit_id.as_str(), &c.id, publish.clone())
            .await
            .unwrap();
        assert_eq!(
            publication
                .publish_audio_release(ALICE, unit_id.as_str(), &c.id, publish)
                .await
                .unwrap(),
            release
        );
        let withdraw = WithdrawAudioReleaseRequest {
            operation_id: operation(),
            release_id: release.release.unwrap().id,
            expected_pointer_version: 1,
            reason: "Synthetic withdrawal".into(),
        };
        assert!(matches!(
            faulty
                .withdraw_audio_release(ALICE, unit_id.as_str(), withdraw.clone())
                .await,
            Err(PublicationStoreError::Storage(StoreError::Unavailable))
        ));
        let withdrawn = publication
            .withdraw_audio_release(ALICE, unit_id.as_str(), withdraw.clone())
            .await
            .unwrap();
        assert_eq!(
            publication
                .withdraw_audio_release(ALICE, unit_id.as_str(), withdraw)
                .await
                .unwrap(),
            withdrawn
        );
        assert!(matches!(
            s.revoke_audio_submission(ALICE, unit_id.as_str(), &req.id)
                .await,
            Err(StoreError::Unavailable)
        ));
        f.store()
            .revoke_audio_submission(ALICE, unit_id.as_str(), &req.id)
            .await
            .unwrap();
        f.store()
            .revoke_audio_submission(ALICE, unit_id.as_str(), &req.id)
            .await
            .unwrap();
        for table in [
            "audio_submissions",
            "audio_renditions",
            "audio_release_candidates",
            "audio_distribution_clearances",
            "audio_publication_approvals",
            "audio_releases",
            "audio_release_withdrawals",
            "audio_submission_revocations",
        ] {
            assert_eq!(
                f.count(&format!("SELECT count(*) FROM {table}"), &[]).await,
                1,
                "{table}"
            );
        }
        assert_eq!(f.count("SELECT count(*) FROM audio_reviews", &[]).await, 3);
        assert_eq!(
            f.count("SELECT count(*) FROM catalog_content_revisions", &[])
                .await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM production_briefs", &[]).await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM production_producer_grants", &[])
                .await,
            1
        );
    }
}
