//! Internal challenge persistence. Receipts are historical records, never setup capabilities.

use std::fmt;

use cantos_store_turso::{
    error::{CommitFault, StoreFault},
    params_from_iter, Row, Value, WriteTransaction,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use super::{IdentityStore, IdentityStoreError, SUBJECT_QUERY};
use crate::domain::{
    admission::{AccountState, Purpose},
    ids::{InstanceEpoch, NilIdentifier, OperationId, SubjectId},
    mailbox_proof::{
        MailboxProof, ProofAction, ProofBinding, ProofDenied, ProofId, ProofIntent, ProofState,
        ProofTime, ProofTransition,
    },
    security_version::SecurityVersion,
};

const CURRENT_QUERY: &str = "SELECT w.workflow_id,w.instance_epoch,w.intent,w.recovery_subject,w.recovery_version,w.purpose,i.proof_id,i.issued_at,i.expires_at,w.state,i.secret_digest FROM mailbox_proof_workflows w LEFT JOIN mailbox_proof_issues i ON i.proof_id=w.current_proof_id AND i.workflow_id=w.workflow_id WHERE w.workflow_id=?1";
const RECEIPT_QUERY: &str = "SELECT r.command_id,r.action,r.request_digest,w.workflow_id,w.instance_epoch,w.intent,w.recovery_subject,w.recovery_version,w.purpose,i.proof_id,i.issued_at,i.expires_at,r.result_state,i.secret_digest FROM mailbox_proof_receipts r LEFT JOIN mailbox_proof_workflows w ON w.workflow_id=r.workflow_id LEFT JOIN mailbox_proof_issues i ON i.proof_id=r.result_proof_id AND i.workflow_id=r.workflow_id WHERE r.command_id=?1";

/// Proof mutation namespace, deliberately distinct from security OperationId/workflow IDs.
///
/// ```compile_fail
/// use cantos_identity::{domain::ids::OperationId, persistence::ProofCommandId};
/// let workflow = OperationId::from_bytes([1; 16]).unwrap();
/// let command: ProofCommandId = workflow;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProofCommandId(u128);

impl ProofCommandId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Result<Self, NilIdentifier> {
        let value = u128::from_be_bytes(bytes);
        if value == 0 {
            return Err(NilIdentifier);
        }
        Ok(Self(value))
    }

    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

/// Received/generated proof bytes. Exposure is explicit; Debug never prints their contents.
///
/// ```compile_fail
/// use cantos_identity::persistence::ProofSecret;
/// let received = ProofSecret::from_bytes([1; 32]);
/// let copy = received.clone();
/// ```
pub struct ProofSecret(Zeroizing<[u8; 32]>);

impl ProofSecret {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"cantos-identity-mailbox-secret-v1\0");
        hash.update(self.expose());
        hash.finalize().into()
    }
}

impl fmt::Debug for ProofSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProofSecret([REDACTED])")
    }
}

/// Retain this material across unknown outcomes. Retrying must not mint a new secret/locator.
pub struct ProofMaterial {
    id: ProofId,
    secret: ProofSecret,
}

impl ProofMaterial {
    pub fn generate() -> Result<Self, ProofStoreError> {
        let mut locator = [0; 16];
        let mut secret = Zeroizing::new([0; 32]);
        getrandom::fill(&mut locator).map_err(|_| ProofStoreError::EntropyUnavailable)?;
        getrandom::fill(secret.as_mut()).map_err(|_| ProofStoreError::EntropyUnavailable)?;
        let id = ProofId::from_bytes(locator).map_err(|_| ProofStoreError::EntropyUnavailable)?;
        Ok(Self {
            id,
            secret: ProofSecret(secret),
        })
    }

    pub const fn id(&self) -> ProofId {
        self.id
    }
    pub fn secret(&self) -> &ProofSecret {
        &self.secret
    }
}

impl fmt::Debug for ProofMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProofMaterial([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofReceiptAction {
    Issued,
    Confirmed,
    Cancelled,
    Replaced,
}

impl ProofReceiptAction {
    fn integer(self) -> i64 {
        match self {
            Self::Issued => 1,
            Self::Confirmed => 2,
            Self::Cancelled => 3,
            Self::Replaced => 4,
        }
    }
}

/// A historical challenge outcome. This carries no current verified/setup/login assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofReceipt {
    pub command: ProofCommandId,
    pub action: ProofReceiptAction,
    pub proof: MailboxProof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProofStoreError {
    Identity(IdentityStoreError),
    Storage(StoreFault),
    Commit(CommitFault),
    Denied(ProofDenied),
    Missing,
    Exists,
    Conflict,
    OperationReused,
    InvalidSecret,
    EpochChanged,
    CorruptRecord,
    EntropyUnavailable,
}

impl fmt::Display for ProofStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity(_) => "identity security state unavailable for mailbox operation",
            Self::Storage(_) => "mailbox proof storage unavailable",
            Self::Commit(CommitFault::Rejected(_)) => "mailbox proof commit rejected",
            Self::Commit(CommitFault::Unknown(_)) => {
                "mailbox proof outcome unknown; reconcile receipt"
            }
            Self::Denied(_) => "mailbox proof transition denied",
            Self::Missing => "mailbox workflow missing",
            Self::Exists => "mailbox workflow exists",
            Self::Conflict => "mailbox proof snapshot changed",
            Self::OperationReused => "mailbox command binding changed",
            Self::InvalidSecret => "mailbox proof secret rejected",
            Self::EpochChanged => "mailbox operation instance changed",
            Self::CorruptRecord => "invalid mailbox proof storage record",
            Self::EntropyUnavailable => "mailbox proof randomness unavailable",
        })
    }
}

impl std::error::Error for ProofStoreError {}
impl From<StoreFault> for ProofStoreError {
    fn from(value: StoreFault) -> Self {
        Self::Storage(value)
    }
}
impl From<CommitFault> for ProofStoreError {
    fn from(value: CommitFault) -> Self {
        Self::Commit(value)
    }
}
impl From<IdentityStoreError> for ProofStoreError {
    fn from(value: IdentityStoreError) -> Self {
        Self::Identity(value)
    }
}
impl From<ProofDenied> for ProofStoreError {
    fn from(value: ProofDenied) -> Self {
        Self::Denied(value)
    }
}

struct StoredProof {
    proof: MailboxProof,
    digest: [u8; 32],
}

impl IdentityStore {
    pub async fn mailbox_proof(
        &self,
        workflow: OperationId,
    ) -> Result<Option<MailboxProof>, ProofStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(CURRENT_QUERY, [workflow.to_bytes().to_vec()])
            .await?;
        rows.next()
            .await?
            .map(|row| decode_proof(&row, 0).map(|record| record.proof))
            .transpose()
    }

    /// Whether `secret` is the secret behind this exact locator of this workflow. A read-only
    /// pre-check that lets a caller refuse a wrong proof BEFORE spending KDF work; it consumes
    /// nothing and the authoritative comparison is repeated inside the completing transaction.
    pub async fn proof_secret_matches(
        &self,
        workflow: OperationId,
        id: ProofId,
        secret: &ProofSecret,
    ) -> Result<bool, ProofStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(
                "SELECT secret_digest FROM mailbox_proof_issues WHERE proof_id=?1 AND workflow_id=?2",
                [id.to_bytes().to_vec(), workflow.to_bytes().to_vec()],
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(false);
        };
        let stored: [u8; 32] = bytes(&row, 0)?;
        Ok(bool::from(stored.ct_eq(&secret.digest())))
    }

    pub async fn mailbox_receipt(
        &self,
        command: ProofCommandId,
    ) -> Result<Option<ProofReceipt>, ProofStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(RECEIPT_QUERY, [command.to_bytes().to_vec()])
            .await?;
        rows.next()
            .await?
            .map(|row| decode_receipt(&row).map(|(_, receipt)| receipt))
            .transpose()
    }

    pub async fn issue_mailbox_proof(
        &self,
        command: ProofCommandId,
        binding: ProofBinding,
        material: &ProofMaterial,
        expires: ProofTime,
    ) -> Result<ProofReceipt, ProofStoreError> {
        self.check_proof_epoch(binding)?;
        let tx = self.database.begin_write().await?;
        let result = self
            .issue_mailbox_proof_in_transaction(&tx, command, binding, material, expires)
            .await;
        finish(tx, result).await
    }

    /// Composition primitive for a workflow that must commit extra immutable binding rows with
    /// its first proof. `tx` must originate from this store's database.
    pub(crate) async fn issue_mailbox_proof_in_transaction(
        &self,
        tx: &WriteTransaction<'_>,
        command: ProofCommandId,
        binding: ProofBinding,
        material: &ProofMaterial,
        expires: ProofTime,
    ) -> Result<ProofReceipt, ProofStoreError> {
        self.check_proof_epoch(binding)?;
        let digest = material.secret.digest();
        let request = fingerprint(
            ProofReceiptAction::Issued,
            binding,
            None,
            digest,
            Some(material.id),
            expires.seconds(),
            0,
            0,
        );
        if let Some(receipt) = replay(tx, command, request).await? {
            return Ok(receipt);
        }
        check_recovery(tx, binding).await?;
        if current(tx, binding.operation()).await?.is_some() {
            return Err(ProofStoreError::Exists);
        }
        let proof = MailboxProof::issue(material.id, binding, now(tx)?, expires)?;
        insert_issue(tx, proof, digest).await?;
        let mut values = binding_values(binding);
        values.push(Value::Blob(material.id.to_bytes().to_vec()));
        tx.execute("INSERT INTO mailbox_proof_workflows(workflow_id,instance_epoch,intent,recovery_subject,recovery_version,purpose,current_proof_id,state) VALUES(?1,?2,?3,?4,?5,?6,?7,1)", params_from_iter(values)).await?;
        record(tx, command, ProofReceiptAction::Issued, request, proof).await
    }

    /// Current binding of a workflow, read inside the caller's transaction. The binding is
    /// authoritative stored state, never a decoded request.
    pub(crate) async fn workflow_binding_in_transaction(
        &self,
        tx: &WriteTransaction<'_>,
        workflow: OperationId,
    ) -> Result<Option<MailboxProof>, ProofStoreError> {
        Ok(current(tx, workflow).await?.map(|stored| stored.proof))
    }

    /// Commits challenge consumption only. It must never be used as a setup/login permit.
    pub async fn confirm_mailbox_proof(
        &self,
        command: ProofCommandId,
        binding: ProofBinding,
        id: ProofId,
        secret: &ProofSecret,
        action: ProofAction,
    ) -> Result<ProofReceipt, ProofStoreError> {
        self.check_proof_epoch(binding)?;
        let tx = self.database.begin_write().await?;
        let result = self
            .confirm_mailbox_proof_in_transaction(&tx, command, binding, id, secret, action)
            .await;
        finish(tx, result).await
    }

    /// Future setup must compose this transition with its authorized effect in this SAME tx.
    /// No caller, HTTP action or setup authority is established by this storage primitive.
    /// `tx` must originate from this IdentityStore's database. The shared transaction handle
    /// does not encode database identity in its type; crate callers own that composition.
    pub(crate) async fn confirm_mailbox_proof_in_transaction(
        &self,
        tx: &WriteTransaction<'_>,
        command: ProofCommandId,
        binding: ProofBinding,
        id: ProofId,
        secret: &ProofSecret,
        action: ProofAction,
    ) -> Result<ProofReceipt, ProofStoreError> {
        self.check_proof_epoch(binding)?;
        let digest = secret.digest();
        let explicit = match action {
            ProofAction::Inspect => 0,
            ProofAction::ExplicitConfirmation => 1,
        };
        let request = fingerprint(
            ProofReceiptAction::Confirmed,
            binding,
            Some(id),
            digest,
            None,
            0,
            0,
            explicit,
        );
        if let Some(receipt) = replay(tx, command, request).await? {
            return Ok(receipt);
        }
        if action != ProofAction::ExplicitConfirmation {
            return Err(ProofDenied::NotExplicitConfirmation.into());
        }
        check_recovery(tx, binding).await?;
        let stored = required_current(tx, binding, id).await?;
        if !bool::from(stored.digest.ct_eq(&digest)) {
            return Err(ProofStoreError::InvalidSecret);
        }
        let plan = stored
            .proof
            .plan_confirmation(id, binding, action, now(tx)?)?;
        apply(tx, &stored, plan).await?;
        record(
            tx,
            command,
            ProofReceiptAction::Confirmed,
            request,
            plan.next(),
        )
        .await
    }

    /// Internal owner cancellation; possession/caller authorization belongs to its future shell.
    pub async fn cancel_mailbox_proof(
        &self,
        command: ProofCommandId,
        binding: ProofBinding,
        id: ProofId,
    ) -> Result<ProofReceipt, ProofStoreError> {
        self.check_proof_epoch(binding)?;
        let request = fingerprint(
            ProofReceiptAction::Cancelled,
            binding,
            Some(id),
            [0; 32],
            None,
            0,
            0,
            0,
        );
        let tx = self.database.begin_write().await?;
        let result = async {
            if let Some(receipt) = replay(&tx, command, request).await? {
                return Ok(receipt);
            }
            check_recovery(&tx, binding).await?;
            let stored = required_current(&tx, binding, id).await?;
            let plan = stored.proof.plan_cancellation(id, binding)?;
            apply(&tx, &stored, plan).await?;
            record(
                &tx,
                command,
                ProofReceiptAction::Cancelled,
                request,
                plan.next(),
            )
            .await
        }
        .await;
        finish(tx, result).await
    }

    pub async fn replace_mailbox_proof(
        &self,
        command: ProofCommandId,
        binding: ProofBinding,
        expected_id: ProofId,
        material: &ProofMaterial,
        expires: ProofTime,
        cooldown: u32,
    ) -> Result<ProofReceipt, ProofStoreError> {
        self.check_proof_epoch(binding)?;
        let digest = material.secret.digest();
        let request = fingerprint(
            ProofReceiptAction::Replaced,
            binding,
            Some(expected_id),
            digest,
            Some(material.id),
            expires.seconds(),
            cooldown,
            0,
        );
        let tx = self.database.begin_write().await?;
        let result = async {
            if let Some(receipt) = replay(&tx, command, request).await? {
                return Ok(receipt);
            }
            check_recovery(&tx, binding).await?;
            let stored = required_current(&tx, binding, expected_id).await?;
            let plan = stored.proof.plan_replacement(
                expected_id,
                binding,
                material.id,
                now(&tx)?,
                expires,
                cooldown,
            )?;
            insert_issue(&tx, plan.next(), digest).await?;
            apply(&tx, &stored, plan).await?;
            record(
                &tx,
                command,
                ProofReceiptAction::Replaced,
                request,
                plan.next(),
            )
            .await
        }
        .await;
        finish(tx, result).await
    }

    fn check_proof_epoch(&self, binding: ProofBinding) -> Result<(), ProofStoreError> {
        if binding.epoch() != self.epoch {
            return Err(ProofStoreError::EpochChanged);
        }
        Ok(())
    }
}

fn now(tx: &WriteTransaction<'_>) -> Result<ProofTime, ProofStoreError> {
    ProofTime::new((tx.now().get() / 1_000_000) as u64).map_err(|_| ProofStoreError::CorruptRecord)
}

fn binding_values(binding: ProofBinding) -> Vec<Value> {
    let mut values = vec![
        Value::Blob(binding.operation().to_bytes().to_vec()),
        Value::Blob(binding.epoch().to_bytes().to_vec()),
    ];
    match binding.intent() {
        ProofIntent::TheatreSignup => {
            values.extend([Value::Integer(1), Value::Null, Value::Null, Value::Null])
        }
        ProofIntent::CredentialRecovery {
            subject,
            version,
            purpose,
        } => values.extend([
            Value::Integer(2),
            Value::Blob(subject.to_bytes().to_vec()),
            Value::Integer(version.get() as i64),
            Value::Integer(match purpose {
                Purpose::Theatre => 1,
                Purpose::Studio => 2,
            }),
        ]),
    }
    values
}

fn state_integer(state: ProofState) -> i64 {
    match state {
        ProofState::Pending => 1,
        ProofState::Consumed => 2,
        ProofState::Cancelled => 3,
    }
}

async fn current(
    tx: &WriteTransaction<'_>,
    workflow: OperationId,
) -> Result<Option<StoredProof>, ProofStoreError> {
    let mut rows = tx
        .query(CURRENT_QUERY, [workflow.to_bytes().to_vec()])
        .await?;
    rows.next()
        .await?
        .map(|row| decode_proof(&row, 0))
        .transpose()
}

async fn required_current(
    tx: &WriteTransaction<'_>,
    binding: ProofBinding,
    id: ProofId,
) -> Result<StoredProof, ProofStoreError> {
    let stored = current(tx, binding.operation())
        .await?
        .ok_or(ProofStoreError::Missing)?;
    if stored.proof.binding() != binding {
        return Err(ProofDenied::BindingChanged.into());
    }
    if stored.proof.id() != id {
        return Err(ProofDenied::ProofChanged.into());
    }
    Ok(stored)
}

async fn check_recovery(
    tx: &WriteTransaction<'_>,
    binding: ProofBinding,
) -> Result<(), ProofStoreError> {
    if let ProofIntent::CredentialRecovery {
        subject, version, ..
    } = binding.intent()
    {
        let mut rows = tx
            .query(SUBJECT_QUERY, [subject.to_bytes().to_vec()])
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or(IdentityStoreError::SubjectMissing)?;
        let stored = super::rows::decode_subject(row, binding.epoch())?;
        if stored.state != AccountState::Active {
            return Err(IdentityStoreError::Inactive.into());
        }
        if stored.version != version {
            return Err(IdentityStoreError::Conflict.into());
        }
    }
    Ok(())
}

async fn insert_issue(
    tx: &WriteTransaction<'_>,
    proof: MailboxProof,
    digest: [u8; 32],
) -> Result<(), ProofStoreError> {
    tx.execute("INSERT INTO mailbox_proof_issues(proof_id,workflow_id,secret_digest,issued_at,expires_at) VALUES(?1,?2,?3,?4,?5)", params_from_iter([
        Value::Blob(proof.id().to_bytes().to_vec()), Value::Blob(proof.binding().operation().to_bytes().to_vec()), Value::Blob(digest.to_vec()),
        Value::Integer(proof.issued_at().seconds() as i64), Value::Integer(proof.expires_at().seconds() as i64),
    ])).await?;
    Ok(())
}

async fn apply(
    tx: &WriteTransaction<'_>,
    stored: &StoredProof,
    plan: ProofTransition,
) -> Result<(), ProofStoreError> {
    if stored.proof != plan.expected() {
        return Err(ProofStoreError::Conflict);
    }
    let expected = plan.expected();
    let mut values = vec![
        Value::Blob(plan.next().id().to_bytes().to_vec()),
        Value::Integer(state_integer(plan.next().state())),
    ];
    values.extend(binding_values(expected.binding()));
    values.extend([
        Value::Blob(expected.id().to_bytes().to_vec()),
        Value::Integer(state_integer(expected.state())),
        Value::Integer(expected.issued_at().seconds() as i64),
        Value::Integer(expected.expires_at().seconds() as i64),
        Value::Blob(stored.digest.to_vec()),
    ]);
    let changed = tx.execute("UPDATE mailbox_proof_workflows SET current_proof_id=?1,state=?2 WHERE workflow_id=?3 AND instance_epoch=?4 AND intent=?5 AND recovery_subject IS ?6 AND recovery_version IS ?7 AND purpose IS ?8 AND current_proof_id=?9 AND state=?10 AND EXISTS(SELECT 1 FROM mailbox_proof_issues i WHERE i.proof_id=?9 AND i.workflow_id=?3 AND i.issued_at=?11 AND i.expires_at=?12 AND i.secret_digest=?13)", params_from_iter(values)).await?;
    if changed != 1 {
        return Err(ProofStoreError::Conflict);
    }
    Ok(())
}

async fn record(
    tx: &WriteTransaction<'_>,
    command: ProofCommandId,
    action: ProofReceiptAction,
    request: [u8; 32],
    proof: MailboxProof,
) -> Result<ProofReceipt, ProofStoreError> {
    tx.execute("INSERT INTO mailbox_proof_receipts(command_id,workflow_id,action,request_digest,result_proof_id,result_state) VALUES(?1,?2,?3,?4,?5,?6)", params_from_iter([
        Value::Blob(command.to_bytes().to_vec()), Value::Blob(proof.binding().operation().to_bytes().to_vec()), Value::Integer(action.integer()),
        Value::Blob(request.to_vec()), Value::Blob(proof.id().to_bytes().to_vec()), Value::Integer(state_integer(proof.state())),
    ])).await?;
    Ok(ProofReceipt {
        command,
        action,
        proof,
    })
}

async fn replay(
    tx: &WriteTransaction<'_>,
    command: ProofCommandId,
    request: [u8; 32],
) -> Result<Option<ProofReceipt>, ProofStoreError> {
    let mut rows = tx
        .query(RECEIPT_QUERY, [command.to_bytes().to_vec()])
        .await?;
    let found = rows
        .next()
        .await?
        .map(|row| decode_receipt(&row))
        .transpose()?;
    match found {
        Some((stored, _)) if !bool::from(stored.ct_eq(&request)) => {
            Err(ProofStoreError::OperationReused)
        }
        Some((_, receipt)) => Ok(Some(receipt)),
        None => Ok(None),
    }
}

async fn finish<T>(
    tx: WriteTransaction<'_>,
    result: Result<T, ProofStoreError>,
) -> Result<T, ProofStoreError> {
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

#[allow(clippy::too_many_arguments)]
fn fingerprint(
    action: ProofReceiptAction,
    binding: ProofBinding,
    expected: Option<ProofId>,
    secret: [u8; 32],
    candidate: Option<ProofId>,
    expires: u64,
    cooldown: u32,
    explicit: u8,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"cantos-identity-proof-command-v1\0");
    hash.update([action.integer() as u8]);
    hash.update(binding.operation().to_bytes());
    hash.update(binding.epoch().to_bytes());
    let (intent, subject, version, purpose): (u8, [u8; 16], u64, u8) = match binding.intent() {
        ProofIntent::TheatreSignup => (1, [0; 16], 0, 0),
        ProofIntent::CredentialRecovery {
            subject,
            version,
            purpose,
        } => (
            2,
            subject.to_bytes(),
            version.get(),
            match purpose {
                Purpose::Theatre => 1,
                Purpose::Studio => 2,
            },
        ),
    };
    hash.update([intent]);
    hash.update(subject);
    hash.update(version.to_be_bytes());
    hash.update([purpose]);
    hash.update(expected.map_or([0; 16], ProofId::to_bytes));
    hash.update(secret);
    hash.update(candidate.map_or([0; 16], ProofId::to_bytes));
    hash.update(expires.to_be_bytes());
    hash.update(cooldown.to_be_bytes());
    hash.update([explicit]);
    hash.finalize().into()
}

fn cell(row: &Row, index: usize) -> Result<Value, ProofStoreError> {
    row.get_value(index)
        .map_err(|_| ProofStoreError::CorruptRecord)
}

fn integer(row: &Row, index: usize) -> Result<i64, ProofStoreError> {
    match cell(row, index)? {
        Value::Integer(value) => Ok(value),
        _ => Err(ProofStoreError::CorruptRecord),
    }
}

fn bytes<const N: usize>(row: &Row, index: usize) -> Result<[u8; N], ProofStoreError> {
    match cell(row, index)? {
        Value::Blob(value) => value.try_into().map_err(|_| ProofStoreError::CorruptRecord),
        _ => Err(ProofStoreError::CorruptRecord),
    }
}

fn decode_proof(row: &Row, offset: usize) -> Result<StoredProof, ProofStoreError> {
    let operation =
        OperationId::from_bytes(bytes(row, offset)?).map_err(|_| ProofStoreError::CorruptRecord)?;
    let epoch = InstanceEpoch::from_bytes(bytes(row, offset + 1)?)
        .map_err(|_| ProofStoreError::CorruptRecord)?;
    let intent = match integer(row, offset + 2)? {
        1 if matches!(cell(row, offset + 3)?, Value::Null)
            && matches!(cell(row, offset + 4)?, Value::Null)
            && matches!(cell(row, offset + 5)?, Value::Null) =>
        {
            ProofIntent::TheatreSignup
        }
        2 => {
            let subject = SubjectId::from_bytes(bytes(row, offset + 3)?)
                .map_err(|_| ProofStoreError::CorruptRecord)?;
            let raw = u64::try_from(integer(row, offset + 4)?)
                .map_err(|_| ProofStoreError::CorruptRecord)?;
            let version = SecurityVersion::new(raw).map_err(|_| ProofStoreError::CorruptRecord)?;
            let purpose = match integer(row, offset + 5)? {
                1 => Purpose::Theatre,
                2 => Purpose::Studio,
                _ => return Err(ProofStoreError::CorruptRecord),
            };
            ProofIntent::CredentialRecovery {
                subject,
                version,
                purpose,
            }
        }
        _ => return Err(ProofStoreError::CorruptRecord),
    };
    let id =
        ProofId::from_bytes(bytes(row, offset + 6)?).map_err(|_| ProofStoreError::CorruptRecord)?;
    let issued =
        u64::try_from(integer(row, offset + 7)?).map_err(|_| ProofStoreError::CorruptRecord)?;
    let expires =
        u64::try_from(integer(row, offset + 8)?).map_err(|_| ProofStoreError::CorruptRecord)?;
    let state = match integer(row, offset + 9)? {
        1 => ProofState::Pending,
        2 => ProofState::Consumed,
        3 => ProofState::Cancelled,
        _ => return Err(ProofStoreError::CorruptRecord),
    };
    let proof = MailboxProof::restore(
        id,
        ProofBinding::new(operation, epoch, intent),
        ProofTime::new(issued).map_err(|_| ProofStoreError::CorruptRecord)?,
        ProofTime::new(expires).map_err(|_| ProofStoreError::CorruptRecord)?,
        state,
    )
    .map_err(|_| ProofStoreError::CorruptRecord)?;
    Ok(StoredProof {
        proof,
        digest: bytes(row, offset + 10)?,
    })
}

fn decode_receipt(row: &Row) -> Result<([u8; 32], ProofReceipt), ProofStoreError> {
    let command =
        ProofCommandId::from_bytes(bytes(row, 0)?).map_err(|_| ProofStoreError::CorruptRecord)?;
    let action = match integer(row, 1)? {
        1 => ProofReceiptAction::Issued,
        2 => ProofReceiptAction::Confirmed,
        3 => ProofReceiptAction::Cancelled,
        4 => ProofReceiptAction::Replaced,
        _ => return Err(ProofStoreError::CorruptRecord),
    };
    let proof = decode_proof(row, 3)?.proof;
    let valid = matches!(
        (action, proof.state()),
        (
            ProofReceiptAction::Issued | ProofReceiptAction::Replaced,
            ProofState::Pending
        ) | (ProofReceiptAction::Confirmed, ProofState::Consumed)
            | (ProofReceiptAction::Cancelled, ProofState::Cancelled)
    );
    if !valid {
        return Err(ProofStoreError::CorruptRecord);
    }
    Ok((
        bytes(row, 2)?,
        ProofReceipt {
            command,
            action,
            proof,
        },
    ))
}

#[cfg(test)]
mod tests {
    use cantos_store_turso::{
        registry::Intent,
        testing::{FixedClock, TestRun},
    };

    use super::*;

    fn valid_projection() -> Vec<Value> {
        vec![
            Value::Blob([1; 16].to_vec()),
            Value::Blob([2; 16].to_vec()),
            Value::Integer(1),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Blob([3; 16].to_vec()),
            Value::Integer(10),
            Value::Integer(20),
            Value::Integer(1),
            Value::Blob([4; 32].to_vec()),
        ]
    }

    async fn store(run: &TestRun) -> IdentityStore {
        IdentityStore::open(
            run.path("identity.db"),
            Intent::Migrate,
            FixedClock::new(10_000_000),
        )
        .await
        .unwrap()
    }

    // Read deliberately malformed projections from the real engine. Stored schema constraints
    // and guards remain enabled; no malformed record is inserted into an authoritative table.
    async fn projection(store: &IdentityStore, values: Vec<Value>) -> Row {
        let reader = store.database.reader().await.unwrap();
        let select = format!(
            "SELECT {}",
            (1..=values.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut rows = reader
            .query(&select, params_from_iter(values))
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert!(rows.next().await.unwrap().is_none());
        row
    }

    #[test]
    fn secrets_are_redacted_and_digest_matches_an_independent_sha256_vector() {
        let secret = ProofSecret::from_bytes(std::array::from_fn(|index| index as u8));
        assert_eq!(format!("{secret:?}"), "ProofSecret([REDACTED])");
        assert_eq!(secret.expose(), &std::array::from_fn(|index| index as u8));
        // Python hashlib SHA256(prefix || bytes(range(32))), not derived from this helper.
        assert_eq!(
            secret.digest(),
            [
                0x08, 0x1a, 0x77, 0xc9, 0xd7, 0xf9, 0xbb, 0x2e, 0xe5, 0x4a, 0xf7, 0xda, 0xae, 0xaa,
                0x8d, 0x72, 0x93, 0x3e, 0x72, 0x90, 0x15, 0x18, 0xd2, 0x50, 0x6e, 0x6a, 0xf3, 0x66,
                0x72, 0x26, 0xa6, 0x47,
            ]
        );
        let material = ProofMaterial {
            id: ProofId::from_bytes([1; 16]).unwrap(),
            secret,
        };
        assert_eq!(format!("{material:?}"), "ProofMaterial([REDACTED])");
        assert_eq!(ProofCommandId::from_bytes([0; 16]), Err(NilIdentifier));
        assert_eq!(
            ProofCommandId::from_bytes([1; 16]).unwrap().to_bytes(),
            [1; 16]
        );
    }

    #[tokio::test]
    async fn real_select_hydration_preserves_terminal_state_and_refuses_malformed_proofs() {
        let run = TestRun::new("identity-proof-hydration");
        let store = store(&run).await;
        for (raw_state, state) in [
            (1, ProofState::Pending),
            (2, ProofState::Consumed),
            (3, ProofState::Cancelled),
        ] {
            let mut values = valid_projection();
            values[9] = Value::Integer(raw_state);
            let row = projection(&store, values).await;
            let proof = decode_proof(&row, 0).unwrap().proof;
            assert_eq!(proof.state(), state);
            assert_eq!(
                proof.binding().epoch(),
                InstanceEpoch::from_bytes([2; 16]).unwrap()
            );
            assert_eq!(proof.issued_at().seconds(), 10);
            assert_eq!(proof.expires_at().seconds(), 20);
        }
        let malformed = [
            (0, Value::Blob(vec![1; 15])),
            (0, Value::Blob(vec![0; 16])),
            (1, Value::Null),
            (1, Value::Blob(vec![0; 16])),
            (2, Value::Integer(0)),
            (2, Value::Text("1".to_owned())),
            (3, Value::Blob(vec![5; 16])),
            (4, Value::Integer(1)),
            (5, Value::Integer(1)),
            (6, Value::Blob(vec![0; 16])),
            (6, Value::Blob(vec![1; 17])),
            (7, Value::Integer(-1)),
            (8, Value::Integer(10)),
            (8, Value::Integer(-1)),
            (9, Value::Integer(4)),
            (10, Value::Blob(vec![4; 31])),
            (10, Value::Integer(4)),
        ];
        for (index, value) in malformed {
            let mut values = valid_projection();
            values[index] = value;
            let row = projection(&store, values).await;
            assert_eq!(
                decode_proof(&row, 0).map(|stored| stored.proof),
                Err(ProofStoreError::CorruptRecord),
                "column {index}"
            );
        }
        let mut recovery = valid_projection();
        recovery[2] = Value::Integer(2);
        recovery[3] = Value::Blob([5; 16].to_vec());
        recovery[4] = Value::Integer(1);
        recovery[5] = Value::Integer(2);
        let row = projection(&store, recovery.clone()).await;
        assert_eq!(
            decode_proof(&row, 0).unwrap().proof.binding().intent(),
            ProofIntent::CredentialRecovery {
                subject: SubjectId::from_bytes([5; 16]).unwrap(),
                version: SecurityVersion::new(1).unwrap(),
                purpose: Purpose::Studio,
            }
        );
        for (index, value) in [
            (3, Value::Null),
            (3, Value::Blob(vec![0; 16])),
            (4, Value::Integer(0)),
            (4, Value::Integer(-1)),
            (5, Value::Integer(0)),
        ] {
            let mut values = recovery.clone();
            values[index] = value;
            let row = projection(&store, values).await;
            assert_eq!(
                decode_proof(&row, 0).map(|stored| stored.proof),
                Err(ProofStoreError::CorruptRecord),
                "recovery column {index}"
            );
        }
    }

    #[tokio::test]
    async fn real_select_receipt_hydration_refuses_inconsistent_action_or_malformed_shape() {
        let run = TestRun::new("identity-proof-receipt-hydration");
        let store = store(&run).await;
        for (action, state) in [(1, 1), (2, 2), (3, 3), (4, 1)] {
            let mut values = vec![
                Value::Blob(vec![7; 16]),
                Value::Integer(action),
                Value::Blob(vec![8; 32]),
            ];
            values.extend(valid_projection());
            values[12] = Value::Integer(state);
            let row = projection(&store, values).await;
            assert_eq!(decode_receipt(&row).unwrap().1.action.integer(), action);
        }
        let mut valid = vec![
            Value::Blob(vec![7; 16]),
            Value::Integer(2),
            Value::Blob(vec![8; 32]),
        ];
        valid.extend(valid_projection());
        valid[12] = Value::Integer(2);
        for (index, value) in [
            (0, Value::Blob(vec![0; 16])),
            (0, Value::Null),
            (1, Value::Integer(0)),
            (1, Value::Integer(1)),
            (2, Value::Blob(vec![8; 31])),
            (3, Value::Null),
            (12, Value::Integer(1)),
        ] {
            let mut values = valid.clone();
            values[index] = value;
            let row = projection(&store, values).await;
            assert_eq!(
                decode_receipt(&row),
                Err(ProofStoreError::CorruptRecord),
                "receipt column {index}"
            );
        }
    }
}
