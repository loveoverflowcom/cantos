//! Core orchestration tests use the same test-only ports as the HTTP flow tests.
use super::test_support::*;
use super::*;
use crate::platform::theatre_registration::domain::Phase;

#[tokio::test]
async fn prepare_never_dispatches_network_or_creates_an_identity_before_email_proof() {
    let h = Harness::new();
    let service = h.service();
    let mail = h.challenge(&service).await;
    assert_eq!(h.store.phase(), Phase::AwaitingVerification);
    assert!(h.events.lock().unwrap().is_empty());
    assert!(mail
        .link()
        .starts_with("https://cantos.example/#register-verify?token="));
    assert_eq!(mail.recipient(), "Reader@example.com");
    assert!(service
        .prepare("reader@example.com", h.network())
        .await
        .unwrap()
        .is_none());
    assert_eq!(h.count("create"), 0);
}

#[tokio::test]
async fn verification_dispatch_refuses_a_setup_mail_without_a_durable_provisioning_fence() {
    let h = Harness::new();
    let service = h.service();
    let id = Uuid::from_u128(1);
    let registration =
        PendingRegistration::new(id, Email::parse("Reader@example.com").unwrap()).unwrap();
    let person = CreatedPerson::confirmed(id, id).unwrap();
    let link = SetupLink::new("https://issuer.example/ui/reset?token=fixture".into()).unwrap();
    let mail = RegistrationMail::credential_setup(&registration, &person, link).unwrap();
    assert_eq!(
        service.send_verification(mail).await,
        Err(RegistrationError::InvalidState)
    );
    assert!(h.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn verified_email_runs_exactly_one_new_person_and_issuer_owned_setup_sequence() {
    let h = Harness::new();
    let service = h.service();
    let mail = h.challenge(&service).await;
    service.send_verification(mail.clone()).await.unwrap();
    assert_eq!(
        service.verify(token(&mail)).await,
        Ok(VerificationOutcome::SetupSent)
    );
    assert_eq!(h.store.phase(), Phase::Complete);
    assert_eq!(
        *h.events.lock().unwrap(),
        vec![
            "verification_mail",
            "claim",
            "create",
            "created",
            "begin_setup",
            "setup",
            "begin_delivery",
            "setup_mail",
            "complete"
        ]
    );
    let attempts = h.mailer.attempts.lock().unwrap();
    assert_eq!(attempts[1].kind(), MailKind::CredentialSetup);
    assert_eq!(attempts[0].login(), None);
    assert_eq!(
        attempts[1].login(),
        Some(
            format!(
                "cantos_{}",
                h.store
                    .row
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .registration
                    .id()
                    .simple()
            )
            .as_str()
        )
    );
    assert_eq!(
        attempts[1].link(),
        "https://issuer.example/ui/reset?token=fixture"
    );
}

#[tokio::test]
async fn replay_and_concurrent_verification_cannot_dispatch_another_create_or_setup() {
    let h = Harness::new();
    let service = h.service();
    let mail = h.challenge(&service).await;
    let (one, two) = tokio::join!(service.verify(token(&mail)), service.verify(token(&mail)));
    assert_eq!(one, Ok(VerificationOutcome::SetupSent));
    assert_eq!(two, Ok(VerificationOutcome::Unavailable));
    assert_eq!(
        service.verify(token(&mail)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert_eq!(h.count("create"), 1);
    assert_eq!(h.count("setup"), 1);
    assert_eq!(h.count("setup_mail"), 1);
}

#[tokio::test]
async fn expiry_and_malformed_tokens_never_create_or_reveal_an_account() {
    let h = Harness::new();
    let service = h.service();
    let mail = h.challenge(&service).await;
    *h.store.now.lock().unwrap() = 1000;
    for proof in [token(&mail), "bad", &"b".repeat(64)] {
        assert_eq!(
            service.verify(proof).await,
            Ok(VerificationOutcome::Unavailable)
        );
    }
    assert_eq!(h.count("create"), 0);
}

#[tokio::test]
async fn verification_mail_rejection_or_unknown_recovers_by_rotating_only_the_unconsumed_proof() {
    for error in [
        RegistrationError::DeliveryRejected,
        RegistrationError::DeliveryUncertain,
    ] {
        let mut h = Harness::new();
        Arc::get_mut(&mut h.mailer).unwrap().verification_error = Some(error);
        let service = h.service();
        let old = h.challenge(&service).await;
        assert_eq!(service.send_verification(old.clone()).await, Err(error));
        *h.store.now.lock().unwrap() = 160;
        let fresh = h.challenge(&service).await;
        assert_ne!(token(&old), token(&fresh));
        assert_eq!(
            service.verify(token(&old)).await,
            Ok(VerificationOutcome::Unavailable)
        );
        assert_eq!(
            service.verify(token(&fresh)).await,
            Ok(VerificationOutcome::SetupSent)
        );
        assert_eq!(h.count("create"), 1);
    }
}

#[tokio::test]
async fn issuer_create_rejection_or_unknown_never_resets_an_existing_person_or_retries() {
    for error in [
        RegistrationError::IssuerRejected,
        RegistrationError::IssuerUncertain,
        RegistrationError::Conflict,
    ] {
        let mut h = Harness::new();
        Arc::get_mut(&mut h.issuer).unwrap().create_error = Some(error);
        let service = h.service();
        let mail = h.challenge(&service).await;
        assert_eq!(
            service.verify(token(&mail)).await,
            Ok(VerificationOutcome::Unavailable)
        );
        assert_eq!(h.store.phase(), Phase::Blocked);
        *h.store.now.lock().unwrap() = 10000;
        let restarted = h.service();
        assert!(restarted
            .prepare("Reader@example.com", h.network())
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            restarted.verify(token(&mail)).await,
            Ok(VerificationOutcome::Unavailable)
        );
        assert_eq!(h.count("create"), 1);
        assert_eq!(h.count("setup"), 0);
    }
}

#[tokio::test]
async fn setup_uncertainty_never_reissues_the_credential_intent() {
    let mut h = Harness::new();
    Arc::get_mut(&mut h.issuer).unwrap().setup_error = Some(RegistrationError::IssuerUncertain);
    let service = h.service();
    let mail = h.challenge(&service).await;
    assert_eq!(
        service.verify(token(&mail)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert_eq!(h.store.phase(), Phase::Blocked);
    assert_eq!(
        h.service().verify(token(&mail)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert_eq!(h.count("setup"), 1);
    assert_eq!(h.count("setup_mail"), 0);
}

#[tokio::test]
async fn setup_mail_rejection_or_unknown_never_redispatches_a_reset() {
    for error in [
        RegistrationError::DeliveryRejected,
        RegistrationError::DeliveryUncertain,
    ] {
        let mut h = Harness::new();
        Arc::get_mut(&mut h.mailer).unwrap().setup_error = Some(error);
        let service = h.service();
        let mail = h.challenge(&service).await;
        assert_eq!(
            service.verify(token(&mail)).await,
            Ok(VerificationOutcome::Unavailable)
        );
        assert_eq!(h.store.phase(), Phase::Blocked);
        assert_eq!(
            h.service().verify(token(&mail)).await,
            Ok(VerificationOutcome::Unavailable)
        );
        assert_eq!(h.count("create"), 1);
        assert_eq!(h.count("setup"), 1);
        assert_eq!(h.count("setup_mail"), 1);
    }
}

#[tokio::test]
async fn storage_failure_after_create_preserves_the_unknown_fence_before_setup() {
    let mut h = Harness::new();
    Arc::get_mut(&mut h.store).unwrap().fail_created = true;
    let service = h.service();
    let mail = h.challenge(&service).await;
    assert_eq!(
        service.verify(token(&mail)).await,
        Err(RegistrationError::Unavailable)
    );
    assert_eq!(h.store.phase(), Phase::Creating);
    assert_eq!(
        h.service().verify(token(&mail)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert_eq!(h.count("create"), 1);
    assert_eq!(h.count("setup"), 0);
}

#[tokio::test]
async fn dropping_the_network_future_after_claim_cannot_create_twice_on_replay() {
    let mut h = Harness::new();
    Arc::get_mut(&mut h.issuer).unwrap().create_waits = true;
    let service = h.service();
    let mail = h.challenge(&service).await;
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(1),
        service.verify(token(&mail)),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(h.store.phase(), Phase::Creating);
    assert_eq!(
        h.service().verify(token(&mail)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert_eq!(h.count("create"), 1);
    assert_eq!(h.count("setup"), 0);
}

#[tokio::test]
async fn a_mismatched_create_witness_can_never_reset_another_person() {
    let mut h = Harness::new();
    Arc::get_mut(&mut h.issuer).unwrap().mismatch = true;
    let service = h.service();
    let mail = h.challenge(&service).await;
    assert_eq!(
        service.verify(token(&mail)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert_eq!(
        h.store.row.lock().unwrap().as_ref().unwrap().reason,
        Some(BlockReason::BindingMismatch)
    );
    assert_eq!(h.count("setup"), 0);
}

#[tokio::test]
async fn cancellation_invalidates_only_the_proof_and_fresh_signup_can_retry_after_cooldown() {
    let h = Harness::new();
    let service = h.service();
    let old = h.challenge(&service).await;
    service.cancel(token(&old)).await.unwrap();
    service.cancel("malformed").await.unwrap();
    assert_eq!(h.store.phase(), Phase::Cancelled);
    assert_eq!(
        service.verify(token(&old)).await,
        Ok(VerificationOutcome::Unavailable)
    );
    assert!(service
        .prepare("Reader@example.com", h.network())
        .await
        .unwrap()
        .is_none());
    *h.store.now.lock().unwrap() = 160;
    let fresh = h.challenge(&service).await;
    assert_eq!(
        service.verify(token(&fresh)).await,
        Ok(VerificationOutcome::SetupSent)
    );
    service.cancel(token(&fresh)).await.unwrap();
    assert_eq!(h.store.phase(), Phase::Complete);
    assert_eq!(h.count("create"), 1);
}
