use super::*;
use crate::contexts::catalog::domain::{
    content::{ContentBody, ContentDetails, ManuscriptText},
    NewWork, ScriptRef, SourceRef, Visibility, WorkDetails, WorkKind,
};

const WORK_A: &str = "10000000-0000-4000-8000-00000000000a";
const WORK_B: &str = "10000000-0000-4000-8000-00000000000b";
const GRANT: &str = "40000000-0000-4000-8000-000000000001";
const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ISSUED: i64 = 1_000_000;
const EXPIRES: i64 = ISSUED + 7 * 24 * 60 * 60 * 1000;

fn owner(id: &str) -> OwnerId {
    OwnerId::parse(id).unwrap()
}
fn work(id: &str, owner_id: &str) -> Work {
    Work::new(NewWork {
        id: WorkId::parse(id).unwrap(),
        kind: WorkKind::Novel,
        owner_id: owner(owner_id),
        details: WorkDetails {
            title: "Mùa trăng qua hiên".into(),
            synopsis: "Một câu chuyện Việt Nam.".into(),
            language: "vi-VN".into(),
            visibility: Visibility::Private,
            rights_refs: vec![],
        },
        at: 1_000,
    })
    .unwrap()
}
fn actions(values: &[DelegatedAction]) -> DelegatedActions {
    DelegatedActions::new(values.iter().copied()).unwrap()
}
fn grant(granted: &[DelegatedAction]) -> DelegationGrant {
    DelegationGrant::issue(
        GrantRequest {
            id: DelegationGrantId::parse(GRANT).unwrap(),
            work_id: WorkId::parse(WORK_A).unwrap(),
            delegate: owner("agent-editor"),
            actions: actions(granted),
            expires_at: EXPIRES,
        },
        owner("alice"),
        ISSUED,
    )
    .unwrap()
}
fn current(now: i64) -> CurrentFacts {
    CurrentFacts {
        now,
        revoked: false,
        grantor_active: true,
        delegate_active: true,
    }
}

#[test]
fn action_labels_are_a_closed_exact_vocabulary() {
    for action in DelegatedAction::ALL {
        assert_eq!(DelegatedAction::parse(action.as_str()), Ok(action));
    }
    for label in [
        "",
        "work:*",
        "WORK:READ",
        "content:write ",
        "publication:stage",
        "script:read",
        "brief:read",
        "audio:upload",
        "grant:issue",
    ] {
        assert_eq!(
            DelegatedAction::parse(label),
            Err(DelegationError::UnknownAction),
            "{label:?}"
        );
    }
}

#[test]
fn action_sets_are_non_empty_and_reject_duplicates_instead_of_merging() {
    assert_eq!(DelegatedActions::new([]), Err(DelegationError::NoActions));
    assert_eq!(
        DelegatedActions::new([DelegatedAction::ContentRead, DelegatedAction::ContentRead]),
        Err(DelegationError::DuplicateAction)
    );
    let set = actions(&[DelegatedAction::ContentWrite, DelegatedAction::WorkRead]);
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        [DelegatedAction::WorkRead, DelegatedAction::ContentWrite],
        "canonical order is independent of request order"
    );
}

#[test]
fn grant_lifetime_is_positive_bounded_and_overflow_safe() {
    let issue = |issued_at: i64, expires_at: i64| {
        DelegationGrant::restore(
            DelegationGrantId::parse(GRANT).unwrap(),
            WorkId::parse(WORK_A).unwrap(),
            owner("alice"),
            owner("agent-editor"),
            actions(&[DelegatedAction::WorkRead]),
            issued_at,
            expires_at,
        )
        .map(|grant| (grant.issued_at(), grant.expires_at()))
    };
    assert_eq!(issue(ISSUED, ISSUED + 1), Ok((ISSUED, ISSUED + 1)));
    assert_eq!(
        issue(ISSUED, ISSUED + MAX_DELEGATION_LIFETIME_MS),
        Ok((ISSUED, ISSUED + MAX_DELEGATION_LIFETIME_MS))
    );
    for (issued_at, expires_at) in [
        (ISSUED, ISSUED),
        (ISSUED, ISSUED - 1),
        (ISSUED, ISSUED + MAX_DELEGATION_LIFETIME_MS + 1),
        (-1, 10),
        (i64::MIN, i64::MAX),
        (i64::MAX, i64::MIN),
    ] {
        assert_eq!(
            issue(issued_at, expires_at),
            Err(DelegationError::InvalidLifetime),
            "{issued_at}..{expires_at}"
        );
    }
}

#[test]
fn an_owner_cannot_delegate_to_themself() {
    let request = GrantRequest {
        id: DelegationGrantId::parse(GRANT).unwrap(),
        work_id: WorkId::parse(WORK_A).unwrap(),
        delegate: owner("alice"),
        actions: actions(&[DelegatedAction::WorkRead]),
        expires_at: EXPIRES,
    };
    assert_eq!(
        DelegationGrant::issue(request, owner("alice"), ISSUED),
        Err(DelegationError::SelfDelegation)
    );
}

#[test]
fn grant_ids_accept_only_the_canonical_uuid_spelling() {
    assert_eq!(DelegationGrantId::parse(GRANT).unwrap().as_str(), GRANT);
    for raw in [
        "",
        "40000000-0000-4000-8000-00000000000A",
        "40000000000040008000000000000001",
    ] {
        assert_eq!(
            DelegationGrantId::parse(raw),
            Err(DelegationError::InvalidId),
            "{raw}"
        );
    }
}

#[test]
fn secret_encoding_round_trips_and_every_other_spelling_is_not_a_credential() {
    let bytes: [u8; 32] = std::array::from_fn(|index| index as u8);
    let secret = DelegationSecret::from_random(bytes);
    let encoded = secret.encode();
    assert_eq!(
        encoded,
        "cantos_dlg_000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
    );
    assert_eq!(DelegationSecret::parse(&encoded), Ok(secret.clone()));
    let hex = &encoded[DELEGATION_TOKEN_PREFIX.len()..];
    for raw in [
        hex.to_owned(),
        encoded.to_uppercase(),
        format!("cantos_dlg_{}", hex.to_uppercase()),
        format!("cantos_pat_{hex}"),
        format!("{encoded}0"),
        encoded[..encoded.len() - 1].to_owned(),
        format!(" {encoded}"),
        format!("{encoded}\n"),
        format!("cantos_dlg_{}g", &hex[1..]),
        String::new(),
    ] {
        assert_eq!(
            DelegationSecret::parse(&raw),
            Err(DelegationError::InvalidToken),
            "{raw:?}"
        );
    }
    let debug = format!("{secret:?}");
    assert!(
        !debug.contains(hex) && debug.contains("redacted"),
        "{debug}"
    );
}

#[test]
fn lookup_hash_matches_an_independent_oracle_and_is_purpose_separated() {
    let secret = DelegationSecret::from_random(std::array::from_fn(|index| index as u8));
    // Independently computed with Python hashlib over the documented domain separator.
    let expected = "fb778b9d005918cbd7dbb7f722ad893b4e4196c0829f130e8347231aeef1e442";
    let actual: String = secret
        .lookup_hash()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(actual, expected);
    // Studio sessions hash the bare token text; producer reads use another domain.
    let encoded = secret.encode();
    let session: [u8; 32] = Sha256::digest(encoded.as_bytes()).into();
    let mut producer = Sha256::new();
    producer.update(b"cantos/producer-read/v1\n");
    producer.update(encoded.as_bytes());
    let producer: [u8; 32] = producer.finalize().into();
    assert_ne!(secret.lookup_hash(), session);
    assert_ne!(secret.lookup_hash(), producer);
    let other = DelegationSecret::from_random([0xff; 32]);
    assert_ne!(secret.lookup_hash(), other.lookup_hash());
}

/// Independent oracle: the documented precedence written as an ordered list.
fn expected_admission(
    grant: &DelegationGrant,
    facts: &CurrentFacts,
) -> Result<(), DelegationDenial> {
    let ordered = [
        (facts.revoked, DelegationDenial::Revoked),
        (facts.now < grant.issued_at(), DelegationDenial::NotYetValid),
        (facts.now >= grant.expires_at(), DelegationDenial::Expired),
        (!facts.grantor_active, DelegationDenial::GrantorInactive),
        (!facts.delegate_active, DelegationDenial::DelegateInactive),
    ];
    match ordered.into_iter().find(|(failed, _)| *failed) {
        Some((_, denial)) => Err(denial),
        None => Ok(()),
    }
}

#[test]
fn admission_is_exhaustively_exact_over_lifecycle_facts_and_clock_boundaries() {
    let grant = grant(&[DelegatedAction::WorkRead]);
    let mut admitted = 0;
    for now in [
        ISSUED - 1,
        ISSUED,
        EXPIRES - 1,
        EXPIRES,
        EXPIRES + 1,
        i64::MIN,
        i64::MAX,
    ] {
        for bits in 0..8u8 {
            let facts = CurrentFacts {
                now,
                revoked: bits & 1 != 0,
                grantor_active: bits & 2 == 0,
                delegate_active: bits & 4 == 0,
            };
            let expected = expected_admission(&grant, &facts);
            assert_eq!(grant.admit(&facts), expected, "{facts:?}");
            admitted += usize::from(expected.is_ok());
        }
    }
    // Non-vacuity: exactly the two in-window instants with all-good facts admit.
    assert_eq!(admitted, 2);
}

#[test]
fn authorization_requires_the_exact_current_unarchived_work_and_a_granted_action() {
    let grant = grant(&[DelegatedAction::WorkRead, DelegatedAction::ContentWrite]);
    let facts = current(ISSUED + 1);
    let work_a = work(WORK_A, "alice");
    let permit = grant
        .authorize(&facts, &work_a, DelegatedAction::ContentWrite)
        .unwrap();
    assert_eq!(permit.delegate().as_str(), "agent-editor");
    assert_ne!(permit.delegate(), grant.grantor());
    assert_eq!(permit.grant().as_str(), GRANT);
    assert_eq!(permit.action(), DelegatedAction::ContentWrite);

    assert_eq!(
        grant.authorize(&facts, &work_a, DelegatedAction::ContentRead),
        Err(DelegationDenial::ActionNotGranted)
    );
    assert_eq!(
        grant.authorize(&facts, &work(WORK_B, "alice"), DelegatedAction::WorkRead),
        Err(DelegationDenial::OutsideWork)
    );
    // A foreign Work reveals nothing else: it is outside even if archived or owned by another.
    let foreign = work(WORK_B, "bob").archive(1, 2_000).unwrap();
    assert_eq!(
        grant.authorize(&facts, &foreign, DelegatedAction::WorkRead),
        Err(DelegationDenial::OutsideWork)
    );
    assert_eq!(
        grant.authorize(&facts, &work(WORK_A, "bob"), DelegatedAction::WorkRead),
        Err(DelegationDenial::GrantorNotOwner)
    );
    let archived = work_a.archive(1, 2_000).unwrap();
    assert_eq!(
        grant.authorize(&facts, &archived, DelegatedAction::WorkRead),
        Err(DelegationDenial::WorkArchived)
    );
    // Lifecycle denials precede resource facts, so a revoked grant learns nothing about Works.
    let revoked = CurrentFacts {
        revoked: true,
        ..facts
    };
    assert_eq!(
        grant.authorize(&revoked, &work(WORK_B, "bob"), DelegatedAction::WorkRead),
        Err(DelegationDenial::Revoked)
    );
}

#[test]
fn every_action_subset_authorizes_exactly_its_members() {
    let work_a = work(WORK_A, "alice");
    let facts = current(ISSUED);
    let mut subsets = 0;
    for mask in 1..8u8 {
        let granted: Vec<_> = DelegatedAction::ALL
            .into_iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, action)| action)
            .collect();
        let grant = grant(&granted);
        for action in DelegatedAction::ALL {
            let result = grant.authorize(&facts, &work_a, action);
            if granted.contains(&action) {
                assert_eq!(result.map(|permit| permit.action()), Ok(action));
            } else {
                assert_eq!(result, Err(DelegationDenial::ActionNotGranted));
            }
        }
        subsets += 1;
    }
    assert_eq!(subsets, 7);
}

#[test]
fn delegated_content_may_not_bind_sources_or_scripts() {
    let details = |body| {
        ContentDetails::new(
            "vi-VN".into(),
            body,
            vec![],
            "Bản thảo do tác tử biên tập.".into(),
        )
        .unwrap()
    };
    let text = || ManuscriptText::new("Người kể chuyện đã trở về.").unwrap();
    let source = || SourceRef::new("source-vi".into(), SHA256.into()).unwrap();
    let script = || {
        ScriptRef::new(
            "30000000-0000-4000-8000-000000000001".into(),
            1,
            format!("sir-e1:sha256:{SHA256}"),
        )
        .unwrap()
    };
    for body in [
        ContentBody::Recording,
        ContentBody::Text {
            text: text(),
            source: None,
        },
    ] {
        assert_eq!(delegated_content_admissible(&details(body)), Ok(()));
    }
    for body in [
        ContentBody::Text {
            text: text(),
            source: Some(source()),
        },
        ContentBody::Source { source: source() },
        ContentBody::Script {
            script: script(),
            source: None,
        },
    ] {
        assert_eq!(
            delegated_content_admissible(&details(body)),
            Err(DelegationDenial::LinkNotDelegated)
        );
    }
}

#[test]
fn request_digest_is_canonical_and_binds_every_owner_chosen_field() {
    let request =
        |work: &str, id: &str, delegate: &str, granted: &[DelegatedAction], expires_at| {
            GrantRequest {
                id: DelegationGrantId::parse(id).unwrap(),
                work_id: WorkId::parse(work).unwrap(),
                delegate: owner(delegate),
                actions: actions(granted),
                expires_at,
            }
            .digest()
        };
    use DelegatedAction::{ContentRead, WorkRead};
    let base = request(
        WORK_A,
        GRANT,
        "agent-editor",
        &[WorkRead, ContentRead],
        EXPIRES,
    );
    assert!(base.starts_with("dg-request1:sha256:") && base.len() == 19 + 64);
    assert_eq!(
        request(
            WORK_A,
            GRANT,
            "agent-editor",
            &[ContentRead, WorkRead],
            EXPIRES
        ),
        base,
        "action order is not part of the intent"
    );
    let changed = [
        request(
            WORK_B,
            GRANT,
            "agent-editor",
            &[WorkRead, ContentRead],
            EXPIRES,
        ),
        request(
            WORK_A,
            "40000000-0000-4000-8000-000000000002",
            "agent-editor",
            &[WorkRead, ContentRead],
            EXPIRES,
        ),
        request(
            WORK_A,
            GRANT,
            "agent-other",
            &[WorkRead, ContentRead],
            EXPIRES,
        ),
        request(WORK_A, GRANT, "agent-editor", &[WorkRead], EXPIRES),
        request(
            WORK_A,
            GRANT,
            "agent-editor",
            &[WorkRead, ContentRead],
            EXPIRES + 1,
        ),
    ];
    for (index, digest) in changed.iter().enumerate() {
        assert_ne!(digest, &base, "field {index}");
    }
}
