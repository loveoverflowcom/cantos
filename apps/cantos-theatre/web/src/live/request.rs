//! Pure listener capability/probe state. A successful socket never supplies a publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Live,
    Mock,
}
impl Mode {
    pub fn from_query(query: &str) -> Self {
        if query == "?mode=mock" {
            Self::Mock
        } else {
            Self::Live
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    ListenerApiNotImplemented,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Transport,
    Protocol,
    Unauthenticated,
    Forbidden,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProbePhase {
    #[default]
    Idle,
    Pending,
    Unsupported(Unsupported),
    Failed(Failure),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeState {
    pub generation: u64,
    pub phase: ProbePhase,
}
impl ProbeState {
    pub fn begin(self) -> Option<Self> {
        Some(Self {
            generation: self.generation.checked_add(1)?,
            phase: ProbePhase::Pending,
        })
    }
    pub fn cancel(self) -> Option<Self> {
        Some(Self {
            generation: self.generation.checked_add(1)?,
            phase: ProbePhase::Idle,
        })
    }
    pub fn finish(self, generation: u64, phase: ProbePhase) -> Self {
        if self.generation == generation
            && matches!(self.phase, ProbePhase::Pending)
            && matches!(phase, ProbePhase::Unsupported(_) | ProbePhase::Failed(_))
        {
            Self { phase, ..self }
        } else {
            self
        }
    }
}
pub fn classify(status: u16, body: &[u8]) -> ProbePhase {
    match status {
        404 if serde_json::from_slice::<cantos_contracts::ApiError>(body).is_ok_and(|e| {
            e.code == cantos_contracts::ErrorCode::NotFound
                && e.current_revision.is_none()
                && e.issues.is_empty()
        }) =>
        {
            ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented)
        }
        401 => ProbePhase::Failed(Failure::Unauthenticated),
        403 => ProbePhase::Failed(Failure::Forbidden),
        503 => ProbePhase::Failed(Failure::Unavailable),
        _ => ProbePhase::Failed(Failure::Protocol),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mock_is_explicit_and_unknown_mode_never_falls_back_to_fake_success() {
        assert_eq!(Mode::from_query(""), Mode::Live);
        assert_eq!(Mode::from_query("?mode=mock"), Mode::Mock);
        assert_eq!(Mode::from_query("?mode=unknown"), Mode::Live);
        assert_eq!(classify(200, b"{}"), ProbePhase::Failed(Failure::Protocol));
        assert_eq!(
            classify(
                404,
                br#"{"code":"not_found","current_revision":null,"issues":[]}"#
            ),
            ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented)
        );
        assert_eq!(
            classify(404, b"html"),
            ProbePhase::Failed(Failure::Protocol)
        );
    }
    #[test]
    fn cancel_newer_request_and_duplicate_result_cannot_replace_current_probe() {
        let first = ProbeState::default().begin().unwrap();
        let cancelled = first.cancel().unwrap();
        assert_eq!(
            cancelled.finish(first.generation, ProbePhase::Failed(Failure::Transport)),
            cancelled
        );
        let newer = first.begin().unwrap();
        assert_eq!(
            newer.finish(first.generation, ProbePhase::Failed(Failure::Transport)),
            newer
        );
        let finished = newer.finish(
            newer.generation,
            ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented),
        );
        assert_eq!(
            finished.finish(newer.generation, ProbePhase::Failed(Failure::Transport)),
            finished
        );
        assert_eq!(
            ProbeState {
                generation: u64::MAX,
                phase: ProbePhase::Idle
            }
            .begin(),
            None
        );
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    /// Proposition: terminal replies cannot replace cancelled, settled or newer state.
    /// Domain: all u64 current/answered generations, four phase classes. No assumptions;
    /// no allocation, network, Kotlin or UI. Production ProbeState::finish is reached.
    /// Unwind2 suffices: this production path has no loops. Nothing is stubbed.
    #[kani::proof]
    #[kani::unwind(2)]
    fn completion_requires_current_pending_generation() {
        let generation: u64 = kani::any();
        let answered: u64 = kani::any();
        let phase = match kani::any::<u8>() % 4 {
            0 => ProbePhase::Idle,
            1 => ProbePhase::Pending,
            2 => ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented),
            _ => ProbePhase::Failed(Failure::Transport),
        };
        let state = ProbeState { generation, phase };
        let result = state.finish(answered, ProbePhase::Failed(Failure::Protocol));
        if generation != answered || phase != ProbePhase::Pending {
            assert_eq!(result, state);
        } else {
            assert_eq!(result.phase, ProbePhase::Failed(Failure::Protocol));
        }
    }
    /// All u64 generations, no assumptions/stubs. Production checked increment must refuse
    /// exhaustion and every admitted increment fences the previous request. Unwind2, no loops.
    #[kani::proof]
    #[kani::unwind(2)]
    fn cancellation_never_wraps_or_accepts_previous_completion() {
        let state = ProbeState {
            generation: kani::any(),
            phase: ProbePhase::Pending,
        };
        match state.cancel() {
            None => assert_eq!(state.generation, u64::MAX),
            Some(cancelled) => {
                assert!(cancelled.generation > state.generation);
                assert_eq!(
                    cancelled.finish(state.generation, ProbePhase::Failed(Failure::Transport)),
                    cancelled
                );
            }
        }
    }
}

#[cfg(test)]
mod compatibility {
    use super::*;
    #[test]
    fn canonical_shared_error_fixtures_agree_with_kotlin() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/client-boundary/v1/probe-cases.json"
        ))
        .unwrap();
        let cases = cases["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        for case in cases {
            let expected = match case["expected"].as_str().unwrap() {
                "unsupported" => ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented),
                "unauthenticated" => ProbePhase::Failed(Failure::Unauthenticated),
                "forbidden" => ProbePhase::Failed(Failure::Forbidden),
                "unavailable" => ProbePhase::Failed(Failure::Unavailable),
                "protocol" => ProbePhase::Failed(Failure::Protocol),
                _ => panic!("Unknown fixture expectation"),
            };
            let body = serde_json::to_vec(&case["body"]).unwrap();
            assert_eq!(
                classify(case["status"].as_u64().unwrap() as u16, &body),
                expected
            );
        }
    }
}
