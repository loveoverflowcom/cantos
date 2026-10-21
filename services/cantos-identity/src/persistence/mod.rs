//! Identity-owned file-backed security state. This is internal storage, not a credential
//! verifier, public provisioning API, AuthenticationFact or authorization boundary.

pub mod proofs;
mod rows;
pub use proofs::{
    ProofCommandId, ProofMaterial, ProofReceipt, ProofReceiptAction, ProofSecret, ProofStoreError,
};

use std::{fmt, path::PathBuf, sync::Arc};

use cantos_store_turso::{
    clock::Clock,
    error::{CommitFault, OpenError, StoreFault},
    params_from_iter,
    registry::{DatabaseKind, Intent, Migration, Registry, RegistryError},
    OpenRequest, OwnedDatabase, Value, WriteTransaction,
};

use crate::domain::{
    admission::{AccountState, SecuritySnapshot},
    ids::{InstanceEpoch, OperationId, SubjectId},
    security_version::SecurityVersion,
};

use rows::{decode_epoch, decode_event, decode_receipt, decode_subject, integer, positive};

const MIGRATIONS: [Migration; 2] = [
    Migration {
        version: 1,
        name: "identity_security_state",
        sql: include_str!("schema.sql"),
    },
    Migration {
        version: 2,
        name: "identity_mailbox_proofs",
        sql: include_str!("proofs.sql"),
    },
];
const RECEIPT_QUERY: &str = "SELECT operation_id,subject_id,kind,expected_version,committed_version,event_seq FROM operation_receipts WHERE operation_id=?1";
const SUBJECT_QUERY: &str =
    "SELECT subject_id,security_version,state FROM identity_subjects WHERE subject_id=?1";
pub const MAX_EVENT_BATCH: u32 = 128;

/// The one Identity schema registry. Migration and serving compositions must share it.
pub fn registry() -> Result<Registry, RegistryError> {
    Registry::new(DatabaseKind::Identity, 1, &MIGRATIONS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Created,
    Disabled,
}

impl ChangeKind {
    fn integer(self) -> i64 {
        match self {
            Self::Created => 1,
            Self::Disabled => 2,
        }
    }
}

/// A current storage observation; callers still need real credential/caller/freshness checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredSubject {
    pub subject: SubjectId,
    pub epoch: InstanceEpoch,
    pub version: SecurityVersion,
    pub state: AccountState,
}

impl StoredSubject {
    pub fn snapshot(self) -> SecuritySnapshot {
        SecuritySnapshot::new(self.subject, self.epoch, self.version, self.state)
    }
}

/// Immutable operation outcome, deliberately distinct from current security state.
/// Replaying a creation receipt after disable must never yield a fresh Active snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationReceipt {
    pub operation: OperationId,
    pub subject: SubjectId,
    pub epoch: InstanceEpoch,
    pub kind: ChangeKind,
    pub expected_version: Option<SecurityVersion>,
    pub committed_version: SecurityVersion,
    pub event_seq: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecurityEvent {
    pub seq: u64,
    pub subject: SubjectId,
    pub version: SecurityVersion,
    pub state: AccountState,
    pub kind: ChangeKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityStoreError {
    Open(OpenError),
    Storage(StoreFault),
    Commit(CommitFault),
    InitializationCommit(CommitFault),
    InvalidRegistry,
    CorruptRecord,
    Uninitialized,
    EpochChanged,
    OperationReused,
    SubjectExists,
    SubjectMissing,
    Conflict,
    Inactive,
    VersionExhausted,
    EventSequenceExhausted,
    InvalidEventRange,
    EntropyUnavailable,
}

impl fmt::Display for IdentityStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep row values, paths, SQL and low-level diagnostics out of outward errors.
        f.write_str(match self {
            Self::Open(_) => "identity database open did not complete",
            Self::Storage(_) => "identity storage unavailable",
            Self::Commit(CommitFault::Rejected(_)) => "identity commit rejected",
            Self::Commit(CommitFault::Unknown(_)) => {
                "identity commit outcome unknown; reconcile receipt"
            }
            Self::InitializationCommit(CommitFault::Rejected(_)) => {
                "identity initialization commit rejected"
            }
            Self::InitializationCommit(CommitFault::Unknown(_)) => {
                "identity initialization outcome unknown; reopen instance metadata"
            }
            Self::InvalidRegistry => "invalid identity schema registry",
            Self::CorruptRecord => "invalid identity storage record",
            Self::Uninitialized => "identity instance is not initialized",
            Self::EpochChanged => "identity operation belongs to a different instance",
            Self::OperationReused => "identity operation binding changed",
            Self::SubjectExists => "identity subject already exists",
            Self::SubjectMissing => "identity subject missing",
            Self::Conflict => "identity security state changed",
            Self::Inactive => "identity subject is inactive",
            Self::VersionExhausted => "identity security version exhausted",
            Self::EventSequenceExhausted => "identity event sequence exhausted",
            Self::InvalidEventRange => "invalid identity event range",
            Self::EntropyUnavailable => "identity epoch randomness unavailable",
        })
    }
}

impl std::error::Error for IdentityStoreError {}

impl From<StoreFault> for IdentityStoreError {
    fn from(error: StoreFault) -> Self {
        Self::Storage(error)
    }
}

impl From<CommitFault> for IdentityStoreError {
    fn from(error: CommitFault) -> Self {
        Self::Commit(error)
    }
}

pub struct IdentityStore {
    database: OwnedDatabase,
    epoch: InstanceEpoch,
}

impl IdentityStore {
    /// Explicit migrate may initialize an empty Identity instance. Serve never creates a
    /// schema or epoch. Ordinary reopen retains the epoch; restore rotation is not supplied.
    pub async fn open(
        path: impl Into<PathBuf>,
        intent: Intent,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, IdentityStoreError> {
        let schema = registry().map_err(|_| IdentityStoreError::InvalidRegistry)?;
        let database = OwnedDatabase::open(OpenRequest::new(path, schema, intent, clock))
            .await
            .map_err(IdentityStoreError::Open)?;
        let reader = database.reader().await?;
        let mut rows = reader
            .query(
                "SELECT instance_epoch FROM identity_instance WHERE singleton=1",
                (),
            )
            .await?;
        let existing = rows.next().await?.map(decode_epoch).transpose()?;
        drop(rows);
        drop(reader);
        let epoch = match existing {
            Some(epoch) => epoch,
            None if intent == Intent::Migrate => initialize_instance(&database).await?,
            None => return Err(IdentityStoreError::Uninitialized),
        };
        Ok(Self { database, epoch })
    }

    pub const fn epoch(&self) -> InstanceEpoch {
        self.epoch
    }

    pub async fn subject(
        &self,
        subject: SubjectId,
    ) -> Result<Option<StoredSubject>, IdentityStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(SUBJECT_QUERY, [subject.to_bytes().to_vec()])
            .await?;
        rows.next()
            .await?
            .map(|row| decode_subject(row, self.epoch))
            .transpose()
    }

    pub async fn receipt(
        &self,
        operation: OperationId,
    ) -> Result<Option<OperationReceipt>, IdentityStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader
            .query(RECEIPT_QUERY, [operation.to_bytes().to_vec()])
            .await?;
        rows.next()
            .await?
            .map(|row| decode_receipt(row, self.epoch))
            .transpose()
    }

    /// Internal owner-authorized subject creation only. There is no email, credential,
    /// ActorId, Studio grant or authentication fact in this operation or its receipt.
    pub async fn create_subject(
        &self,
        operation: OperationId,
        subject: SubjectId,
        expected_epoch: InstanceEpoch,
    ) -> Result<OperationReceipt, IdentityStoreError> {
        if expected_epoch != self.epoch {
            return Err(IdentityStoreError::EpochChanged);
        }
        let tx = self.database.begin_write().await?;
        let result = async {
            if let Some(receipt) = self
                .replay(&tx, operation, subject, ChangeKind::Created, None)
                .await?
            {
                return Ok(receipt);
            }
            let mut rows = tx
                .query(SUBJECT_QUERY, [subject.to_bytes().to_vec()])
                .await?;
            if rows.next().await?.is_some() {
                return Err(IdentityStoreError::SubjectExists);
            }
            drop(rows);
            tx.execute(
                "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(?1,1,1)",
                [subject.to_bytes().to_vec()],
            )
            .await?;
            let first = SecurityVersion::new(1).map_err(|_| IdentityStoreError::CorruptRecord)?;
            self.record(&tx, operation, subject, ChangeKind::Created, None, first)
                .await
        }
        .await;
        finish(tx, result).await
    }

    /// Only the named Active→Disabled transition is implemented. The caller must authorize
    /// it; this storage primitive does not validate recent authentication or operator rights.
    pub async fn disable_subject(
        &self,
        operation: OperationId,
        subject: SubjectId,
        expected: SecurityVersion,
        expected_epoch: InstanceEpoch,
    ) -> Result<OperationReceipt, IdentityStoreError> {
        if expected_epoch != self.epoch {
            return Err(IdentityStoreError::EpochChanged);
        }
        let tx = self.database.begin_write().await?;
        let result = async {
            if let Some(receipt) = self.replay(&tx, operation, subject, ChangeKind::Disabled, Some(expected)).await? {
                return Ok(receipt);
            }
            let mut rows = tx.query(SUBJECT_QUERY, [subject.to_bytes().to_vec()]).await?;
            let current = rows.next().await?.ok_or(IdentityStoreError::SubjectMissing)?;
            let current = decode_subject(current, self.epoch)?;
            drop(rows);
            let version = disabled_version(current, expected)?;
            let changed = tx.execute("UPDATE identity_subjects SET security_version=?1,state=2 WHERE subject_id=?2 AND security_version=?3 AND state=1", params_from_iter([
                Value::Integer(version.get() as i64), Value::Blob(subject.to_bytes().to_vec()), Value::Integer(expected.get() as i64),
            ])).await?;
            if changed != 1 { return Err(IdentityStoreError::Conflict); }
            self.record(&tx, operation, subject, ChangeKind::Disabled, Some(expected), version).await
        }.await;
        finish(tx, result).await
    }

    pub async fn events(
        &self,
        after_seq: u64,
        limit: u32,
    ) -> Result<Vec<SecurityEvent>, IdentityStoreError> {
        let after = i64::try_from(after_seq).map_err(|_| IdentityStoreError::InvalidEventRange)?;
        if limit == 0 || limit > MAX_EVENT_BATCH {
            return Err(IdentityStoreError::InvalidEventRange);
        }
        let reader = self.database.reader().await?;
        let mut rows = reader.query("SELECT seq,subject_id,security_version,state,kind FROM security_events WHERE seq>?1 ORDER BY seq LIMIT ?2", [after, i64::from(limit)]).await?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            events.push(decode_event(row)?);
        }
        Ok(events)
    }

    async fn replay(
        &self,
        tx: &WriteTransaction<'_>,
        operation: OperationId,
        subject: SubjectId,
        kind: ChangeKind,
        expected: Option<SecurityVersion>,
    ) -> Result<Option<OperationReceipt>, IdentityStoreError> {
        let mut rows = tx
            .query(RECEIPT_QUERY, [operation.to_bytes().to_vec()])
            .await?;
        let receipt = rows
            .next()
            .await?
            .map(|row| decode_receipt(row, self.epoch))
            .transpose()?;
        if let Some(receipt) = receipt {
            if receipt.subject != subject
                || receipt.kind != kind
                || receipt.expected_version != expected
            {
                return Err(IdentityStoreError::OperationReused);
            }
        }
        Ok(receipt)
    }

    async fn record(
        &self,
        tx: &WriteTransaction<'_>,
        operation: OperationId,
        subject: SubjectId,
        kind: ChangeKind,
        expected: Option<SecurityVersion>,
        version: SecurityVersion,
    ) -> Result<OperationReceipt, IdentityStoreError> {
        let mut rows = tx
            .query("SELECT coalesce(max(seq),0) FROM security_events", ())
            .await?;
        let last = integer(
            &rows
                .next()
                .await?
                .ok_or(IdentityStoreError::CorruptRecord)?,
            0,
        )?;
        if last < 0 {
            return Err(IdentityStoreError::CorruptRecord);
        }
        let seq = last
            .checked_add(1)
            .ok_or(IdentityStoreError::EventSequenceExhausted)?;
        drop(rows);
        let state = match kind {
            ChangeKind::Created => 1,
            ChangeKind::Disabled => 2,
        };
        tx.execute("INSERT INTO security_events(seq,subject_id,security_version,state,kind,recorded_at) VALUES(?1,?2,?3,?4,?5,?6)", params_from_iter([
            Value::Integer(seq), Value::Blob(subject.to_bytes().to_vec()), Value::Integer(version.get() as i64), Value::Integer(state), Value::Integer(kind.integer()), Value::Integer(tx.now().get()),
        ])).await?;
        tx.execute("INSERT INTO operation_receipts(operation_id,subject_id,kind,expected_version,committed_version,event_seq) VALUES(?1,?2,?3,?4,?5,?6)", params_from_iter([
            Value::Blob(operation.to_bytes().to_vec()), Value::Blob(subject.to_bytes().to_vec()), Value::Integer(kind.integer()), Value::Integer(expected.map_or(0, |v| v.get() as i64)), Value::Integer(version.get() as i64), Value::Integer(seq),
        ])).await?;
        Ok(OperationReceipt {
            operation,
            subject,
            epoch: self.epoch,
            kind,
            expected_version: expected,
            committed_version: version,
            event_seq: positive(seq)?,
        })
    }
}

fn disabled_version(
    current: StoredSubject,
    expected: SecurityVersion,
) -> Result<SecurityVersion, IdentityStoreError> {
    if current.version != expected {
        return Err(IdentityStoreError::Conflict);
    }
    if current.state != AccountState::Active {
        return Err(IdentityStoreError::Inactive);
    }
    current
        .version
        .successor()
        .map_err(|_| IdentityStoreError::VersionExhausted)
}

async fn finish<T>(
    tx: WriteTransaction<'_>,
    result: Result<T, IdentityStoreError>,
) -> Result<T, IdentityStoreError> {
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

async fn initialize_instance(
    database: &OwnedDatabase,
) -> Result<InstanceEpoch, IdentityStoreError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| IdentityStoreError::EntropyUnavailable)?;
    let epoch =
        InstanceEpoch::from_bytes(bytes).map_err(|_| IdentityStoreError::EntropyUnavailable)?;
    let tx = database.begin_write().await?;
    let result = async {
        let mut rows = tx.query("SELECT EXISTS(SELECT 1 FROM identity_subjects) OR EXISTS(SELECT 1 FROM security_events) OR EXISTS(SELECT 1 FROM operation_receipts) OR EXISTS(SELECT 1 FROM mailbox_proof_issues) OR EXISTS(SELECT 1 FROM mailbox_proof_workflows) OR EXISTS(SELECT 1 FROM mailbox_proof_receipts)", ()).await?;
        let occupied = integer(&rows.next().await?.ok_or(IdentityStoreError::CorruptRecord)?, 0)?;
        if occupied != 0 { return Err(IdentityStoreError::CorruptRecord); }
        drop(rows);
        tx.execute("INSERT INTO identity_instance(singleton,instance_epoch) VALUES(1,?1)", [bytes.to_vec()]).await?;
        Ok(epoch)
    }.await;
    finish(tx, result).await.map_err(|error| match error {
        IdentityStoreError::Commit(fault) => IdentityStoreError::InitializationCommit(fault),
        other => other,
    })
}

#[cfg(test)]
mod file_tests;

#[cfg(test)]
mod row_tests;

#[cfg(test)]
mod proof_file_tests;
