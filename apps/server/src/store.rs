//! PostgreSQL adapter. Every accepted edit, head transition and actor record commits together.
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

use crate::auth::{Actor, authorize_owner};
use crate::editorial::{
    self, AcceptRequest, CreateRequest, EditError, NewScript, RightsInput, SaveRequest, SourceInput,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Edit(#[from] EditError),
    #[error("storage error")]
    Database(#[from] sqlx::Error),
}

#[derive(Serialize)]
pub(crate) struct SavedVersion {
    pub script_id: Uuid,
    pub version: i64,
    pub base_version: i64,
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub content: Value,
    pub digest: String,
}

#[derive(Serialize)]
pub(crate) struct SourceRecord {
    #[serde(flatten)]
    input: SourceInput,
    sha256: String,
    actor_id: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(crate) struct RightsRecord {
    #[serde(flatten)]
    input: RightsInput,
    status: String,
    actor_id: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub(crate) struct ScriptView {
    pub id: Uuid,
    owner_id: Uuid,
    pub head: SavedVersion,
    source: SourceRecord,
    rights: RightsRecord,
}

#[derive(Serialize)]
pub(crate) struct AcceptedRevision {
    id: Uuid,
    script_id: Uuid,
    version: i64,
    operation_id: Uuid,
    actor_id: Uuid,
    created_at: DateTime<Utc>,
    digest: String,
    content: Value,
}

#[derive(Serialize)]
pub(crate) struct History {
    versions: Vec<SavedVersion>,
    revisions: Vec<AcceptedRevision>,
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

async fn authorized_head(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: Uuid,
    lock: bool,
) -> Result<i64, StoreError> {
    let query = if lock {
        "SELECT owner_id, head_version FROM scripts WHERE id = $1 FOR UPDATE"
    } else {
        "SELECT owner_id, head_version FROM scripts WHERE id = $1"
    };
    let row = sqlx::query(query)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(StoreError::NotFound)?;
    if !authorize_owner(actor, row.try_get("owner_id")?) {
        return Err(StoreError::NotFound);
    }
    Ok(row.try_get("head_version")?)
}

fn decode_version(row: &PgRow) -> Result<SavedVersion, StoreError> {
    let canonical: Vec<u8> = row.try_get("canonical")?;
    let digest: String = row.try_get("digest")?;
    let content = editorial::verify_stored(
        row.try_get("content")?,
        &canonical,
        &digest,
        &row.try_get::<Vec<u8>, _>("wire_digest")?,
    )?;
    Ok(SavedVersion {
        script_id: row.try_get("script_id")?,
        version: row.try_get("version")?,
        base_version: row.try_get("base_version")?,
        operation_id: row.try_get("operation_id")?,
        actor_id: row.try_get("actor_id")?,
        created_at: row.try_get("created_at")?,
        content: content.as_json(),
        digest,
    })
}

async fn version_row(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    version: i64,
) -> Result<PgRow, StoreError> {
    sqlx::query("SELECT * FROM script_versions WHERE script_id = $1 AND version = $2")
        .bind(id)
        .bind(version)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(StoreError::NotFound)
}

async fn script_view(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    owner: Uuid,
    version: i64,
) -> Result<ScriptView, StoreError> {
    let head = decode_version(&version_row(tx, id, version).await?)?;
    let source = sqlx::query("SELECT * FROM source_records WHERE script_id = $1")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    let rights = sqlx::query("SELECT * FROM rights_records WHERE script_id = $1")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    let original_text: String = source.try_get("original_text")?;
    let source_digest: String = source.try_get("sha256")?;
    let content = crate::script_ir::parse(head.content.clone()).map_err(|_| EditError::Corrupt)?;
    if format!("{:x}", Sha256::digest(original_text.as_bytes())) != source_digest
        || content.source_ref() != source.try_get::<String, _>("id")?
        || content.rights_ref() != rights.try_get::<String, _>("id")?
    {
        return Err(EditError::Corrupt.into());
    }
    let kind: String = source.try_get("kind")?;
    let kind = match kind.as_str() {
        "original" => editorial::SourceKind::Original,
        "imported" => editorial::SourceKind::Imported,
        _ => return Err(EditError::Corrupt.into()),
    };
    Ok(ScriptView {
        id,
        owner_id: owner,
        head,
        source: SourceRecord {
            input: SourceInput {
                id: source.try_get("id")?,
                kind,
                reference: source.try_get("reference")?,
                original_text: source.try_get("original_text")?,
            },
            sha256: source.try_get("sha256")?,
            actor_id: source.try_get("actor_id")?,
            created_at: source.try_get("created_at")?,
        },
        rights: RightsRecord {
            input: RightsInput {
                id: rights.try_get("id")?,
                holder: rights.try_get("holder")?,
                evidence_reference: rights.try_get("evidence_reference")?,
            },
            status: rights.try_get("status")?,
            actor_id: rights.try_get("actor_id")?,
            created_at: rights.try_get("created_at")?,
        },
    })
}

pub(crate) async fn create(
    pool: &PgPool,
    actor: &Actor,
    raw: CreateRequest,
) -> Result<ScriptView, StoreError> {
    let command = editorial::prepare_create(raw)?;
    let mut tx = pool.begin().await?;
    // Serializes creation and retries for this actor; no global lock or in-memory dedupe.
    sqlx::query("SELECT id FROM creators WHERE id = $1 FOR UPDATE")
        .bind(actor.id())
        .fetch_one(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query(
        "SELECT id, create_request FROM scripts WHERE owner_id = $1 AND create_operation_id = $2",
    )
    .bind(actor.id())
    .bind(command.operation_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        let request: Vec<u8> = row.try_get("create_request")?;
        if request != command.request {
            return Err(EditError::OperationReused.into());
        }
        let view = script_view(&mut tx, row.try_get("id")?, actor.id(), 1).await?;
        tx.commit().await?;
        return Ok(view);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO scripts (id, owner_id, work_id, episode_id, head_version, create_operation_id, create_request) VALUES ($1,$2,$3,$4,1,$5,$6)")
        .bind(id).bind(actor.id()).bind(command.content.work_id()).bind(command.content.episode_id())
        .bind(command.operation_id).bind(&command.request).execute(&mut *tx).await?;
    insert_provenance(&mut tx, id, actor.id(), &command).await?;
    insert_version(
        &mut tx,
        id,
        actor.id(),
        0,
        command.operation_id,
        &command.content,
        &command.request,
    )
    .await?;
    let view = script_view(&mut tx, id, actor.id(), 1).await?;
    tx.commit().await?;
    Ok(view)
}

async fn insert_provenance(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    actor: Uuid,
    command: &NewScript,
) -> Result<(), StoreError> {
    let kind = match command.source.kind {
        editorial::SourceKind::Original => "original",
        editorial::SourceKind::Imported => "imported",
    };
    let source_digest = format!(
        "{:x}",
        Sha256::digest(command.source.original_text.as_bytes())
    );
    sqlx::query("INSERT INTO source_records (script_id,id,kind,reference,original_text,sha256,actor_id) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(id).bind(&command.source.id).bind(kind).bind(&command.source.reference)
        .bind(&command.source.original_text).bind(source_digest).bind(actor).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO rights_records (script_id,id,holder,evidence_reference,actor_id) VALUES ($1,$2,$3,$4,$5)")
        .bind(id).bind(&command.rights.id).bind(&command.rights.holder).bind(&command.rights.evidence_reference).bind(actor)
        .execute(&mut **tx).await?;
    Ok(())
}

async fn insert_version(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    actor: Uuid,
    base: i64,
    operation: Uuid,
    content: &crate::script_ir::ScriptContent,
    request: &[u8],
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO script_versions (script_id,version,base_version,operation_id,actor_id,content,canonical,digest,request,wire_digest) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(id).bind(base + 1).bind(base).bind(operation).bind(actor).bind(content.as_json())
        .bind(content.canonical_bytes()).bind(content.digest()).bind(request).bind(editorial::fingerprint(&content.as_json())?).execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn get(pool: &PgPool, actor: &Actor, id: Uuid) -> Result<ScriptView, StoreError> {
    let mut tx = pool.begin().await?;
    let head = authorized_head(&mut tx, actor, id, false).await?;
    let view = script_view(&mut tx, id, actor.id(), head).await?;
    tx.commit().await?;
    Ok(view)
}

pub(crate) async fn get_version(
    pool: &PgPool,
    actor: &Actor,
    id: Uuid,
    version: i64,
) -> Result<SavedVersion, StoreError> {
    let mut tx = pool.begin().await?;
    authorized_head(&mut tx, actor, id, false).await?;
    let saved = decode_version(&version_row(&mut tx, id, version).await?)?;
    tx.commit().await?;
    Ok(saved)
}

pub(crate) async fn save(
    pool: &PgPool,
    actor: &Actor,
    id: Uuid,
    raw: SaveRequest,
) -> Result<SavedVersion, StoreError> {
    // Resource authorization precedes validation and all replay lookups.
    let mut tx = pool.begin().await?;
    let head = authorized_head(&mut tx, actor, id, true).await?;
    let command = editorial::prepare_save(raw)?;
    let prior = sqlx::query(
        "SELECT version, request FROM script_versions WHERE script_id = $1 AND operation_id = $2",
    )
    .bind(id)
    .bind(command.operation_id)
    .fetch_optional(&mut *tx)
    .await?;
    let prior = prior
        .map(|row| -> Result<(i64, Vec<u8>), sqlx::Error> {
            Ok((row.try_get("version")?, row.try_get("request")?))
        })
        .transpose()?;
    let version = editorial::decide_save(
        head,
        command.base_version,
        &command.request,
        prior
            .as_ref()
            .map(|(version, request)| (*version, request.as_slice())),
    )?;
    if prior.is_none() {
        let previous_row = version_row(&mut tx, id, head).await?;
        let previous = editorial::verify_stored(
            previous_row.try_get("content")?,
            &previous_row.try_get::<Vec<u8>, _>("canonical")?,
            &previous_row.try_get::<String, _>("digest")?,
            &previous_row.try_get::<Vec<u8>, _>("wire_digest")?,
        )?;
        editorial::require_identity(&command.content, &previous)?;
        insert_version(
            &mut tx,
            id,
            actor.id(),
            command.base_version,
            command.operation_id,
            &command.content,
            &command.request,
        )
        .await?;
        let changed =
            sqlx::query("UPDATE scripts SET head_version = $1 WHERE id = $2 AND head_version = $3")
                .bind(version)
                .bind(id)
                .bind(command.base_version)
                .execute(&mut *tx)
                .await?;
        if changed.rows_affected() != 1 {
            return Err(EditError::Conflict { head_version: head }.into());
        }
    }
    let saved = decode_version(&version_row(&mut tx, id, version).await?)?;
    tx.commit().await?;
    Ok(saved)
}

async fn revision(
    tx: &mut Transaction<'_, Postgres>,
    script: Uuid,
    id: Uuid,
) -> Result<AcceptedRevision, StoreError> {
    let row = sqlx::query("SELECT * FROM script_revisions WHERE script_id = $1 AND id = $2")
        .bind(script)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(StoreError::NotFound)?;
    let version = decode_version(&version_row(tx, script, row.try_get("version")?).await?)?;
    Ok(AcceptedRevision {
        id,
        script_id: script,
        version: version.version,
        operation_id: row.try_get("operation_id")?,
        actor_id: row.try_get("actor_id")?,
        created_at: row.try_get("created_at")?,
        digest: version.digest,
        content: version.content,
    })
}

pub(crate) async fn accept(
    pool: &PgPool,
    actor: &Actor,
    id: Uuid,
    raw: AcceptRequest,
) -> Result<AcceptedRevision, StoreError> {
    let mut tx = pool.begin().await?;
    let head = authorized_head(&mut tx, actor, id, true).await?;
    if raw.operation_id.is_nil() || raw.version < 1 {
        return Err(EditError::InvalidRequest.into());
    }
    if let Some(row) = sqlx::query("SELECT version, revision_id FROM revision_operations WHERE script_id = $1 AND operation_id = $2")
        .bind(id).bind(raw.operation_id).fetch_optional(&mut *tx).await? {
        if row.try_get::<i64, _>("version")? != raw.version { return Err(EditError::OperationReused.into()); }
        let result = revision(&mut tx, id, row.try_get("revision_id")?).await?;
        tx.commit().await?;
        return Ok(result);
    }
    // Accepting an already accepted saved version returns its immutable original revision.
    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM script_revisions WHERE script_id = $1 AND version = $2",
    )
    .bind(id)
    .bind(raw.version)
    .fetch_optional(&mut *tx)
    .await?;
    let revision_id = if let Some(existing) = existing {
        existing
    } else {
        editorial::require_head(head, raw.version)?;
        decode_version(&version_row(&mut tx, id, raw.version).await?)?;
        let revision_id = Uuid::new_v4();
        sqlx::query("INSERT INTO script_revisions (id,script_id,version,operation_id,actor_id) VALUES ($1,$2,$3,$4,$5)")
            .bind(revision_id).bind(id).bind(raw.version).bind(raw.operation_id).bind(actor.id()).execute(&mut *tx).await?;
        revision_id
    };
    sqlx::query("INSERT INTO revision_operations (script_id,operation_id,version,revision_id,actor_id) VALUES ($1,$2,$3,$4,$5)")
        .bind(id).bind(raw.operation_id).bind(raw.version).bind(revision_id).bind(actor.id()).execute(&mut *tx).await?;
    let result = revision(&mut tx, id, revision_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn get_revision(
    pool: &PgPool,
    actor: &Actor,
    script: Uuid,
    id: Uuid,
) -> Result<AcceptedRevision, StoreError> {
    let mut tx = pool.begin().await?;
    authorized_head(&mut tx, actor, script, false).await?;
    let result = revision(&mut tx, script, id).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn history(
    pool: &PgPool,
    actor: &Actor,
    script: Uuid,
    after: i64,
    limit: i64,
) -> Result<History, StoreError> {
    let mut tx = pool.begin().await?;
    authorized_head(&mut tx, actor, script, false).await?;
    if after < 0 || !(1..=100).contains(&limit) {
        return Err(EditError::InvalidRequest.into());
    }
    let rows = sqlx::query("SELECT * FROM script_versions WHERE script_id = $1 AND version > $2 ORDER BY version LIMIT $3")
        .bind(script).bind(after).bind(limit).fetch_all(&mut *tx).await?;
    let versions = rows
        .iter()
        .map(decode_version)
        .collect::<Result<Vec<_>, _>>()?;
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM script_revisions WHERE script_id = $1 AND version > $2 ORDER BY version LIMIT $3")
        .bind(script).bind(after).bind(limit).fetch_all(&mut *tx).await?;
    let mut revisions = Vec::new();
    for id in ids {
        revisions.push(revision(&mut tx, script, id).await?);
    }
    tx.commit().await?;
    Ok(History {
        versions,
        revisions,
    })
}
