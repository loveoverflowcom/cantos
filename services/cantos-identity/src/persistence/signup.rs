//! Signup workflows, verified mailbox ownership and password credentials.
//!
//! A pending signup is a mailbox-proof workflow plus an immutable email binding written in the
//! same transaction as the first proof. Completion consumes the proof and creates the subject,
//! verified email and credential in ONE transaction: a `Consumed` proof row, a receipt or a
//! status lookup is never a setup permit. Hashing happens before this module is called; the
//! stored PHC text is opaque here beyond a bounded-shape check.

use std::fmt;

use cantos_store_turso::{
    error::{CommitFault, StoreFault},
    params_from_iter, Row, Value, WriteTransaction,
};

use super::{
    proofs::{ProofCommandId, ProofMaterial, ProofReceipt, ProofSecret, ProofStoreError},
    IdentityStore, IdentityStoreError,
};
use crate::domain::{
    admission::{
        check_completion, AccountState, Assurance, CompletionDenied, RequestBinding,
        SecuritySnapshot,
    },
    email::EmailAddress,
    ids::{InstanceEpoch, OperationId, SubjectId},
    mailbox_proof::{
        MailboxProof, ProofAction, ProofBinding, ProofDenied, ProofId, ProofIntent, ProofState,
        ProofTime,
    },
    security_version::SecurityVersion,
    signup_start::{decide_start, StartDecision, StartFacts, StartSkip, StoredOperation},
};

const MIN_PHC_BYTES: usize = 32;
const MAX_PHC_BYTES: usize = 512;
const PHC_PREFIX: &str = "$argon2id$v=19$";

const WORKFLOW_EMAIL: &str = "SELECT email FROM signup_workflows WHERE workflow_id=?1";
const EMAIL_SUBJECT: &str = "SELECT subject_id FROM identity_emails WHERE email=?1";
const LOGIN_QUERY: &str = "SELECT s.subject_id,s.security_version,s.state,c.credential_version,c.phc FROM identity_emails e JOIN identity_subjects s ON s.subject_id=e.subject_id JOIN password_credentials c ON c.subject_id=s.subject_id WHERE e.email=?1";
const FINAL_QUERY: &str = "SELECT s.subject_id,s.security_version,s.state,c.credential_version FROM identity_subjects s JOIN password_credentials c ON c.subject_id=s.subject_id WHERE s.subject_id=?1";
const COMPLETION_QUERY: &str = "SELECT d.workflow_id,d.subject_id,c.phc FROM signup_completions d JOIN password_credentials c ON c.subject_id=d.subject_id WHERE d.command_id=?1";

/// Stored password hash text. Only the bounded Argon2id PHC shape is checked here; parameter
/// floors and ceilings belong to the verifier, which must run them before any KDF work.
#[derive(Clone, PartialEq, Eq)]
pub struct PhcString(String);

impl PhcString {
    pub fn new(text: String) -> Result<Self, SignupStoreError> {
        let shaped = text.len() >= MIN_PHC_BYTES
            && text.len() <= MAX_PHC_BYTES
            && text.starts_with(PHC_PREFIX)
            && text.bytes().all(|b| b.is_ascii_graphic());
        if shaped {
            Ok(Self(text))
        } else {
            Err(SignupStoreError::CorruptRecord)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PhcString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PhcString([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignupStoreError {
    Proof(ProofStoreError),
    Identity(IdentityStoreError),
    Storage(StoreFault),
    Commit(CommitFault),
    /// The stored workflow is not a Theatre signup bound to a mailbox.
    NotSignup,
    WorkflowMissing,
    /// A different account already owns this verified email. Nothing was changed.
    EmailTaken,
    /// The operation exists with a different email binding.
    BindingChanged,
    CorruptRecord,
}

impl fmt::Display for SignupStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Proof(_) => "signup mailbox proof was not accepted",
            Self::Identity(_) => "identity security state rejected the signup",
            Self::Storage(_) => "signup storage unavailable",
            Self::Commit(CommitFault::Rejected(_)) => "signup commit rejected",
            Self::Commit(CommitFault::Unknown(_)) => {
                "signup commit outcome unknown; retry the identical command"
            }
            Self::NotSignup => "operation is not a theatre signup",
            Self::WorkflowMissing => "signup operation missing",
            Self::EmailTaken => "email is already registered",
            Self::BindingChanged => "signup email binding changed",
            Self::CorruptRecord => "invalid signup storage record",
        })
    }
}

impl std::error::Error for SignupStoreError {}

impl From<ProofStoreError> for SignupStoreError {
    fn from(value: ProofStoreError) -> Self {
        Self::Proof(value)
    }
}
impl From<IdentityStoreError> for SignupStoreError {
    fn from(value: IdentityStoreError) -> Self {
        Self::Identity(value)
    }
}
impl From<StoreFault> for SignupStoreError {
    fn from(value: StoreFault) -> Self {
        Self::Storage(value)
    }
}
impl From<CommitFault> for SignupStoreError {
    fn from(value: CommitFault) -> Self {
        Self::Commit(value)
    }
}

/// The immutable binding of a pending signup and the proof currently guarding it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignupRecord {
    pub operation: OperationId,
    pub email: EmailAddress,
    pub proof: MailboxProof,
}

/// The committed outcome of a start. Only `Issued` and `Replaced` changed storage, and only
/// they entitle the caller to mail `material`'s secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignupStart {
    Issued(ProofReceipt),
    Replaced(ProofReceipt),
    Skipped(StartSkip),
}

/// Everything one completion needs. The password was hashed outside any transaction.
pub struct SignupCompletionRequest<'a> {
    pub command: ProofCommandId,
    pub operation: OperationId,
    pub proof: ProofId,
    pub secret: &'a ProofSecret,
    pub phc: &'a PhcString,
    pub subject: SubjectId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignupCompletion {
    Completed {
        subject: SubjectId,
        epoch: InstanceEpoch,
    },
    /// An identical earlier command created the account. A historical result: it asserts
    /// nothing about the account's current state and is not an authentication fact.
    Replayed {
        subject: SubjectId,
        epoch: InstanceEpoch,
        /// The stored credential the replaying caller's password must match. Not a grant.
        credential: PhcString,
    },
}

/// A credential observation for login. Stale the moment it is read: the verifier must hand it
/// back to [`IdentityStore::finalize_login`] after the KDF finishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginRecord {
    pub subject: SubjectId,
    pub epoch: InstanceEpoch,
    pub version: SecurityVersion,
    pub state: AccountState,
    pub credential_version: u64,
    pub phc: PhcString,
}

impl LoginRecord {
    pub fn snapshot(&self) -> SecuritySnapshot {
        SecuritySnapshot::new(self.subject, self.epoch, self.version, self.state)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginFinalization {
    /// State and credential were unchanged at `at_micros`, read under the writer lock, which
    /// serializes this check with every security-changing write. It promises nothing later.
    Fresh {
        version: SecurityVersion,
        epoch: InstanceEpoch,
        at_micros: i64,
    },
    Denied(CompletionDenied),
    /// The credential row changed after it was read.
    CredentialChanged,
}

fn cell(row: &Row, index: usize) -> Result<Value, SignupStoreError> {
    row.get_value(index)
        .map_err(|_| SignupStoreError::CorruptRecord)
}

fn integer(row: &Row, index: usize) -> Result<i64, SignupStoreError> {
    match cell(row, index)? {
        Value::Integer(value) => Ok(value),
        _ => Err(SignupStoreError::CorruptRecord),
    }
}

fn text(row: &Row, index: usize) -> Result<String, SignupStoreError> {
    match cell(row, index)? {
        Value::Text(value) => Ok(value),
        _ => Err(SignupStoreError::CorruptRecord),
    }
}

fn id_bytes(row: &Row, index: usize) -> Result<[u8; 16], SignupStoreError> {
    match cell(row, index)? {
        Value::Blob(value) => value
            .try_into()
            .map_err(|_| SignupStoreError::CorruptRecord),
        _ => Err(SignupStoreError::CorruptRecord),
    }
}

fn subject_at(row: &Row, index: usize) -> Result<SubjectId, SignupStoreError> {
    SubjectId::from_bytes(id_bytes(row, index)?).map_err(|_| SignupStoreError::CorruptRecord)
}

fn operation_at(row: &Row, index: usize) -> Result<OperationId, SignupStoreError> {
    OperationId::from_bytes(id_bytes(row, index)?).map_err(|_| SignupStoreError::CorruptRecord)
}

fn version_at(row: &Row, index: usize) -> Result<SecurityVersion, SignupStoreError> {
    let raw = u64::try_from(integer(row, index)?).map_err(|_| SignupStoreError::CorruptRecord)?;
    SecurityVersion::new(raw).map_err(|_| SignupStoreError::CorruptRecord)
}

fn state_at(row: &Row, index: usize) -> Result<AccountState, SignupStoreError> {
    match integer(row, index)? {
        1 => Ok(AccountState::Active),
        2 => Ok(AccountState::Disabled),
        _ => Err(SignupStoreError::CorruptRecord),
    }
}

fn credential_version_at(row: &Row, index: usize) -> Result<u64, SignupStoreError> {
    u64::try_from(integer(row, index)?)
        .ok()
        .filter(|v| *v >= 1)
        .ok_or(SignupStoreError::CorruptRecord)
}

fn email_at(row: &Row, index: usize) -> Result<EmailAddress, SignupStoreError> {
    let stored = text(row, index)?;
    let parsed = EmailAddress::parse(&stored).map_err(|_| SignupStoreError::CorruptRecord)?;
    // Only canonical spellings are ever written; anything else was inserted around the typed path.
    if parsed.as_str() != stored {
        return Err(SignupStoreError::CorruptRecord);
    }
    Ok(parsed)
}

async fn workflow_email(
    tx: &WriteTransaction<'_>,
    operation: OperationId,
) -> Result<Option<EmailAddress>, SignupStoreError> {
    let mut rows = tx
        .query(WORKFLOW_EMAIL, [operation.to_bytes().to_vec()])
        .await?;
    let bound = rows
        .next()
        .await?
        .map(|row| email_at(&row, 0))
        .transpose()?;
    Ok(bound)
}

async fn finish<T>(
    tx: WriteTransaction<'_>,
    result: Result<T, SignupStoreError>,
) -> Result<T, SignupStoreError> {
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

impl IdentityStore {
    pub async fn email_registered(&self, email: &EmailAddress) -> Result<bool, SignupStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(EMAIL_SUBJECT, [email.as_str().to_owned()])
            .await?;
        Ok(rows.next().await?.is_some())
    }

    /// The binding and current proof of a pending-or-concluded signup. Two reads, so a caller
    /// treats it as advisory and re-checks inside its own transaction.
    pub async fn signup(
        &self,
        operation: OperationId,
    ) -> Result<Option<SignupRecord>, SignupStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(WORKFLOW_EMAIL, [operation.to_bytes().to_vec()])
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let email = email_at(&row, 0)?;
        drop(rows);
        drop(reader);
        let proof = self
            .mailbox_proof(operation)
            .await?
            .ok_or(SignupStoreError::CorruptRecord)?;
        if proof.binding().intent() != ProofIntent::TheatreSignup {
            return Err(SignupStoreError::CorruptRecord);
        }
        Ok(Some(SignupRecord {
            operation,
            email,
            proof,
        }))
    }

    /// Writes the pending signup, its immutable mailbox binding and the first proof digest in
    /// one transaction. The proof secret exists only in `material`; it is never stored.
    pub async fn begin_signup(
        &self,
        command: ProofCommandId,
        operation: OperationId,
        email: &EmailAddress,
        material: &ProofMaterial,
        expires: ProofTime,
    ) -> Result<ProofReceipt, SignupStoreError> {
        let tx = self.database.begin_write().await?;
        let result = self
            .begin_signup_in(&tx, command, operation, email, material, expires)
            .await;
        finish(tx, result).await
    }

    async fn begin_signup_in(
        &self,
        tx: &WriteTransaction<'_>,
        command: ProofCommandId,
        operation: OperationId,
        email: &EmailAddress,
        material: &ProofMaterial,
        expires: ProofTime,
    ) -> Result<ProofReceipt, SignupStoreError> {
        let binding = ProofBinding::new(operation, self.epoch, ProofIntent::TheatreSignup);
        let receipt = self
            .issue_mailbox_proof_in_transaction(tx, command, binding, material, expires)
            .await?;
        match workflow_email(tx, operation).await? {
            Some(bound) if bound == *email => {}
            Some(_) => return Err(SignupStoreError::BindingChanged),
            None => {
                tx.execute(
                    "INSERT INTO signup_workflows(workflow_id,email,requested_at) VALUES(?1,?2,?3)",
                    params_from_iter([
                        Value::Blob(operation.to_bytes().to_vec()),
                        Value::Text(email.as_str().to_owned()),
                        Value::Integer(tx.now().get() / 1_000_000),
                    ]),
                )
                .await?;
            }
        }
        Ok(receipt)
    }

    /// The whole Start/resend decision in ONE writer transaction: the registered-address check,
    /// the operation's immutable binding, its current proof and the resend cooldown are read
    /// where they are applied, so concurrent starts are serialized into one winner and never
    /// observe each other as a conflict. `Issued`/`Replaced` carry the only secret that may be
    /// mailed; `Skipped` wrote nothing. The caller mails AFTER this returns, outside the
    /// transaction. Expiry is derived from the writer's clock.
    pub async fn start_or_resend_signup(
        &self,
        command: ProofCommandId,
        operation: OperationId,
        email: &EmailAddress,
        material: &ProofMaterial,
        proof_ttl_seconds: u64,
        cooldown_seconds: u32,
    ) -> Result<SignupStart, SignupStoreError> {
        let tx = self.database.begin_write().await?;
        let decided = self
            .decide_and_apply_start(
                &tx,
                command,
                operation,
                email,
                material,
                proof_ttl_seconds,
                cooldown_seconds,
            )
            .await;
        match decided {
            Ok(start @ SignupStart::Skipped(_)) => {
                tx.rollback().await?;
                Ok(start)
            }
            other => finish(tx, other).await,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn decide_and_apply_start(
        &self,
        tx: &WriteTransaction<'_>,
        command: ProofCommandId,
        operation: OperationId,
        email: &EmailAddress,
        material: &ProofMaterial,
        proof_ttl_seconds: u64,
        cooldown_seconds: u32,
    ) -> Result<SignupStart, SignupStoreError> {
        let mut rows = tx.query(EMAIL_SUBJECT, [email.as_str().to_owned()]).await?;
        let requested_registered = rows.next().await?.is_some();
        drop(rows);
        let bound = workflow_email(tx, operation).await?;
        let proof = self.workflow_binding_in_transaction(tx, operation).await?;
        let stored = match (bound, proof) {
            (None, None) => StoredOperation::Unused,
            (Some(email), Some(proof)) => StoredOperation::Signup { email, proof },
            // A workflow with another intent owns the id.
            (None, Some(proof)) if proof.binding().intent() != ProofIntent::TheatreSignup => {
                StoredOperation::Foreign
            }
            // A signup binding and its proof are written together; one without the other, or a
            // signup proof without its binding, was not written by this code.
            _ => return Err(SignupStoreError::CorruptRecord),
        };
        let now = Self::proof_now(tx)?;
        let decision = decide_start(&StartFacts {
            requested: email,
            requested_registered,
            stored: &stored,
            now_seconds: now.seconds(),
            cooldown_seconds,
        });
        let expires = ProofTime::new(now.seconds().saturating_add(proof_ttl_seconds))
            .map_err(|_| SignupStoreError::CorruptRecord)?;
        match decision {
            StartDecision::Skip(reason) => Ok(SignupStart::Skipped(reason)),
            StartDecision::Issue => self
                .begin_signup_in(tx, command, operation, email, material, expires)
                .await
                .map(SignupStart::Issued),
            StartDecision::Replace { expected } => {
                let binding = ProofBinding::new(operation, self.epoch, ProofIntent::TheatreSignup);
                self.replace_mailbox_proof_in_transaction(
                    tx,
                    command,
                    binding,
                    expected,
                    material,
                    expires,
                    cooldown_seconds,
                )
                .await
                .map(SignupStart::Replaced)
                .map_err(Into::into)
            }
        }
    }

    /// A historical completion for `command`, with the stored hash a replaying caller must
    /// verify its password against. It grants nothing.
    pub async fn completion_for_command(
        &self,
        command: ProofCommandId,
    ) -> Result<Option<(OperationId, SubjectId, PhcString)>, SignupStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(COMPLETION_QUERY, [command.to_bytes().to_vec()])
            .await?;
        rows.next()
            .await?
            .map(|row| {
                Ok((
                    operation_at(&row, 0)?,
                    subject_at(&row, 1)?,
                    PhcString::new(text(&row, 2)?)?,
                ))
            })
            .transpose()
    }

    /// Consumes the mailbox proof and creates subject, verified email, credential and
    /// completion record, or none of them. Every precondition is re-read under the writer lock:
    /// the binding is the stored workflow, never the request.
    pub async fn complete_signup(
        &self,
        request: &SignupCompletionRequest<'_>,
    ) -> Result<SignupCompletion, SignupStoreError> {
        let tx = self.database.begin_write().await?;
        let result = self.complete_in(&tx, request).await;
        finish(tx, result).await
    }

    async fn complete_in(
        &self,
        tx: &WriteTransaction<'_>,
        request: &SignupCompletionRequest<'_>,
    ) -> Result<SignupCompletion, SignupStoreError> {
        let stored = self
            .workflow_binding_in_transaction(tx, request.operation)
            .await?
            .ok_or(SignupStoreError::WorkflowMissing)?;
        let binding = stored.binding();
        if binding.intent() != ProofIntent::TheatreSignup {
            return Err(SignupStoreError::NotSignup);
        }
        let mut rows = tx
            .query(COMPLETION_QUERY, [request.command.to_bytes().to_vec()])
            .await?;
        let earlier = rows
            .next()
            .await?
            .map(|row| {
                Ok::<_, SignupStoreError>((
                    operation_at(&row, 0)?,
                    subject_at(&row, 1)?,
                    PhcString::new(text(&row, 2)?)?,
                ))
            })
            .transpose()?;
        drop(rows);
        // A consumption receipt of this command that is NOT a signup completion (the storage
        // primitive used on its own) must never be replayed into an account: without an earlier
        // completion the proof has to be pending right now.
        if earlier.is_none() {
            match stored.state() {
                ProofState::Pending => {}
                ProofState::Consumed => {
                    return Err(ProofStoreError::Denied(ProofDenied::Consumed).into())
                }
                ProofState::Cancelled => {
                    return Err(ProofStoreError::Denied(ProofDenied::Cancelled).into())
                }
            }
        }
        // Validates the exact payload of a replay (or consumes the proof for a first run).
        self.confirm_mailbox_proof_in_transaction(
            tx,
            request.command,
            binding,
            request.proof,
            request.secret,
            ProofAction::ExplicitConfirmation,
        )
        .await?;
        if let Some((workflow, subject, credential)) = earlier {
            if workflow != request.operation {
                return Err(ProofStoreError::OperationReused.into());
            }
            return Ok(SignupCompletion::Replayed {
                subject,
                epoch: self.epoch,
                credential,
            });
        }
        let mut rows = tx
            .query(WORKFLOW_EMAIL, [request.operation.to_bytes().to_vec()])
            .await?;
        let email = email_at(
            &rows.next().await?.ok_or(SignupStoreError::CorruptRecord)?,
            0,
        )?;
        drop(rows);
        let mut rows = tx.query(EMAIL_SUBJECT, [email.as_str().to_owned()]).await?;
        if rows.next().await?.is_some() {
            return Err(SignupStoreError::EmailTaken);
        }
        drop(rows);
        self.create_subject_in_transaction(tx, request.operation, request.subject)
            .await?;
        let now_seconds = tx.now().get() / 1_000_000;
        tx.execute(
            "INSERT INTO identity_emails(email,subject_id,verified_at,signup_workflow) VALUES(?1,?2,?3,?4)",
            params_from_iter([
                Value::Text(email.as_str().to_owned()),
                Value::Blob(request.subject.to_bytes().to_vec()),
                Value::Integer(now_seconds),
                Value::Blob(request.operation.to_bytes().to_vec()),
            ]),
        )
        .await?;
        tx.execute(
            "INSERT INTO password_credentials(subject_id,credential_version,phc,created_at) VALUES(?1,1,?2,?3)",
            params_from_iter([
                Value::Blob(request.subject.to_bytes().to_vec()),
                Value::Text(request.phc.as_str().to_owned()),
                Value::Integer(now_seconds),
            ]),
        )
        .await?;
        tx.execute(
            "INSERT INTO signup_completions(command_id,workflow_id,subject_id) VALUES(?1,?2,?3)",
            params_from_iter([
                Value::Blob(request.command.to_bytes().to_vec()),
                Value::Blob(request.operation.to_bytes().to_vec()),
                Value::Blob(request.subject.to_bytes().to_vec()),
            ]),
        )
        .await?;
        Ok(SignupCompletion::Completed {
            subject: request.subject,
            epoch: self.epoch,
        })
    }

    pub async fn login_record(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<LoginRecord>, SignupStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(LOGIN_QUERY, [email.as_str().to_owned()])
            .await?;
        rows.next()
            .await?
            .map(|row| {
                Ok(LoginRecord {
                    subject: subject_at(&row, 0)?,
                    epoch: self.epoch,
                    version: version_at(&row, 1)?,
                    state: state_at(&row, 2)?,
                    credential_version: credential_version_at(&row, 3)?,
                    phc: PhcString::new(text(&row, 4)?)?,
                })
            })
            .transpose()
    }

    /// The final step of a password login, after the KDF. It takes the serialized writer so it
    /// cannot interleave with a disable or credential change, re-reads state and credential and
    /// runs the pure completion check against the observation the verification started from.
    /// The transaction writes nothing and always rolls back.
    pub async fn finalize_login(
        &self,
        started: &LoginRecord,
        binding: RequestBinding,
    ) -> Result<LoginFinalization, SignupStoreError> {
        let tx = self.database.begin_write().await?;
        let result = async {
            let mut rows = tx
                .query(FINAL_QUERY, [started.subject.to_bytes().to_vec()])
                .await?;
            let Some(row) = rows.next().await? else {
                return Ok(LoginFinalization::Denied(CompletionDenied::SubjectChanged));
            };
            let current = SecuritySnapshot::new(
                subject_at(&row, 0)?,
                self.epoch,
                version_at(&row, 1)?,
                state_at(&row, 2)?,
            );
            let credential_version = credential_version_at(&row, 3)?;
            drop(rows);
            if let Err(denied) = check_completion(
                started.snapshot(),
                binding,
                current,
                binding,
                Assurance::Password,
            ) {
                return Ok(LoginFinalization::Denied(denied));
            }
            if credential_version != started.credential_version {
                return Ok(LoginFinalization::CredentialChanged);
            }
            Ok(LoginFinalization::Fresh {
                version: started.version,
                epoch: self.epoch,
                at_micros: tx.now().get(),
            })
        }
        .await;
        match result {
            Ok(value) => {
                tx.rollback().await?;
                Ok(value)
            }
            Err(error) => {
                // The original error is the one worth reporting; a failed rollback is secondary.
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}
