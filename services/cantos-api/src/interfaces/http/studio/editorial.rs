//! Compatibility facade for the existing Studio Store API. Internal use cases and SQL use
//! Editorial facts; only this adapter constructs revision/history/review HTTP DTOs.
use cantos_contracts::{
    HistoryResponse, ReviewRequest, ReviewResponse, RevisionResponse, RevisionSummary,
    SaveRevisionRequest,
};

use crate::contexts::editorial::{document, ReviewFacts, RevisionOperation};
use crate::platform::postgres::{Store, StoreError};

fn review(facts: ReviewFacts) -> ReviewResponse {
    ReviewResponse {
        id: facts.id,
        script_id: facts.script_id,
        revision: facts.revision,
        operation_id: facts.operation_id,
        reviewed_by: facts.reviewed_by,
        reviewed_at: facts.reviewed_at,
        content_digest: facts.content_digest,
        export_digest: facts.export_digest,
    }
}

impl Store {
    pub async fn load(
        &self,
        token: &str,
        script: &str,
        revision: Option<u64>,
    ) -> Result<RevisionResponse, StoreError> {
        self.load_editorial_revision(token, script, revision)
            .await
            .map(document::revision)
    }

    pub async fn save(
        &self,
        token: &str,
        script: &str,
        request: SaveRevisionRequest,
    ) -> Result<RevisionResponse, StoreError> {
        self.save_editorial_revision(
            token,
            script,
            RevisionOperation {
                expected_revision: request.expected_revision,
                operation_id: &request.operation_id,
            },
            &request.script_json,
        )
        .await
        .map(document::revision)
    }

    pub async fn history(
        &self,
        token: &str,
        script: &str,
        after: u64,
        limit: u64,
    ) -> Result<HistoryResponse, StoreError> {
        let history = self.editorial_history(token, script, after, limit).await?;
        Ok(HistoryResponse {
            revisions: history
                .revisions
                .into_iter()
                .map(|facts| RevisionSummary {
                    script_id: facts.script_id,
                    revision: facts.revision,
                    accepted_by: facts.accepted_by,
                    accepted_at: facts.accepted_at,
                    content_digest: facts.content_digest,
                    export_digest: facts.export_digest,
                })
                .collect(),
            reviews: history.reviews.into_iter().map(review).collect(),
            next_after: history.next_after,
        })
    }

    pub async fn review(
        &self,
        token: &str,
        script: &str,
        request: ReviewRequest,
    ) -> Result<ReviewResponse, StoreError> {
        self.review_editorial_revision(token, script, request.revision, &request.operation_id)
            .await
            .map(review)
    }
}
