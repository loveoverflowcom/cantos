use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
    task::JoinHandle,
};
use uuid::Uuid;

use super::*;
use crate::platform::theatre_registration::{
    domain::{Email, PendingRegistration},
    firebase_prototype::core::{BindingField, Phase, PollStatus},
};

const INSPECTION: &str = r#"{"email":"Victim@example.test","requestType":"VERIFY_EMAIL"}"#;
// Official confirmation samples may omit emailVerified. Successful consumption of the
// same checked action plus exact UID/email is the candidate witness; no bool-only path.
const CONSUMPTION: &str = r#"{"localId":"dedicated-new-uid","email":"Victim@example.test"}"#;
const CODE: &str = "synthetic-action-code";

#[derive(Debug)]
struct Captured {
    request_line: String,
    headers: String,
    body: Vec<u8>,
}

enum Reply {
    Http {
        status: u16,
        mime: &'static str,
        body: String,
        chunked: bool,
    },
    Disconnect,
}

struct Step {
    reply: Reply,
    gate: Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>,
}

impl Step {
    fn json(body: &str) -> Self {
        Self::http(200, "application/json", body)
    }
    fn http(status: u16, mime: &'static str, body: &str) -> Self {
        Self {
            reply: Reply::Http {
                status,
                mime,
                body: body.into(),
                chunked: false,
            },
            gate: None,
        }
    }
    fn held(mut self) -> (Self, oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (reached_tx, reached_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        self.gate = Some((reached_tx, release_rx));
        (self, reached_rx, release_tx)
    }
}

struct Fixture {
    address: SocketAddr,
    captured: Arc<Mutex<Vec<Captured>>>,
    task: JoinHandle<()>,
}

impl Fixture {
    async fn start(steps: Vec<Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = captured.clone();
        let task = tokio::spawn(async move {
            for step in steps {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut header_bytes = Vec::new();
                while !header_bytes.ends_with(b"\r\n\r\n") {
                    header_bytes.push(stream.read_u8().await.unwrap());
                    assert!(header_bytes.len() < 8192);
                }
                let headers = String::from_utf8(header_bytes).unwrap();
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                assert!(length < 8192);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).await.unwrap();
                sink.lock().unwrap().push(Captured {
                    request_line: headers.lines().next().unwrap().to_owned(),
                    headers,
                    body,
                });
                if let Some((reached, release)) = step.gate {
                    reached.send(()).unwrap();
                    let _ = release.await;
                }
                match step.reply {
                    Reply::Disconnect => {}
                    Reply::Http {
                        status,
                        mime,
                        body,
                        chunked,
                    } => {
                        let payload = if chunked {
                            format!("HTTP/1.1 {status} Fixture\r\nContent-Type: {mime}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len())
                        } else {
                            format!("HTTP/1.1 {status} Fixture\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nLocation: /must-not-follow\r\nConnection: close\r\n\r\n{body}", body.len())
                        };
                        // A timeout or dropped response is an intended fixture fault.
                        let _ = stream.write_all(payload.as_bytes()).await;
                    }
                }
            }
        });
        Self {
            address,
            captured,
            task,
        }
    }

    fn transport(&self) -> CandidateTransport {
        CandidateTransport::fixture(self.address, Duration::from_secs(2)).unwrap()
    }

    async fn finish(self) -> Vec<Captured> {
        self.task.await.unwrap();
        Arc::try_unwrap(self.captured)
            .unwrap()
            .into_inner()
            .unwrap()
    }
}

fn binding(epoch: u64) -> Binding {
    Binding::candidate(
        PendingRegistration::new(
            Uuid::from_u128(1),
            Email::parse("Victim@example.test").unwrap(),
        )
        .unwrap(),
        "candidate-project",
        "dedicated-new-uid",
        epoch,
        200,
    )
    .unwrap()
}

fn local() -> (Arc<LocalFence>, Binding, Arc<FixtureClock>) {
    let binding = binding(1);
    (
        Arc::new(LocalFence(Mutex::new(Record::new(binding.clone())))),
        binding,
        Arc::new(FixtureClock(AtomicU64::new(100))),
    )
}

async fn confirm(
    transport: &CandidateTransport,
    local: &LocalFence,
    binding: &Binding,
    clock: &FixtureClock,
) -> Result<ActionProof, ConfirmationError> {
    transport
        .confirm(local, binding, ActionCode::parse(CODE).unwrap(), clock)
        .await
}

#[tokio::test]
async fn same_purpose_checked_code_is_consumed_once_with_exact_official_rest_shapes() {
    let fixture = Fixture::start(vec![Step::json(INSPECTION), Step::json(CONSUMPTION)]).await;
    let (local, binding, clock) = local();
    let transport = fixture.transport();
    confirm(&transport, &local, &binding, &clock).await.unwrap();
    assert_eq!(
        core::poll(&local.0.lock().unwrap()),
        PollStatus::ActionAccepted
    );
    assert_eq!(
        confirm(&transport, &local, &binding, &clock).await,
        Err(ConfirmationError::BeforeConsume(Refusal::Consumed))
    );
    let captured = fixture.finish().await;
    assert_eq!(captured.len(), 2);
    assert_eq!(
        captured[0].request_line,
        "POST /v1/accounts:resetPassword?key=synthetic-fixture-key HTTP/1.1"
    );
    assert_eq!(
        captured[1].request_line,
        "POST /v1/accounts:update?key=synthetic-fixture-key HTTP/1.1"
    );
    assert_eq!(captured[0].body, br#"{"oobCode":"synthetic-action-code"}"#);
    assert_eq!(captured[0].body, captured[1].body);
    for request in captured {
        assert!(request
            .headers
            .to_ascii_lowercase()
            .contains("content-type: application/json"));
        assert!(!request
            .headers
            .to_ascii_lowercase()
            .contains("authorization:"));
    }
}

#[tokio::test]
async fn actual_inspection_cannot_be_consumed_by_another_equivalent_client() {
    let fixture = Fixture::start(vec![Step::json(INSPECTION)]).await;
    let original = fixture.transport();
    let other = fixture.transport();
    let local_binding = binding(1);
    let expected = ExpectedActionBinding::new(
        local_binding.registration().clone(),
        FirebaseProjectId::parse(local_binding.project()).unwrap(),
        DedicatedUid::parse(local_binding.uid()).unwrap(),
    );
    let inspected = original
        .0
        .inspect(expected, ActionCode::parse(CODE).unwrap())
        .await
        .unwrap();
    assert_eq!(
        other.0.consume(inspected).await,
        Err(ConsumptionError::BeforeConsume(ActionError::WrongClient))
    );
    assert_eq!(fixture.finish().await.len(), 1);
}

#[tokio::test]
async fn wrong_purpose_email_and_verified_bool_only_never_reach_consume() {
    for (reply, refusal) in [
        (
            r#"{"email":"Victim@example.test","requestType":"PASSWORD_RESET"}"#,
            Refusal::WrongPurpose,
        ),
        (
            r#"{"email":"victim@example.test","requestType":"VERIFY_EMAIL"}"#,
            Refusal::InvalidBinding(BindingField::Email),
        ),
        (
            r#"{"email":"Victim@example.test","requestType":"VERIFY_EMAIL","newEmail":"other@example.test"}"#,
            Refusal::InvalidBinding(BindingField::Email),
        ),
        (
            r#"{"email":"Victim@example.test","emailVerified":true}"#,
            Refusal::Protocol,
        ),
    ] {
        let fixture = Fixture::start(vec![Step::json(reply)]).await;
        let (local, binding, clock) = local();
        assert_eq!(
            confirm(&fixture.transport(), &local, &binding, &clock).await,
            Err(ConfirmationError::BeforeConsume(refusal))
        );
        assert_eq!(
            core::poll(&local.0.lock().unwrap()),
            PollStatus::ActionRequired
        );
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn malformed_hostile_and_provider_inspection_responses_stop_before_consume() {
    let mut chunked = Step::json(&"x".repeat(MAX_RESPONSE_BYTES + 1));
    if let Reply::Http { chunked: flag, .. } = &mut chunked.reply {
        *flag = true;
    }
    let cases = vec![
        (Step::json("{"), Refusal::Protocol),
        (
            Step::json(&"x".repeat(MAX_RESPONSE_BYTES + 1)),
            Refusal::TooLarge,
        ),
        (chunked, Refusal::TooLarge),
        (
            Step::http(200, "text/html", "<html>not proof</html>"),
            Refusal::NotJson,
        ),
        (Step::http(302, "application/json", "{}"), Refusal::Redirect),
        (Step::http(429, "application/json", "{}"), Refusal::Quota),
        (Step::http(503, "application/json", "{}"), Refusal::Provider),
        (
            Step::http(
                400,
                "application/json",
                r#"{"error":{"message":"EXPIRED_OOB_CODE"}}"#,
            ),
            Refusal::Expired,
        ),
        (
            Step::http(
                400,
                "application/json",
                r#"{"error":{"message":"INVALID_OOB_CODE"}}"#,
            ),
            Refusal::InvalidCode,
        ),
        (
            Step::http(
                400,
                "application/json",
                r#"{"error":{"message":"TOO_MANY_ATTEMPTS_TRY_LATER"}}"#,
            ),
            Refusal::Quota,
        ),
        (
            Step::http(
                403,
                "application/json",
                r#"{"error":{"message":"OPERATION_NOT_ALLOWED"}}"#,
            ),
            Refusal::Rejected,
        ),
    ];
    for (reply, refusal) in cases {
        let fixture = Fixture::start(vec![reply]).await;
        let (local, binding, clock) = local();
        assert_eq!(
            confirm(&fixture.transport(), &local, &binding, &clock).await,
            Err(ConfirmationError::BeforeConsume(refusal))
        );
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn consume_malformed_lost_provider_and_binding_failures_are_uncertain_without_replay() {
    let cases = vec![
        (Step::json("{"), Refusal::Protocol),
        (
            Step::json(&"x".repeat(MAX_RESPONSE_BYTES + 1)),
            Refusal::TooLarge,
        ),
        (Step::http(200, "text/html", "<html/>"), Refusal::NotJson),
        (Step::http(302, "application/json", "{}"), Refusal::Redirect),
        (Step::http(500, "application/json", "{}"), Refusal::Provider),
        (
            Step {
                reply: Reply::Disconnect,
                gate: None,
            },
            Refusal::Network,
        ),
        (
            Step::json(
                r#"{"localId":"existing-uid","email":"Victim@example.test","emailVerified":true}"#,
            ),
            Refusal::InvalidBinding(BindingField::Uid),
        ),
        (
            Step::json(
                r#"{"localId":"dedicated-new-uid","email":"other@example.test","emailVerified":true}"#,
            ),
            Refusal::InvalidBinding(BindingField::Email),
        ),
        (
            Step::json(
                r#"{"localId":"dedicated-new-uid","email":"Victim@example.test","emailVerified":false}"#,
            ),
            Refusal::Unverified,
        ),
        (
            Step::json(
                r#"{"localId":"dedicated-new-uid","email":"Victim@example.test","newEmail":"other@example.test"}"#,
            ),
            Refusal::InvalidBinding(BindingField::Email),
        ),
    ];
    for (reply, refusal) in cases {
        let fixture = Fixture::start(vec![Step::json(INSPECTION), reply]).await;
        let transport = fixture.transport();
        let (local, binding, clock) = local();
        assert_eq!(
            confirm(&transport, &local, &binding, &clock).await,
            Err(ConfirmationError::ConsumeUncertain(refusal))
        );
        assert_eq!(local.0.lock().unwrap().phase(), Phase::Uncertain);
        assert_eq!(
            confirm(&transport, &local, &binding, &clock).await,
            Err(ConfirmationError::BeforeConsume(Refusal::Uncertain))
        );
        assert_eq!(fixture.finish().await.len(), 2);
    }
}

#[tokio::test]
async fn local_fences_are_rechecked_after_inspection_and_after_consumption() {
    for consuming in [false, true] {
        for change in ["expiry", "cancel", "epoch"] {
            let (held, reached, release) =
                Step::json(if consuming { CONSUMPTION } else { INSPECTION }).held();
            let steps = if consuming {
                vec![Step::json(INSPECTION), held]
            } else {
                vec![held]
            };
            let fixture = Fixture::start(steps).await;
            let transport = fixture.transport();
            let (local, binding, clock) = local();
            let (task_local, task_clock, task_binding) =
                (local.clone(), clock.clone(), binding.clone());
            let task = tokio::spawn(async move {
                confirm(&transport, &task_local, &task_binding, &task_clock).await
            });
            reached.await.unwrap();
            let refusal = match change {
                "expiry" => {
                    clock.0.store(200, Ordering::SeqCst);
                    Refusal::Expired
                }
                "cancel" => {
                    let mut record = local.0.lock().unwrap();
                    *record = record.with_phase(Phase::Cancelled);
                    Refusal::Cancelled
                }
                "epoch" => {
                    let mut record = local.0.lock().unwrap();
                    let phase = record.phase();
                    *record = Record::new(super::fixture_tests::binding(2)).with_phase(phase);
                    Refusal::InvalidBinding(BindingField::Epoch)
                }
                _ => unreachable!(),
            };
            release.send(()).unwrap();
            let expected = if consuming {
                ConfirmationError::ConsumeUncertain(refusal)
            } else {
                ConfirmationError::BeforeConsume(refusal)
            };
            assert_eq!(task.await.unwrap(), Err(expected));
            assert_eq!(fixture.finish().await.len(), if consuming { 2 } else { 1 });
        }
    }
}

#[tokio::test]
async fn concurrent_duplicate_loses_local_reservation_without_second_request() {
    let (held, reached, release) = Step::json(INSPECTION).held();
    let fixture = Fixture::start(vec![held, Step::json(CONSUMPTION)]).await;
    let transport = Arc::new(fixture.transport());
    let (local, binding, clock) = local();
    let (task_transport, task_local, task_binding, task_clock) = (
        transport.clone(),
        local.clone(),
        binding.clone(),
        clock.clone(),
    );
    let first = tokio::spawn(async move {
        confirm(&task_transport, &task_local, &task_binding, &task_clock).await
    });
    reached.await.unwrap();
    assert_eq!(
        confirm(&transport, &local, &binding, &clock).await,
        Err(ConfirmationError::BeforeConsume(Refusal::InFlight))
    );
    release.send(()).unwrap();
    first.await.unwrap().unwrap();
    assert_eq!(fixture.finish().await.len(), 2);
}

#[tokio::test]
async fn timeouts_preserve_the_consuming_stage_and_never_replay() {
    for consuming in [false, true] {
        let (held, reached, release) =
            Step::json(if consuming { CONSUMPTION } else { INSPECTION }).held();
        let fixture = Fixture::start(if consuming {
            vec![Step::json(INSPECTION), held]
        } else {
            vec![held]
        })
        .await;
        let transport =
            CandidateTransport::fixture(fixture.address, Duration::from_millis(100)).unwrap();
        let (local, binding, clock) = local();
        let (task_local, task_binding, task_clock) =
            (local.clone(), binding.clone(), clock.clone());
        let task = tokio::spawn(async move {
            confirm(&transport, &task_local, &task_binding, &task_clock).await
        });
        reached.await.unwrap();
        assert_eq!(
            task.await.unwrap(),
            Err(if consuming {
                ConfirmationError::ConsumeUncertain(Refusal::Timeout)
            } else {
                ConfirmationError::BeforeConsume(Refusal::Timeout)
            })
        );
        if consuming {
            assert_eq!(local.0.lock().unwrap().phase(), Phase::Uncertain);
            assert_eq!(
                confirm(&fixture.transport(), &local, &binding, &clock).await,
                Err(ConfirmationError::BeforeConsume(Refusal::Uncertain))
            );
        } else {
            assert_eq!(local.0.lock().unwrap().phase(), Phase::AwaitingAction);
        }
        release.send(()).unwrap();
        assert_eq!(fixture.finish().await.len(), if consuming { 2 } else { 1 });
    }
}

#[tokio::test]
async fn dropping_future_during_inspection_keeps_the_local_fence_without_consuming() {
    let (held, reached, release) = Step::json(INSPECTION).held();
    let fixture = Fixture::start(vec![held]).await;
    let transport = fixture.transport();
    let (local, binding, clock) = local();
    let (task_local, task_binding, task_clock) = (local.clone(), binding.clone(), clock.clone());
    let task =
        tokio::spawn(
            async move { confirm(&transport, &task_local, &task_binding, &task_clock).await },
        );
    reached.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(local.0.lock().unwrap().phase(), Phase::Inspecting);
    assert_eq!(
        confirm(&fixture.transport(), &local, &binding, &clock).await,
        Err(ConfirmationError::BeforeConsume(Refusal::InFlight))
    );
    release.send(()).unwrap();
    assert_eq!(fixture.finish().await.len(), 1);
}

#[tokio::test]
async fn dropping_future_after_consumption_begins_keeps_the_local_reservation_fenced() {
    let (held, reached, release) = Step::json(CONSUMPTION).held();
    let fixture = Fixture::start(vec![Step::json(INSPECTION), held]).await;
    let transport = fixture.transport();
    let (local, binding, clock) = local();
    let (task_local, task_binding, task_clock) = (local.clone(), binding.clone(), clock.clone());
    let task =
        tokio::spawn(
            async move { confirm(&transport, &task_local, &task_binding, &task_clock).await },
        );
    reached.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(local.0.lock().unwrap().phase(), Phase::Consuming);
    assert_eq!(
        confirm(&fixture.transport(), &local, &binding, &clock).await,
        Err(ConfirmationError::BeforeConsume(Refusal::InFlight))
    );
    release.send(()).unwrap();
    assert_eq!(fixture.finish().await.len(), 2);
}

#[test]
fn literal_loopback_seam_and_action_code_refuse_unsafe_inputs() {
    assert_eq!(
        format!("{:?}", ActionCode::parse(CODE).unwrap()),
        "ActionCode([REDACTED])"
    );
    assert_eq!(
        CandidateTransport::fixture("192.0.2.1:1234".parse().unwrap(), Duration::from_secs(1))
            .err(),
        Some(Refusal::Protocol)
    );
    for invalid in ["", "contains whitespace", "line\nbreak"] {
        assert_eq!(
            ActionCode::parse(invalid).unwrap_err(),
            ActionError::InvalidCode
        );
    }
}
