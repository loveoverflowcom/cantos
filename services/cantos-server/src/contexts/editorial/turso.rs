//! Editorial tables: scripts and members, immutable revisions with their evidence links,
//! evidence records and editorial reviews. Every load re-checks stored integrity.
use super::{ReviewFacts, RevisionFacts};
use crate::platform::store::Row;
use uuid::Uuid;

use super::domain::revisions::AcceptedRevision;
use super::domain::{export_digest, EvidenceKind};
use crate::platform::store::{utc_timestamp, StoreError, Tx};
use crate::script_ir::{read_canonical_script, WRITE_VERSION};

const REVISION_COLUMNS: &str = "script_id, revision, expected_revision, accepted_by, accepted_at, schema_version, canonical_export, content_digest, export_digest";

fn review_query() -> String {
    format!("SELECT r.id,r.operation_id,r.reviewed_by,r.reviewed_at,{REVISION_COLUMNS} FROM script_reviews r JOIN script_revisions USING(script_id,revision)")
}

/// The stored canonical bytes must still be the admitted Script IR with both recorded digests.
fn decode_revision(row: &Row) -> Result<RevisionFacts, StoreError> {
    let revision = accepted_sequence(row)?;
    let bytes: Vec<u8> = row.try_get("canonical_export")?;
    let content = read_canonical_script(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    let digest: String = row.try_get("content_digest")?;
    let export_hash: String = row.try_get("export_digest")?;
    if row.try_get::<_, String>("schema_version")? != WRITE_VERSION
        || content.content_digest().to_string() != digest
        || export_digest(&bytes) != export_hash
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(RevisionFacts {
        script_id: row.try_get("script_id")?,
        revision: revision.get(),
        accepted_by: row.try_get("accepted_by")?,
        accepted_at: utc_timestamp(row.try_get("accepted_at")?)?,
        content_digest: digest,
        export_digest: export_hash,
        canonical_export: String::from_utf8(bytes).map_err(|_| StoreError::CorruptRevision)?,
    })
}

fn accepted_sequence(row: &Row) -> Result<AcceptedRevision, StoreError> {
    AcceptedRevision::from_sequence(
        row.try_get("revision")
            .map_err(|_| StoreError::CorruptRevision)?,
        row.try_get("expected_revision")
            .map_err(|_| StoreError::CorruptRevision)?,
    )
    .map_err(|_| StoreError::CorruptRevision)
}

fn decode_review(row: &Row) -> Result<ReviewFacts, StoreError> {
    let revision = decode_revision(row)?;
    Ok(ReviewFacts {
        id: row.try_get("id")?,
        script_id: revision.script_id,
        revision: revision.revision,
        operation_id: row.try_get("operation_id")?,
        reviewed_by: row.try_get("reviewed_by")?,
        reviewed_at: utc_timestamp(row.try_get("reviewed_at")?)?,
        content_digest: revision.content_digest,
        export_digest: revision.export_digest,
    })
}

/// Owner and head inside the caller's serialized write unit or pinned read snapshot.
pub(crate) async fn script(tx: &Tx<'_>, script: &str) -> Result<Option<(String, i64)>, StoreError> {
    tx.query_opt(
        "SELECT owner_id,head_revision FROM scripts WHERE id=?1",
        &[&script],
    )
    .await?
    .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
    .transpose()
}

pub(crate) async fn member_role(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
) -> Result<Option<String>, StoreError> {
    tx.query_opt(
        "SELECT role FROM script_members WHERE script_id=?1 AND actor_id=?2",
        &[&script, &actor],
    )
    .await?
    .map(|row| row.try_get(0).map_err(StoreError::from))
    .transpose()
}

pub(crate) async fn script_exists(tx: &Tx<'_>, script: &str) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt("SELECT id FROM scripts WHERE id=?1", &[&script])
        .await?
        .is_some())
}

/// Writer admission orders first saves. Pre-check instead of ON CONFLICT: immutable-identity
/// guards run before conflict resolution and intentionally reject even an ignored replacement.
pub(super) async fn create_script(
    tx: &Tx<'_>,
    script: &str,
    owner: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO scripts(id,owner_id) SELECT ?1,?2 WHERE NOT EXISTS(SELECT 1 FROM scripts WHERE id=?1)",
        &[&script, &owner],
    )
    .await?;
    Ok(())
}

pub(crate) async fn find_revision(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
) -> Result<Option<RevisionFacts>, StoreError> {
    let query = format!(
        "SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=?1 AND revision=?2"
    );
    tx.query_opt(&query, &[&script, &revision])
        .await?
        .as_ref()
        .map(decode_revision)
        .transpose()
}

/// For a revision the caller already knows exists; a missing row is a database failure.
pub(crate) async fn load_revision(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
) -> Result<RevisionFacts, StoreError> {
    let query = format!(
        "SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=?1 AND revision=?2"
    );
    decode_revision(&tx.query_one(&query, &[&script, &revision]).await?)
}

pub(super) async fn revisions_after(
    tx: &Tx<'_>,
    script: &str,
    after: i64,
    limit: i64,
) -> Result<Vec<RevisionFacts>, StoreError> {
    let query = format!("SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=?1 AND revision>?2 ORDER BY revision LIMIT ?3");
    tx.query(&query, &[&script, &after, &limit])
        .await?
        .iter()
        .map(decode_revision)
        .collect()
}

/// An operation already accepted for this script and actor, kept undecoded: its bytes identify
/// a replay, and its integrity is checked only if it is replayed.
pub(super) struct AcceptedOperation {
    row: Row,
    revision: AcceptedRevision,
}

impl AcceptedOperation {
    pub(super) fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub(super) fn expected_revision(&self) -> u64 {
        self.revision.expected()
    }

    pub(super) fn export(&self) -> Result<Vec<u8>, StoreError> {
        self.row
            .try_get("canonical_export")
            .map_err(StoreError::from)
    }

    pub(super) fn into_revision(self) -> Result<RevisionFacts, StoreError> {
        decode_revision(&self.row)
    }
}

pub(super) async fn accepted_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<AcceptedOperation>, StoreError> {
    let query = format!("SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=?1 AND accepted_by=?2 AND operation_id=?3");
    tx.query_opt(&query, &[&script, &actor, &operation])
        .await?
        .map(|row| {
            let revision = accepted_sequence(&row)?;
            Ok(AcceptedOperation { row, revision })
        })
        .transpose()
}

pub(super) struct NewRevision<'a> {
    pub script: &'a str,
    pub revision: i64,
    pub expected: i64,
    pub operation: &'a str,
    pub actor: &'a str,
    pub export: &'a [u8],
    pub content_digest: &'a str,
    pub export_digest: &'a str,
}

pub(super) async fn insert_revision(tx: &Tx<'_>, new: NewRevision<'_>) -> Result<(), StoreError> {
    tx.execute("INSERT INTO script_revisions(script_id,revision,expected_revision,operation_id,accepted_by,schema_version,canonical_export,content_digest,export_digest,accepted_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            &[&new.script,&new.revision,&new.expected,&new.operation,&new.actor,&WRITE_VERSION,&new.export,&new.content_digest,&new.export_digest,&tx.now_micros()]).await?;
    Ok(())
}

pub(super) async fn link_revision_evidence(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
    owner: &str,
    kind: &str,
    id: &str,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO revision_evidence(script_id,revision,owner_id,kind,evidence_id) VALUES(?1,?2,?3,?4,?5)", &[&script,&revision,&owner,&kind,&id]).await?;
    Ok(())
}

/// Moves the head only from the expected revision; returns whether it moved.
pub(super) async fn advance_head(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
    expected: i64,
) -> Result<bool, StoreError> {
    let changed = tx
        .execute(
            "UPDATE scripts SET head_revision=?1 WHERE id=?2 AND head_revision=?3",
            &[&revision, &script, &expected],
        )
        .await?;
    Ok(changed == 1)
}

pub(crate) async fn evidence_exists(
    tx: &Tx<'_>,
    owner: &str,
    kind: &str,
    id: &str,
) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT id FROM script_evidence WHERE owner_id=?1 AND kind=?2 AND id=?3",
            &[&owner, &kind, &id],
        )
        .await?
        .is_some())
}

/// Whether a rights record of this owner is cited by some accepted revision of `script`.
pub(crate) async fn rights_cited_by_script(
    tx: &Tx<'_>,
    owner: &str,
    id: &str,
    script: &str,
) -> Result<bool, StoreError> {
    Ok(tx.query_opt("SELECT id FROM script_evidence e WHERE owner_id=?1 AND kind='rights' AND id=?2 AND EXISTS(SELECT 1 FROM revision_evidence r WHERE r.owner_id=e.owner_id AND r.kind=e.kind AND r.evidence_id=e.id AND r.script_id=?3)", &[&owner,&id,&script]).await?.is_some())
}

/// The Editorial owner proves the immutable source citation in the caller's snapshot/unit.
/// Ingestion then reads its own source row without reaching Editorial tables directly.
pub(crate) async fn source_cited_by_script(
    tx: &Tx<'_>,
    owner: &str,
    id: &str,
    script: &str,
) -> Result<bool, StoreError> {
    Ok(tx.query_opt(
        "SELECT 1 FROM revision_evidence WHERE owner_id=?1 AND kind='source' AND evidence_id=?2 AND script_id=?3 LIMIT 1",
        &[&owner, &id, &script],
    ).await?.is_some())
}

/// Registers a new evidence record; an identity collision is a database failure.
pub(crate) async fn register_evidence(
    tx: &Tx<'_>,
    owner: &str,
    kind: EvidenceKind,
    id: &str,
    description: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,?2,?3,?4)",
        &[&owner, &kind.as_str(), &id, &description],
    )
    .await?;
    Ok(())
}

/// Read an owner-bound immutable claim, never an anonymous disclosure.
pub(crate) async fn evidence_description(
    tx: &Tx<'_>,
    owner: &str,
    kind: &str,
    id: &str,
) -> Result<Option<String>, StoreError> {
    tx.query_opt(
        "SELECT description FROM script_evidence WHERE owner_id=?1 AND kind=?2 AND id=?3",
        &[&owner, &kind, &id],
    )
    .await?
    .map(|row| row.try_get(0).map_err(StoreError::from))
    .transpose()
}

/// Registers an evidence record unless that identity is already recorded.
pub(crate) async fn ensure_evidence(
    tx: &Tx<'_>,
    owner: &str,
    kind: EvidenceKind,
    id: &str,
    description: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO script_evidence(owner_id,kind,id,description) SELECT ?1,?2,?3,?4 WHERE NOT EXISTS(SELECT 1 FROM script_evidence WHERE owner_id=?1 AND kind=?2 AND id=?3)",
        &[&owner, &kind.as_str(), &id, &description],
    )
    .await?;
    Ok(())
}

/// Whether `owner` recorded an editorial review of exactly this revision while still active.
pub(crate) async fn reviewed_by_active_owner(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
    owner: &str,
) -> Result<bool, StoreError> {
    // Current owner and active reviewer are both required; a historical receipt cannot confer scope.
    if tx
        .query_opt(
            "SELECT id FROM script_reviews WHERE script_id=?1 AND revision=?2 AND reviewed_by=?3",
            &[&script, &revision, &owner],
        )
        .await?
        .is_none()
    {
        return Ok(false);
    }
    Ok(
        crate::platform::store::active_actors(tx, &[owner.to_owned()])
            .await?
            .contains(owner),
    )
}

pub(super) async fn reviews_between(
    tx: &Tx<'_>,
    script: &str,
    after: i64,
    last: i64,
) -> Result<Vec<ReviewFacts>, StoreError> {
    let query = format!(
        "{} WHERE script_id=?1 AND revision>?2 AND revision<=?3 ORDER BY revision",
        review_query()
    );
    tx.query(&query, &[&script, &after, &last])
        .await?
        .iter()
        .map(decode_review)
        .collect()
}

pub(super) async fn review_exists(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
) -> Result<bool, StoreError> {
    let query = format!("{} WHERE script_id=?1 AND revision=?2", review_query());
    Ok(tx.query_opt(&query, &[&script, &revision]).await?.is_some())
}

pub(super) async fn load_review(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
) -> Result<ReviewFacts, StoreError> {
    let query = format!("{} WHERE script_id=?1 AND revision=?2", review_query());
    decode_review(&tx.query_one(&query, &[&script, &revision]).await?)
}

pub(super) async fn review_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<u64>, StoreError> {
    tx.query_opt("SELECT revision FROM script_review_operations WHERE script_id=?1 AND actor_id=?2 AND operation_id=?3", &[&script,&actor,&operation])
        .await?
        .map(|row| {
            AcceptedRevision::from_number(row.try_get(0).map_err(|_| StoreError::CorruptRevision)?)
                .map(AcceptedRevision::get)
                .map_err(|_| StoreError::CorruptRevision)
        })
        .transpose()
}

pub(super) async fn insert_review(
    tx: &Tx<'_>,
    script: &str,
    revision: i64,
    operation: &str,
    actor: &str,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO script_reviews(id,script_id,revision,operation_id,reviewed_by,reviewed_at) VALUES(?1,?2,?3,?4,?5,?6)", &[&Uuid::new_v4().to_string(),&script,&revision,&operation,&actor,&tx.now_micros()]).await?;
    Ok(())
}

pub(super) async fn insert_review_operation(
    tx: &Tx<'_>,
    script: &str,
    actor: &str,
    operation: &str,
    revision: i64,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO script_review_operations(script_id,actor_id,operation_id,revision) VALUES(?1,?2,?3,?4)", &[&script,&actor,&operation,&revision]).await?;
    Ok(())
}

/// Owner-scoped catalog link admission. A script membership cannot transfer ownership.
pub(crate) async fn owned_revision(
    tx: &Tx<'_>,
    owner: &str,
    script: &str,
    revision: i64,
) -> Result<Option<RevisionFacts>, StoreError> {
    let Some((actual_owner, _)) = self::script(tx, script).await? else {
        return Ok(None);
    };
    if actual_owner != owner {
        return Ok(None);
    }
    find_revision(tx, script, revision).await
}
