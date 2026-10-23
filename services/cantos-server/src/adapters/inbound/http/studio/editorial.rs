//! Compatibility facade for the existing Studio Store API. Internal use cases and SQL use
//! Editorial facts; only this adapter constructs revision/history/review HTTP DTOs.
use crate::contract::studio::{
    HistoryResponse, ReviewRequest, ReviewResponse, RevisionResponse, RevisionSummary,
    SaveRevisionRequest,
};

use crate::application::{Store, StoreError};
use crate::contexts::editorial::{document, ReviewFacts, RevisionHistory, RevisionOperation};

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

fn history(facts: RevisionHistory) -> HistoryResponse {
    HistoryResponse {
        revisions: facts
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
        reviews: facts.reviews.into_iter().map(review).collect(),
        next_after: facts.next_after,
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
        self.editorial_history(token, script, after, limit)
            .await
            .map(history)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::editorial::RevisionFacts;
    use serde_json::json;

    fn reviewed() -> ReviewFacts {
        ReviewFacts {
            id: "review-id".into(),
            script_id: "script-id".into(),
            revision: 8,
            operation_id: "review-operation".into(),
            reviewed_by: "reviewer".into(),
            reviewed_at: "2026-10-19T01:02:03.123456Z".into(),
            content_digest: "content-eight".into(),
            export_digest: "export-eight".into(),
        }
    }

    fn review_json() -> serde_json::Value {
        json!({
            "id": "review-id", "script_id": "script-id", "revision": 8,
            "operation_id": "review-operation", "reviewed_by": "reviewer",
            "reviewed_at": "2026-10-19T01:02:03.123456Z",
            "content_digest": "content-eight", "export_digest": "export-eight"
        })
    }

    #[test]
    fn review_projection_preserves_receipt_identity_digests_and_microseconds() {
        assert_eq!(
            serde_json::to_value(review(reviewed())).unwrap(),
            review_json()
        );
    }

    #[test]
    fn history_projection_preserves_order_reviews_and_cursor_without_manuscript_bodies() {
        let projected = history(RevisionHistory {
            revisions: vec![
                RevisionFacts {
                    script_id: "script-id".into(),
                    revision: 8,
                    accepted_by: "editor-eight".into(),
                    accepted_at: "2026-10-19T01:00:00.000008Z".into(),
                    content_digest: "content-eight".into(),
                    export_digest: "export-eight".into(),
                    canonical_export: "private-manuscript-eight".into(),
                },
                RevisionFacts {
                    script_id: "script-id".into(),
                    revision: 9,
                    accepted_by: "editor-nine".into(),
                    accepted_at: "2026-10-19T01:00:00.000009Z".into(),
                    content_digest: "content-nine".into(),
                    export_digest: "export-nine".into(),
                    canonical_export: "private-manuscript-nine".into(),
                },
            ],
            reviews: vec![reviewed()],
            next_after: Some(9),
        });
        assert_eq!(
            serde_json::to_value(projected).unwrap(),
            json!({
                "revisions": [
                    {"script_id": "script-id", "revision": 8, "accepted_by": "editor-eight",
                     "accepted_at": "2026-10-19T01:00:00.000008Z",
                     "content_digest": "content-eight", "export_digest": "export-eight"},
                    {"script_id": "script-id", "revision": 9, "accepted_by": "editor-nine",
                     "accepted_at": "2026-10-19T01:00:00.000009Z",
                     "content_digest": "content-nine", "export_digest": "export-nine"}
                ],
                "reviews": [review_json()], "next_after": 9
            })
        );
    }

    #[test]
    fn empty_history_keeps_empty_arrays_and_explicit_null_cursor() {
        assert_eq!(
            serde_json::to_value(history(RevisionHistory {
                revisions: vec![],
                reviews: vec![],
                next_after: None,
            }))
            .unwrap(),
            json!({"revisions": [], "reviews": [], "next_after": null})
        );
    }
}
