//! Access facts are separate from Catalog drafts and metadata generations. Receipts are history.
use crate::contract::{
    ViewerAccessChange, ViewerAccessReceipt, ViewerAccessRequest, ViewerAccessState,
    ViewerAudience, ViewerGroupReceipt, ViewerGroups,
};

#[derive(Clone, Default)]
pub struct ViewerFacts {
    pub matrix: Option<crate::contract::ChapterMatrixPage>,
    pub current: Option<ViewerAccessState>,
    pub receipt: Option<ViewerAccessReceipt>,
    pub groups: Option<ViewerGroups>,
    pub group_receipt: Option<ViewerGroupReceipt>,
}

/// A frozen confirmation. A policy reload never rewrites the version being confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudienceReview {
    pub work: String,
    pub expected_version: u64,
    pub audience: ViewerAudience,
    pub publication_version: u64,
    pub published_revision: Option<u64>,
    pub unit_count: usize,
}
impl AudienceReview {
    pub fn new(
        state: &ViewerAccessState,
        publication: &crate::contract::MetadataPublicationState,
        audience: ViewerAudience,
    ) -> Option<Self> {
        (state.work_id == publication.work_id
            && super::metadata_publication::valid_state(publication)
            && state.audience != audience)
            .then(|| Self {
                work: state.work_id.clone(),
                expected_version: state.version,
                audience,
                publication_version: publication.version,
                published_revision: publication.generation.as_ref().map(|g| g.work_revision),
                unit_count: publication
                    .generation
                    .as_ref()
                    .map_or(0, |g| g.entries.len()),
            })
    }
    pub fn request(&self, operation_id: String, confirmed: bool) -> Option<ViewerAccessRequest> {
        confirmed.then(|| ViewerAccessRequest {
            operation_id,
            expected_version: self.expected_version,
            change: ViewerAccessChange::SetAudience {
                audience: self.audience,
                confirm_metadata_rights: self.audience == ViewerAudience::Public,
            },
        })
    }
}

/// Preserve the access-specific fixed error code without treating a malformed body as conflict.
pub fn http_error(status: u16, body: &[u8]) -> super::ClientError {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ErrorBody {
        code: String,
        current_version: Option<u64>,
    }
    let Ok(error) = serde_json::from_slice::<ErrorBody>(body) else {
        return super::ClientError::http(status, body);
    };
    use crate::contract::{ApiError, ErrorCode, FieldIssue};
    let code = match (status, error.code.as_str()) {
        (400, "invalid_request") => ErrorCode::InvalidRequest,
        (404, "not_found") => ErrorCode::NotFound,
        (409, "stale_policy") if error.current_version.is_some() => ErrorCode::StaleRevision,
        (409, "rights_unavailable") => ErrorCode::EvidenceUnavailable,
        (409, "operation_reused") => ErrorCode::OperationReused,
        (409, "access_bound" | "duplicate" | "version_limit") => ErrorCode::CatalogConflict,
        (503, "unavailable") => ErrorCode::Unavailable,
        _ => return super::ClientError::Protocol { status },
    };
    super::ClientError::Http {
        status,
        error: ApiError {
            code,
            current_revision: error.current_version,
            issues: vec![FieldIssue {
                path: "viewer_access".into(),
                rule: error.code,
            }],
        },
    }
}

pub fn failure_key(error: &super::ClientError) -> crate::localization::generated::live::Key {
    use crate::localization::generated::live::Key;
    if let super::ClientError::Http { error, .. } = error {
        if let Some(issue) = error.issues.iter().find(|i| i.path == "viewer_access") {
            return match issue.rule.as_str() {
                "stale_policy" => Key::AccessConflict,
                "rights_unavailable" => Key::AccessRights,
                "duplicate" => Key::AccessDuplicate,
                "access_bound" | "version_limit" => Key::AccessBound,
                "invalid_request" => Key::AccessInvalid,
                _ => Key::AccessFailed,
            };
        }
    }
    Key::AccessFailed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contract::*,
        live::{Command, Facts, Reply},
    };
    const WORK: &str = "06520000-0000-4000-8000-000000000002";
    const OP: &str = "06520000-0000-4000-8000-000000000001";

    fn policy() -> ViewerAccessState {
        ViewerAccessState {
            work_id: WORK.into(),
            version: 7,
            audience: ViewerAudience::Restricted,
            grants: vec![],
            pending: vec![],
        }
    }
    #[test]
    fn service_owned_corpus_decodes_in_the_consumer_without_service_imports() {
        let corpus: Vec<serde_json::Value> = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/viewer-access/v1/fixtures/corpus.json")).unwrap();
        for row in corpus {
            let value = row["value"].clone();
            match row["type"].as_str().unwrap() {
                "ViewerAccessRequest" => {
                    serde_json::from_value::<ViewerAccessRequest>(value).unwrap();
                }
                "ViewerGroupRequest" => {
                    serde_json::from_value::<ViewerGroupRequest>(value).unwrap();
                }
                "ViewerAccessReceipt" => {
                    serde_json::from_value::<ViewerAccessReceipt>(value).unwrap();
                }
                "ViewerAccessState" => {
                    serde_json::from_value::<ViewerAccessState>(value).unwrap();
                }
                "ChapterMatrixPage" => {
                    let _: crate::contract::ChapterMatrixPage =
                        serde_json::from_value(value).unwrap();
                }
                "UnitViewerAccessState" => {
                    let _: crate::contract::UnitViewerAccessState =
                        serde_json::from_value(value).unwrap();
                }
                "ViewerGroups" => {
                    serde_json::from_value::<ViewerGroups>(value).unwrap();
                }
                "ViewerGroupReceipt" => {
                    serde_json::from_value::<ViewerGroupReceipt>(value).unwrap();
                }
                other => panic!("uncovered fixture {other}"),
            }
        }
    }
    #[test]
    fn frozen_audience_review_needs_confirmation_and_retains_policy_and_publication_versions() {
        let state = policy();
        let mut publication: MetadataPublicationState=serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/metadata-publication/v1/fixtures/active.json")).unwrap();
        publication.work_id = WORK.into();
        let review = AudienceReview::new(&state, &publication, ViewerAudience::Public).unwrap();
        assert_eq!(review.expected_version, 7);
        assert_eq!(review.publication_version, 2);
        assert_eq!(review.request(OP.into(), false), None);
        assert!(matches!(
            review.request(OP.into(), true).unwrap().change,
            ViewerAccessChange::SetAudience {
                audience: ViewerAudience::Public,
                confirm_metadata_rights: true
            }
        ));
        let mut changed = state.clone();
        changed.version = 8;
        assert_eq!(review.request(OP.into(), true).unwrap().expected_version, 7);
        assert_eq!(
            AudienceReview::new(&changed, &publication, ViewerAudience::Restricted),
            None
        );
    }
    #[test]
    fn access_ack_invalidates_only_access_facts_and_never_becomes_current_policy() {
        let work: CatalogWork = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/work.json"
        ))
        .unwrap();
        let metadata:MetadataPublicationState=serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/metadata-publication/v1/fixtures/active.json")).unwrap();
        let mut facts = Facts {
            work: Some(work),
            ..Facts::default()
        };
        facts.metadata.current = Some(metadata);
        facts.viewer.current = Some(policy());
        let before = facts.metadata.current.clone();
        let request = ViewerAccessRequest {
            operation_id: OP.into(),
            expected_version: 7,
            change: ViewerAccessChange::ShareMailbox {
                share_id: WORK.into(),
                email: " A+Tag@EXAMPLE.COM ".into(),
            },
        };
        let command = Command::ViewerChange {
            work: WORK.into(),
            request,
        };
        assert_eq!(command.method(), "POST");
        assert_eq!(
            command.credentials(),
            crate::live::Credentials::OwnerSession
        );
        assert_eq!(
            command.path().unwrap(),
            format!("/api/v1/studio/works/{WORK}/viewer-access")
        );
        assert!(command
            .json()
            .unwrap()
            .unwrap()
            .contains(" A+Tag@EXAMPLE.COM "));
        let receipt = ViewerAccessReceipt {
            operation_id: OP.into(),
            work_id: WORK.into(),
            version: 8,
            replayed: true,
        };
        assert_eq!(
            command.check_reply(&Reply::ViewerReceipt(receipt.clone())),
            Ok(())
        );
        let next = facts.apply_command(&command, Reply::ViewerReceipt(receipt.clone()));
        assert!(next.viewer.current.is_none());
        assert_eq!(next.viewer.receipt, Some(receipt));
        assert_eq!(next.metadata.current, before);
        assert_eq!(next.work, facts.work);
    }
    #[test]
    fn foreign_or_wrong_operation_receipt_is_never_an_acknowledgement() {
        let command = Command::ViewerChange {
            work: WORK.into(),
            request: ViewerAccessRequest {
                operation_id: OP.into(),
                expected_version: 7,
                change: ViewerAccessChange::RevokeShare {
                    share_id: WORK.into(),
                },
            },
        };
        for (op, work, version) in [(WORK, WORK, 8), (OP, OP, 8), (OP, WORK, 9)] {
            assert_eq!(
                command.check_reply(&Reply::ViewerReceipt(ViewerAccessReceipt {
                    operation_id: op.into(),
                    work_id: work.into(),
                    version,
                    replayed: false
                })),
                Err(crate::live::ClientError::ReplyMismatch)
            );
        }
    }
}
