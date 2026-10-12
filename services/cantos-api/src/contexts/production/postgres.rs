//! Script-scoped append-only casting and authorization on the existing identity/evidence store.
use cantos_contracts::{
    ApproveProductionRequest, FieldIssue, FreezeProductionRequest, FrozenProductionDocument,
    ProductionApprovalResponse, ProductionEligibility, ProductionFinding, ProductionFindingCode,
    ProductionPreviewResponse, ProductionRightsClaim, ProductionRightsClaimResponse,
    ProductionRightsSubject, ProductionSettings, ProductionSettingsResponse,
    ProductionSnapshotResponse, ProductionSnapshotSummary, ProductionStateResponse,
    SaveProductionRightsRequest, SaveProductionSettingsRequest,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio_postgres::Row;
use uuid::Uuid;

use crate::contexts::editorial::{
    authorize, ensure_evidence, export_digest, find_revision, load_revision,
    reviewed_by_active_owner, rights_cited_by_script, EvidenceKind,
};
use crate::contexts::editorial::{permits, Access, Action};
use crate::contexts::production::{
    admit_frozen_document, evaluate_snapshot, prepare_inputs, validate_rights_declaration,
    validate_settings,
};
use crate::platform::postgres::{
    authenticate, checked_number, normalized_uuid, LockMode, Store, StoreError,
};
use crate::script_ir::read_canonical_script;

const SETTINGS_COLUMNS: &str = "script_id,version,script_revision,operation_id,recorded_by,settings_bytes,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
const RIGHTS_COLUMNS: &str = "record_id,version,operation_id,recorded_by,claim_bytes,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
const SNAPSHOT_COLUMNS: &str = "id,script_id,owner_id,script_revision,settings_version,operation_id,created_by,input_digest,document_bytes,to_char(recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
const APPROVAL_COLUMNS: &str = "id,snapshot_id,script_id,operation_id,approved_by,input_digest,to_char(approved_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS approved_at";
const MAX_RIGHTS: usize = 128;
const HISTORY_LIMIT: i64 = 20;

fn encode<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(value).map_err(|_| StoreError::InvalidRequest)?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(StoreError::InvalidRequest);
    }
    Ok(bytes)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRevision)
}

fn positive(value: i64) -> Result<u64, StoreError> {
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

/// The session lock already prevents concurrent revocation; active actor changes must wait too.
async fn production_actor(
    tx: &deadpool_postgres::Transaction<'_>,
    token: &str,
) -> Result<String, StoreError> {
    let actor = authenticate(tx, token).await?;
    let active: Option<bool> = tx
        .query_one("SELECT production_lock_actor($1)", &[&actor])
        .await?
        .get(0);
    if active != Some(true) {
        return Err(StoreError::Unauthenticated);
    }
    Ok(actor)
}

async fn now(tx: &deadpool_postgres::Transaction<'_>) -> Result<i64, StoreError> {
    Ok(tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .await?
        .get(0))
}

async fn settings(
    tx: &deadpool_postgres::Transaction<'_>,
    script: &str,
) -> Result<Option<ProductionSettingsResponse>, StoreError> {
    let query = format!("SELECT {SETTINGS_COLUMNS} FROM production_settings WHERE script_id=$1 ORDER BY version DESC LIMIT 1");
    tx.query_opt(&query, &[&script])
        .await?
        .as_ref()
        .map(decode_settings)
        .transpose()
}

async fn rights(
    tx: &deadpool_postgres::Transaction<'_>,
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

async fn reviewed(
    tx: &deadpool_postgres::Transaction<'_>,
    script: &str,
    revision: u64,
    owner: &str,
) -> Result<bool, StoreError> {
    reviewed_by_active_owner(tx, script, checked_number(revision)?, owner).await
}

async fn revision(
    tx: &deadpool_postgres::Transaction<'_>,
    script: &str,
    head: u64,
) -> Result<cantos_contracts::RevisionResponse, StoreError> {
    load_revision(tx, script, checked_number(head)?).await
}

struct Facts {
    owner: String,
    head: u64,
    settings: Option<ProductionSettingsResponse>,
    rights: Vec<ProductionRightsClaimResponse>,
    reviewed: bool,
}

async fn facts(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    script: &str,
    action: Action,
    write: bool,
) -> Result<Facts, StoreError> {
    // All production writes lock this row before rights/settings rows. Reads share the same order.
    let lock = if write {
        LockMode::Update
    } else {
        LockMode::Share
    };
    let (owner, head) = authorize(tx, actor, script, action, lock).await?;
    if actor != owner {
        let role: Option<String> = tx
            .query_one("SELECT production_lock_member($1,$2)", &[&script, &actor])
            .await?
            .get(0);
        let access = match role.as_deref() {
            Some("reader") => Access::Reader,
            Some("editor") => Access::Editor,
            None => return Err(StoreError::NotFound),
            Some(_) => return Err(StoreError::CorruptRevision),
        };
        if !permits(access, action) {
            return Err(StoreError::Forbidden);
        }
    }
    let owner_active: Option<bool> = tx
        .query_one("SELECT production_lock_actor($1)", &[&owner])
        .await?
        .get(0);
    Ok(Facts {
        settings: settings(tx, script).await?,
        rights: rights(tx, script, &owner).await?,
        reviewed: owner_active == Some(true) && reviewed(tx, script, head, &owner).await?,
        owner,
        head,
    })
}

fn blocked(findings: &[ProductionFinding]) -> StoreError {
    StoreError::ProductionBlocked(
        findings
            .iter()
            .map(|finding| FieldIssue {
                path: finding.path.clone(),
                rule: serde_json::to_value(finding.code)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "production_blocked".into()),
            })
            .collect(),
    )
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

async fn snapshot(
    tx: &deadpool_postgres::Transaction<'_>,
    row: &Row,
    facts: &Facts,
) -> Result<ProductionSnapshotResponse, StoreError> {
    let doc = read_snapshot_document(row)?;
    let id: String = row.get("id");
    let script: String = row.get("script_id");
    let operation_id: String = row.get("operation_id");
    for value in [&id, &script, &operation_id] {
        normalized_uuid(value).map_err(|_| StoreError::CorruptRevision)?;
    }
    // Numbers and self-consistent hashes do not establish which settled receipts were pinned.
    // Rebuild from the actual historical rows, including rights actor/time/version metadata.
    let pinned_revision = find_revision(tx, &script, checked_number(doc.revision.revision)?)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let query = format!(
        "SELECT {SETTINGS_COLUMNS} FROM production_settings WHERE script_id=$1 AND version=$2"
    );
    let pinned_settings = decode_settings(
        &tx.query_opt(&query, &[&script, &checked_number(doc.settings.version)?])
            .await?
            .ok_or(StoreError::CorruptRevision)?,
    )?;
    let record_ids: Vec<_> = doc
        .rights
        .iter()
        .map(|claim| claim.claim.declaration.record_id.clone())
        .collect();
    let versions: Vec<_> = doc
        .rights
        .iter()
        .map(|claim| checked_number(claim.claim.version))
        .collect::<Result<_, _>>()?;
    let pinned_rows = tx.query("SELECT c.record_id,c.version,c.operation_id,c.recorded_by,c.claim_bytes,to_char(c.recorded_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at FROM production_rights_claims c JOIN unnest($3::text[],$4::bigint[]) AS pin(record_id,version) ON pin.record_id=c.record_id AND pin.version=c.version WHERE c.script_id=$1 AND c.owner_id=$2 ORDER BY c.record_id", &[&script,&doc.owner_id,&record_ids,&versions]).await?;
    if pinned_rows.len() != doc.rights.len() {
        return Err(StoreError::CorruptRevision);
    }
    let pinned_rights: Vec<_> = pinned_rows
        .iter()
        .map(decode_rights)
        .collect::<Result<_, _>>()?;
    admit_frozen_document(&doc, &pinned_revision, &pinned_settings, &pinned_rights)
        .map_err(|_| StoreError::CorruptRevision)?;
    let query = format!(
        "SELECT {APPROVAL_COLUMNS} FROM production_approvals WHERE snapshot_id=$1 AND script_id=$2"
    );
    let approval = tx
        .query_opt(&query, &[&id, &script])
        .await?
        .as_ref()
        .map(decode_approval)
        .transpose()?;
    if approval
        .as_ref()
        .is_some_and(|approval| approval.input_digest != doc.input_digest)
    {
        return Err(StoreError::CorruptRevision);
    }
    let approver_current = approval
        .as_ref()
        .is_some_and(|approval| approval.approved_by == facts.owner);
    let mut eligibility = evaluate_snapshot(
        &doc,
        facts.head,
        facts
            .settings
            .as_ref()
            .map_or(0, |settings| settings.version),
        &facts.rights,
        facts.reviewed,
        approval.as_ref().filter(|_| approver_current),
        now(tx).await?,
    );
    if doc.owner_id != facts.owner || (approval.is_some() && !approver_current) {
        eligibility.inputs_eligible = false;
        eligibility.approval_current = false;
        eligibility.findings.push(ProductionFinding {
            code: ProductionFindingCode::ApprovalScopeMismatch,
            path: "/approval".into(),
            detail: "The current owner no longer holds the recorded approval scope".into(),
        });
    }
    Ok(ProductionSnapshotResponse {
        id,
        script_id: script,
        created_by: row.get("created_by"),
        recorded_at: row.get("recorded_at"),
        operation_id,
        document: doc,
        approval,
        eligibility,
    })
}

/// Durable workers reuse the exact historical reconstruction and current authority.
/// Locks follow the existing script -> actor -> settings/rights order; never across TTS.
pub(crate) async fn current_snapshot(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    script: &str,
    id: &str,
) -> Result<ProductionSnapshotResponse, StoreError> {
    let active: Option<bool> = tx
        .query_one("SELECT production_lock_actor($1)", &[&actor])
        .await?
        .get(0);
    if active != Some(true) {
        return Err(StoreError::Unauthenticated);
    }
    let facts = facts(tx, actor, script, Action::Review, true).await?;
    let query =
        format!("SELECT {SNAPSHOT_COLUMNS} FROM production_snapshots WHERE id=$1 AND script_id=$2");
    let row = tx
        .query_opt(&query, &[&id, &script])
        .await?
        .ok_or(StoreError::NotFound)?;
    let candidate = snapshot(tx, &row, &facts).await?;
    if !candidate.eligibility.approval_current {
        return Err(blocked(&candidate.eligibility.findings));
    }
    Ok(candidate)
}

pub(crate) async fn durable_actor(
    tx: &deadpool_postgres::Transaction<'_>,
    token: &str,
) -> Result<String, StoreError> {
    production_actor(tx, token).await
}

impl Store {
    pub async fn production_state(
        &self,
        token: &str,
        script: &str,
    ) -> Result<ProductionStateResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Read, false).await?;
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
        let result = ProductionStateResponse {
            settings: facts.settings,
            rights: facts.rights,
            snapshots,
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn save_production_settings(
        &self,
        token: &str,
        script: &str,
        request: SaveProductionSettingsRequest,
    ) -> Result<ProductionSettingsResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 256 * 1024)?;
        let settings_bytes = encode(&request.settings, 256 * 1024)?;
        validate_settings(&request.settings).map_err(StoreError::InvalidRequestFields)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Write, true).await?;
        let query = format!("SELECT {SETTINGS_COLUMNS},request_bytes FROM production_settings WHERE script_id=$1 AND recorded_by=$2 AND operation_id=$3");
        if let Some(row) = tx.query_opt(&query, &[&script, &actor, &operation]).await? {
            if row.get::<_, Vec<u8>>("request_bytes") != bytes {
                return Err(StoreError::OperationReused);
            }
            let result = decode_settings(&row)?;
            tx.commit().await?;
            return Ok(result);
        }
        let current = facts
            .settings
            .as_ref()
            .map_or(0, |settings| settings.version);
        if facts.head != request.expected_revision {
            return Err(StoreError::StaleRevision(facts.head));
        }
        if current != request.expected_settings_version {
            return Err(StoreError::StaleProductionInputs);
        }
        let version = checked_number(current.checked_add(1).ok_or(StoreError::InvalidRequest)?)?;
        tx.execute("INSERT INTO production_settings(script_id,version,script_revision,operation_id,recorded_by,request_bytes,settings_bytes) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&script,&version,&checked_number(facts.head)?,&operation,&actor,&bytes,&settings_bytes]).await?;
        let query = format!(
            "SELECT {SETTINGS_COLUMNS} FROM production_settings WHERE script_id=$1 AND version=$2"
        );
        let result = decode_settings(&tx.query_one(&query, &[&script, &version]).await?)?;
        production_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// A script-scoped assertion, never a legal verdict or an owner-wide consent grant.
    pub async fn save_production_rights(
        &self,
        token: &str,
        script: &str,
        request: SaveProductionRightsRequest,
    ) -> Result<ProductionRightsClaimResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 64 * 1024)?;
        validate_rights_declaration(&request.claim).map_err(StoreError::InvalidRequestFields)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Review, true).await?;
        let query = format!("SELECT {RIGHTS_COLUMNS},request_bytes FROM production_rights_claims WHERE script_id=$1 AND recorded_by=$2 AND operation_id=$3");
        if let Some(row) = tx.query_opt(&query, &[&script, &actor, &operation]).await? {
            if row.get::<_, Vec<u8>>("request_bytes") != bytes {
                return Err(StoreError::OperationReused);
            }
            let result = decode_rights(&row)?;
            tx.commit().await?;
            return Ok(result);
        }
        let prior = facts
            .rights
            .iter()
            .find(|claim| claim.claim.declaration.record_id == request.claim.record_id);
        if prior.map_or(0, |claim| claim.claim.version) != request.expected_version {
            return Err(StoreError::StaleProductionInputs);
        }
        if prior.is_none() && facts.rights.len() >= MAX_RIGHTS {
            return Err(StoreError::InvalidRequest);
        }
        // A record identity cannot be rebound from a source claim to a different voice.
        if prior.is_some_and(|claim| claim.claim.declaration.subject != request.claim.subject) {
            return Err(StoreError::InvalidRequest);
        }
        match &request.claim.subject {
            ProductionRightsSubject::Evidence { record_id } => {
                if !rights_cited_by_script(&tx, &facts.owner, record_id, &script).await? {
                    return Err(StoreError::EvidenceUnavailable);
                }
            }
            ProductionRightsSubject::Voice { .. } => {
                ensure_evidence(
                    &tx,
                    &facts.owner,
                    EvidenceKind::Rights,
                    &request.claim.record_id,
                    "Script-scoped recorded voice permission assertion; not legal verification or provider consent",
                )
                .await?;
            }
        }
        let version = request
            .expected_version
            .checked_add(1)
            .ok_or(StoreError::InvalidRequest)?;
        let claim = ProductionRightsClaim {
            version,
            declaration: request.claim,
        };
        let claim_bytes = encode(&claim, 64 * 1024)?;
        tx.execute("INSERT INTO production_rights_claims(script_id,owner_id,record_id,version,operation_id,recorded_by,request_bytes,claim_bytes) VALUES($1,$2,$3,$4,$5,$6,$7,$8)", &[&script,&facts.owner,&claim.declaration.record_id,&checked_number(version)?,&operation,&actor,&bytes,&claim_bytes]).await?;
        let query = format!("SELECT {RIGHTS_COLUMNS} FROM production_rights_claims WHERE script_id=$1 AND record_id=$2 AND version=$3");
        let result = decode_rights(
            &tx.query_one(
                &query,
                &[
                    &script,
                    &claim.declaration.record_id,
                    &checked_number(version)?,
                ],
            )
            .await?,
        )?;
        production_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn production_preview(
        &self,
        token: &str,
        script: &str,
    ) -> Result<ProductionPreviewResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Read, false).await?;
        let settings = facts
            .settings
            .as_ref()
            .ok_or(StoreError::StaleProductionInputs)?;
        let revision = revision(&tx, &script, facts.head).await?;
        let prepared = prepare_inputs(
            &facts.owner,
            &revision,
            settings,
            &facts.rights,
            facts.reviewed,
            now(&tx).await?,
        )
        .map_err(StoreError::InvalidRequestFields)?;
        tx.commit().await?;
        Ok(prepared.preview())
    }

    pub async fn freeze_production(
        &self,
        token: &str,
        script: &str,
        request: FreezeProductionRequest,
    ) -> Result<ProductionSnapshotResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 64 * 1024)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Review, true).await?;
        let query = format!("SELECT {SNAPSHOT_COLUMNS},request_bytes FROM production_snapshots WHERE script_id=$1 AND created_by=$2 AND operation_id=$3");
        if let Some(row) = tx.query_opt(&query, &[&script, &actor, &operation]).await? {
            if row.get::<_, Vec<u8>>("request_bytes") != bytes {
                return Err(StoreError::OperationReused);
            }
            let result = snapshot(&tx, &row, &facts).await?;
            tx.commit().await?;
            return Ok(result);
        }
        if facts.head != request.expected_revision {
            return Err(StoreError::StaleRevision(facts.head));
        }
        let settings = facts
            .settings
            .as_ref()
            .ok_or(StoreError::StaleProductionInputs)?;
        if settings.version != request.settings_version || settings.script_revision != facts.head {
            return Err(StoreError::StaleProductionInputs);
        }
        let revision = revision(&tx, &script, facts.head).await?;
        let prepared = prepare_inputs(
            &facts.owner,
            &revision,
            settings,
            &facts.rights,
            facts.reviewed,
            now(&tx).await?,
        )
        .map_err(StoreError::InvalidRequestFields)?;
        let document = prepared.document();
        if document.input_digest != request.input_digest {
            return Err(StoreError::StaleProductionInputs);
        }
        // Freeze is preservation only. Pending/missing rights can be inspected, never approved.
        let doc_bytes = encode(&document, 4 * 1024 * 1024)?;
        let id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO production_snapshots(id,script_id,owner_id,script_revision,settings_version,operation_id,created_by,input_digest,request_bytes,document_bytes) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)", &[&id,&script,&facts.owner,&checked_number(facts.head)?,&checked_number(settings.version)?,&operation,&actor,&document.input_digest,&bytes,&doc_bytes]).await?;
        let query = format!(
            "SELECT {SNAPSHOT_COLUMNS} FROM production_snapshots WHERE id=$1 AND script_id=$2"
        );
        let result = snapshot(&tx, &tx.query_one(&query, &[&id, &script]).await?, &facts).await?;
        production_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn production_snapshot(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<ProductionSnapshotResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Read, false).await?;
        let query = format!(
            "SELECT {SNAPSHOT_COLUMNS} FROM production_snapshots WHERE id=$1 AND script_id=$2"
        );
        let row = tx
            .query_opt(&query, &[&id, &script])
            .await?
            .ok_or(StoreError::NotFound)?;
        let result = snapshot(&tx, &row, &facts).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn production_eligibility(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<ProductionEligibility, StoreError> {
        Ok(self
            .production_snapshot(token, script, id)
            .await?
            .eligibility)
    }

    pub async fn approve_production(
        &self,
        token: &str,
        script: &str,
        id: &str,
        request: ApproveProductionRequest,
    ) -> Result<ProductionApprovalResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 64 * 1024)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Review, true).await?;
        let query = format!("SELECT {APPROVAL_COLUMNS},request_bytes FROM production_approvals WHERE script_id=$1 AND approved_by=$2 AND operation_id=$3");
        if let Some(row) = tx.query_opt(&query, &[&script, &actor, &operation]).await? {
            if row.get::<_, Vec<u8>>("request_bytes") != bytes
                || row.get::<_, String>("snapshot_id") != id
            {
                return Err(StoreError::OperationReused);
            }
            let result = decode_approval(&row)?;
            tx.commit().await?;
            return Ok(result);
        }
        let query = format!(
            "SELECT {SNAPSHOT_COLUMNS} FROM production_snapshots WHERE id=$1 AND script_id=$2"
        );
        let row = tx
            .query_opt(&query, &[&id, &script])
            .await?
            .ok_or(StoreError::NotFound)?;
        let candidate = snapshot(&tx, &row, &facts).await?;
        if candidate.document.input_digest != request.input_digest {
            return Err(StoreError::StaleProductionInputs);
        }
        if !candidate.eligibility.inputs_eligible {
            return Err(blocked(&candidate.eligibility.findings));
        }
        if candidate.approval.is_some() {
            return Err(StoreError::StaleProductionInputs);
        }
        let approval_id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO production_approvals(id,snapshot_id,script_id,operation_id,approved_by,input_digest,request_bytes) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&approval_id,&id,&script,&operation,&actor,&request.input_digest,&bytes]).await?;
        // A rights term can expire while waiting for a lock. Evaluate again immediately before commit.
        let final_candidate = snapshot(&tx, &row, &facts).await?;
        if !final_candidate.eligibility.inputs_eligible
            || !final_candidate.eligibility.approval_current
        {
            return Err(blocked(&final_candidate.eligibility.findings));
        }
        production_actor(&tx, token).await?;
        let result = final_candidate
            .approval
            .ok_or(StoreError::CorruptRevision)?;
        tx.commit().await?;
        Ok(result)
    }
}
