//! Additive editorial operations over the existing revision and authorization boundary.
use cantos_contracts::{
    HistoryResponse, ReviewRequest, ReviewResponse, RevisionSummary, SourceResponse,
};
use sha2::{Digest, Sha256};
use tokio_postgres::Row;
use uuid::Uuid;

use super::{
    authenticate, authorize, checked_number, decode_revision, normalized_uuid, Store, StoreError,
    REVISION_COLUMNS,
};
use crate::revisions::{decide_review, Action, ReviewDecision};

fn review_query() -> String {
    format!("SELECT r.id,r.operation_id,r.reviewed_by,to_char(r.reviewed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS reviewed_at,{REVISION_COLUMNS} FROM script_reviews r JOIN script_revisions USING(script_id,revision)")
}

fn decode_review(row: &Row) -> Result<ReviewResponse, StoreError> {
    let revision = decode_revision(row)?;
    Ok(ReviewResponse {
        id: row.get("id"),
        script_id: revision.script_id,
        revision: revision.revision,
        operation_id: row.get("operation_id"),
        reviewed_by: row.get("reviewed_by"),
        reviewed_at: row.get("reviewed_at"),
        content_digest: revision.content_digest,
        export_digest: revision.export_digest,
    })
}

impl Store {
    /// Every page is reauthorized and pinned to the locked head observed for that page.
    pub async fn history(
        &self,
        token: &str,
        script: &str,
        after: u64,
        limit: u64,
    ) -> Result<HistoryResponse, StoreError> {
        if !(1..=50).contains(&limit) {
            return Err(StoreError::InvalidRequest);
        }
        let script = normalized_uuid(script)?;
        let after = checked_number(after)?;
        let limit = checked_number(limit)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (_, head) = authorize(&tx, &actor, &script, Action::Read, false).await?;
        let query = format!("SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND revision>$2 ORDER BY revision LIMIT $3");
        let rows = tx.query(&query, &[&script, &after, &limit]).await?;
        let mut revisions = Vec::with_capacity(rows.len());
        for row in rows {
            let saved = decode_revision(&row)?;
            revisions.push(RevisionSummary {
                script_id: saved.script_id,
                revision: saved.revision,
                accepted_by: saved.accepted_by,
                accepted_at: saved.accepted_at,
                content_digest: saved.content_digest,
                export_digest: saved.export_digest,
            });
        }
        let last = revisions.last().map(|revision| revision.revision);
        let mut reviews = Vec::new();
        if let Some(last) = last {
            let query = format!(
                "{} WHERE script_id=$1 AND revision>$2 AND revision<=$3 ORDER BY revision",
                review_query()
            );
            for row in tx
                .query(&query, &[&script, &after, &checked_number(last)?])
                .await?
            {
                reviews.push(decode_review(&row)?);
            }
        }
        tx.commit().await?;
        Ok(HistoryResponse {
            revisions,
            reviews,
            next_after: last.filter(|last| *last < head),
        })
    }

    /// Editorial review is an owner action, distinct from storage acceptance and publication.
    pub async fn review(
        &self,
        token: &str,
        script: &str,
        request: ReviewRequest,
    ) -> Result<ReviewResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let revision = checked_number(request.revision)?;
        if revision < 1 {
            return Err(StoreError::InvalidRequest);
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (_, head) = authorize(&tx, &actor, &script, Action::Review, true).await?;
        let prior = tx.query_opt("SELECT revision FROM script_review_operations WHERE script_id=$1 AND actor_id=$2 AND operation_id=$3", &[&script,&actor,&operation]).await?;
        let prior_revision = prior.map(|row| row.get::<_, i64>(0) as u64);
        let query = format!("{} WHERE script_id=$1 AND revision=$2", review_query());
        let existing = tx.query_opt(&query, &[&script, &revision]).await?;
        match decide_review(head, request.revision, prior_revision, existing.is_some()) {
            ReviewDecision::OperationReused => return Err(StoreError::OperationReused),
            ReviewDecision::Stale { current_revision } => {
                return Err(StoreError::StaleRevision(current_revision))
            }
            ReviewDecision::Replay => {
                if existing.is_none() {
                    return Err(StoreError::CorruptRevision);
                }
            }
            ReviewDecision::Create => {
                // Never record a review for corrupted stored content.
                let query = format!("SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND revision=$2");
                let row = tx.query_one(&query, &[&script, &revision]).await?;
                decode_revision(&row)?;
                tx.execute("INSERT INTO script_reviews(id,script_id,revision,operation_id,reviewed_by) VALUES($1,$2,$3,$4,$5)", &[&Uuid::new_v4().to_string(),&script,&revision,&operation,&actor]).await?;
            }
        }
        if prior_revision.is_none() {
            tx.execute("INSERT INTO script_review_operations(script_id,actor_id,operation_id,revision) VALUES($1,$2,$3,$4)", &[&script,&actor,&operation,&revision]).await?;
        }
        let query = format!("{} WHERE script_id=$1 AND revision=$2", review_query());
        let result = decode_review(&tx.query_one(&query, &[&script, &revision]).await?)?;
        tx.commit().await?;
        Ok(result)
    }

    /// Private source bytes are reachable only through a currently authorized script link.
    pub async fn source(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<SourceResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (owner, _) = authorize(&tx, &actor, &script, Action::Read, false).await?;
        let row = tx.query_opt("SELECT id,reference,original_text,original_bytes,sha256,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at FROM source_records s WHERE owner_id=$1 AND id=$2 AND EXISTS(SELECT 1 FROM revision_evidence e WHERE e.script_id=$3 AND e.owner_id=s.owner_id AND e.kind='source' AND e.evidence_id=s.id)", &[&owner,&id,&script]).await?.ok_or(StoreError::NotFound)?;
        // The legacy source contract is text-only; binary intake uses /imports/{id}/original.
        let original_text: String = row
            .get::<_, Option<String>>("original_text")
            .ok_or(StoreError::InvalidRequest)?;
        let bytes: Option<Vec<u8>> = row.get("original_bytes");
        let sha256: String = row.get("sha256");
        if bytes
            .as_ref()
            .is_some_and(|bytes| bytes.as_slice() != original_text.as_bytes())
            || format!(
                "{:x}",
                Sha256::digest(bytes.as_deref().unwrap_or(original_text.as_bytes()))
            ) != sha256
        {
            return Err(StoreError::CorruptRevision);
        }
        let response = SourceResponse {
            id: row.get("id"),
            reference: row.get("reference"),
            original_text,
            sha256,
            recorded_at: row.get("recorded_at"),
        };
        tx.commit().await?;
        Ok(response)
    }
}
