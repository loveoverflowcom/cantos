//! Presentation only: the shared request owner remains the exchange authority.
use super::{ClientError, Command, Phase, RequestState};
use crate::{
    contract::MetadataPublicationLifecycle as Lifecycle, localization::generated::live::Key,
};

pub(super) fn status(request: &RequestState, archived: bool, current: Option<Lifecycle>) -> Key {
    match (request.command(), request.phase()) {
        (Some(Command::MetadataState { .. }), Phase::Pending) => return Key::MetadataReading,
        (Some(Command::MetadataState { .. }), Phase::Failed | Phase::Ambiguous) => {
            return Key::MetadataReadFailed;
        }
        (Some(Command::MetadataCommand { .. }), Phase::Pending) => return Key::MetadataPending,
        (Some(Command::MetadataCommand { .. }), Phase::Ambiguous) => return Key::MetadataUnknown,
        (Some(Command::MetadataCommand { .. }), Phase::Failed) => {
            return if matches!(request.error(), Some(ClientError::Http { status: 409, .. })) {
                Key::MetadataConflict
            } else {
                Key::MetadataFailed
            };
        }
        _ => {}
    }
    if archived {
        return Key::MetadataArchived;
    }
    match current {
        Some(Lifecycle::NeverPublished) => Key::MetadataNever,
        Some(Lifecycle::Active) => Key::MetadataActive,
        Some(Lifecycle::Withdrawn) => Key::MetadataWithdrawn,
        None => Key::MetadataUnread,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_publication_state_never_reports_a_write() {
        let state = RequestState::default().context("owner", "work").unwrap();
        let (state, ticket) = state
            .begin(Command::MetadataState {
                work: "work".into(),
            })
            .unwrap();
        assert_eq!(status(&state, false, None), Key::MetadataReading);
        let state = state.finish(ticket, Err(ClientError::Transport));
        assert_eq!(status(&state, false, None), Key::MetadataReadFailed);
    }

    #[test]
    fn settled_lifecycle_is_distinct_from_unread_and_pending() {
        let state = RequestState::default();
        for (current, expected) in [
            (None, Key::MetadataUnread),
            (Some(Lifecycle::NeverPublished), Key::MetadataNever),
            (Some(Lifecycle::Active), Key::MetadataActive),
            (Some(Lifecycle::Withdrawn), Key::MetadataWithdrawn),
        ] {
            assert_eq!(status(&state, false, current), expected);
        }
        assert_eq!(
            status(&state, true, Some(Lifecycle::Active)),
            Key::MetadataArchived
        );
    }

    #[test]
    fn sending_unknown_and_conflict_keep_the_write_identity() {
        let state = RequestState::default().context("owner", "work").unwrap();
        let command = Command::MetadataCommand {
            work: "work".into(),
            request: crate::contract::MetadataPublicationRequest::Withdraw {
                operation_id: "operation".into(),
                expected_version: 2,
            },
        };
        let (pending, ticket) = state.begin(command).unwrap();
        assert_eq!(
            status(&pending, false, Some(Lifecycle::Active)),
            Key::MetadataPending
        );
        let unknown = pending.finish(ticket, Err(ClientError::Transport));
        assert_eq!(
            status(&unknown, false, Some(Lifecycle::Active)),
            Key::MetadataUnknown
        );
        for (code, key) in [(409, Key::MetadataConflict), (422, Key::MetadataFailed)] {
            let error = serde_json::from_str(
                r#"{"code":"invalid_request","current_revision":null,"issues":[]}"#,
            )
            .unwrap();
            let failed = pending.finish(
                ticket,
                Err(ClientError::Http {
                    status: code,
                    error,
                }),
            );
            assert_eq!(status(&failed, false, Some(Lifecycle::Active)), key);
        }
    }
}
