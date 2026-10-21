//! Pure listener capability/probe state. A successful socket never supplies a publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Live,
    /// Isolated development previews exist only in a build with the `preview` feature.
    #[cfg(feature = "preview")]
    Mock,
    #[cfg(feature = "preview")]
    MembershipPreview,
    #[cfg(feature = "preview")]
    AuthPreview,
}
impl Mode {
    /// A production build has no preview to select: every query, including the preview ones,
    /// is the live mode. Only a build with the `preview` feature honors the exact preview URLs.
    pub fn from_query(query: &str) -> Self {
        match query {
            #[cfg(feature = "preview")]
            "?mode=mock" => Self::Mock,
            #[cfg(feature = "preview")]
            "?mode=membership-preview" => Self::MembershipPreview,
            #[cfg(feature = "preview")]
            "?mode=auth-preview" => Self::AuthPreview,
            _ => Self::Live,
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
/// Probe requests must be issued through `begin`, never fabricated by callers.
///
/// ```compile_fail
/// use cantos_theatre::live::{ProbePhase, ProbeState};
/// let unissued = ProbeState { generation: 1, phase: ProbePhase::Pending };
/// ```
pub struct ProbeState {
    generation: u64,
    phase: ProbePhase,
}
impl ProbeState {
    pub const fn generation(self) -> u64 {
        self.generation
    }
    pub const fn phase(self) -> ProbePhase {
        self.phase
    }
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
        assert_eq!(
            Mode::from_query("?mode=membership-preview"),
            Mode::MembershipPreview
        );
        assert_eq!(
            Mode::from_query("?mode=membership-preview&fixture=1"),
            Mode::Live
        );
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

    #[test]
    fn completion_matrix_accepts_only_current_pending_terminal_replies() {
        let phases = [
            ProbePhase::Idle,
            ProbePhase::Pending,
            ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented),
            ProbePhase::Failed(Failure::Transport),
            ProbePhase::Failed(Failure::Protocol),
            ProbePhase::Failed(Failure::Unauthenticated),
            ProbePhase::Failed(Failure::Forbidden),
            ProbePhase::Failed(Failure::Unavailable),
        ];
        for generation in [0, 1, 2, u64::MAX - 1, u64::MAX] {
            for answered in [0, 1, 2, u64::MAX - 1, u64::MAX] {
                for phase in phases {
                    for reply in phases {
                        let state = ProbeState { generation, phase };
                        // Authored transition table: no implementation helper is the oracle.
                        let terminal = !matches!(reply, ProbePhase::Idle | ProbePhase::Pending);
                        let expected =
                            if generation == answered && phase == ProbePhase::Pending && terminal {
                                ProbeState {
                                    generation,
                                    phase: reply,
                                }
                            } else {
                                state
                            };
                        assert_eq!(state.finish(answered, reply), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn both_probe_commands_refuse_exhaustion_without_changing_any_fact() {
        for phase in [
            ProbePhase::Idle,
            ProbePhase::Pending,
            ProbePhase::Unsupported(Unsupported::ListenerApiNotImplemented),
            ProbePhase::Failed(Failure::Transport),
        ] {
            let exhausted = ProbeState {
                generation: u64::MAX,
                phase,
            };
            assert_eq!(exhausted.begin(), None);
            assert_eq!(exhausted.cancel(), None);
            assert_eq!(exhausted.generation(), u64::MAX);
            assert_eq!(exhausted.phase(), phase);
            let almost = ProbeState {
                generation: u64::MAX - 1,
                phase,
            };
            assert_eq!(
                almost.begin(),
                Some(ProbeState {
                    generation: u64::MAX,
                    phase: ProbePhase::Pending
                })
            );
            assert_eq!(
                almost.cancel(),
                Some(ProbeState {
                    generation: u64::MAX,
                    phase: ProbePhase::Idle
                })
            );
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
