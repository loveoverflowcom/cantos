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

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn status_key(
    has_current: bool,
    state: &super::RequestState,
) -> crate::localization::generated::live::Key {
    use super::{Command, Phase};
    use crate::localization::generated::live::Key;
    if matches!(
        state.command(),
        Some(
            Command::ViewerState { .. }
                | Command::ViewerChange { .. }
                | Command::ViewerGroups
                | Command::ViewerGroupChange { .. }
        )
    ) {
        match state.phase() {
            Phase::Pending => {
                return if state.command().is_some_and(Command::is_write) {
                    Key::AccessChanging
                } else {
                    Key::AccessLoading
                }
            }
            Phase::Ambiguous => return Key::AccessUnknown,
            Phase::Failed => return state.error().map(failure_key).unwrap_or(Key::AccessFailed),
            Phase::Ready if state.command().is_some_and(Command::is_write) => {
                return Key::AccessReceipt
            }
            Phase::Ready | Phase::Idle => (),
        }
    }
    if has_current {
        Key::AccessCurrent
    } else {
        Key::AccessUnread
    }
}

/// Presentation of already-read facts only; this never changes audience or request guards.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn audience_reason(
    current: Option<&ViewerAccessState>,
    publication_read: bool,
    candidate: ViewerAudience,
) -> Option<crate::localization::generated::live::Key> {
    use crate::localization::generated::live::Key;
    match current {
        None => Some(Key::AccessUnread),
        Some(state) if state.audience == candidate => Some(Key::AccessUnchanged),
        Some(_) if !publication_read => Some(Key::MetadataUnread),
        Some(_) => None,
    }
}

/// Names come only from the owner's authorized group projection, never an actor lookup.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn grant_name(
    grant: &crate::contract::ViewerGrant,
    facts: &ViewerFacts,
) -> Option<String> {
    let crate::contract::ViewerPrincipal::Group { group_id } = &grant.principal else {
        return None;
    };
    facts
        .groups
        .as_ref()?
        .items
        .iter()
        .find(|group| {
            group.group_id == *group_id && !group.revoked && !group.name.trim().is_empty()
        })
        .map(|group| group.name.clone())
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RecipientLabel {
    Named(String),
    Actor,
    Group,
}
#[cfg(any(target_arch = "wasm32", test))]
impl RecipientLabel {
    pub fn text(&self, locale: crate::localization::Locale) -> String {
        use crate::localization::generated::live::{text, Key};
        match self {
            Self::Named(name) => name.clone(),
            Self::Actor => text(Key::AccessActorUnnamed, locale).into(),
            Self::Group => text(Key::AccessGroupUnnamed, locale).into(),
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn revoke_label(
    command: &super::Command,
    facts: &ViewerFacts,
) -> Option<RecipientLabel> {
    use crate::contract::ViewerPrincipal;
    let super::Command::ViewerChange { work, request } = command else {
        return None;
    };
    let state = facts
        .current
        .as_ref()
        .filter(|state| state.work_id == *work)?;
    match &request.change {
        ViewerAccessChange::RevokeGrant { grant_id } => {
            let grant = state
                .grants
                .iter()
                .find(|grant| grant.grant_id == *grant_id)?;
            Some(
                grant_name(grant, facts)
                    .map(RecipientLabel::Named)
                    .unwrap_or(match grant.principal {
                        ViewerPrincipal::Actor { .. } => RecipientLabel::Actor,
                        ViewerPrincipal::Group { .. } => RecipientLabel::Group,
                    }),
            )
        }
        ViewerAccessChange::RevokeShare { share_id } => state
            .pending
            .iter()
            .find(|share| share.share_id == *share_id)
            .map(|share| RecipientLabel::Named(share.email.clone())),
        _ => None,
    }
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
    fn audience_availability_distinguishes_unread_current_and_publication_prerequisite() {
        use crate::localization::generated::live::Key;
        let current = policy();
        assert_eq!(
            audience_reason(None, true, ViewerAudience::Public),
            Some(Key::AccessUnread)
        );
        assert_eq!(
            audience_reason(Some(&current), false, ViewerAudience::Restricted),
            Some(Key::AccessUnchanged)
        );
        assert_eq!(
            audience_reason(Some(&current), false, ViewerAudience::Public),
            Some(Key::MetadataUnread)
        );
        assert_eq!(
            audience_reason(Some(&current), true, ViewerAudience::Public),
            None
        );
    }

    #[test]
    fn access_feedback_keeps_read_current_pending_error_and_receipt_distinct() {
        use crate::{
            live::{ClientError, RequestState},
            localization::generated::live::Key,
        };
        let idle = RequestState::default()
            .context("owner", "workspace")
            .unwrap();
        assert_eq!(status_key(false, &idle), Key::AccessUnread);
        assert_eq!(status_key(true, &idle), Key::AccessCurrent);
        let (pending, ticket) = idle
            .begin(Command::ViewerState { work: WORK.into() })
            .unwrap();
        assert_eq!(status_key(false, &pending), Key::AccessLoading);
        let failed = pending.finish(ticket, Err(ClientError::Transport));
        assert_eq!(status_key(false, &failed), Key::AccessFailed);
        let read = pending.finish(ticket, Ok(Reply::ViewerState(policy())));
        assert_eq!(status_key(true, &read), Key::AccessCurrent);
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
        let (pending, ticket) = idle.begin(command).unwrap();
        assert_eq!(status_key(true, &pending), Key::AccessChanging);
        let unknown = pending.finish(ticket, Err(ClientError::Transport));
        assert_eq!(status_key(true, &unknown), Key::AccessUnknown);
        let ready = pending.finish(
            ticket,
            Ok(Reply::ViewerReceipt(ViewerAccessReceipt {
                work_id: WORK.into(),
                operation_id: OP.into(),
                version: 8,
                replayed: false,
            })),
        );
        assert_eq!(status_key(false, &ready), Key::AccessReceipt);
    }

    #[test]
    fn revoke_uses_authorized_names_without_rewriting_its_exact_intent() {
        let group = ViewerGroupState {
            group_id: "group".into(),
            name: "Nhóm biên tập Ánh Đèn".into(),
            version: 2,
            revoked: false,
            members: vec![],
        };
        let mut current = policy();
        current.grants = vec![
            ViewerGrant {
                grant_id: "grant".into(),
                principal: ViewerPrincipal::Group {
                    group_id: "group".into(),
                },
                expires_at_us: None,
            },
            ViewerGrant {
                grant_id: "actor-grant".into(),
                principal: ViewerPrincipal::Actor {
                    actor_id: "opaque-actor".into(),
                },
                expires_at_us: None,
            },
        ];
        current.pending = vec![PendingViewerShare {
            share_id: "share".into(),
            email: "Bien.Tap+Viet@example.com".into(),
            expires_at_us: 1900000000000000,
        }];
        let mut facts = ViewerFacts {
            current: Some(current),
            groups: Some(ViewerGroups { items: vec![group] }),
            ..ViewerFacts::default()
        };
        for (change, expected) in [
            (
                ViewerAccessChange::RevokeGrant {
                    grant_id: "grant".into(),
                },
                RecipientLabel::Named("Nhóm biên tập Ánh Đèn".into()),
            ),
            (
                ViewerAccessChange::RevokeGrant {
                    grant_id: "actor-grant".into(),
                },
                RecipientLabel::Actor,
            ),
            (
                ViewerAccessChange::RevokeShare {
                    share_id: "share".into(),
                },
                RecipientLabel::Named("Bien.Tap+Viet@example.com".into()),
            ),
        ] {
            let command = Command::ViewerChange {
                work: WORK.into(),
                request: ViewerAccessRequest {
                    operation_id: OP.into(),
                    expected_version: 7,
                    change,
                },
            };
            let before = command.json().unwrap();
            let label = revoke_label(&command, &facts).unwrap();
            assert_eq!(label, expected);
            let renamed = facts.groups.as_mut().unwrap().items[0].name.clone();
            facts.groups.as_mut().unwrap().items[0].name = "Another name".into();
            assert_eq!(
                label, expected,
                "confirmation retains the reviewed display snapshot"
            );
            facts.groups.as_mut().unwrap().items[0].name = renamed;
            assert_eq!(command.json().unwrap(), before);
        }
        facts.groups.as_mut().unwrap().items[0].revoked = true;
        assert_eq!(
            grant_name(&facts.current.as_ref().unwrap().grants[0], &facts),
            None
        );
        let wrong_work = Command::ViewerChange {
            work: OP.into(),
            request: ViewerAccessRequest {
                operation_id: OP.into(),
                expected_version: 7,
                change: ViewerAccessChange::RevokeShare {
                    share_id: "share".into(),
                },
            },
        };
        assert_eq!(revoke_label(&wrong_work, &facts), None);
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
