use super::*;

#[test]
fn email_validation_refuses_ambiguous_or_header_injected_addresses() {
    for invalid in [
        "",
        "a",
        "a@b",
        "a@@b.com",
        "a\r\nb@example.com",
        "a@-example.com",
        "a@e..com",
        ".a@example.com",
        "a..b@example.com",
        "tên@example.com",
        "a@example.com\0",
    ] {
        assert_eq!(
            Email::parse(invalid),
            Err(RegistrationError::InvalidEmail),
            "{invalid:?}"
        );
    }
    let email = Email::parse("  Person+Book@EXAMPLE.COM  ").unwrap();
    assert_eq!(email.as_str(), "Person+Book@example.com");
    assert_eq!(email.canonical_key(), "person+book@example.com");
    assert_eq!(
        Email::parse("person+book@example.com")
            .unwrap()
            .canonical_key(),
        email.canonical_key()
    );
}

#[test]
fn delivery_and_deduplication_do_not_link_identities() {
    let email = Email::parse("Reader@example.com").unwrap();
    let one = PendingRegistration::new(Uuid::from_u128(1), email.clone()).unwrap();
    let two = PendingRegistration::new(Uuid::from_u128(2), email).unwrap();
    assert_ne!(one.id(), two.id());
    assert_ne!(one.provider_name(), two.provider_name());
    assert_eq!(
        one.provider_name(),
        "cantos_00000000000000000000000000000001"
    );
}

#[test]
fn only_a_positive_exact_new_person_binding_can_form_the_setup_witness() {
    let id = Uuid::from_u128(1);
    assert_eq!(
        CreatedPerson::confirmed(id, Uuid::from_u128(2)),
        Err(RegistrationError::InvalidState)
    );
    assert_eq!(
        CreatedPerson::confirmed(Uuid::nil(), Uuid::nil()),
        Err(RegistrationError::InvalidState)
    );
    let person = CreatedPerson::confirmed(id, id).unwrap();
    assert_eq!((person.registration_id(), person.subject()), (id, id));
}

#[test]
fn the_effect_sequence_never_rewinds_or_reissues_after_start() {
    let phases = [
        Phase::AwaitingVerification,
        Phase::Creating,
        Phase::Created,
        Phase::SetupStarted,
        Phase::Delivering,
        Phase::Complete,
        Phase::Blocked,
        Phase::Cancelled,
    ];
    for (from_index, from) in phases.iter().enumerate() {
        for (to_index, to) in phases.iter().enumerate() {
            let forward = from_index < 5 && to_index == from_index + 1;
            let blocked = (1..=4).contains(&from_index) && *to == Phase::Blocked;
            let cancelled = *from == Phase::AwaitingVerification && *to == Phase::Cancelled;
            assert_eq!(
                from.can_advance(*to),
                forward || blocked || cancelled,
                "{from:?} -> {to:?}"
            );
        }
        assert_eq!(
            from.may_claim(99, 100),
            *from == Phase::AwaitingVerification
        );
        assert!(!from.may_claim(100, 100));
        assert_eq!(
            from.may_replace_challenge(160, 100, 60),
            matches!(from, Phase::AwaitingVerification | Phase::Cancelled)
        );
        assert!(!from.may_replace_challenge(159, 100, 60));
        assert!(!from.may_replace_challenge(99, 100, 60));
    }
}

#[test]
fn token_hash_accepts_only_the_full_random_token_and_keeps_no_raw_value() {
    let token = "a".repeat(64);
    let hash = TokenHash::from_token(&token).unwrap();
    assert_eq!(hash.as_bytes().len(), 32);
    assert_ne!(hash.as_bytes().as_slice(), token.as_bytes());
    // Python hashlib reference vector fixes the registration-purpose binding independently.
    assert_eq!(
        hash.as_bytes(),
        &[
            178, 247, 3, 194, 131, 159, 194, 96, 157, 47, 239, 92, 5, 119, 223, 249, 16, 255, 143,
            147, 145, 93, 147, 31, 122, 124, 248, 226, 119, 4, 48, 122
        ]
    );
    for invalid in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
    ] {
        assert_eq!(
            TokenHash::from_token(&invalid),
            Err(RegistrationError::InvalidToken)
        );
    }
}

#[test]
fn privacy_debug_never_contains_email_or_bearer_links() {
    let email = Email::parse("private@example.com").unwrap();
    let setup = SetupLink::new("https://issuer.example/ui/reset?token=secret".into()).unwrap();
    let id = Uuid::from_u128(1);
    let registration = PendingRegistration::new(id, email.clone()).unwrap();
    let person = CreatedPerson::confirmed(id, id).unwrap();
    let mail = RegistrationMail::credential_setup(&registration, &person, setup.clone()).unwrap();
    let debug = format!("{email:?} {setup:?} {mail:?}");
    assert!(!debug.contains("private"));
    assert!(!debug.contains("secret"));
    assert!(!debug.contains("https"));
}

#[test]
fn only_setup_mail_contains_the_exact_confirmed_person_login() {
    let id = Uuid::from_u128(42);
    let email = Email::parse("Reader@example.com").unwrap();
    let registration = PendingRegistration::new(id, email.clone()).unwrap();
    let person = CreatedPerson::confirmed(id, id).unwrap();
    let setup = SetupLink::new("https://issuer.example/ui/reset?token=fixture".into()).unwrap();
    let mail = RegistrationMail::credential_setup(&registration, &person, setup.clone()).unwrap();
    assert_eq!(
        mail.login(),
        Some("cantos_0000000000000000000000000000002a")
    );
    assert_eq!(mail.recipient(), "Reader@example.com");
    let verification = RegistrationMail::verification(
        email,
        "https://cantos.example/#register-verify?token=fixture".into(),
        id,
    );
    assert_eq!(verification.login(), None);
    let other = CreatedPerson::confirmed(Uuid::from_u128(43), Uuid::from_u128(43)).unwrap();
    assert_eq!(
        RegistrationMail::credential_setup(&registration, &other, setup).err(),
        Some(RegistrationError::InvalidState)
    );
}

#[test]
fn activation_policy_is_explicit_bounded_and_has_no_unlimited_mode() {
    assert_eq!(
        RegistrationPolicy::new(900, 60, 3600, 100, 5)
            .unwrap()
            .max_requests(),
        100
    );
    for args in [
        (0, 60, 3600, 100, 5),
        (900, 0, 3600, 100, 5),
        (900, 901, 3600, 100, 5),
        (900, 60, 0, 100, 5),
        (900, 60, 3600, 0, 5),
        (900, 60, 3600, 100, 101),
    ] {
        assert_eq!(
            RegistrationPolicy::new(args.0, args.1, args.2, args.3, args.4),
            Err(RegistrationError::Configuration)
        );
    }
}

#[test]
fn network_partition_is_deployment_keyed_and_domain_separated_from_proofs() {
    let peer = "192.0.2.1".parse().unwrap();
    let one = NetworkKey::from_peer(peer, &[1; 32]).unwrap();
    let two = NetworkKey::from_peer(peer, &[2; 32]).unwrap();
    assert_ne!(one, two);
    assert_eq!(one, NetworkKey::from_peer(peer, &[1; 32]).unwrap());
    assert_eq!(
        one,
        NetworkKey::from_peer("::ffff:192.0.2.1".parse().unwrap(), &[1; 32]).unwrap()
    );
    assert_eq!(format!("{one:?}"), "NetworkKey([REDACTED])");
}
