use super::{
    admit_preview, plan_reorder, CatalogError, ContentUnit, ContentUnitId, ContentUnitKind, IdKind,
    Lifecycle, LinkPatch, NewContentUnit, NewWork, OwnerId, ScriptRef, SourceRef,
    StoredContentUnit, StoredWork, UnitDetails, UnitPatch, Visibility, Work, WorkDetails, WorkId,
    WorkKind, WorkPatch, DESCRIPTION_MAX_BYTES, MAX_RIGHTS_REFS, TITLE_MAX_BYTES,
};

const WORK_ID: &str = "10000000-0000-4000-8000-000000000001";
const OTHER_WORK_ID: &str = "10000000-0000-4000-8000-000000000002";
const UNIT_ID: &str = "20000000-0000-4000-8000-000000000001";
const SECOND_UNIT_ID: &str = "20000000-0000-4000-8000-000000000002";
const THIRD_UNIT_ID: &str = "20000000-0000-4000-8000-000000000003";
const SCRIPT_ID: &str = "30000000-0000-4000-8000-000000000001";
const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn work_details() -> WorkDetails {
    WorkDetails {
        title: "Mùa trăng qua hiên".into(),
        synopsis: "Một câu chuyện Việt Nam.\nLời kể vẫn còn.".into(),
        language: "vi-VN".into(),
        visibility: Visibility::Private,
        rights_refs: vec!["rights-source-vi".into()],
    }
}

fn unit_details() -> UnitDetails {
    UnitDetails {
        title: "Chương một — Những lời chưa nói".into(),
        description: "".into(),
        source: None,
        script: None,
        content_digest: None,
    }
}

fn new_work(kind: WorkKind, details: WorkDetails, at: i64) -> Result<Work, CatalogError> {
    Work::new(NewWork {
        id: WorkId::parse(WORK_ID).unwrap(),
        kind,
        owner_id: OwnerId::parse("actor-owner").unwrap(),
        details,
        at,
    })
}

fn work(kind: WorkKind) -> Work {
    new_work(kind, work_details(), 1_000).unwrap()
}

fn unit(parent: &Work, id: &str, position: u32) -> ContentUnit {
    ContentUnit::new(
        parent,
        NewContentUnit {
            id: ContentUnitId::parse(id).unwrap(),
            kind: parent.kind().content_unit_kind(),
            details: unit_details(),
            position,
            at: 1_000,
        },
    )
    .unwrap()
}

fn stored_work(parent: &Work) -> StoredWork {
    StoredWork {
        id: parent.id().clone(),
        kind: parent.kind(),
        owner_id: parent.owner_id().clone(),
        details: parent.details().clone(),
        lifecycle: parent.lifecycle(),
        revision: parent.revision(),
        created_at: parent.created_at(),
        updated_at: parent.updated_at(),
    }
}

fn stored_unit(unit: &ContentUnit) -> StoredContentUnit {
    StoredContentUnit {
        id: unit.id().clone(),
        work_id: unit.work_id().clone(),
        kind: unit.kind(),
        details: unit.details().clone(),
        position: unit.position(),
        lifecycle: unit.lifecycle(),
        revision: unit.revision(),
        created_at: unit.created_at(),
        updated_at: unit.updated_at(),
    }
}

#[test]
fn typed_entity_ids_accept_only_existing_canonical_uuid_spelling() {
    assert_eq!(WorkId::parse(WORK_ID).unwrap().as_str(), WORK_ID);
    assert_eq!(ContentUnitId::parse(UNIT_ID).unwrap().as_str(), UNIT_ID);
    for bad in [
        "",
        "work-1",
        "10000000000040008000000000000001",
        "{10000000-0000-4000-8000-000000000001}",
        "aaaaaaaa-AAAA-4000-8000-000000000001",
    ] {
        assert_eq!(
            WorkId::parse(bad),
            Err(CatalogError::InvalidId(IdKind::Work))
        );
        assert_eq!(
            ContentUnitId::parse(bad),
            Err(CatalogError::InvalidId(IdKind::ContentUnit))
        );
    }
}

#[test]
fn owner_ids_preserve_existing_actor_tokens_and_refuse_bad_boundaries() {
    for valid in ["actor-owner", "a", "a_b-0", WORK_ID] {
        assert_eq!(OwnerId::parse(valid).unwrap().as_str(), valid);
    }
    assert_eq!(OwnerId::parse(&"a".repeat(64)).unwrap().as_str().len(), 64);
    for bad in [
        "",
        "Actor-owner",
        "actor owner",
        "../actor",
        "nguyễn",
        &"a".repeat(65),
    ] {
        assert_eq!(
            OwnerId::parse(bad),
            Err(CatalogError::InvalidId(IdKind::Owner))
        );
    }
}

#[test]
fn catalog_categories_and_lifecycles_are_closed_and_serialize_without_defaults() {
    for (name, expected) in [
        ("novel", WorkKind::Novel),
        ("story", WorkKind::Story),
        ("podcast", WorkKind::Podcast),
    ] {
        assert_eq!(WorkKind::parse(name), Ok(expected));
        assert_eq!(expected.as_str(), name);
        assert_eq!(
            serde_json::to_string(&expected).unwrap(),
            format!("\"{name}\"")
        );
    }
    for name in ["drama", "Novel", "", " novel"] {
        assert_eq!(WorkKind::parse(name), Err(CatalogError::UnknownWorkKind));
    }
    assert_eq!(
        ContentUnitKind::parse("chapter"),
        Ok(ContentUnitKind::Chapter)
    );
    assert_eq!(
        ContentUnitKind::parse("episode"),
        Ok(ContentUnitKind::Episode)
    );
    assert_eq!(
        ContentUnitKind::parse("scene"),
        Err(CatalogError::UnknownContentUnitKind)
    );
    assert_eq!(Visibility::parse("private"), Ok(Visibility::Private));
    assert_eq!(Visibility::parse("unlisted"), Ok(Visibility::Unlisted));
    assert_eq!(Visibility::parse("public"), Ok(Visibility::Public));
    assert_eq!(
        Visibility::parse("released"),
        Err(CatalogError::UnknownVisibility)
    );
    assert_eq!(Lifecycle::parse("draft"), Ok(Lifecycle::Draft));
    assert_eq!(Lifecycle::parse("archived"), Ok(Lifecycle::Archived));
    assert_eq!(
        Lifecycle::parse("published"),
        Err(CatalogError::UnknownLifecycle)
    );
}

#[test]
fn all_work_kind_and_unit_kind_pairs_obey_the_current_catalog_matrix() {
    for kind in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
        let parent = work(kind);
        for unit_kind in [ContentUnitKind::Chapter, ContentUnitKind::Episode] {
            let actual = ContentUnit::new(
                &parent,
                NewContentUnit {
                    id: ContentUnitId::parse(UNIT_ID).unwrap(),
                    kind: unit_kind,
                    details: unit_details(),
                    position: 0,
                    at: 1_000,
                },
            );
            if kind.content_unit_kind() == unit_kind {
                assert_eq!(actual.unwrap().kind(), unit_kind);
            } else {
                assert_eq!(
                    actual,
                    Err(CatalogError::WrongContentUnitKind {
                        work: kind,
                        unit: unit_kind
                    })
                );
            }
        }
    }
}

#[test]
fn work_titles_are_required_and_bounded_in_bytes_including_vietnamese() {
    for title in ["", " \t\n"] {
        let mut details = work_details();
        details.title = title.into();
        assert_eq!(
            new_work(WorkKind::Story, details, 1_000),
            Err(CatalogError::BlankField { field: "title" })
        );
    }
    let mut details = work_details();
    details.title = "Đ".repeat(TITLE_MAX_BYTES / 2);
    assert_eq!(
        new_work(WorkKind::Story, details.clone(), 1_000)
            .unwrap()
            .details()
            .title
            .len(),
        TITLE_MAX_BYTES
    );
    details.title.push('a');
    assert_eq!(
        new_work(WorkKind::Story, details, 1_000),
        Err(CatalogError::FieldTooLong {
            field: "title",
            max_bytes: TITLE_MAX_BYTES
        })
    );
}

#[test]
fn prose_allows_newlines_but_refuses_nul_and_titles_refuse_line_breaks() {
    let mut details = work_details();
    details.title = "Mùa trăng\nqua hiên".into();
    assert_eq!(
        new_work(WorkKind::Story, details, 1_000),
        Err(CatalogError::ControlCharacter { field: "title" })
    );
    let mut details = work_details();
    details.synopsis = "Một lời\0khác".into();
    assert_eq!(
        new_work(WorkKind::Story, details, 1_000),
        Err(CatalogError::ControlCharacter { field: "synopsis" })
    );
    let mut details = work_details();
    details.synopsis = "\tLời thứ nhất\r\nLời thứ hai".into();
    assert_eq!(
        new_work(WorkKind::Story, details.clone(), 1_000)
            .unwrap()
            .details(),
        &details
    );
    details.synopsis = "a".repeat(DESCRIPTION_MAX_BYTES);
    assert!(new_work(WorkKind::Story, details.clone(), 1_000).is_ok());
    details.synopsis.push('a');
    assert_eq!(
        new_work(WorkKind::Story, details, 1_000),
        Err(CatalogError::FieldTooLong {
            field: "synopsis",
            max_bytes: DESCRIPTION_MAX_BYTES
        })
    );
}

#[test]
fn language_tags_are_bounded_structural_metadata_preserved_without_normalizing() {
    for language in ["vi", "vi-VN", "en-Latn-US", "zh-Hant", "en-US-u-ca-gregory"] {
        let mut details = work_details();
        details.language = language.into();
        assert_eq!(
            new_work(WorkKind::Novel, details, 1_000)
                .unwrap()
                .details()
                .language,
            language
        );
    }
    for language in [
        "",
        "v",
        "vi_VN",
        "vi-",
        "vi--VN",
        "vi-Việt",
        "vi/../../",
        &"v".repeat(64),
    ] {
        let mut details = work_details();
        details.language = language.into();
        assert_eq!(
            new_work(WorkKind::Novel, details, 1_000),
            Err(CatalogError::InvalidLanguage)
        );
    }
}

#[test]
fn rights_references_are_existing_token_shapes_without_duplicates_or_unbounded_lists() {
    let mut details = work_details();
    details.rights_refs = vec!["rights-1".into(), "rights-1".into()];
    assert_eq!(
        new_work(WorkKind::Novel, details, 1_000),
        Err(CatalogError::DuplicateRightsRef)
    );
    let mut details = work_details();
    details.rights_refs = vec!["rights/../1".into()];
    assert_eq!(
        new_work(WorkKind::Novel, details, 1_000),
        Err(CatalogError::InvalidRightsRef)
    );
    let mut details = work_details();
    details.rights_refs = (0..MAX_RIGHTS_REFS)
        .map(|n| format!("rights-{n}"))
        .collect();
    assert_eq!(
        new_work(WorkKind::Novel, details.clone(), 1_000)
            .unwrap()
            .details()
            .rights_refs
            .len(),
        MAX_RIGHTS_REFS
    );
    details.rights_refs.push("rights-more".into());
    assert_eq!(
        new_work(WorkKind::Novel, details, 1_000),
        Err(CatalogError::TooManyRightsRefs)
    );
}

#[test]
fn work_edits_return_new_snapshots_with_stable_identity_and_exact_old_metadata() {
    let original = work(WorkKind::Story);
    let original_bytes = serde_json::to_vec(&original).unwrap();
    let mut details = original.details().clone();
    details.title = "Những ngày còn nhớ".into();
    let edited = original.edit(1, details.clone(), 2_000).unwrap();
    assert_eq!(edited.id(), original.id());
    assert_eq!(edited.owner_id(), original.owner_id());
    assert_eq!(edited.kind(), original.kind());
    assert_eq!(edited.created_at(), 1_000);
    assert_eq!(edited.updated_at(), 2_000);
    assert_eq!(edited.revision(), 2);
    assert_eq!(edited.details(), &details);
    assert_eq!(serde_json::to_vec(&original).unwrap(), original_bytes);
}

#[test]
fn work_patch_retains_omitted_fields_and_validates_the_resulting_snapshot() {
    let original = work(WorkKind::Story);
    let before = original.clone();
    let patch = WorkPatch {
        title: Some("Những ngày còn nhớ".into()),
        rights_refs: Some(Vec::new()),
        ..WorkPatch::default()
    };
    let edited = original
        .edit(1, patch.apply(original.details()), 2_000)
        .unwrap();
    assert_eq!(edited.details().title, "Những ngày còn nhớ");
    assert_eq!(edited.details().synopsis, original.details().synopsis);
    assert_eq!(edited.details().language, original.details().language);
    assert_eq!(edited.details().visibility, original.details().visibility);
    assert!(edited.details().rights_refs.is_empty());
    assert_eq!(original, before);
    let invalid = WorkPatch {
        title: Some(" ".into()),
        ..WorkPatch::default()
    };
    assert_eq!(
        original.edit(1, invalid.apply(original.details()), 2_000),
        Err(CatalogError::BlankField { field: "title" })
    );
}

#[test]
fn unit_patch_keeps_pinned_links_until_an_explicit_set_or_clear() {
    let source = SourceRef::new("source-vi".into(), SHA256.into()).unwrap();
    let script = ScriptRef::new(SCRIPT_ID.into(), 9, format!("sir-e1:sha256:{SHA256}")).unwrap();
    let original = UnitDetails {
        source: Some(source.clone()),
        script: Some(script.clone()),
        content_digest: Some(format!("sir-c1:sha256:{SHA256}")),
        ..unit_details()
    };
    let before = original.clone();
    let kept = UnitPatch {
        title: Some("Chương hai".into()),
        ..UnitPatch::default()
    }
    .apply(&original);
    assert_eq!(kept.title, "Chương hai");
    assert_eq!(kept.description, original.description);
    assert_eq!(kept.source, Some(source.clone()));
    assert_eq!(kept.script, Some(script.clone()));
    assert_eq!(kept.content_digest, original.content_digest);
    let cleared = UnitPatch {
        source: LinkPatch::Clear,
        script: LinkPatch::Clear,
        content_digest: LinkPatch::Clear,
        ..UnitPatch::default()
    }
    .apply(&kept);
    assert_eq!(cleared.source, None);
    assert_eq!(cleared.script, None);
    assert_eq!(cleared.content_digest, None);
    let set = UnitPatch {
        source: LinkPatch::Set(source),
        script: LinkPatch::Set(script),
        content_digest: LinkPatch::Set(format!("sir-c1:sha256:{SHA256}")),
        ..UnitPatch::default()
    }
    .apply(&cleared);
    assert_eq!(set.source, original.source);
    assert_eq!(set.script, original.script);
    assert_eq!(set.content_digest, original.content_digest);
    assert_eq!(original, before);
}

#[test]
fn optional_link_patch_serialization_distinguishes_keep_set_and_clear() {
    let states = [
        LinkPatch::Keep,
        LinkPatch::Set(SHA256.to_owned()),
        LinkPatch::Clear,
    ];
    let bytes: Vec<_> = states
        .iter()
        .map(|state| serde_json::to_vec(state).unwrap())
        .collect();
    assert_eq!(
        bytes,
        vec![
            br#"{"action":"keep"}"#.to_vec(),
            format!(r#"{{"action":"set","value":"{SHA256}"}}"#).into_bytes(),
            br#"{"action":"clear"}"#.to_vec(),
        ]
    );
}

#[test]
fn archived_work_permission_revision_retains_its_lifecycle_and_metadata() {
    let archived = work(WorkKind::Story).archive(1, 2_000).unwrap();
    let original = archived.clone();
    let changed = archived.with_permission_change(2, 3_000).unwrap();
    assert_eq!(changed.revision(), 3);
    assert_eq!(changed.lifecycle(), Lifecycle::Archived);
    assert_eq!(changed.details(), archived.details());
    assert_eq!(changed.id(), archived.id());
    assert_eq!(changed.owner_id(), archived.owner_id());
    assert_eq!(changed.created_at(), archived.created_at());
    assert_eq!(archived, original);
    assert_eq!(
        archived.with_permission_change(1, 3_000),
        Err(CatalogError::RevisionConflict {
            current_revision: 2,
            expected_revision: 1,
        })
    );
    assert_eq!(
        archived.edit(2, archived.details().clone(), 3_000),
        Err(CatalogError::WorkArchived)
    );
}

#[test]
fn revision_conflict_and_timestamp_regression_leave_the_work_unchanged() {
    let original = work(WorkKind::Novel);
    let old = original.clone();
    assert_eq!(
        original.edit(0, work_details(), 2_000),
        Err(CatalogError::RevisionConflict {
            current_revision: 1,
            expected_revision: 0
        })
    );
    assert_eq!(
        original.edit(1, work_details(), 999),
        Err(CatalogError::TimestampRegression)
    );
    assert_eq!(
        original.edit(1, work_details(), -1),
        Err(CatalogError::InvalidTimestamp)
    );
    assert_eq!(original, old);
}

#[test]
fn archive_and_restore_preserve_every_identity_and_require_explicit_transitions() {
    let original = work(WorkKind::Podcast);
    let archived = original.archive(1, 2_000).unwrap();
    assert_eq!(archived.lifecycle(), Lifecycle::Archived);
    assert_eq!(archived.revision(), 2);
    assert_eq!(archived.id(), original.id());
    assert_eq!(archived.owner_id(), original.owner_id());
    assert_eq!(archived.details(), original.details());
    assert_eq!(
        archived.edit(2, work_details(), 3_000),
        Err(CatalogError::WorkArchived)
    );
    assert_eq!(archived.touch(2, 3_000), Err(CatalogError::WorkArchived));
    assert_eq!(
        archived.archive(2, 3_000),
        Err(CatalogError::UnexpectedLifecycle {
            expected: Lifecycle::Draft,
            actual: Lifecycle::Archived
        })
    );
    assert_eq!(
        original.restore(1, 2_000),
        Err(CatalogError::UnexpectedLifecycle {
            expected: Lifecycle::Archived,
            actual: Lifecycle::Draft
        })
    );
    let restored = archived.restore(2, 3_000).unwrap();
    assert_eq!(restored.lifecycle(), Lifecycle::Draft);
    assert_eq!(restored.revision(), 3);
    assert_eq!(restored.details(), original.details());
    assert_eq!(original.lifecycle(), Lifecycle::Draft);
}

#[test]
fn restored_storage_cannot_bypass_revision_metadata_or_timestamp_validation() {
    let parent = work(WorkKind::Novel);
    assert_eq!(Work::from_stored(stored_work(&parent)), Ok(parent.clone()));
    for invalid in [0, i64::MAX as u64 + 1] {
        let mut raw = stored_work(&parent);
        raw.revision = invalid;
        assert_eq!(Work::from_stored(raw), Err(CatalogError::InvalidRevision));
    }
    let mut raw = stored_work(&parent);
    raw.details.title = " ".into();
    assert_eq!(
        Work::from_stored(raw),
        Err(CatalogError::BlankField { field: "title" })
    );
    let mut raw = stored_work(&parent);
    raw.created_at = -1;
    assert_eq!(Work::from_stored(raw), Err(CatalogError::InvalidTimestamp));
    let mut raw = stored_work(&parent);
    raw.updated_at = 999;
    assert_eq!(
        Work::from_stored(raw),
        Err(CatalogError::TimestampRegression)
    );
}

#[test]
fn revisions_never_wrap_and_child_touch_produces_only_a_new_parent_guard() {
    let original = work(WorkKind::Story);
    let next = original.touch(1, 1_000).unwrap();
    assert_eq!(next.revision(), 2);
    assert_eq!(next.details(), original.details());
    let mut raw = stored_work(&original);
    raw.revision = i64::MAX as u64;
    let last = Work::from_stored(raw).unwrap();
    assert_eq!(
        last.touch(i64::MAX as u64, 2_000),
        Err(CatalogError::RevisionLimit)
    );
    assert_eq!(
        last.archive(i64::MAX as u64, 2_000),
        Err(CatalogError::RevisionLimit)
    );
}

#[test]
fn all_catalog_kinds_allow_units_without_a_script_source_digest_or_cast() {
    for kind in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
        let parent = work(kind);
        let unit = unit(&parent, UNIT_ID, 0);
        assert_eq!(unit.details().source, None);
        assert_eq!(unit.details().script, None);
        assert_eq!(unit.details().content_digest, None);
        assert_eq!(unit.work_id(), parent.id());
        assert_eq!(unit.revision(), 1);
    }
}

#[test]
fn optional_source_and_script_links_preserve_exact_digests_and_pinned_revision() {
    let source = SourceRef::new("source-vi".into(), SHA256.into()).unwrap();
    let script = ScriptRef::new(SCRIPT_ID.into(), 9, format!("sir-e1:sha256:{SHA256}")).unwrap();
    let parent = work(WorkKind::Story);
    let mut details = unit_details();
    details.source = Some(source.clone());
    details.script = Some(script.clone());
    details.content_digest = Some(format!("sir-c1:sha256:{SHA256}"));
    let original = ContentUnit::new(
        &parent,
        NewContentUnit {
            id: ContentUnitId::parse(UNIT_ID).unwrap(),
            kind: ContentUnitKind::Chapter,
            details,
            position: 0,
            at: 1_000,
        },
    )
    .unwrap();
    let original_bytes = serde_json::to_vec(&original).unwrap();
    let mut next_details = original.details().clone();
    next_details.title = "Chương hai".into();
    let edited = original.edit(&parent, 1, next_details, 2_000).unwrap();
    assert_eq!(edited.details().source, Some(source));
    assert_eq!(edited.details().script, Some(script.clone()));
    assert_eq!(script.id(), SCRIPT_ID);
    assert_eq!(script.revision(), 9);
    assert_eq!(script.export_digest(), format!("sir-e1:sha256:{SHA256}"));
    assert_eq!(serde_json::to_vec(&original).unwrap(), original_bytes);
}

#[test]
fn link_admission_refuses_unpinned_revisions_noncanonical_ids_and_wrong_digest_families() {
    assert_eq!(
        SourceRef::new("../source".into(), SHA256.into()),
        Err(CatalogError::InvalidSourceId)
    );
    assert_eq!(
        SourceRef::new("source-vi".into(), SHA256.to_uppercase()),
        Err(CatalogError::InvalidSourceDigest)
    );
    assert_eq!(
        SourceRef::new("source-vi".into(), format!("sha256:{SHA256}")),
        Err(CatalogError::InvalidSourceDigest)
    );
    assert_eq!(
        ScriptRef::new("script-1".into(), 1, format!("sir-e1:sha256:{SHA256}")),
        Err(CatalogError::InvalidScriptId)
    );
    for id in [
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".to_owned(),
        format!("{{{SCRIPT_ID}}}"),
        SCRIPT_ID.replace('-', ""),
    ] {
        assert_eq!(
            ScriptRef::new(id, 1, format!("sir-e1:sha256:{SHA256}")),
            Err(CatalogError::InvalidScriptId)
        );
    }
    for revision in [0, i64::MAX as u64 + 1] {
        assert_eq!(
            ScriptRef::new(
                SCRIPT_ID.into(),
                revision,
                format!("sir-e1:sha256:{SHA256}")
            ),
            Err(CatalogError::InvalidRevision)
        );
    }
    for digest in [
        SHA256.to_owned(),
        format!("sir-c1:sha256:{SHA256}"),
        format!("sir-e1:sha256:{}", SHA256.to_uppercase()),
    ] {
        assert_eq!(
            ScriptRef::new(SCRIPT_ID.into(), 1, digest),
            Err(CatalogError::InvalidScriptDigest)
        );
    }
}

#[test]
fn content_checksums_accept_only_sha256_or_existing_script_content_digest_spelling() {
    let parent = work(WorkKind::Podcast);
    let original = unit(&parent, UNIT_ID, 0);
    for digest in [SHA256.to_owned(), format!("sir-c1:sha256:{SHA256}")] {
        let mut details = unit_details();
        details.content_digest = Some(digest.clone());
        assert_eq!(
            original
                .edit(&parent, 1, details, 2_000)
                .unwrap()
                .details()
                .content_digest
                .as_deref(),
            Some(digest.as_str())
        );
    }
    for digest in [
        "".into(),
        SHA256.to_uppercase(),
        format!("sir-e1:sha256:{SHA256}"),
    ] {
        let mut details = unit_details();
        details.content_digest = Some(digest);
        assert_eq!(
            original.edit(&parent, 1, details, 2_000),
            Err(CatalogError::InvalidContentDigest)
        );
    }
}

#[test]
fn unit_position_is_zero_based_bounded_and_never_part_of_identity() {
    let parent = work(WorkKind::Novel);
    let original = unit(&parent, UNIT_ID, 0);
    let moved = original
        .with_position(&parent, 1, i32::MAX as u32, 2_000)
        .unwrap();
    assert_eq!(moved.id(), original.id());
    assert_eq!(moved.work_id(), original.work_id());
    assert_eq!(moved.position(), i32::MAX as u32);
    assert_eq!(moved.revision(), 2);
    assert_eq!(original.position(), 0);
    assert_eq!(
        original.with_position(&parent, 1, i32::MAX as u32 + 1, 2_000),
        Err(CatalogError::InvalidOrder)
    );
}

#[test]
fn archived_units_preserve_metadata_and_only_explicit_restore_makes_them_editable() {
    let parent = work(WorkKind::Novel);
    let original = unit(&parent, UNIT_ID, 3);
    let archived = original.archive(&parent, 1, 2_000).unwrap();
    assert_eq!(archived.lifecycle(), Lifecycle::Archived);
    assert_eq!(archived.position(), 3);
    assert_eq!(archived.details(), original.details());
    assert_eq!(
        archived.edit(&parent, 2, unit_details(), 3_000),
        Err(CatalogError::UnitArchived)
    );
    assert_eq!(
        archived.with_position(&parent, 2, 0, 3_000),
        Err(CatalogError::UnitArchived)
    );
    assert_eq!(
        archived.archive(&parent, 2, 3_000),
        Err(CatalogError::UnexpectedLifecycle {
            expected: Lifecycle::Draft,
            actual: Lifecycle::Archived
        })
    );
    let restored = archived.restore(&parent, 2, 3_000).unwrap();
    assert_eq!(restored.id(), original.id());
    assert_eq!(restored.lifecycle(), Lifecycle::Draft);
    assert_eq!(restored.revision(), 3);
    assert_eq!(restored.details(), original.details());
    assert_eq!(restored.position(), 3);
}

#[test]
fn archived_parents_block_unit_creation_edit_archive_restore_and_reorder() {
    let parent = work(WorkKind::Story);
    let original = unit(&parent, UNIT_ID, 0);
    let archived = original.archive(&parent, 1, 2_000).unwrap();
    let parent = parent.archive(1, 3_000).unwrap();
    assert_eq!(
        ContentUnit::new(
            &parent,
            NewContentUnit {
                id: ContentUnitId::parse(SECOND_UNIT_ID).unwrap(),
                kind: ContentUnitKind::Chapter,
                details: unit_details(),
                position: 1,
                at: 4_000
            }
        ),
        Err(CatalogError::WorkArchived)
    );
    assert_eq!(
        original.edit(&parent, 1, unit_details(), 4_000),
        Err(CatalogError::WorkArchived)
    );
    assert_eq!(
        original.archive(&parent, 1, 4_000),
        Err(CatalogError::WorkArchived)
    );
    assert_eq!(
        archived.restore(&parent, 2, 4_000),
        Err(CatalogError::WorkArchived)
    );
    assert_eq!(
        plan_reorder(
            &parent,
            std::slice::from_ref(&original),
            &[original.id().clone()]
        ),
        Err(CatalogError::WorkArchived)
    );
}

#[test]
fn a_unit_cannot_be_edited_archived_or_reordered_through_another_parent() {
    let parent = work(WorkKind::Story);
    let original = unit(&parent, UNIT_ID, 0);
    let mut raw = stored_work(&parent);
    raw.id = WorkId::parse(OTHER_WORK_ID).unwrap();
    let other = Work::from_stored(raw).unwrap();
    let wrong_work = CatalogError::WrongWork {
        unit: original.id().clone(),
    };
    assert_eq!(
        original.edit(&other, 1, unit_details(), 2_000),
        Err(wrong_work.clone())
    );
    assert_eq!(original.archive(&other, 1, 2_000), Err(wrong_work.clone()));
    assert_eq!(
        plan_reorder(
            &other,
            std::slice::from_ref(&original),
            &[original.id().clone()]
        ),
        Err(wrong_work)
    );
}

#[test]
fn stored_unit_admission_checks_parent_kind_revision_position_text_and_time() {
    let parent = work(WorkKind::Podcast);
    let original = unit(&parent, UNIT_ID, 0);
    assert_eq!(
        ContentUnit::from_stored(stored_unit(&original), parent.kind()),
        Ok(original.clone())
    );
    assert_eq!(
        ContentUnit::from_stored(stored_unit(&original), WorkKind::Novel),
        Err(CatalogError::WrongContentUnitKind {
            work: WorkKind::Novel,
            unit: ContentUnitKind::Episode
        })
    );
    let mut raw = stored_unit(&original);
    raw.revision = 0;
    assert_eq!(
        ContentUnit::from_stored(raw, parent.kind()),
        Err(CatalogError::InvalidRevision)
    );
    let mut raw = stored_unit(&original);
    raw.position = u32::MAX;
    assert_eq!(
        ContentUnit::from_stored(raw, parent.kind()),
        Err(CatalogError::InvalidOrder)
    );
    let mut raw = stored_unit(&original);
    raw.details.title = "".into();
    assert_eq!(
        ContentUnit::from_stored(raw, parent.kind()),
        Err(CatalogError::BlankField { field: "title" })
    );
    let mut raw = stored_unit(&original);
    raw.updated_at = 999;
    assert_eq!(
        ContentUnit::from_stored(raw, parent.kind()),
        Err(CatalogError::TimestampRegression)
    );
}

#[test]
fn unit_rejections_never_change_a_snapshot_or_wrap_its_revision() {
    let parent = work(WorkKind::Novel);
    let original = unit(&parent, UNIT_ID, 0);
    let bytes = serde_json::to_vec(&original).unwrap();
    assert_eq!(
        original.edit(&parent, 0, unit_details(), 2_000),
        Err(CatalogError::RevisionConflict {
            current_revision: 1,
            expected_revision: 0
        })
    );
    assert_eq!(
        original.archive(&parent, 1, 999),
        Err(CatalogError::TimestampRegression)
    );
    assert_eq!(serde_json::to_vec(&original).unwrap(), bytes);
    let mut raw = stored_unit(&original);
    raw.revision = i64::MAX as u64;
    let last = ContentUnit::from_stored(raw, parent.kind()).unwrap();
    assert_eq!(
        last.with_position(&parent, i64::MAX as u64, 1, 2_000),
        Err(CatalogError::RevisionLimit)
    );
}

#[test]
fn reorder_permutates_existing_active_slots_and_preserves_archived_tombstone_positions() {
    let parent = work(WorkKind::Novel);
    let first = unit(&parent, UNIT_ID, 0);
    let archived = unit(&parent, SECOND_UNIT_ID, 1)
        .archive(&parent, 1, 2_000)
        .unwrap();
    let third = unit(&parent, THIRD_UNIT_ID, 7);
    let units = vec![third.clone(), first.clone(), archived.clone()];
    let original = units.clone();
    let plan = plan_reorder(&parent, &units, &[third.id().clone(), first.id().clone()]).unwrap();
    assert_eq!(
        plan.assignments()
            .iter()
            .map(|item| (item.unit_id().clone(), item.position()))
            .collect::<Vec<_>>(),
        vec![(third.id().clone(), 0), (first.id().clone(), 7)]
    );
    assert_eq!(units, original);
    assert_eq!(archived.position(), 1);
    let rearranged_input = vec![archived, first, third];
    assert_eq!(
        plan_reorder(
            &parent,
            &rearranged_input,
            &[
                ContentUnitId::parse(THIRD_UNIT_ID).unwrap(),
                ContentUnitId::parse(UNIT_ID).unwrap()
            ]
        ),
        Ok(plan)
    );
}

#[test]
fn reorder_requires_an_exact_active_permutation_and_refuses_duplicates_unknowns_and_archives() {
    let parent = work(WorkKind::Novel);
    let first = unit(&parent, UNIT_ID, 0);
    let second = unit(&parent, SECOND_UNIT_ID, 1);
    let units = vec![first.clone(), second.clone()];
    assert_eq!(
        plan_reorder(&parent, &units, &[first.id().clone(), first.id().clone()]),
        Err(CatalogError::DuplicateReorderUnit {
            unit: first.id().clone()
        })
    );
    assert_eq!(
        plan_reorder(&parent, &units, &[first.id().clone()]),
        Err(CatalogError::MissingReorderUnit {
            unit: second.id().clone()
        })
    );
    let unknown = ContentUnitId::parse(THIRD_UNIT_ID).unwrap();
    assert_eq!(
        plan_reorder(&parent, &units, &[unknown.clone(), second.id().clone()]),
        Err(CatalogError::UnknownReorderUnit { unit: unknown })
    );
    let archived = second.archive(&parent, 1, 2_000).unwrap();
    assert_eq!(
        plan_reorder(
            &parent,
            &[first.clone(), archived.clone()],
            &[first.id().clone(), archived.id().clone()]
        ),
        Err(CatalogError::ArchivedReorderUnit {
            unit: archived.id().clone()
        })
    );
    assert_eq!(
        plan_reorder(&parent, &[first.clone(), archived], &[first.id().clone()])
            .unwrap()
            .assignments()
            .len(),
        1
    );
}

#[test]
fn reorder_refuses_corrupt_duplicate_entities_or_positions_and_accepts_empty_catalogs() {
    let parent = work(WorkKind::Story);
    let first = unit(&parent, UNIT_ID, 0);
    let second = unit(&parent, SECOND_UNIT_ID, 0);
    assert_eq!(
        plan_reorder(
            &parent,
            &[first.clone(), first.clone()],
            &[first.id().clone()]
        ),
        Err(CatalogError::DuplicateContentUnit {
            unit: first.id().clone()
        })
    );
    assert_eq!(
        plan_reorder(
            &parent,
            &[first.clone(), second.clone()],
            &[first.id().clone(), second.id().clone()]
        ),
        Err(CatalogError::DuplicatePosition { position: 0 })
    );
    assert_eq!(plan_reorder(&parent, &[], &[]).unwrap().assignments(), &[]);
    let archived = first.archive(&parent, 1, 2_000).unwrap();
    assert_eq!(
        plan_reorder(&parent, &[archived], &[])
            .unwrap()
            .assignments(),
        &[]
    );
}

#[test]
fn preview_admits_only_the_named_saved_head_of_a_live_work() {
    let first = work(WorkKind::Story);
    assert!(admit_preview(&first, 1).is_ok());
    let second = first.edit(1, work_details(), 2_000).unwrap();
    assert!(admit_preview(&second, 2).is_ok());
    // A moved head is stale, never a silent preview of the newer revision.
    assert!(matches!(
        admit_preview(&second, 1),
        Err(CatalogError::RevisionConflict {
            current_revision: 2,
            expected_revision: 1
        })
    ));
    // A revision that does not exist yet is just as stale.
    assert!(matches!(
        admit_preview(&second, 3),
        Err(CatalogError::RevisionConflict {
            current_revision: 2,
            expected_revision: 3
        })
    ));
}

#[test]
fn preview_refuses_an_archived_work_at_its_head_and_reports_a_moved_head_as_stale() {
    let archived = work(WorkKind::Podcast).archive(1, 2_000).unwrap();
    assert!(matches!(
        admit_preview(&archived, archived.revision()),
        Err(CatalogError::WorkArchived)
    ));
    assert!(matches!(
        admit_preview(&archived, 1),
        Err(CatalogError::RevisionConflict { .. })
    ));
}
