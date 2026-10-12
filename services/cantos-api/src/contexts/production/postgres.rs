//! Production tables: versioned settings and rights claims, frozen snapshots and approvals, all
//! append-only. Every decoder re-checks stored bytes against the pure admission rules.
use cantos_contracts::{
    FrozenProductionDocument, ProductionApprovalResponse, ProductionRightsClaim,
    ProductionRightsClaimResponse, ProductionSettings, ProductionSettingsResponse,
    ProductionSnapshotSummary,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio_postgres::Row;

use super::domain::{
    admit_frozen_document, validate_rights_declaration, validate_settings, MAX_RIGHTS,
};
use crate::contexts::editorial::export_digest;
use crate::platform::postgres::{normalized_uuid, StoreError, Tx};
use crate::script_ir::read_canonical_script;

const SETTINGS_COLUMNS: &str = "script_id,version,script_revision,operation_id,recorded_by,settings_bytes,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
const RIGHTS_COLUMNS: &str = "record_id,version,operation_id,recorded_by,claim_bytes,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
const SNAPSHOT_COLUMNS: &str = "id,script_id,owner_id,script_revision,settings_version,operation_id,created_by,input_digest,document_bytes,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
const APPROVAL_COLUMNS: &str = "id,snapshot_id,script_id,operation_id,approved_by,input_digest,to_char(approved_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS approved_at";
const HISTORY_LIMIT: i64 = 20;

/// Bounded request or record bytes; an empty or oversized encoding is refused.
pub(super) fn encode<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(value).map_err(|_| StoreError::InvalidRequest)?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(StoreError::InvalidRequest);
    }
    Ok(bytes)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRevision)
}

pub(super) fn positive(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(StoreError::CorruptRevision)
}

fn decode_settings(row: &Row) -> Result<ProductionSettingsResponse, StoreError> {
    let settings: ProductionSettings = decode(&row.get::<_, Vec<u8>>("settings_bytes"))?;
    validate_settings(&settings).map_err(|_| StoreError::CorruptRevision)?;
    let response = ProductionSettingsResponse {
        script_id: row.get("script_id"),
        version: positive(row.get("version"))?,
        script_revision: positive(row.get("script_revision"))?,
        operation_id: row.get("operation_id"),
        recorded_by: row.get("recorded_by"),
        recorded_at: row.get("recorded_at"),
        settings,
    };
    normalized_uuid(&response.script_id).map_err(|_| StoreError::CorruptRevision)?;
    normalized_uuid(&response.operation_id).map_err(|_| StoreError::CorruptRevision)?;
    Ok(response)
}

fn decode_rights(row: &Row) -> Result<ProductionRightsClaimResponse, StoreError> {
    let claim: ProductionRightsClaim = decode(&row.get::<_, Vec<u8>>("claim_bytes"))?;
    validate_rights_declaration(&claim.declaration).map_err(|_| StoreError::CorruptRevision)?;
    if claim.version != positive(row.get("version"))?
        || claim.declaration.record_id != row.get::<_, String>("record_id")
    {
        return Err(StoreError::CorruptRevision);
    }
    let response = ProductionRightsClaimResponse {
        claim,
        operation_id: row.get("operation_id"),
        recorded_by: row.get("recorded_by"),
        recorded_at: row.get("recorded_at"),
    };
    normalized_uuid(&response.operation_id).map_err(|_| StoreError::CorruptRevision)?;
    Ok(response)
}

fn decode_approval(row: &Row) -> Result<ProductionApprovalResponse, StoreError> {
    let response = ProductionApprovalResponse {
        id: row.get("id"),
        snapshot_id: row.get("snapshot_id"),
        approved_by: row.get("approved_by"),
        approved_at: row.get("approved_at"),
        operation_id: row.get("operation_id"),
        input_digest: row.get("input_digest"),
    };
    for id in [&response.id, &response.snapshot_id, &response.operation_id] {
        normalized_uuid(id).map_err(|_| StoreError::CorruptRevision)?;
    }
    Ok(response)
}

fn read_snapshot_document(row: &Row) -> Result<FrozenProductionDocument, StoreError> {
    let doc: FrozenProductionDocument = decode(&row.get::<_, Vec<u8>>("document_bytes"))?;
    let script: String = row.get("script_id");
    if doc.owner_id != row.get::<_, String>("owner_id")
        || doc.revision.script_id != script
        || doc.settings.script_id != script
        || doc.revision.revision != positive(row.get("script_revision"))?
        || doc.settings.version != positive(row.get("settings_version"))?
        || doc.settings.script_revision != doc.revision.revision
        || doc.input_digest != row.get::<_, String>("input_digest")
    {
        return Err(StoreError::CorruptRevision);
    }
    let content = read_canonical_script(doc.revision.script_json.as_bytes())
        .map_err(|_| StoreError::CorruptRevision)?;
    if content.content_digest().to_string() != doc.revision.content_digest
        || export_digest(doc.revision.script_json.as_bytes()) != doc.revision.export_digest
    {
        return Err(StoreError::CorruptRevision);
    }
    admit_frozen_document(&doc, &doc.revision, &doc.settings, &doc.rights)
        .map_err(|_| StoreError::CorruptRevision)?;
    Ok(doc)
}

/// Whether `actor` is still active, locking the actor row against a concurrent change.
pub(super) async fn lock_actor_active(tx: &Tx<'_>, actor: &str) -> Result<bool, StoreError> {
    let active: Option<bool> = tx
        .query_one("SELECT production_lock_actor($1)", &[&actor])
        .await?
        .get(0);
    Ok(active == Some(true))
}

/// The member role of `actor` on `script`, locking the membership against a concurrent change.
pub(super) async fn lock_member_role(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_one("SELECT production_lock_member($1,$2)", &[&script, &actor])
        .await?
        .get(0))
}

/// The database clock in whole Unix seconds: one instant for every rights term in a decision.
pub(super) async fn now(tx: &Tx<'_>) -> Result<i64, StoreError> {
    Ok(tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .await?
        .get(0))
}

pub(super) async fn current_settings(
    tx: &Tx<'_>,
    script: &str,
) -> Result<Option<ProductionSettingsResponse>, StoreError> {
    let query = format!("SELECT {SETTINGS_COLUMNS} FROM production_settings WHERE script_id=$1 ORDER BY version DESC LIMIT 1");
    tx.query_opt(&query, &[&script])
        .await?
        .as_ref()
        .map(decode_settings)
        .transpose()
}

pub(super) async fn current_rights(
    tx: &Tx<'_>,
    script: &str,
    owner: &str,
) -> Result<Vec<ProductionRightsClaimResponse>, StoreError> {
    let query = format!("SELECT DISTINCT ON(record_id) {RIGHTS_COLUMNS} FROM production_rights_claims WHERE script_id=$1 AND owner_id=$2 ORDER BY record_id,version DESC LIMIT 129");
    let rows = tx.query(&query, &[&script, &owner]).await?;
    if rows.len() > MAX_RIGHTS {
        return Err(StoreError::CorruptRevision);
    }
    rows.iter().map(decode_rights).collect()
}

/// A command already recorded under its operation ID. Its request bytes decide a replay; the
/// stored record is decoded, and so integrity-checked, only when it is replayed.
pub(super) struct RecordedOperation(Row);

impl RecordedOperation {
    pub(super) fn request_bytes(&self) -> Vec<u8> {
        self.0.get("request_bytes")
    }

    pub(super) fn snapshot_id(&self) -> String {
        self.0.get("snapshot_id")
    }

    pub(super) fn settings(&self) -> Result<ProductionSettingsResponse, StoreError> {
        decode_settings(&self.0)
    }

    pub(super) fn rights(&self) -> Result<ProductionRightsClaimResponse, StoreError> {
        decode_rights(&self.0)
    }

    pub(super) fn approval(&self) -> Result<ProductionApprovalResponse, StoreError> {
        decode_approval(&self.0)
    }

    pub(super) fn snapshot(self) -> SnapshotRow {
        SnapshotRow(self.0)
    }
}

pub(super) async fn settings_by_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<RecordedOperation>, StoreError> {
    let query = format!("SELECT {SETTINGS_COLUMNS},request_bytes FROM production_settings WHERE script_id=$1 AND recorded_by=$2 AND operation_id=$3");
    Ok(tx
        .query_opt(&query, &[&script, &actor, &operation])
        .await?
        .map(RecordedOperation))
}

pub(super) struct NewSettings<'a> {
    pub script: &'a str,
    pub version: i64,
    pub script_revision: i64,
    pub operation: &'a str,
    pub actor: &'a str,
    pub request_bytes: &'a [u8],
    pub settings_bytes: &'a [u8],
}

pub(super) async fn insert_settings(tx: &Tx<'_>, new: NewSettings<'_>) -> Result<(), StoreError> {
    tx.execute("INSERT INTO production_settings(script_id,version,script_revision,operation_id,recorded_by,request_bytes,settings_bytes) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&new.script,&new.version,&new.script_revision,&new.operation,&new.actor,&new.request_bytes,&new.settings_bytes]).await?;
    Ok(())
}

pub(super) async fn find_settings(
    tx: &Tx<'_>,
    script: &str,
    version: i64,
) -> Result<Option<ProductionSettingsResponse>, StoreError> {
    let query = format!(
        "SELECT {SETTINGS_COLUMNS} FROM production_settings WHERE script_id=$1 AND version=$2"
    );
    tx.query_opt(&query, &[&script, &version])
        .await?
        .as_ref()
        .map(decode_settings)
        .transpose()
}

pub(super) async fn load_settings(
    tx: &Tx<'_>,
    script: &str,
    version: i64,
) -> Result<ProductionSettingsResponse, StoreError> {
    let query = format!(
        "SELECT {SETTINGS_COLUMNS} FROM production_settings WHERE script_id=$1 AND version=$2"
    );
    decode_settings(&tx.query_one(&query, &[&script, &version]).await?)
}

pub(super) async fn rights_by_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<RecordedOperation>, StoreError> {
    let query = format!("SELECT {RIGHTS_COLUMNS},request_bytes FROM production_rights_claims WHERE script_id=$1 AND recorded_by=$2 AND operation_id=$3");
    Ok(tx
        .query_opt(&query, &[&script, &actor, &operation])
        .await?
        .map(RecordedOperation))
}

pub(super) struct NewRights<'a> {
    pub script: &'a str,
    pub owner: &'a str,
    pub record_id: &'a str,
    pub version: i64,
    pub operation: &'a str,
    pub actor: &'a str,
    pub request_bytes: &'a [u8],
    pub claim_bytes: &'a [u8],
}

pub(super) async fn insert_rights(tx: &Tx<'_>, new: NewRights<'_>) -> Result<(), StoreError> {
    tx.execute("INSERT INTO production_rights_claims(script_id,owner_id,record_id,version,operation_id,recorded_by,request_bytes,claim_bytes) VALUES($1,$2,$3,$4,$5,$6,$7,$8)", &[&new.script,&new.owner,&new.record_id,&new.version,&new.operation,&new.actor,&new.request_bytes,&new.claim_bytes]).await?;
    Ok(())
}

pub(super) async fn load_rights(
    tx: &Tx<'_>,
    script: &str,
    record_id: &str,
    version: i64,
) -> Result<ProductionRightsClaimResponse, StoreError> {
    let query = format!("SELECT {RIGHTS_COLUMNS} FROM production_rights_claims WHERE script_id=$1 AND record_id=$2 AND version=$3");
    decode_rights(
        &tx.query_one(&query, &[&script, &record_id, &version])
            .await?,
    )
}

/// The exact claim versions a snapshot pinned, by record; rows that do not exist are absent.
pub(super) async fn pinned_rights(
    tx: &Tx<'_>,
    script: &str,
    owner: &str,
    record_ids: &[String],
    versions: &[i64],
) -> Result<Vec<ProductionRightsClaimResponse>, StoreError> {
    let rows = tx.query("SELECT c.record_id,c.version,c.operation_id,c.recorded_by,c.claim_bytes,to_char(c.recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at FROM production_rights_claims c JOIN unnest($3::text[],$4::bigint[]) AS pin(record_id,version) ON pin.record_id=c.record_id AND pin.version=c.version WHERE c.script_id=$1 AND c.owner_id=$2 ORDER BY c.record_id", &[&script,&owner,&record_ids,&versions]).await?;
    rows.iter().map(decode_rights).collect()
}

/// One `production_snapshots` row.
pub(super) struct SnapshotRow(Row);

impl SnapshotRow {
    pub(super) fn id(&self) -> String {
        self.0.get("id")
    }

    pub(super) fn script_id(&self) -> String {
        self.0.get("script_id")
    }

    pub(super) fn operation_id(&self) -> String {
        self.0.get("operation_id")
    }

    pub(super) fn created_by(&self) -> String {
        self.0.get("created_by")
    }

    pub(super) fn recorded_at(&self) -> String {
        self.0.get("recorded_at")
    }

    /// The frozen document, consistent with its row and admissible against its own pins.
    pub(super) fn document(&self) -> Result<FrozenProductionDocument, StoreError> {
        read_snapshot_document(&self.0)
    }
}

pub(super) async fn find_snapshot(
    tx: &Tx<'_>,
    id: &str,
    script: &str,
) -> Result<Option<SnapshotRow>, StoreError> {
    let query =
        format!("SELECT {SNAPSHOT_COLUMNS} FROM production_snapshots WHERE id=$1 AND script_id=$2");
    Ok(tx
        .query_opt(&query, &[&id, &script])
        .await?
        .map(SnapshotRow))
}

pub(super) async fn load_snapshot(
    tx: &Tx<'_>,
    id: &str,
    script: &str,
) -> Result<SnapshotRow, StoreError> {
    let query =
        format!("SELECT {SNAPSHOT_COLUMNS} FROM production_snapshots WHERE id=$1 AND script_id=$2");
    Ok(SnapshotRow(tx.query_one(&query, &[&id, &script]).await?))
}

pub(super) async fn snapshot_by_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<RecordedOperation>, StoreError> {
    let query = format!("SELECT {SNAPSHOT_COLUMNS},request_bytes FROM production_snapshots WHERE script_id=$1 AND created_by=$2 AND operation_id=$3");
    Ok(tx
        .query_opt(&query, &[&script, &actor, &operation])
        .await?
        .map(RecordedOperation))
}

/// The most recent snapshots of a script, newest first.
pub(super) async fn snapshot_history(
    tx: &Tx<'_>,
    script: &str,
) -> Result<Vec<ProductionSnapshotSummary>, StoreError> {
    let query = "SELECT id,script_id,created_by,input_digest,script_revision,settings_version,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at FROM production_snapshots WHERE script_id=$1 ORDER BY recorded_at DESC,id DESC LIMIT $2";
    let mut snapshots = Vec::new();
    for row in tx.query(query, &[&script, &HISTORY_LIMIT]).await? {
        let summary = ProductionSnapshotSummary {
            id: row.get("id"),
            script_id: row.get("script_id"),
            created_by: row.get("created_by"),
            recorded_at: row.get("recorded_at"),
            input_digest: row.get("input_digest"),
            script_revision: positive(row.get("script_revision"))?,
            settings_version: positive(row.get("settings_version"))?,
        };
        normalized_uuid(&summary.id).map_err(|_| StoreError::CorruptRevision)?;
        snapshots.push(summary);
    }
    Ok(snapshots)
}

pub(super) struct NewSnapshot<'a> {
    pub id: &'a str,
    pub script: &'a str,
    pub owner: &'a str,
    pub script_revision: i64,
    pub settings_version: i64,
    pub operation: &'a str,
    pub actor: &'a str,
    pub input_digest: &'a str,
    pub request_bytes: &'a [u8],
    pub document_bytes: &'a [u8],
}

pub(super) async fn insert_snapshot(tx: &Tx<'_>, new: NewSnapshot<'_>) -> Result<(), StoreError> {
    tx.execute("INSERT INTO production_snapshots(id,script_id,owner_id,script_revision,settings_version,operation_id,created_by,input_digest,request_bytes,document_bytes) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)", &[&new.id,&new.script,&new.owner,&new.script_revision,&new.settings_version,&new.operation,&new.actor,&new.input_digest,&new.request_bytes,&new.document_bytes]).await?;
    Ok(())
}

pub(super) async fn approval_for(
    tx: &Tx<'_>,
    snapshot: &str,
    script: &str,
) -> Result<Option<ProductionApprovalResponse>, StoreError> {
    let query = format!(
        "SELECT {APPROVAL_COLUMNS} FROM production_approvals WHERE snapshot_id=$1 AND script_id=$2"
    );
    tx.query_opt(&query, &[&snapshot, &script])
        .await?
        .as_ref()
        .map(decode_approval)
        .transpose()
}

pub(super) async fn approval_by_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<RecordedOperation>, StoreError> {
    let query = format!("SELECT {APPROVAL_COLUMNS},request_bytes FROM production_approvals WHERE script_id=$1 AND approved_by=$2 AND operation_id=$3");
    Ok(tx
        .query_opt(&query, &[&script, &actor, &operation])
        .await?
        .map(RecordedOperation))
}

pub(super) struct NewApproval<'a> {
    pub id: &'a str,
    pub snapshot: &'a str,
    pub script: &'a str,
    pub operation: &'a str,
    pub actor: &'a str,
    pub input_digest: &'a str,
    pub request_bytes: &'a [u8],
}

pub(super) async fn insert_approval(tx: &Tx<'_>, new: NewApproval<'_>) -> Result<(), StoreError> {
    tx.execute("INSERT INTO production_approvals(id,snapshot_id,script_id,operation_id,approved_by,input_digest,request_bytes) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&new.id,&new.snapshot,&new.script,&new.operation,&new.actor,&new.input_digest,&new.request_bytes]).await?;
    Ok(())
}
