//! Deterministic binding decisions. IDs and digests are facts admitted by the shell; no HTTP,
//! database, storage, clock or caller-authored profile policy enters this core.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentBinding<'a> {
    pub script_id: &'a str,
    pub revision: u64,
    pub content_digest: &'a str,
    pub profile_id: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareDecision {
    Replay,
    Create,
    OperationReused,
    StaleRevision(u64),
    ContentDigestMismatch,
}

pub fn decide_prepare(
    head: u64,
    current_digest: &str,
    requested: IntentBinding<'_>,
    prior: Option<IntentBinding<'_>>,
) -> PrepareDecision {
    if let Some(prior) = prior {
        return if prior == requested {
            PrepareDecision::Replay
        } else {
            PrepareDecision::OperationReused
        };
    }
    if requested.revision != head {
        return PrepareDecision::StaleRevision(head);
    }
    if requested.content_digest != current_digest {
        return PrepareDecision::ContentDigestMismatch;
    }
    PrepareDecision::Create
}

#[cfg(test)]
mod tests {
    use super::{decide_prepare, IntentBinding, PrepareDecision};

    fn binding() -> IntentBinding<'static> {
        IntentBinding {
            script_id: "script-one",
            revision: 1,
            content_digest: "digest-one",
            profile_id: "opus-mono-20-v1",
        }
    }

    #[test]
    fn exact_operation_replays_after_revision_advances() {
        assert_eq!(
            decide_prepare(2, "digest-two", binding(), Some(binding())),
            PrepareDecision::Replay
        );
    }

    #[test]
    fn operation_cannot_change_any_pinned_binding() {
        for changed in [
            IntentBinding {
                script_id: "script-two",
                ..binding()
            },
            IntentBinding {
                revision: 2,
                ..binding()
            },
            IntentBinding {
                content_digest: "digest-two",
                ..binding()
            },
            IntentBinding {
                profile_id: "opus-mono-40-v1",
                ..binding()
            },
        ] {
            assert_eq!(
                decide_prepare(2, "digest-two", changed, Some(binding())),
                PrepareDecision::OperationReused
            );
        }
    }

    #[test]
    fn new_intent_needs_exact_current_revision_and_digest() {
        assert_eq!(
            decide_prepare(2, "digest-two", binding(), None),
            PrepareDecision::StaleRevision(2)
        );
        assert_eq!(
            decide_prepare(1, "digest-two", binding(), None),
            PrepareDecision::ContentDigestMismatch
        );
        assert_eq!(
            decide_prepare(1, "digest-one", binding(), None),
            PrepareDecision::Create
        );
    }
}
