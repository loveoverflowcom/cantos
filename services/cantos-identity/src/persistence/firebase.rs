//! Durable provider-effect fences beside the unchanged native mailbox proof transaction.
//! The database stores AES-GCM ciphertext, never native proof secrets or Firebase actions.
use super::{IdentityStore, ProofCommandId, ProofMaterial, SignupStoreError};
use crate::domain::{
    email::EmailAddress,
    ids::OperationId,
    mailbox_proof::{ProofId, ProofIntent, ProofState, ProofTime},
};
use cantos_store_turso::{params_from_iter, Row, Value, WriteTransaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum State {
    CreateReserved = 1,
    Created = 2,
    SendReserved = 3,
    Sent = 4,
    SendRejected = 5,
    SendUncertain = 6,
    CreateRejected = 7,
    CreateUncertain = 8,
    ConsumeReserved = 9,
    ReceiptReady = 10,
    Expired = 11,
    ConsumeRejected = 12,
}
impl State {
    fn parse(value: i64) -> Result<Self, SignupStoreError> {
        Ok(match value {
            1 => Self::CreateReserved,
            2 => Self::Created,
            3 => Self::SendReserved,
            4 => Self::Sent,
            5 => Self::SendRejected,
            6 => Self::SendUncertain,
            7 => Self::CreateRejected,
            8 => Self::CreateUncertain,
            9 => Self::ConsumeReserved,
            10 => Self::ReceiptReady,
            11 => Self::Expired,
            12 => Self::ConsumeRejected,
            _ => return Err(SignupStoreError::CorruptRecord),
        })
    }
}

pub struct Enrollment {
    pub operation: OperationId,
    pub proof: ProofId,
    pub email: EmailAddress,
    pub envelope: Vec<u8>,
    pub expires: u64,
    pub state: State,
    pub created: bool,
    pub last_send: u64,
    pub request_digest: Option<[u8; 32]>,
}
impl std::fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FirebaseEnrollment([REDACTED])")
    }
}
pub enum Start {
    Skip,
    Create(Enrollment),
    Send(Enrollment),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    Won,
    Ready,
    Pending,
    Denied,
}
const COLUMNS: &str =
    "operation_id,proof_id,email,envelope,expires_at,state,created,last_send,request_digest";
fn cell(row: &Row, index: usize) -> Result<Value, SignupStoreError> {
    row.get_value(index)
        .map_err(|_| SignupStoreError::CorruptRecord)
}
fn blob(row: &Row, index: usize) -> Result<Vec<u8>, SignupStoreError> {
    match cell(row, index)? {
        Value::Blob(value) => Ok(value),
        _ => Err(SignupStoreError::CorruptRecord),
    }
}
fn number(row: &Row, index: usize) -> Result<u64, SignupStoreError> {
    match cell(row, index)? {
        Value::Integer(value) => u64::try_from(value).map_err(|_| SignupStoreError::CorruptRecord),
        _ => Err(SignupStoreError::CorruptRecord),
    }
}
fn decode(row: Row) -> Result<Enrollment, SignupStoreError> {
    let bad = || SignupStoreError::CorruptRecord;
    let operation = OperationId::from_bytes(blob(&row, 0)?.try_into().map_err(|_| bad())?)
        .map_err(|_| bad())?;
    let proof =
        ProofId::from_bytes(blob(&row, 1)?.try_into().map_err(|_| bad())?).map_err(|_| bad())?;
    let email = match cell(&row, 2)? {
        Value::Text(value) => EmailAddress::parse(&value).map_err(|_| bad())?,
        _ => return Err(bad()),
    };
    let envelope = match cell(&row, 3)? {
        Value::Blob(value) if value.len() == 60 => value,
        Value::Null => vec![],
        _ => return Err(bad()),
    };
    let request_digest = match cell(&row, 8)? {
        Value::Blob(value) => Some(value.try_into().map_err(|_| bad())?),
        Value::Null => None,
        _ => return Err(bad()),
    };
    Ok(Enrollment {
        operation,
        proof,
        email,
        envelope,
        expires: number(&row, 4)?,
        state: State::parse(number(&row, 5)? as i64)?,
        created: match number(&row, 6)? {
            0 => false,
            1 => true,
            _ => return Err(bad()),
        },
        last_send: number(&row, 7)?,
        request_digest,
    })
}
async fn in_tx(
    tx: &WriteTransaction<'_>,
    project: &str,
    operation: OperationId,
) -> Result<Option<Enrollment>, SignupStoreError> {
    let mut rows=tx.query(&format!("SELECT {COLUMNS} FROM firebase_signup_enrollments WHERE operation_id=?1 AND project=?2"),params_from_iter([Value::Blob(operation.to_bytes().to_vec()),Value::Text(project.into())])).await?;
    rows.next().await?.map(decode).transpose()
}
impl IdentityStore {
    pub async fn firebase_enrollment(
        &self,
        project: &str,
        operation: OperationId,
    ) -> Result<Option<Enrollment>, SignupStoreError> {
        let reader = self.database.reader().await?;
        let mut rows=reader.query(&format!("SELECT {COLUMNS} FROM firebase_signup_enrollments WHERE operation_id=?1 AND project=?2"),params_from_iter([Value::Blob(operation.to_bytes().to_vec()),Value::Text(project.into())])).await?;
        rows.next().await?.map(decode).transpose()
    }
    /// Serializes mailbox uniqueness, native proof issue and the first provider create fence.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_firebase(
        &self,
        project: &str,
        command: ProofCommandId,
        operation: OperationId,
        email: &EmailAddress,
        material: &ProofMaterial,
        envelope: &[u8],
        expires: u64,
        cooldown: u32,
    ) -> Result<Start, SignupStoreError> {
        let tx = self.database.begin_write().await?;
        let now = Self::proof_now(&tx)?.seconds();
        let result=async{
            let mut rows=tx.query("SELECT subject_id FROM identity_emails WHERE email=?1",[email.as_str().to_owned()]).await?;
            if rows.next().await?.is_some(){return Ok(Start::Skip);}drop(rows);
            let mut slots=tx.query("SELECT operation_id FROM firebase_signup_mailboxes WHERE project=?1 AND email=?2",[project,email.as_str()]).await?;
            let slot=slots.next().await?.map(|row|blob(&row,0)).transpose()?;drop(slots);
            if let Some(slot)=slot {
                let operation=OperationId::from_bytes(slot.try_into().map_err(|_|SignupStoreError::CorruptRecord)?).map_err(|_|SignupStoreError::CorruptRecord)?;
                let record=in_tx(&tx,project,operation).await?.ok_or(SignupStoreError::CorruptRecord)?;
                if now>=record.expires {
                    // Do not reuse a mailbox with a new operation/UID: the emulator resolves
                    // VERIFY_EMAIL actions by email and cannot prove old-code invalidation.
                    tx.execute("UPDATE firebase_signup_enrollments SET state=11,envelope=NULL WHERE operation_id=?1",[operation.to_bytes().to_vec()]).await?;
                    return Ok(Start::Skip);
                } else {
                    if !record.created||matches!(record.state,State::ConsumeReserved|State::ReceiptReady|State::ConsumeRejected|State::SendReserved)||now.saturating_sub(record.last_send)<u64::from(cooldown){return Ok(Start::Skip);}
                    tx.execute("UPDATE firebase_signup_enrollments SET state=3,last_send=?2 WHERE operation_id=?1",params_from_iter([Value::Blob(operation.to_bytes().to_vec()),Value::Integer(now as i64)])).await?;
                    return Ok(Start::Send(record));
                }
            }
            if expires<=now||expires>now.saturating_add(900)||envelope.len()!=60{return Err(SignupStoreError::CorruptRecord);}
            self.begin_signup_in(&tx,command,operation,email,material,ProofTime::new(expires).map_err(|_|SignupStoreError::CorruptRecord)?).await?;
            tx.execute("INSERT INTO firebase_signup_enrollments(operation_id,proof_id,project,email,envelope,expires_at,state,created,last_send) VALUES(?1,?2,?3,?4,?5,?6,1,0,?7)",params_from_iter([Value::Blob(operation.to_bytes().to_vec()),Value::Blob(material.id().to_bytes().to_vec()),Value::Text(project.into()),Value::Text(email.as_str().into()),Value::Blob(envelope.to_vec()),Value::Integer(expires as i64),Value::Integer(now as i64)])).await?;
            tx.execute("INSERT INTO firebase_signup_mailboxes(project,email,operation_id) VALUES(?1,?2,?3)",params_from_iter([Value::Text(project.into()),Value::Text(email.as_str().into()),Value::Blob(operation.to_bytes().to_vec())])).await?;
            Ok(Start::Create(in_tx(&tx,project,operation).await?.ok_or(SignupStoreError::CorruptRecord)?))
        }.await;
        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                tx.rollback().await?;
                Err(error)
            }
        }
    }
    #[cfg(any(feature = "service", test))]
    pub(crate) async fn firebase_transition(
        &self,
        project: &str,
        operation: OperationId,
        expected: State,
        next: State,
        created: bool,
    ) -> Result<bool, SignupStoreError> {
        let permitted = matches!(
            (expected, next, created),
            (State::CreateReserved, State::Created, true)
                | (
                    State::CreateReserved,
                    State::CreateRejected | State::CreateUncertain,
                    false
                )
                | (State::Created, State::SendReserved, false)
                | (
                    State::SendReserved,
                    State::Sent | State::SendRejected | State::SendUncertain,
                    false
                )
                | (
                    State::ConsumeReserved,
                    State::ReceiptReady | State::ConsumeRejected,
                    false
                )
        );
        if !permitted {
            return Err(SignupStoreError::BindingChanged);
        }
        let tx = self.database.begin_write().await?;
        let record = in_tx(&tx, project, operation).await?;
        let matches = record.is_some_and(|record| {
            record.state == expected && (!created || expected == State::CreateReserved)
        });
        if !matches {
            tx.rollback().await?;
            return Ok(false);
        }
        tx.execute("UPDATE firebase_signup_enrollments SET state=?2,created=MAX(created,?3) WHERE operation_id=?1",params_from_iter([Value::Blob(operation.to_bytes().to_vec()),Value::Integer(next as i64),Value::Integer(i64::from(created))])).await?;
        tx.commit().await?;
        Ok(true)
    }
    /// Exactly one inspected action wins; neither a lost response nor a different password
    /// can reserve another consume. Ready permits only an identical request reconciliation.
    pub async fn claim_firebase(
        &self,
        project: &str,
        operation: OperationId,
        proof: ProofId,
        digest: [u8; 32],
    ) -> Result<Claim, SignupStoreError> {
        let tx = self.database.begin_write().await?;
        let record = in_tx(&tx, project, operation).await?;
        let Some(record) = record else {
            tx.rollback().await?;
            return Ok(Claim::Denied);
        };
        let native = self.workflow_binding_in_transaction(&tx, operation).await?;
        let pending = native.is_some_and(|native| {
            native.id() == proof
                && native.state() == ProofState::Pending
                && native.binding().epoch() == self.epoch
                && native.binding().intent() == ProofIntent::TheatreSignup
        });
        let result = if record.proof != proof
            || !record.created
            || Self::proof_now(&tx)?.seconds() >= record.expires
        {
            Claim::Denied
        } else if record.request_digest == Some(digest) {
            match record.state {
                State::ReceiptReady => Claim::Ready,
                State::ConsumeReserved => Claim::Pending,
                _ => Claim::Denied,
            }
        } else if pending
            && record.request_digest.is_none()
            && matches!(
                record.state,
                State::Created | State::Sent | State::SendRejected | State::SendUncertain
            )
        {
            tx.execute("UPDATE firebase_signup_enrollments SET state=9,request_digest=?2 WHERE operation_id=?1",params_from_iter([Value::Blob(operation.to_bytes().to_vec()),Value::Blob(digest.to_vec())])).await?;
            Claim::Won
        } else {
            Claim::Denied
        };
        tx.commit().await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_store_turso::{
        registry::Intent,
        testing::{FixedClock, TestRun},
    };
    use std::sync::Arc;
    const NOW: u64 = 1_700_000_000;
    fn op(value: u8) -> OperationId {
        OperationId::from_bytes([value; 16]).unwrap()
    }
    fn cmd(value: u8) -> ProofCommandId {
        ProofCommandId::from_bytes([value; 16]).unwrap()
    }
    async fn open_store(run: &TestRun, clock: Arc<FixedClock>) -> Arc<IdentityStore> {
        Arc::new(
            IdentityStore::open(run.path("firebase.db"), Intent::Migrate, clock)
                .await
                .unwrap(),
        )
    }
    async fn start(store: &IdentityStore, value: u8, email: &EmailAddress) -> Start {
        let material = ProofMaterial::generate().unwrap();
        store
            .start_firebase(
                "demo-cantos-storage",
                cmd(value),
                op(value),
                email,
                &material,
                &[5; 60],
                NOW + 900,
                60,
            )
            .await
            .unwrap()
    }
    async fn sent(store: &IdentityStore, record: &Enrollment) {
        assert!(store
            .firebase_transition(
                "demo-cantos-storage",
                record.operation,
                State::CreateReserved,
                State::Created,
                true
            )
            .await
            .unwrap());
        assert!(store
            .firebase_transition(
                "demo-cantos-storage",
                record.operation,
                State::Created,
                State::SendReserved,
                false
            )
            .await
            .unwrap());
        assert!(store
            .firebase_transition(
                "demo-cantos-storage",
                record.operation,
                State::SendReserved,
                State::Sent,
                false
            )
            .await
            .unwrap());
    }
    #[tokio::test]
    async fn resend_reuses_only_a_positively_created_uid_and_keeps_the_absolute_expiry() {
        let run = TestRun::new("firebase-resend");
        let clock = FixedClock::new((NOW * 1_000_000) as i64);
        let store = open_store(&run, clock.clone()).await;
        let email = EmailAddress::parse("owner@example.test").unwrap();
        let Start::Create(first) = start(&store, 1, &email).await else {
            panic!("first request reserves a create");
        };
        clock.set(((NOW + 60) * 1_000_000) as i64);
        assert!(
            matches!(start(&store, 2, &email).await, Start::Skip),
            "an unacknowledged create cannot be adopted"
        );
        sent(&store, &first).await;
        let Start::Send(resent) = start(&store, 3, &email).await else {
            panic!("cooled explicit resend reserves one send");
        };
        assert_eq!(resent.operation, first.operation);
        assert_eq!(resent.proof, first.proof);
        assert_eq!(resent.expires, NOW + 900);
        assert!(
            matches!(start(&store, 4, &email).await, Start::Skip),
            "a reserved send is never automatically repeated"
        );
    }
    #[tokio::test]
    async fn consumption_has_one_durable_winner_and_only_an_identical_acknowledged_receipt_reconciles(
    ) {
        let run = TestRun::new("firebase-claim");
        let clock = FixedClock::new((NOW * 1_000_000) as i64);
        let store = open_store(&run, clock.clone()).await;
        let email = EmailAddress::parse("owner@example.test").unwrap();
        let Start::Create(record) = start(&store, 1, &email).await else {
            panic!("create");
        };
        assert_eq!(
            store
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    [1; 32]
                )
                .await
                .unwrap(),
            Claim::Denied
        );
        sent(&store, &record).await;
        let (one, two) = tokio::join!(
            store.claim_firebase(
                "demo-cantos-storage",
                record.operation,
                record.proof,
                [1; 32]
            ),
            store.claim_firebase(
                "demo-cantos-storage",
                record.operation,
                record.proof,
                [2; 32]
            )
        );
        let (one, two) = (one.unwrap(), two.unwrap());
        assert!(matches!(
            (one, two),
            (Claim::Won, Claim::Denied) | (Claim::Denied, Claim::Won)
        ));
        let winner = if one == Claim::Won { [1; 32] } else { [2; 32] };
        drop(store);
        let reopened = open_store(&run, clock.clone()).await;
        assert_eq!(
            reopened
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    winner
                )
                .await
                .unwrap(),
            Claim::Pending,
            "lost consume response never becomes confirmed by lookup"
        );
        assert!(reopened
            .firebase_transition(
                "demo-cantos-storage",
                record.operation,
                State::ConsumeReserved,
                State::ReceiptReady,
                false
            )
            .await
            .unwrap());
        assert_eq!(
            reopened
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    winner
                )
                .await
                .unwrap(),
            Claim::Ready
        );
        assert_eq!(
            reopened
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    [9; 32]
                )
                .await
                .unwrap(),
            Claim::Denied
        );
        clock.set(((NOW + 900) * 1_000_000) as i64);
        assert_eq!(
            reopened
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    winner
                )
                .await
                .unwrap(),
            Claim::Denied
        );
        assert!(
            reopened
                .subject(crate::domain::ids::SubjectId::from_bytes([1; 16]).unwrap())
                .await
                .unwrap()
                .is_none(),
            "provider receipts alone create no subject"
        );
    }
    #[tokio::test]
    async fn acknowledged_consume_rejection_is_terminal_and_never_becomes_uncertain_or_resends() {
        let run = TestRun::new("firebase-rejected");
        let clock = FixedClock::new((NOW * 1_000_000) as i64);
        let store = open_store(&run, clock.clone()).await;
        let email = EmailAddress::parse("rejected@example.test").unwrap();
        let Start::Create(record) = start(&store, 8, &email).await else {
            panic!("new enrollment")
        };
        assert!(store
            .firebase_transition(
                "demo-cantos-storage",
                record.operation,
                State::CreateReserved,
                State::Created,
                true
            )
            .await
            .unwrap());
        assert_eq!(
            store
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    [8; 32]
                )
                .await
                .unwrap(),
            Claim::Won
        );
        assert!(store
            .firebase_transition(
                "demo-cantos-storage",
                record.operation,
                State::ConsumeReserved,
                State::ConsumeRejected,
                false
            )
            .await
            .unwrap());
        assert_eq!(
            store
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    [8; 32]
                )
                .await
                .unwrap(),
            Claim::Denied
        );
        clock.set(((NOW + 61) * 1_000_000) as i64);
        assert!(matches!(start(&store, 9, &email).await, Start::Skip));
    }
    #[tokio::test]
    async fn expired_enrollment_is_terminal_and_never_rebinds_an_old_firebase_action_to_a_new_operation(
    ) {
        let run = TestRun::new("firebase-expiry");
        let clock = FixedClock::new((NOW * 1_000_000) as i64);
        let store = open_store(&run, clock.clone()).await;
        let email = EmailAddress::parse("owner@example.test").unwrap();
        let Start::Create(record) = start(&store, 1, &email).await else {
            panic!("create");
        };
        sent(&store, &record).await;
        clock.set(((NOW + 900) * 1_000_000) as i64);
        let material = ProofMaterial::generate().unwrap();
        assert!(matches!(
            store
                .start_firebase(
                    "demo-cantos-storage",
                    cmd(2),
                    op(2),
                    &email,
                    &material,
                    &[6; 60],
                    NOW + 1800,
                    60
                )
                .await
                .unwrap(),
            Start::Skip
        ));
        let old = store
            .firebase_enrollment("demo-cantos-storage", record.operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(old.state, State::Expired);
        assert!(old.envelope.is_empty());
        let old_proof = store.signup(record.operation).await.unwrap().unwrap().proof;
        assert_eq!(old_proof.id(), record.proof);
        assert_eq!(old_proof.expires_at().seconds(), NOW + 900);
        assert!(store.signup(op(2)).await.unwrap().is_none());
        assert_eq!(
            store
                .claim_firebase(
                    "demo-cantos-storage",
                    record.operation,
                    record.proof,
                    [8; 32]
                )
                .await
                .unwrap(),
            Claim::Denied
        );
    }
}
