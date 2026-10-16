//! Production orchestration with an in-memory fence double, real HTTP action client,
//! synthetic Firebase replies and fake bootstrap/Kanidm ports. These are not SQL/live evidence.
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};
use uuid::Uuid;

use super::{application::*, domain::*};
use crate::platform::theatre_registration::{
    application::{test_support::FakeIssuer, Issuer},
    domain::{
        CreatedPerson, Email, NetworkKey, PendingRegistration, RegistrationError,
        RegistrationPolicy, SetupLink,
    },
    firebase_action::FirebaseActionClient,
};

pub(crate) type Events = Arc<Mutex<Vec<&'static str>>>;

#[derive(Clone)]
struct Row {
    enrollment: Enrollment,
    phase: Phase,
    expires: u64,
    sent: u64,
    reason: Option<FailureStage>,
}

pub(crate) struct Memory {
    row: Mutex<Option<Row>>,
    now: AtomicU64,
    events: Events,
    fail_handoff_commit: AtomicBool,
}

impl Memory {
    pub(crate) fn new(events: Events) -> Self {
        Self {
            row: Mutex::new(None),
            now: AtomicU64::new(100),
            events,
            fail_handoff_commit: AtomicBool::new(false),
        }
    }

    pub(crate) fn awaiting(events: Events) -> Self {
        let memory = Self::new(events);
        *memory.row.lock().unwrap() = Some(Row {
            enrollment: Enrollment::new(
                PendingRegistration::new(
                    Uuid::from_u128(1),
                    Email::parse("Victim@example.test").unwrap(),
                )
                .unwrap(),
                "candidate-project",
            )
            .unwrap(),
            phase: Phase::AwaitingAction,
            expires: 200,
            sent: 100,
            reason: None,
        });
        memory
    }

    fn current(&self) -> Row {
        self.row.lock().unwrap().as_ref().unwrap().clone()
    }

    pub(crate) fn enrollment(&self) -> Option<Enrollment> {
        self.row
            .lock()
            .unwrap()
            .as_ref()
            .map(|row| row.enrollment.clone())
    }

    pub(crate) fn phase(&self) -> Option<Phase> {
        self.row.lock().unwrap().as_ref().map(|row| row.phase)
    }

    fn trace(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }

    fn advance(
        &self,
        id: Uuid,
        from: Phase,
        to: Phase,
        event: &'static str,
        fresh: bool,
    ) -> Result<bool, RegistrationError> {
        self.trace(event);
        let mut rows = self.row.lock().unwrap();
        let Some(row) = rows.as_mut() else {
            return Ok(false);
        };
        if row.enrollment.id() != id
            || row.phase != from
            || (fresh && self.now.load(Ordering::SeqCst) >= row.expires)
        {
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
        candidate: Enrollment,
        _: NetworkKey,
        policy: RegistrationPolicy,
    ) -> EnrollmentFuture<'_, PrepareOutcome> {
        Box::pin(async move {
            self.trace("prepare");
            let now = self.now.load(Ordering::SeqCst);
            let mut rows = self.row.lock().unwrap();
            if let Some(row) = rows.as_mut() {
                if row.enrollment.registration().email().canonical_key()
                    != candidate.registration().email().canonical_key()
                {
                    return Ok(PrepareOutcome::Suppressed);
                }
                if row
                    .phase
                    .may_resend(now, row.expires, row.sent, policy.resend_seconds())
                {
                    row.phase = Phase::SendStarted;
                    row.sent = now;
                    return Ok(PrepareOutcome::Resend(row.enrollment.clone()));
                }
                if !row.phase.may_have_successor(now, row.expires) {
                    return Ok(PrepareOutcome::Suppressed);
                }
            }
            *rows = Some(Row {
                enrollment: candidate.clone(),
                phase: Phase::BootstrapStarted,
                expires: now + u64::from(policy.verification_seconds()),
                sent: now,
                reason: None,
            });
            Ok(PrepareOutcome::Bootstrap(candidate))
        })
    }

    fn dispatchable(&self, id: Uuid, phase: Phase) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            if !matches!(phase, Phase::BootstrapStarted | Phase::SendStarted) {
                return Err(RegistrationError::InvalidState);
            }
            let row = self.current();
            Ok(row.enrollment.id() == id
                && row.phase == phase
                && self.now.load(Ordering::SeqCst) < row.expires)
        })
    }

    fn ready(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(
            async move { self.advance(id, Phase::BootstrapStarted, Phase::Ready, "ready", false) },
        )
    }
    fn begin_send(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(
            async move { self.advance(id, Phase::Ready, Phase::SendStarted, "send_fence", true) },
        )
    }
    fn begin_delivery(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            self.advance(
                id,
                Phase::SendStarted,
                Phase::MailStarted,
                "mail_fence",
                true,
            )
        })
    }
    fn sent(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            self.advance(id, Phase::MailStarted, Phase::AwaitingAction, "sent", true)
        })
    }
    fn claim_action(&self, id: Uuid) -> EnrollmentFuture<'_, Option<Enrollment>> {
        Box::pin(async move {
            if self.advance(id, Phase::AwaitingAction, Phase::Inspecting, "claim", true)? {
                Ok(Some(self.current().enrollment))
            } else {
                Ok(None)
            }
        })
    }
    fn release_inspection(&self, id: Uuid) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            self.advance(
                id,
                Phase::Inspecting,
                Phase::AwaitingAction,
                "release_inspection",
                false,
            )?;
            Ok(())
        })
    }
    fn begin_consume(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            self.advance(
                id,
                Phase::Inspecting,
                Phase::Consuming,
                "consume_fence",
                true,
            )
        })
    }
    fn cancel_consumed(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(
            async move { self.advance(id, Phase::Consuming, Phase::Cancelled, "cancelled", true) },
        )
    }
    fn begin_create(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            self.advance(id, Phase::Consuming, Phase::Creating, "create_fence", true)
        })
    }
    fn created(&self, id: Uuid, person: CreatedPerson) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            assert_eq!(person.registration_id(), id);
            assert_eq!(person.subject(), id);
            self.advance(id, Phase::Creating, Phase::Created, "created", false)
        })
    }
    fn begin_setup(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            self.advance(id, Phase::Created, Phase::SetupStarted, "setup_fence", true)
        })
    }
    fn begin_handoff(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            if self.fail_handoff_commit.load(Ordering::SeqCst) {
                return Err(RegistrationError::Unavailable);
            }
            self.advance(
                id,
                Phase::SetupStarted,
                Phase::HandedOff,
                "handoff_fence",
                true,
            )
        })
    }
    fn block(&self, id: Uuid, reason: FailureStage) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            let mut rows = self.row.lock().unwrap();
            let row = rows.as_mut().unwrap();
            assert_eq!(row.enrollment.id(), id);
            assert!(row.phase.can_advance(Phase::Blocked));
            row.phase = Phase::Blocked;
            row.reason = Some(reason);
            Ok(())
        })
    }
}

struct Mailbox {
    events: Events,
    create_error: Option<RegistrationError>,
    send_error: Option<RegistrationError>,
}
impl MailboxProvider for Mailbox {
    fn create_unverified(&self, _: Enrollment) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            self.events.lock().unwrap().push("bootstrap");
            self.create_error.map_or(Ok(()), Err)
        })
    }
    fn prepare_verification(
        &self,
        enrollment: Enrollment,
    ) -> EnrollmentFuture<'_, VerificationCredential> {
        Box::pin(async move {
            self.events.lock().unwrap().push("exchange");
            VerificationCredential::new(enrollment, "synthetic-id-token".into())
        })
    }
    fn send_verification(
        &self,
        enrollment: Enrollment,
        credential: VerificationCredential,
    ) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            assert!(credential.matches(&enrollment));
            self.events.lock().unwrap().push("send");
            self.send_error.map_or(Ok(()), Err)
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Held {
    Inspect,
    Consume,
}

pub(crate) struct ActionServer {
    address: SocketAddr,
    task: tokio::task::JoinHandle<()>,
    pub(crate) bodies: Arc<Mutex<Vec<serde_json::Value>>>,
    wrong_purpose: Arc<AtomicBool>,
    provider_disabled: Arc<AtomicBool>,
    disconnect_consume: Arc<AtomicBool>,
    reached: Arc<Notify>,
    release: Arc<Notify>,
}

impl ActionServer {
    pub(crate) async fn start(memory: Arc<Memory>, held: Option<Held>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let wrong_purpose = Arc::new(AtomicBool::new(false));
        let provider_disabled = Arc::new(AtomicBool::new(false));
        let disabled = provider_disabled.clone();
        let disconnect_consume = Arc::new(AtomicBool::new(false));
        let reached = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let (sink, wrong, disconnect, arrive, resume) = (
            bodies.clone(),
            wrong_purpose.clone(),
            disconnect_consume.clone(),
            reached.clone(),
            release.clone(),
        );
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    bytes.push(stream.read_u8().await.unwrap());
                    assert!(bytes.len() < 8192);
                }
                let headers = String::from_utf8(bytes).unwrap();
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                assert!(length < 8192);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).await.unwrap();
                sink.lock()
                    .unwrap()
                    .push(serde_json::from_slice(&body).unwrap());
                let consuming = headers.starts_with("POST /v1/accounts:update?");
                assert!(consuming || headers.starts_with("POST /v1/accounts:resetPassword?"));
                assert_eq!(
                    memory.current().phase,
                    if consuming {
                        Phase::Consuming
                    } else {
                        Phase::Inspecting
                    }
                );
                memory.trace(if consuming { "consume" } else { "inspect" });
                if held
                    == Some(if consuming {
                        Held::Consume
                    } else {
                        Held::Inspect
                    })
                {
                    arrive.notify_one();
                    resume.notified().await;
                }
                if consuming && disconnect.load(Ordering::SeqCst) {
                    continue;
                }
                if !consuming && disabled.load(Ordering::SeqCst) {
                    let error = r#"{"error":{"code":400,"message":"PASSWORD_LOGIN_DISABLED"}}"#;
                    let wire = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{error}", error.len());
                    let _ = stream.write_all(wire.as_bytes()).await;
                    continue;
                }
                let enrollment = memory.current().enrollment;
                let response = if consuming {
                    serde_json::json!({"localId":enrollment.uid(),"email":enrollment.registration().email().as_str()})
                } else {
                    serde_json::json!({"requestType": if wrong.load(Ordering::SeqCst) {"PASSWORD_RESET"} else {"VERIFY_EMAIL"}, "email":enrollment.registration().email().as_str()})
                }.to_string();
                let wire = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len());
                let _ = stream.write_all(wire.as_bytes()).await;
            }
        });
        Self {
            address,
            task,
            bodies,
            wrong_purpose,
            provider_disabled,
            disconnect_consume,
            reached,
            release,
        }
    }
    fn client(&self) -> Arc<FirebaseActionClient> {
        Arc::new(FirebaseActionClient::fixture(self.address, Duration::from_secs(2)).unwrap())
    }
}
impl Drop for ActionServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn policy() -> RegistrationPolicy {
    RegistrationPolicy::new(600, 60, 3600, 100, 10).unwrap()
}
fn network() -> NetworkKey {
    NetworkKey::from_peer("127.0.0.1".parse().unwrap(), &[1; 32]).unwrap()
}
fn issuer(
    events: Events,
    create_error: Option<RegistrationError>,
    setup_error: Option<RegistrationError>,
) -> Arc<dyn Issuer> {
    Arc::new(FakeIssuer {
        create_error,
        setup_error,
        create_waits: false,
        mismatch: false,
        events,
    })
}
pub(crate) fn service(
    memory: Arc<Memory>,
    server: &ActionServer,
    create_error: Option<RegistrationError>,
    setup_error: Option<RegistrationError>,
) -> Arc<FirebaseEnrollmentService> {
    let events = memory.events.clone();
    Arc::new(
        FirebaseEnrollmentService::new(
            memory,
            Arc::new(Mailbox {
                events: events.clone(),
                create_error: None,
                send_error: None,
            }),
            server.client(),
            issuer(events, create_error, setup_error),
            "candidate-project",
            policy(),
            "https://issuer.example/oauth2/openid/cantos-theatre-web-bff",
        )
        .unwrap(),
    )
}

#[test]
fn permanent_operation_fences_never_reopen_or_reuse_a_uid() {
    for phase in [
        Phase::BootstrapStarted,
        Phase::Ready,
        Phase::SendStarted,
        Phase::MailStarted,
        Phase::Consuming,
        Phase::Creating,
        Phase::Created,
        Phase::SetupStarted,
        Phase::HandedOff,
        Phase::Blocked,
    ] {
        assert!(!phase.may_have_successor(1000, 200));
        assert!(!phase.may_claim(100, 200));
        assert!(!phase.may_resend(1000, 200, 100, 60));
    }
    for phase in [
        Phase::Cancelled,
        Phase::Expired,
        Phase::HandedOff,
        Phase::Blocked,
    ] {
        assert!(!phase.can_advance(Phase::AwaitingAction));
    }
    let first = Enrollment::new(
        PendingRegistration::new(Uuid::from_u128(1), Email::parse("V@example.test").unwrap())
            .unwrap(),
        "candidate-project",
    )
    .unwrap();
    let next = Enrollment::new(
        PendingRegistration::new(Uuid::from_u128(2), Email::parse("V@example.test").unwrap())
            .unwrap(),
        "candidate-project",
    )
    .unwrap();
    assert_ne!(first.uid(), next.uid());
    assert_eq!(first.uid(), "cantos_00000000000000000000000000000001");
    assert!(Phase::AwaitingAction.may_resend(160, 200, 100, 60));
    assert!(!Phase::AwaitingAction.may_resend(200, 200, 100, 60));
}

#[test]
fn one_shot_setup_accepts_only_the_pinned_kanidm_token_destination() {
    let registration =
        PendingRegistration::new(Uuid::from_u128(1), Email::parse("V@example.test").unwrap())
            .unwrap();
    let origin = url::Url::parse("https://issuer.example/").unwrap();
    for link in [
        "https://other.example/ui/reset?token=x",
        "https://issuer.example/other?token=x",
        "https://issuer.example/ui/reset?token=x&next=evil",
        "https://issuer.example/ui/reset?token=x#leak",
        "https://issuer.example/ui/reset",
        "https://issuer.example/ui/reset?token=",
        "https://issuer.example/ui/reset?token=%0a",
        "https://issuer.example/ui/reset?token=+",
    ] {
        assert!(matches!(
            OneShotSetup::new(&registration, SetupLink::new(link.into()).unwrap(), &origin),
            Err(RegistrationError::InvalidState)
        ));
    }
    let setup = OneShotSetup::new(
        &registration,
        SetupLink::new("https://issuer.example/ui/reset?token=fixture".into()).unwrap(),
        &origin,
    )
    .unwrap();
    assert_eq!(setup.login(), registration.provider_name());
    assert_eq!(format!("{setup:?}"), "OneShotSetup([REDACTED])");
}

#[tokio::test]
async fn bootstrap_send_and_setup_follow_durable_effect_fences_and_never_replay() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::new(events.clone()));
    let server = ActionServer::start(memory.clone(), None).await;
    let service = service(memory.clone(), &server, None, None);
    let work = service
        .prepare("Victim@example.test", network())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*events.lock().unwrap(), ["prepare"]);
    service.dispatch(work).await.unwrap();
    let id = memory.current().enrollment.id();
    assert_eq!(memory.current().phase, Phase::AwaitingAction);
    assert!(!events.lock().unwrap().contains(&"create"));
    let ConfirmationOutcome::SetupReady(setup) =
        service.confirm(id, "actual-action").await.unwrap()
    else {
        panic!("setup absent")
    };
    assert_eq!(
        setup.login(),
        memory.current().enrollment.registration().provider_name()
    );
    assert_eq!(
        setup.link().as_str(),
        "https://issuer.example/ui/reset?token=fixture"
    );
    drop(setup); // Simulated lost response: no retained secret or reset/recovery path.
    assert!(matches!(
        service.confirm(id, "actual-action").await.unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    assert!(service
        .prepare("Victim@example.test", network())
        .await
        .unwrap()
        .is_none());
    assert_eq!(memory.current().phase, Phase::HandedOff);
    assert_eq!(
        *events.lock().unwrap(),
        [
            "prepare",
            "bootstrap",
            "ready",
            "send_fence",
            "exchange",
            "mail_fence",
            "send",
            "sent",
            "claim",
            "inspect",
            "consume_fence",
            "consume",
            "create_fence",
            "create",
            "created",
            "setup_fence",
            "setup",
            "handoff_fence",
            "claim",
            "prepare"
        ]
    );
    assert_eq!(
        *server.bodies.lock().unwrap(),
        [
            serde_json::json!({"oobCode":"actual-action"}),
            serde_json::json!({"oobCode":"actual-action"})
        ]
    );
}

#[tokio::test]
async fn rejected_or_uncertain_bootstrap_and_send_are_terminal_without_provisioning() {
    for (create_error, send_error, expected) in [
        (
            Some(RegistrationError::IssuerRejected),
            None,
            FailureStage::BootstrapRejected,
        ),
        (
            Some(RegistrationError::IssuerUncertain),
            None,
            FailureStage::BootstrapUncertain,
        ),
        (
            None,
            Some(RegistrationError::DeliveryUncertain),
            FailureStage::SendUncertain,
        ),
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let memory = Arc::new(Memory::new(events.clone()));
        let server = ActionServer::start(memory.clone(), None).await;
        let service = FirebaseEnrollmentService::new(
            memory.clone(),
            Arc::new(Mailbox {
                events: events.clone(),
                create_error,
                send_error,
            }),
            server.client(),
            issuer(events.clone(), None, None),
            "candidate-project",
            policy(),
            "https://issuer.example/oauth2/openid/cantos-theatre-web-bff",
        )
        .unwrap();
        let work = service
            .prepare("Victim@example.test", network())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            service.dispatch(work).await,
            Err(create_error.or(send_error).unwrap())
        );
        assert_eq!(memory.current().reason, Some(expected));
        memory.now.store(1000, Ordering::SeqCst);
        assert!(service
            .prepare("Victim@example.test", network())
            .await
            .unwrap()
            .is_none());
        assert!(!events.lock().unwrap().contains(&"create"));
    }
}

#[tokio::test]
async fn resend_preserves_uid_and_absolute_expiry_and_expired_attempt_gets_new_uid() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events));
    let server = ActionServer::start(memory.clone(), None).await;
    let service = service(memory.clone(), &server, None, None);
    let old = memory.current();
    memory.now.store(160, Ordering::SeqCst);
    let work = service
        .prepare("Victim@example.test", network())
        .await
        .unwrap()
        .unwrap();
    service.dispatch(work).await.unwrap();
    assert_eq!(memory.current().enrollment, old.enrollment);
    assert_eq!(memory.current().expires, old.expires);
    memory.now.store(200, Ordering::SeqCst);
    let _new_work = service
        .prepare("Victim@example.test", network())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(memory.current().enrollment.uid(), old.enrollment.uid());
}

#[tokio::test]
async fn successor_email_conflict_requires_reconciliation_without_adoption_or_provisioning() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events.clone()));
    let previous = memory.current().enrollment;
    memory.now.store(200, Ordering::SeqCst);
    let server = ActionServer::start(memory.clone(), None).await;
    let service = FirebaseEnrollmentService::new(
        memory.clone(),
        Arc::new(Mailbox {
            events: events.clone(),
            create_error: Some(RegistrationError::Conflict),
            send_error: None,
        }),
        server.client(),
        issuer(events.clone(), None, None),
        "candidate-project",
        policy(),
        "https://issuer.example/oauth2/openid/cantos-theatre-web-bff",
    )
    .unwrap();
    let work = service
        .prepare("Victim@example.test", network())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(memory.current().enrollment.uid(), previous.uid());
    assert_eq!(
        service.dispatch(work).await,
        Err(RegistrationError::Conflict)
    );
    assert_eq!(
        memory.current().reason,
        Some(FailureStage::BootstrapRejected)
    );
    assert!(service
        .prepare("Victim@example.test", network())
        .await
        .unwrap()
        .is_none());
    assert_eq!(*events.lock().unwrap(), ["prepare", "bootstrap", "prepare"]);
}

#[tokio::test]
async fn a_project_without_password_login_is_a_configuration_fault_not_an_unusable_link() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events.clone()));
    let server = ActionServer::start(memory.clone(), None).await;
    let service = service(memory.clone(), &server, None, None);
    server.provider_disabled.store(true, Ordering::SeqCst);
    assert!(matches!(
        service.confirm(Uuid::from_u128(1), "actual-action").await,
        Err(RegistrationError::Configuration)
    ));
    // Nothing consumed, nothing provisioned, and the lease is released for a later retry.
    assert_eq!(memory.current().phase, Phase::AwaitingAction);
    assert_eq!(
        *events.lock().unwrap(),
        ["claim", "inspect", "release_inspection"]
    );
    server.provider_disabled.store(false, Ordering::SeqCst);
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "actual-action")
            .await
            .unwrap(),
        ConfirmationOutcome::SetupReady(_)
    ));
}

#[tokio::test]
async fn wrong_purpose_releases_only_the_read_only_inspection_lease() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events.clone()));
    let server = ActionServer::start(memory.clone(), None).await;
    let service = service(memory.clone(), &server, None, None);
    server.wrong_purpose.store(true, Ordering::SeqCst);
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "wrong-action")
            .await
            .unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    assert_eq!(memory.current().phase, Phase::AwaitingAction);
    assert_eq!(
        *events.lock().unwrap(),
        ["claim", "inspect", "release_inspection"]
    );
    server.wrong_purpose.store(false, Ordering::SeqCst);
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "actual-action")
            .await
            .unwrap(),
        ConfirmationOutcome::SetupReady(_)
    ));
}

#[tokio::test]
async fn expiration_after_each_provider_wait_prevents_the_next_mutating_effect() {
    for held in [Held::Inspect, Held::Consume] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let memory = Arc::new(Memory::awaiting(events.clone()));
        let server = ActionServer::start(memory.clone(), Some(held)).await;
        let service = service(memory.clone(), &server, None, None);
        let runner = {
            let service = service.clone();
            tokio::spawn(async move { service.confirm(Uuid::from_u128(1), "actual-action").await })
        };
        server.reached.notified().await;
        memory.now.store(200, Ordering::SeqCst);
        server.release.notify_one();
        assert!(matches!(
            runner.await.unwrap().unwrap(),
            ConfirmationOutcome::Unavailable
        ));
        assert!(!events.lock().unwrap().contains(&"create"));
        if held == Held::Inspect {
            assert!(!events.lock().unwrap().contains(&"consume"));
        }
    }
}

#[tokio::test]
async fn dropped_consume_future_and_duplicate_confirm_do_not_repeat_or_provision() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events.clone()));
    let server = ActionServer::start(memory.clone(), Some(Held::Consume)).await;
    let service = service(memory.clone(), &server, None, None);
    let runner = {
        let service = service.clone();
        tokio::spawn(async move { service.confirm(Uuid::from_u128(1), "actual-action").await })
    };
    server.reached.notified().await;
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "other-action")
            .await
            .unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    runner.abort();
    assert!(runner.await.unwrap_err().is_cancelled());
    server.release.notify_one();
    assert_eq!(memory.current().phase, Phase::Consuming);
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "actual-action")
            .await
            .unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| **event == "consume")
            .count(),
        1
    );
    assert!(!events.lock().unwrap().contains(&"create"));
}

#[tokio::test]
async fn lost_consume_response_is_terminal_and_never_recovered_from_provider_status() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events.clone()));
    let server = ActionServer::start(memory.clone(), None).await;
    server.disconnect_consume.store(true, Ordering::SeqCst);
    let service = service(memory.clone(), &server, None, None);
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "actual-action")
            .await
            .unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    assert_eq!(
        memory.current().reason,
        Some(FailureStage::ConsumeUncertain)
    );
    assert!(matches!(
        service
            .confirm(Uuid::from_u128(1), "actual-action")
            .await
            .unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    assert!(!events.lock().unwrap().contains(&"create"));
}

#[tokio::test]
async fn uncertain_create_or_setup_and_failed_handoff_commit_never_reset_or_replay() {
    for (create_error, setup_error, handoff_fail, expected) in [
        (
            Some(RegistrationError::IssuerUncertain),
            None,
            false,
            Some(FailureStage::CreateUncertain),
        ),
        (
            None,
            Some(RegistrationError::IssuerUncertain),
            false,
            Some(FailureStage::SetupUncertain),
        ),
        (None, None, true, None),
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let memory = Arc::new(Memory::awaiting(events.clone()));
        memory
            .fail_handoff_commit
            .store(handoff_fail, Ordering::SeqCst);
        let server = ActionServer::start(memory.clone(), None).await;
        let service = service(memory.clone(), &server, create_error, setup_error);
        let first = service.confirm(Uuid::from_u128(1), "actual-action").await;
        if handoff_fail {
            assert!(matches!(first, Err(RegistrationError::Unavailable)));
        } else {
            assert!(matches!(first, Ok(ConfirmationOutcome::Unavailable)));
        }
        assert_eq!(memory.current().reason, expected);
        assert!(matches!(
            service
                .confirm(Uuid::from_u128(1), "actual-action")
                .await
                .unwrap(),
            ConfirmationOutcome::Unavailable
        ));
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| **event == "create")
                .count(),
            1
        );
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| **event == "setup")
                .count(),
            usize::from(create_error.is_none())
        );
    }
}

#[tokio::test]
async fn delayed_dispatch_after_expiry_does_not_start_bootstrap_or_resend() {
    for initial in [true, false] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let memory = Arc::new(if initial {
            Memory::new(events.clone())
        } else {
            Memory::awaiting(events.clone())
        });
        if !initial {
            memory.now.store(160, Ordering::SeqCst);
        }
        let server = ActionServer::start(memory.clone(), None).await;
        let service = service(memory.clone(), &server, None, None);
        let work = service
            .prepare("Victim@example.test", network())
            .await
            .unwrap()
            .unwrap();
        memory.now.store(memory.current().expires, Ordering::SeqCst);
        service.dispatch(work).await.unwrap();
        assert_eq!(*events.lock().unwrap(), ["prepare"]);
        assert!(server.bodies.lock().unwrap().is_empty());
    }
}

struct HeldExchangeMailbox {
    events: Events,
    reached: Arc<Notify>,
    release: Arc<Notify>,
}

impl MailboxProvider for HeldExchangeMailbox {
    fn create_unverified(&self, _: Enrollment) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            self.events.lock().unwrap().push("bootstrap");
            Ok(())
        })
    }

    fn prepare_verification(
        &self,
        enrollment: Enrollment,
    ) -> EnrollmentFuture<'_, VerificationCredential> {
        Box::pin(async move {
            self.events.lock().unwrap().push("exchange");
            self.reached.notify_one();
            self.release.notified().await;
            VerificationCredential::new(enrollment, "late-synthetic-id-token".into())
        })
    }

    fn send_verification(
        &self,
        _: Enrollment,
        _: VerificationCredential,
    ) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            self.events.lock().unwrap().push("send");
            Ok(())
        })
    }
}

#[tokio::test]
async fn exchange_completing_after_expiry_never_dispatches_initial_or_resend_mail() {
    for initial in [true, false] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let memory = Arc::new(if initial {
            Memory::new(events.clone())
        } else {
            Memory::awaiting(events.clone())
        });
        if !initial {
            memory.now.store(160, Ordering::SeqCst);
        }
        let reached = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let server = ActionServer::start(memory.clone(), None).await;
        let service = Arc::new(
            FirebaseEnrollmentService::new(
                memory.clone(),
                Arc::new(HeldExchangeMailbox {
                    events: events.clone(),
                    reached: reached.clone(),
                    release: release.clone(),
                }),
                server.client(),
                issuer(events.clone(), None, None),
                "candidate-project",
                policy(),
                "https://issuer.example/oauth2/openid/cantos-theatre-web-bff",
            )
            .unwrap(),
        );
        let work = service
            .prepare("Victim@example.test", network())
            .await
            .unwrap()
            .unwrap();
        let runner = {
            let service = service.clone();
            tokio::spawn(async move { service.dispatch(work).await })
        };
        reached.notified().await;
        assert_eq!(memory.phase(), Some(Phase::SendStarted));
        memory.now.store(memory.current().expires, Ordering::SeqCst);
        release.notify_one();
        runner.await.unwrap().unwrap();
        assert_eq!(memory.phase(), Some(Phase::SendStarted));
        assert!(events.lock().unwrap().contains(&"exchange"));
        for forbidden in ["send", "sent", "create", "setup"] {
            assert!(!events.lock().unwrap().contains(&forbidden));
        }
        assert!(service
            .prepare("Victim@example.test", network())
            .await
            .unwrap()
            .is_none());
        assert!(server.bodies.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn actual_action_cancels_durably_without_provisioning_or_replaying() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(Memory::awaiting(events.clone()));
    let server = ActionServer::start(memory.clone(), None).await;
    let service = service(memory.clone(), &server, None, None);
    let id = Uuid::from_u128(1);
    service.cancel(id, "").await.unwrap();
    assert_eq!(memory.current().phase, Phase::AwaitingAction);
    assert!(events.lock().unwrap().is_empty());
    server.wrong_purpose.store(true, Ordering::SeqCst);
    service.cancel(id, "not-email-verification").await.unwrap();
    assert_eq!(memory.current().phase, Phase::AwaitingAction);
    assert!(!events.lock().unwrap().contains(&"consume"));
    server.wrong_purpose.store(false, Ordering::SeqCst);
    service.cancel(id, "actual-action").await.unwrap();
    assert_eq!(memory.current().phase, Phase::Cancelled);
    assert!(matches!(
        service.confirm(id, "actual-action").await.unwrap(),
        ConfirmationOutcome::Unavailable
    ));
    service.cancel(id, "actual-action").await.unwrap();
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| **event == "consume")
            .count(),
        1
    );
    assert!(!events.lock().unwrap().contains(&"create"));
    assert!(!events.lock().unwrap().contains(&"setup"));
}

#[tokio::test]
async fn cancel_and_confirm_have_one_winner_before_action_consumption() {
    for cancel_wins in [true, false] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let memory = Arc::new(Memory::awaiting(events.clone()));
        let server = ActionServer::start(memory.clone(), Some(Held::Consume)).await;
        let service = service(memory.clone(), &server, None, None);
        let winner = {
            let service = service.clone();
            tokio::spawn(async move {
                if cancel_wins {
                    service
                        .cancel(Uuid::from_u128(1), "actual-action")
                        .await
                        .map(|()| None)
                } else {
                    service
                        .confirm(Uuid::from_u128(1), "actual-action")
                        .await
                        .map(Some)
                }
            })
        };
        server.reached.notified().await;
        if cancel_wins {
            assert!(matches!(
                service
                    .confirm(Uuid::from_u128(1), "actual-action")
                    .await
                    .unwrap(),
                ConfirmationOutcome::Unavailable
            ));
        } else {
            service
                .cancel(Uuid::from_u128(1), "actual-action")
                .await
                .unwrap();
        }
        server.release.notify_one();
        let result = winner.await.unwrap().unwrap();
        if cancel_wins {
            assert!(result.is_none());
            assert_eq!(memory.current().phase, Phase::Cancelled);
            assert!(!events.lock().unwrap().contains(&"create"));
        } else {
            assert!(matches!(result, Some(ConfirmationOutcome::SetupReady(_))));
            assert_eq!(memory.current().phase, Phase::HandedOff);
        }
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| **event == "consume")
                .count(),
            1
        );
    }
}
