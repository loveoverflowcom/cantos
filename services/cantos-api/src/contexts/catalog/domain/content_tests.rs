use super::*;

fn revision(text: &str, seq: u64, note: &str, rights: Vec<String>) -> ContentRevision {
    ContentRevision::new(ContentRevisionFacts {
        id: ContentRevisionId::parse("00000000-0000-0000-0000-000000000001").unwrap(),
        work_id: WorkId::parse("00000000-0000-0000-0000-000000000002").unwrap(),
        unit_id: ContentUnitId::parse("00000000-0000-0000-0000-000000000003").unwrap(),
        revision: seq,
        details: ContentDetails::new(
            "vi-VN".into(),
            ContentBody::Text {
                text: ManuscriptText::new(text).unwrap(),
                source: None,
            },
            rights,
            note.into(),
        )
        .unwrap(),
        recorded_by: OwnerId::parse("lan").unwrap(),
        recorded_at: 1000,
    })
    .unwrap()
}

#[test]
fn equivalent_vietnamese_text_has_equal_content_digest_and_no_identity_change() {
    let nfc = revision(
        "Người kể chuyện\nĐã trở về.",
        1,
        "Authored synthetic text",
        vec![],
    );
    let nfd = revision(
        "Người kể chuyện\r\nĐã trở về.",
        1,
        "Authored synthetic text",
        vec![],
    );
    assert_eq!(nfc, nfd);
    assert_eq!(nfc.content_digest(), nfd.content_digest());
    let changed = revision(
        "Người kể chuyện\nChưa trở về.",
        2,
        "Authored synthetic text",
        vec![],
    );
    assert_eq!(nfc.unit_id(), changed.unit_id());
    assert_ne!(nfc.content_digest(), changed.content_digest());
}

#[test]
fn rights_provenance_and_sequence_change_full_revision_but_not_content_binding() {
    let a = revision(
        "Ánh đèn.",
        1,
        "Authored synthetic text",
        vec!["rights-a".into()],
    );
    for b in [
        revision(
            "Ánh đèn.",
            2,
            "Authored synthetic text",
            vec!["rights-a".into()],
        ),
        revision(
            "Ánh đèn.",
            1,
            "Revised attribution",
            vec!["rights-a".into()],
        ),
        revision(
            "Ánh đèn.",
            1,
            "Authored synthetic text",
            vec!["rights-b".into()],
        ),
    ] {
        assert_eq!(a.content_digest(), b.content_digest());
        assert_ne!(a.revision_digest().unwrap(), b.revision_digest().unwrap());
    }
}

#[test]
fn content_sequence_compare_and_swap_has_exact_conflicts_and_overflow() {
    assert_eq!(next_content_revision(0, 0), Ok(1));
    assert_eq!(next_content_revision(2, 2), Ok(3));
    assert_eq!(
        next_content_revision(2, 1),
        Err(CatalogError::RevisionConflict {
            current_revision: 2,
            expected_revision: 1
        })
    );
    assert_eq!(
        next_content_revision(i64::MAX as u64, i64::MAX as u64),
        Err(CatalogError::RevisionLimit)
    );
}

#[test]
fn invalid_text_ids_and_declarations_cannot_construct_admitted_values() {
    for text in ["", " \n\t", "text\0"] {
        assert_eq!(ManuscriptText::new(text), Err(ContentError::InvalidText));
    }
    assert_eq!(
        ManuscriptText::new(&"a".repeat(MAX_TEXT_BYTES + 1)),
        Err(ContentError::InvalidText)
    );
    for id in ["hash-of-text", "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF"] {
        assert_eq!(ContentRevisionId::parse(id), Err(ContentError::InvalidId));
    }
    assert_eq!(
        ContentDetails::new("vi-VN".into(), ContentBody::Recording, vec![], " ".into()),
        Err(ContentError::InvalidProvenance)
    );
    assert_eq!(
        ContentDetails::new(
            "vi-VN".into(),
            ContentBody::Recording,
            vec!["x".into(), "x".into()],
            "Synthetic recording".into()
        ),
        Err(ContentError::InvalidRights)
    );
    assert_eq!(
        ContentDetails::new(
            "invalid_language".into(),
            ContentBody::Recording,
            vec![],
            "Synthetic recording".into()
        ),
        Err(ContentError::InvalidLanguage)
    );
}

#[test]
fn digest_schemes_have_an_independent_literal_oracle() {
    let a = revision("Ánh đèn.", 1, "Authored synthetic text", vec![]);
    // Filled from Python hashlib over the documented sorted JSON, not this Rust encoder.
    assert_eq!(
        a.content_digest().as_str(),
        "cc-c1:sha256:0e8a479a90588fd0875e0d8b5570c6f5e0ab3f91799840a67848460477311c5e"
    );
}
#[test]
fn rights_order_does_not_create_an_owner_only_change() {
    let previous = vec!["b".into(), "a".into()];
    assert!(!rights_change_requires_owner(
        &previous,
        &["a".into(), "b".into()]
    ));
    assert!(rights_change_requires_owner(&previous, &["a".into()]));
    assert!(rights_change_requires_owner(&[], &["a".into()]));
}
