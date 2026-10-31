use proptest::prelude::*;

use super::*;

const PUBLICATION: &str = "80000000-0000-4000-8000-000000000008";

fn key(fill: u8) -> GrantKey {
    GrantKey::new(&[fill; 32]).unwrap()
}

fn subject() -> GrantSubject<'static> {
    GrantSubject {
        publication_id: PUBLICATION,
        rendition_id: "60000000-0000-4000-8000-000000000006",
        manifest_digest:
            "manifest-v1:sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        object_id: "publication-object-never-public",
        byte_length: 100,
        access_scope: None,
    }
}

fn current() -> Option<CurrentRelease> {
    Some(CurrentRelease {
        published_at_ms: 5000,
        rights_valid_until_ms: 10_000,
    })
}

#[test]
fn sealed_grant_round_trips_and_authenticates_only_the_exact_subject() {
    let token = seal(&key(7), &subject(), 9000).unwrap();
    assert_eq!(token.len(), GRANT_TOKEN_LEN);
    assert!(token.starts_with("pg1_80000000000040008000000000000008"));
    // The private delivery object and permit digests never appear in the bearer capability.
    for private in ["publication-object", "manifest-v1", &"a".repeat(64)] {
        assert!(!token.contains(private));
    }
    let claims = GrantClaims::parse(&token).unwrap();
    assert_eq!(claims.publication_id(), PUBLICATION);
    assert_eq!(claims.expires_at_ms(), 9000);
    assert!(claims.authenticates(&key(7), &subject()));

    let other_publication = "90000000-0000-4000-8000-000000000009";
    let other_digest = format!("manifest-v1:sha256:{}", "c".repeat(64));
    let other_sha = "b".repeat(64);
    for dimension in 0..6 {
        let mut changed = subject();
        match dimension {
            0 => changed.publication_id = other_publication,
            1 => changed.rendition_id = "90000000-0000-4000-8000-000000000009",
            2 => changed.manifest_digest = &other_digest,
            3 => changed.sha256 = &other_sha,
            4 => changed.object_id = "another-publication-object",
            5 => changed.byte_length = 101,
            _ => unreachable!("finite identity domain"),
        }
        assert!(
            !claims.authenticates(&key(7), &changed),
            "dimension {dimension}"
        );
    }
    assert!(!claims.authenticates(&key(8), &subject()));
}

#[test]
fn expiry_is_authenticated_and_cannot_be_extended_by_editing_the_token() {
    let token = seal(&key(7), &subject(), 9000).unwrap();
    let extended = format!("{}{:016x}{}", &token[..36], 9_999_u64, &token[52..]);
    let claims = GrantClaims::parse(&extended).unwrap();
    assert_eq!(claims.expires_at_ms(), 9999);
    assert!(!claims.authenticates(&key(7), &subject()));
}

#[test]
fn length_prefixed_preimage_separates_adjacent_fields() {
    // Moving bytes between neighbouring fields must change the authenticator.
    let token = seal(&key(7), &subject(), 9000).unwrap();
    let claims = GrantClaims::parse(&token).unwrap();
    let shifted = GrantSubject {
        rendition_id: "60000000-0000-4000-8000-00000000000",
        manifest_digest:
            "6manifest-v1:sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ..subject()
    };
    assert!(!claims.authenticates(&key(7), &shifted));
}

#[test]
fn token_parsing_admits_only_the_canonical_fixed_shape() {
    let token = seal(&key(7), &subject(), 9000).unwrap();
    let mut cases = vec![
        String::new(),
        token[..GRANT_TOKEN_LEN - 1].to_owned(),
        format!("{token}0"),
        token.replacen("pg1_", "pg2_", 1),
        token.replacen("pg1_", "PG1_", 1),
        token.to_ascii_uppercase(),
        format!("{}g{}", &token[..60], &token[61..]),
        format!("{}+{}", &token[..4], &token[5..]),
        // Expiry above i64::MAX cannot be represented by the backend clock.
        format!("{}{}{}", &token[..36], "8000000000000000", &token[52..]),
    ];
    cases.push(format!("{}é{}", &token[..10], &token[12..]));
    for case in cases {
        assert_eq!(
            GrantClaims::parse(&case),
            Err(GrantError::Malformed),
            "{case:?}"
        );
    }
}

#[test]
fn seal_rejects_non_canonical_publication_and_negative_expiry() {
    for publication in [
        "80000000-0000-4000-8000-00000000000A",
        "8000000000004000800000000000000a",
        "not-a-publication",
    ] {
        let changed = GrantSubject {
            publication_id: publication,
            ..subject()
        };
        assert_eq!(
            seal(&key(7), &changed, 9000),
            Err(GrantError::InvalidSubject)
        );
    }
    assert_eq!(
        seal(&key(7), &subject(), -1),
        Err(GrantError::InvalidExpiry)
    );
}

#[test]
fn grant_key_requires_exact_nonzero_material_and_redacts_debug() {
    for bytes in [vec![], vec![1; 31], vec![1; 33], vec![0; 32]] {
        assert_eq!(GrantKey::new(&bytes).unwrap_err(), GrantError::InvalidKey);
    }
    let secret = GrantKey::new(&[0xab; 32]).unwrap();
    let rendered = format!("{secret:?}");
    assert_eq!(rendered, "GrantKey([redacted])");
    let token = seal(&secret, &subject(), 9000).unwrap();
    let claims = format!("{:?}", GrantClaims::parse(&token).unwrap());
    assert!(claims.contains("[redacted]"));
    assert!(!claims.contains(&token[52..]));
}

#[test]
fn grant_expiry_is_bounded_by_ttl_rights_and_the_present() {
    for (now, until, expected) in [
        (5000, 10_000_000, Some(125_000)),
        (5000, 10_000, Some(10_000)),
        (5000, 5001, Some(5001)),
        (5000, 5000, None),
        (5000, 4000, None),
        (-1, 10_000, None),
        (0, 1, Some(1)),
        (i64::MAX - 1, i64::MAX, Some(i64::MAX)),
        (i64::MAX, i64::MAX, None),
    ] {
        assert_eq!(grant_expiry(now, until), expected, "{now} {until}");
    }
}

#[test]
fn fetch_admission_has_an_exact_fail_closed_precedence() {
    let admitted = MediaFetchFacts {
        now_ms: 6000,
        expires_at_ms: 9000,
        current: current(),
        authenticated: true,
    };
    assert_eq!(decide_media_fetch(admitted), Ok(()));
    for (facts, denial) in [
        (
            MediaFetchFacts {
                now_ms: 9000,
                ..admitted
            },
            MediaFetchDenial::Expired,
        ),
        (
            MediaFetchFacts {
                now_ms: -1,
                ..admitted
            },
            MediaFetchDenial::Expired,
        ),
        (
            MediaFetchFacts {
                expires_at_ms: 6000 + GRANT_TTL_MS + 1,
                current: None,
                ..admitted
            },
            MediaFetchDenial::Unbounded,
        ),
        (
            MediaFetchFacts {
                current: None,
                authenticated: false,
                ..admitted
            },
            MediaFetchDenial::NotCurrent,
        ),
        (
            MediaFetchFacts {
                authenticated: false,
                ..admitted
            },
            MediaFetchDenial::Forged,
        ),
        (
            MediaFetchFacts {
                expires_at_ms: 10_001,
                ..admitted
            },
            MediaFetchDenial::OutsideRights,
        ),
        (
            MediaFetchFacts {
                current: Some(CurrentRelease {
                    published_at_ms: 6001,
                    rights_valid_until_ms: 10_000,
                }),
                ..admitted
            },
            MediaFetchDenial::OutsideRights,
        ),
    ] {
        assert_eq!(decide_media_fetch(facts), Err(denial), "{facts:?}");
    }
    // The last millisecond before expiry is admitted; the expiry instant is not.
    assert_eq!(
        decide_media_fetch(MediaFetchFacts {
            now_ms: 8999,
            ..admitted
        }),
        Ok(())
    );
}

fn specification(f: MediaFetchFacts) -> Result<(), MediaFetchDenial> {
    let now = i128::from(f.now_ms);
    let expires = i128::from(f.expires_at_ms);
    if now < 0 || now >= expires {
        Err(MediaFetchDenial::Expired)
    } else if expires > now + i128::from(GRANT_TTL_MS) {
        Err(MediaFetchDenial::Unbounded)
    } else if let Some(current) = f.current {
        if !f.authenticated {
            Err(MediaFetchDenial::Forged)
        } else if current.published_at_ms < 0
            || i128::from(current.published_at_ms) > now
            || expires > i128::from(current.rights_valid_until_ms)
        {
            Err(MediaFetchDenial::OutsideRights)
        } else {
            Ok(())
        }
    } else {
        Err(MediaFetchDenial::NotCurrent)
    }
}

fn clock() -> impl Strategy<Value = i64> {
    prop_oneof![
        Just(i64::MIN),
        Just(-1_i64),
        Just(0_i64),
        Just(i64::MAX),
        Just(i64::MAX - GRANT_TTL_MS),
        0_i64..20_000,
        any::<i64>(),
    ]
}

proptest! {
    #[test]
    fn fetch_admission_matches_an_independent_widened_specification(
        now in clock(), expires in clock(), published in clock(), until in clock(),
        present in any::<bool>(), authenticated in any::<bool>(),
    ) {
        let facts = MediaFetchFacts {
            now_ms: now,
            expires_at_ms: expires,
            current: present.then_some(CurrentRelease {
                published_at_ms: published,
                rights_valid_until_ms: until,
            }),
            authenticated,
        };
        prop_assert_eq!(decide_media_fetch(facts), specification(facts));
    }

    #[test]
    fn issued_grants_are_admitted_until_expiry_and_never_outlive_rights(
        now in 0_i64..1_000_000, ttl_offset in 1_i64..2_000_000,
    ) {
        let until = now + ttl_offset;
        let expires = grant_expiry(now, until).unwrap();
        prop_assert!(expires > now && expires <= until && expires <= now + GRANT_TTL_MS);
        let facts = |at| MediaFetchFacts {
            now_ms: at,
            expires_at_ms: expires,
            current: Some(CurrentRelease { published_at_ms: now, rights_valid_until_ms: until }),
            authenticated: true,
        };
        prop_assert_eq!(decide_media_fetch(facts(now)), Ok(()));
        prop_assert_eq!(decide_media_fetch(facts(expires - 1)), Ok(()));
        prop_assert_eq!(decide_media_fetch(facts(expires)), Err(MediaFetchDenial::Expired));
    }

    #[test]
    fn any_single_character_change_to_a_sealed_token_fails_closed(
        index in 4_usize..GRANT_TOKEN_LEN, replacement in 0_u8..16,
    ) {
        let token = seal(&key(7), &subject(), 9000).unwrap();
        let digit = char::from_digit(u32::from(replacement), 16).unwrap();
        prop_assume!(token.as_bytes()[index] != digit as u8);
        let mut changed = token.into_bytes();
        changed[index] = digit as u8;
        let changed = String::from_utf8(changed).unwrap();
        match GrantClaims::parse(&changed) {
            Ok(claims) => prop_assert!(!claims.authenticates(&key(7), &subject())),
            Err(error) => prop_assert_eq!(error, GrantError::Malformed),
        }
    }
}

#[test]
fn restricted_grants_cannot_cross_session_policy_purpose_or_public_lane() {
    let public = subject();
    let private = GrantSubject {
        access_scope: Some("actor-a/session-a/media/policy-1"),
        ..public
    };
    let token = seal(&key(7), &private, 9000).unwrap();
    let claims = GrantClaims::parse(&token).unwrap();
    assert!(claims.authenticates(&key(7), &private));
    assert!(!claims.authenticates(&key(7), &public));
    for scope in [
        "actor-b/session-b/media/policy-1",
        "actor-a/session-b/media/policy-1",
        "actor-a/session-a/artwork/policy-1",
        "actor-a/session-a/media/policy-2",
    ] {
        assert!(!claims.authenticates(
            &key(7),
            &GrantSubject {
                access_scope: Some(scope),
                ..public
            }
        ));
    }
    assert!(!GrantClaims::parse(&seal(&key(7), &public, 9000).unwrap())
        .unwrap()
        .authenticates(&key(7), &private));
}
