//! Application doubles prove ordered dispatch and refusal. SQL concurrency is tested separately.
use std::sync::Mutex;

use super::*;
use crate::platform::theatre_registration::domain::Phase;

/// Platform-owned lazy pool composition for mounted HTTP tests. Construction opens no
/// database connection; these scenarios dispatch registration through the in-memory ports.
pub(crate) fn fixture_store() -> crate::platform::postgres::Store {
    crate::platform::postgres::Store::new(tokio_postgres::Config::new()).unwrap()
}

#[derive(Clone)]
pub(crate) struct Row {
    pub(crate) registration: PendingRegistration,
    pub(crate) hash: TokenHash,
    pub(crate) expires: u64,
    pub(crate) sent: u64,
    pub(crate) phase: Phase,
    pub(crate) reason: Option<BlockReason>,
}
pub(crate) struct Memory {
    pub(crate) now: Mutex<u64>,
    pub(crate) row: Mutex<Option<Row>>,
    pub(crate) fail_created: bool,
    pub(crate) events: Arc<Mutex<Vec<&'static str>>>,
}
impl Memory {
    pub(crate) fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            now: Mutex::new(100),
            row: Mutex::new(None),
            fail_created: false,
            events,
        }
    }
    pub(crate) fn phase(&self) -> Phase {
        self.row.lock().unwrap().as_ref().unwrap().phase
    }
    fn advance(
        &self,
        id: Uuid,
        from: Phase,
        to: Phase,
        event: &'static str,
    ) -> Result<bool, RegistrationError> {
        self.events.lock().unwrap().push(event);
        let mut rows = self.row.lock().unwrap();
        let Some(row) = rows.as_mut() else {
            return Ok(false);
        };
        if row.registration.id() != id || row.phase != from {
            return Ok(false);
        }
        assert!(from.can_advance(to));
        row.phase = to;
        Ok(true)
    }
}
impl Repository for Memory {
    fn prepare(
        &self,
        candidate: Candidate,
        policy: RegistrationPolicy,
    ) -> RegistrationFuture<'_, PrepareOutcome> {
        Box::pin(async move {
            let now = *self.now.lock().unwrap();
            let mut rows = self.row.lock().unwrap();
            if let Some(row) = rows.as_mut() {
                if row.registration.email().canonical_key()
                    != candidate.registration.email().canonical_key()
                    || !row
                        .phase
                        .may_replace_challenge(now, row.sent, policy.resend_seconds())
                {
                    return Ok(PrepareOutcome::Suppressed);
                }
                row.hash = candidate.token_hash;
                row.sent = now;
                row.expires = now + u64::from(policy.verification_seconds());
                row.phase = Phase::AwaitingVerification;
                return Ok(PrepareOutcome::Deliver(row.registration.clone()));
            }
            let registration = candidate.registration;
            *rows = Some(Row {
                registration: registration.clone(),
                hash: candidate.token_hash,
                expires: now + u64::from(policy.verification_seconds()),
                sent: now,
                phase: Phase::AwaitingVerification,
                reason: None,
            });
            Ok(PrepareOutcome::Deliver(registration))
        })
    }
    fn claim(&self, hash: TokenHash) -> RegistrationFuture<'_, Option<PendingRegistration>> {
        Box::pin(async move {
            let now = *self.now.lock().unwrap();
            let mut rows = self.row.lock().unwrap();
            let Some(row) = rows.as_mut() else {
                return Ok(None);
            };
            if row.hash != hash || !row.phase.may_claim(now, row.expires) {
                return Ok(None);
            }
            row.phase = Phase::Creating;
            self.events.lock().unwrap().push("claim");
            Ok(Some(row.registration.clone()))
        })
    }
    fn cancel(&self, hash: TokenHash) -> RegistrationFuture<'_, ()> {
        Box::pin(async move {
            if let Some(row) = self.row.lock().unwrap().as_mut() {
                if row.hash == hash && row.phase == Phase::AwaitingVerification {
                    row.phase = Phase::Cancelled;
                }
            }
            Ok(())
        })
    }
    fn created(&self, id: Uuid, person: CreatedPerson) -> RegistrationFuture<'_, bool> {
        Box::pin(async move {
            if self.fail_created {
                return Err(RegistrationError::Unavailable);
            }
            if person.registration_id() != id || person.subject() != id {
                return Err(RegistrationError::InvalidState);
            }
            self.advance(id, Phase::Creating, Phase::Created, "created")
        })
    }
    fn begin_setup(&self, id: Uuid) -> RegistrationFuture<'_, bool> {
        Box::pin(
            async move { self.advance(id, Phase::Created, Phase::SetupStarted, "begin_setup") },
        )
    }
    fn begin_delivery(&self, id: Uuid) -> RegistrationFuture<'_, bool> {
        Box::pin(async move {
            self.advance(id, Phase::SetupStarted, Phase::Delivering, "begin_delivery")
        })
    }
    fn complete(&self, id: Uuid) -> RegistrationFuture<'_, bool> {
        Box::pin(async move { self.advance(id, Phase::Delivering, Phase::Complete, "complete") })
    }
    fn block(&self, id: Uuid, reason: BlockReason) -> RegistrationFuture<'_, ()> {
        Box::pin(async move {
            let mut rows = self.row.lock().unwrap();
            let row = rows.as_mut().unwrap();
            assert_eq!(row.registration.id(), id);
            assert!(row.phase.can_advance(Phase::Blocked));
            row.phase = Phase::Blocked;
            row.reason = Some(reason);
            Ok(())
        })
    }
}
pub(crate) struct FakeIssuer {
    pub(crate) create_error: Option<RegistrationError>,
    pub(crate) setup_error: Option<RegistrationError>,
    pub(crate) create_waits: bool,
    pub(crate) mismatch: bool,
    pub(crate) events: Arc<Mutex<Vec<&'static str>>>,
}
impl Issuer for FakeIssuer {
    fn create_new(
        &self,
        registration: PendingRegistration,
    ) -> RegistrationFuture<'_, CreatedPerson> {
        Box::pin(async move {
            self.events.lock().unwrap().push("create");
            if self.create_waits {
                std::future::pending::<()>().await;
            }
            if let Some(error) = self.create_error {
                return Err(error);
            }
            let id = if self.mismatch {
                Uuid::from_u128(99)
            } else {
                registration.id()
            };
            CreatedPerson::confirmed(id, id)
        })
    }
    fn setup_intent(&self, person: CreatedPerson) -> RegistrationFuture<'_, SetupLink> {
        Box::pin(async move {
            assert_eq!(person.registration_id(), person.subject());
            self.events.lock().unwrap().push("setup");
            if let Some(error) = self.setup_error {
                return Err(error);
            }
            SetupLink::new("https://issuer.example/ui/reset?token=fixture".into())
        })
    }
}
pub(crate) struct FakeMailer {
    pub(crate) verification_error: Option<RegistrationError>,
    pub(crate) setup_error: Option<RegistrationError>,
    pub(crate) attempts: Mutex<Vec<RegistrationMail>>,
    pub(crate) events: Arc<Mutex<Vec<&'static str>>>,
    changed: tokio::sync::Notify,
}
impl Mailer for FakeMailer {
    fn send(&self, mail: RegistrationMail) -> RegistrationFuture<'_, ()> {
        Box::pin(async move {
            let error = match mail.kind() {
                MailKind::Verification => {
                    self.events.lock().unwrap().push("verification_mail");
                    self.verification_error
                }
                MailKind::CredentialSetup => {
                    self.events.lock().unwrap().push("setup_mail");
                    self.setup_error
                }
            };
            self.attempts.lock().unwrap().push(mail);
            self.changed.notify_one();
            error.map_or(Ok(()), Err)
        })
    }
}
pub(crate) struct Harness {
    pub(crate) store: Arc<Memory>,
    pub(crate) issuer: Arc<FakeIssuer>,
    pub(crate) mailer: Arc<FakeMailer>,
    pub(crate) events: Arc<Mutex<Vec<&'static str>>>,
}
impl Harness {
    pub(crate) fn new() -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        Self {
            store: Arc::new(Memory::new(events.clone())),
            issuer: Arc::new(FakeIssuer {
                create_error: None,
                setup_error: None,
                create_waits: false,
                mismatch: false,
                events: events.clone(),
            }),
            mailer: Arc::new(FakeMailer {
                verification_error: None,
                setup_error: None,
                attempts: Mutex::new(Vec::new()),
                events: events.clone(),
                changed: tokio::sync::Notify::new(),
            }),
            events,
        }
    }
    pub(crate) fn service(&self) -> RegistrationService {
        RegistrationService::new(
            self.store.clone(),
            self.issuer.clone(),
            self.mailer.clone(),
            "https://cantos.example",
            RegistrationPolicy::new(900, 60, 3600, 100, 5).unwrap(),
        )
        .unwrap()
    }
    pub(crate) fn network(&self) -> NetworkKey {
        NetworkKey::from_peer("192.0.2.1".parse().unwrap(), &[1; 32]).unwrap()
    }
    pub(crate) async fn challenge(&self, service: &RegistrationService) -> RegistrationMail {
        service
            .prepare("Reader@example.com", self.network())
            .await
            .unwrap()
            .unwrap()
    }
    pub(crate) fn count(&self, event: &str) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|&&e| e == event)
            .count()
    }

    pub(crate) fn captured_mail(&self, kind: MailKind) -> Option<RegistrationMail> {
        self.mailer
            .attempts
            .lock()
            .unwrap()
            .iter()
            .find(|mail| mail.kind() == kind)
            .cloned()
    }

    pub(crate) async fn wait_for_mail(&self, kind: MailKind) -> RegistrationMail {
        loop {
            let changed = self.mailer.changed.notified();
            if let Some(mail) = self.captured_mail(kind) {
                return mail;
            }
            changed.await;
        }
    }
}
pub(crate) fn token(mail: &RegistrationMail) -> &str {
    mail.link().split("?token=").nth(1).unwrap()
}
