//! PostgreSQL shell: authentication facts, authorization, locked head and immutable exports.
use std::time::Duration;

use cantos_contracts::{FieldIssue, RevisionResponse, SaveRevisionRequest};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use sha2::{Digest, Sha256};
use tokio_postgres::{Config, NoTls, Row};
use uuid::Uuid;

use crate::contexts::editorial::{
    decide_save, permits, Access, Action, PriorOperation, SaveDecision,
};
use crate::script_ir::{read_canonical_script, read_script, ReadError, WRITE_VERSION};

const MIGRATIONS: [(i32, &str); 9] = [
    (
        1,
        include_str!("../../../migrations/0001_script_revisions.sql"),
    ),
    (
        2,
        include_str!("../../../migrations/0002_editorial_handoff.sql"),
    ),
    (
        3,
        include_str!("../../../migrations/0003_manuscript_import.sql"),
    ),
    (
        4,
        include_str!("../../../migrations/0004_ai_adaptation.sql"),
    ),
    (
        5,
        include_str!("../../../migrations/0005_caller_adaptation.sql"),
    ),
    (
        6,
        include_str!("../../../migrations/0006_production_inputs.sql"),
    ),
    (7, include_str!("../../../migrations/0007_durable_jobs.sql")),
    (8, include_str!("../../../migrations/0008_speech_clips.sql")),
    (
        9,
        include_str!("../../../migrations/0009_durable_step_changes.sql"),
    ),
];
pub(crate) const REVISION_COLUMNS: &str = "script_id, revision, expected_revision, accepted_by, canonical_export, content_digest, export_digest, to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS accepted_at";

#[derive(Debug)]
pub enum StoreError {
    Unauthenticated,
    NotFound,
    Forbidden,
    InvalidRequest,
    InvalidRequestFields(Vec<FieldIssue>),
    InvalidScript(ReadError),
    EvidenceUnavailable,
    StaleRevision(u64),
    OperationReused,
    ProposalAlreadySubmitted,
    StaleProductionInputs,
    ProductionBlocked(Vec<FieldIssue>),
    Unavailable,
    CorruptRevision,
}

impl From<tokio_postgres::Error> for StoreError {
    fn from(_: tokio_postgres::Error) -> Self {
        Self::Unavailable
    }
}

#[derive(Clone)]
pub struct Store {
    pool: Pool,
    /// Opt-in experimental read model for `job_status`; absent, every read is PostgreSQL's.
    #[cfg(feature = "dbsp-experimental")]
    pub(crate) job_summaries:
        Option<std::sync::Arc<crate::contexts::synthesis::summary_view::AttachedView>>,
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub(crate) fn export_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"cantos/script-export/e1\n");
    digest.update(bytes);
    format!("sir-e1:sha256:{:x}", digest.finalize())
}

/// Local/test only until TLS, deployment identity and secret delivery are reviewed.
pub fn local_config(url: &str) -> Result<Config, StoreError> {
    let mut config: Config = url.parse().map_err(|_| StoreError::InvalidRequest)?;
    if config.get_hosts().is_empty()
        || !config.get_hosts().iter().all(|host| match host {
            tokio_postgres::config::Host::Tcp(name) => {
                name == "127.0.0.1" || name == "localhost" || name == "::1"
            }
            tokio_postgres::config::Host::Unix(_) => true,
        })
    {
        return Err(StoreError::InvalidRequest);
    }
    config.connect_timeout(Duration::from_secs(5));
    config.options("-c statement_timeout=5000 -c lock_timeout=3000 -c idle_in_transaction_session_timeout=10000");
    Ok(config)
}

impl Store {
    pub fn new(config: Config) -> Result<Self, StoreError> {
        // Verified recycling detects a terminated socket before lending it again.
        let manager = Manager::from_config(
            config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Verified,
            },
        );
        let pool = Pool::builder(manager)
            .max_size(8)
            .wait_timeout(Some(Duration::from_secs(5)))
            .runtime(deadpool_postgres::Runtime::Tokio1)
            .build()
            .map_err(|_| StoreError::Unavailable)?;
        Ok(Self {
            pool,
            #[cfg(feature = "dbsp-experimental")]
            job_summaries: None,
        })
    }

    /// Answers `job_status` through `view` under its read model. Authentication, authorization
    /// and every other read and write keep their PostgreSQL path.
    #[cfg(feature = "dbsp-experimental")]
    pub fn with_job_summary_view(
        mut self,
        view: std::sync::Arc<crate::contexts::synthesis::summary_view::AttachedView>,
    ) -> Self {
        self.job_summaries = Some(view);
        self
    }

    pub(crate) async fn connection(&self) -> Result<deadpool_postgres::Object, StoreError> {
        self.pool.get().await.map_err(|_| StoreError::Unavailable)
    }

    pub async fn authenticate(&self, token: &str) -> Result<String, StoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(actor)
    }

    pub async fn revoke(&self, token: &str) -> Result<(), StoreError> {
        let client = self.connection().await?;
        client
            .execute(
                "UPDATE sessions SET revoked=true WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await?;
        Ok(())
    }

    /// Both head and historical reads reauthorize against current PostgreSQL facts.
    pub async fn load(
        &self,
        token: &str,
        script: &str,
        revision: Option<u64>,
    ) -> Result<RevisionResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (_, head) = authorize(&tx, &actor, &script, Action::Read, false).await?;
        let revision = checked_number(revision.unwrap_or(head))?;
        let query = format!(
            "SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND revision=$2"
        );
        let row = tx
            .query_opt(&query, &[&script, &revision])
            .await?
            .ok_or(StoreError::NotFound)?;
        let response = decode_revision(&row)?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn save(
        &self,
        token: &str,
        script: &str,
        request: SaveRevisionRequest,
    ) -> Result<RevisionResponse, StoreError> {
        let script = normalized_uuid(script)?;
        normalized_uuid(&request.operation_id)?;
        checked_number(request.expected_revision)?;
        // Pure admission happens before starting a transaction; rejection writes nothing.
        let content =
            read_script(request.script_json.as_bytes()).map_err(StoreError::InvalidScript)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let result = save_in_transaction(&tx, &actor, &script, &request, &content).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        // A dropped response after this commit is reconciled by the same operation key.
        Ok(result)
    }
}

/// Shared storage-acceptance path. The caller commits this together with its own receipt.
pub(crate) async fn save_in_transaction(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    script: &str,
    request: &SaveRevisionRequest,
    content: &crate::script_ir::ScriptContent,
) -> Result<RevisionResponse, StoreError> {
    let operation = normalized_uuid(&request.operation_id)?;
    let expected = checked_number(request.expected_revision)?;
    let export = content.export_bytes();
    let digest = content.content_digest().to_string();
    let export_hash = export_digest(&export);
    // Concurrent first saves contend on the unique ID, then the same locked head.
    // A failed/stale/unauthorized first save rolls this provisional script back too.
    tx.execute(
        "INSERT INTO scripts(id,owner_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
        &[&script, &actor],
    )
    .await?;
    let (owner, head) = authorize(tx, actor, script, Action::Write, true).await?;
    let query = format!("SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND accepted_by=$2 AND operation_id=$3");
    let prior = tx.query_opt(&query, &[&script, &actor, &operation]).await?;
    let prior_export: Option<Vec<u8>> = prior.as_ref().map(|row| row.get("canonical_export"));
    let prior_summary = prior
        .as_ref()
        .zip(prior_export.as_deref())
        .map(|(row, bytes)| PriorOperation {
            revision: row.get::<_, i64>("revision") as u64,
            expected_revision: row.get::<_, i64>("expected_revision") as u64,
            export: bytes,
        });
    let revision = match decide_save(head, request.expected_revision, &export, prior_summary) {
        SaveDecision::Replay { .. } => {
            let row = prior.as_ref().ok_or(StoreError::CorruptRevision)?;
            let result = decode_revision(row)?;
            return Ok(result);
        }
        SaveDecision::Append { revision } => revision,
        SaveDecision::Stale { current_revision } => {
            return Err(StoreError::StaleRevision(current_revision))
        }
        SaveDecision::OperationReused => return Err(StoreError::OperationReused),
        SaveDecision::RevisionLimit => return Err(StoreError::InvalidRequest),
    };
    for (kind, id) in content.evidence_refs() {
        if tx
            .query_opt(
                "SELECT id FROM script_evidence WHERE owner_id=$1 AND kind=$2 AND id=$3",
                &[&owner, &kind, &id],
            )
            .await?
            .is_none()
        {
            return Err(StoreError::EvidenceUnavailable);
        }
    }
    let revision_number = checked_number(revision)?;
    tx.execute("INSERT INTO script_revisions(script_id,revision,expected_revision,operation_id,accepted_by,schema_version,canonical_export,content_digest,export_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
            &[&script,&revision_number,&expected,&operation,&actor,&WRITE_VERSION,&export,&digest,&export_hash]).await?;
    for (kind, id) in content.evidence_refs() {
        tx.execute("INSERT INTO revision_evidence(script_id,revision,owner_id,kind,evidence_id) VALUES($1,$2,$3,$4,$5)", &[&script,&revision_number,&owner,&kind,&id]).await?;
    }
    let changed = tx
        .execute(
            "UPDATE scripts SET head_revision=$1 WHERE id=$2 AND head_revision=$3",
            &[&revision_number, &script, &expected],
        )
        .await?;
    if changed != 1 {
        return Err(StoreError::StaleRevision(head));
    }
    let query = format!(
        "SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND revision=$2"
    );
    let result = decode_revision(&tx.query_one(&query, &[&script, &revision_number]).await?)?;
    Ok(result)
}

pub(crate) fn checked_number(number: u64) -> Result<i64, StoreError> {
    i64::try_from(number).map_err(|_| StoreError::InvalidRequest)
}

pub(crate) fn normalized_uuid(id: &str) -> Result<String, StoreError> {
    let parsed = Uuid::parse_str(id).map_err(|_| StoreError::InvalidRequest)?;
    let canonical = parsed.to_string();
    if id != canonical {
        return Err(StoreError::InvalidRequest);
    }
    Ok(canonical)
}

pub(crate) async fn authenticate(
    tx: &deadpool_postgres::Transaction<'_>,
    token: &str,
) -> Result<String, StoreError> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StoreError::Unauthenticated);
    }
    let row = tx.query_opt("SELECT a.id FROM sessions s JOIN actors a ON a.id=s.actor_id WHERE s.token_hash=$1 AND NOT s.revoked AND s.expires_at>clock_timestamp() AND a.active FOR SHARE OF s", &[&token_hash(token)]).await?;
    row.map(|row| row.get(0)).ok_or(StoreError::Unauthenticated)
}

pub(crate) async fn authorize(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    script: &str,
    action: Action,
    write_lock: bool,
) -> Result<(String, u64), StoreError> {
    let lock = if write_lock { "UPDATE" } else { "SHARE" };
    let query = format!("SELECT owner_id,head_revision FROM scripts WHERE id=$1 FOR {lock}");
    let row = tx
        .query_opt(&query, &[&script])
        .await?
        .ok_or(StoreError::NotFound)?;
    let owner: String = row.get(0);
    let head: i64 = row.get(1);
    let access = if owner == actor {
        Access::Owner
    } else {
        match tx
            .query_opt(
                "SELECT role FROM script_members WHERE script_id=$1 AND actor_id=$2",
                &[&script, &actor],
            )
            .await?
        {
            Some(row) => match row.get::<_, String>(0).as_str() {
                "reader" => Access::Reader,
                "editor" => Access::Editor,
                _ => return Err(StoreError::CorruptRevision),
            },
            None => Access::None,
        }
    };
    if !permits(access, action) {
        return Err(if access == Access::None {
            StoreError::NotFound
        } else {
            StoreError::Forbidden
        });
    }
    Ok((
        owner,
        u64::try_from(head).map_err(|_| StoreError::CorruptRevision)?,
    ))
}

pub(crate) fn decode_revision(row: &Row) -> Result<RevisionResponse, StoreError> {
    let bytes: Vec<u8> = row.get("canonical_export");
    let content = read_canonical_script(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    let digest: String = row.get("content_digest");
    let export_hash: String = row.get("export_digest");
    if content.content_digest().to_string() != digest || export_digest(&bytes) != export_hash {
        return Err(StoreError::CorruptRevision);
    }
    Ok(RevisionResponse {
        script_id: row.get("script_id"),
        revision: u64::try_from(row.get::<_, i64>("revision"))
            .map_err(|_| StoreError::CorruptRevision)?,
        accepted_by: row.get("accepted_by"),
        accepted_at: row.get("accepted_at"),
        content_digest: digest,
        export_digest: export_hash,
        script_json: String::from_utf8(bytes).map_err(|_| StoreError::CorruptRevision)?,
    })
}

/// The migration actor owns DDL; the HTTP application role has no DDL rights.
pub async fn migrate(config: &Config) -> Result<(), StoreError> {
    let (mut client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let tx = client.transaction().await?;
    tx.batch_execute("SELECT pg_advisory_xact_lock(1128353364); CREATE TABLE IF NOT EXISTS cantos_migrations(version integer PRIMARY KEY, checksum bytea NOT NULL); REVOKE ALL ON cantos_migrations FROM PUBLIC;").await?;
    for (version, migration) in MIGRATIONS {
        let checksum = token_hash(migration);
        match tx
            .query_opt(
                "SELECT checksum FROM cantos_migrations WHERE version=$1",
                &[&version],
            )
            .await?
        {
            Some(row) if row.get::<_, Vec<u8>>(0) == checksum => (),
            Some(_) => return Err(StoreError::CorruptRevision),
            None => {
                tx.batch_execute(migration).await?;
                tx.execute(
                    "INSERT INTO cantos_migrations VALUES($1,$2)",
                    &[&version, &checksum],
                )
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(())
}
