//! Registration availability is a server fact, never an account or session receipt.
use cantos_contracts::theatre_identity::{
    Configuration, Registration, RegistrationUnavailableReason,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationFailure {
    Unavailable,
    Protocol,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RegistrationPhase {
    #[default]
    Idle,
    Checking,
    Available,
    Unavailable(RegistrationUnavailableReason),
    Failed(ConfigurationFailure),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegistrationState {
    generation: u64,
    phase: RegistrationPhase,
}

impl RegistrationState {
    pub fn phase(self) -> RegistrationPhase {
        self.phase
    }

    pub fn begin(&mut self) -> Option<u64> {
        if self.phase == RegistrationPhase::Checking {
            return None;
        }
        self.generation = self.generation.checked_add(1)?;
        self.phase = RegistrationPhase::Checking;
        Some(self.generation)
    }

    pub fn finish(
        &mut self,
        issued: u64,
        result: Result<Configuration, ConfigurationFailure>,
    ) -> bool {
        if self.generation != issued || self.phase != RegistrationPhase::Checking {
            return false;
        }
        self.phase = match result {
            Ok(Configuration {
                registration: Registration::EmailVerification {},
                ..
            }) => RegistrationPhase::Available,
            Ok(Configuration {
                registration: Registration::Unavailable { reason },
                ..
            }) => RegistrationPhase::Unavailable(reason),
            Err(failure) => RegistrationPhase::Failed(failure),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_contracts::theatre_identity::LoginMethod;

    #[test]
    fn server_unavailability_never_supplies_registration_or_session_success() {
        for reason in [
            RegistrationUnavailableReason::IdentityUnconfigured,
            RegistrationUnavailableReason::ProviderUnsupported,
            RegistrationUnavailableReason::RegistrationUnconfigured,
        ] {
            let mut state = RegistrationState::default();
            let issued = state.begin().unwrap();
            assert_eq!(state.begin(), None);
            assert!(state.finish(
                issued,
                Ok(Configuration {
                    login: LoginMethod::Unconfigured,
                    registration: Registration::Unavailable { reason },
                })
            ));
            assert_eq!(state.phase(), RegistrationPhase::Unavailable(reason));
            let settled = state;
            assert!(!state.finish(issued, Err(ConfigurationFailure::Unavailable)));
            assert_eq!(state, settled);
        }
    }

    #[test]
    fn retry_preserves_exact_failure_and_rejects_old_completion() {
        let mut state = RegistrationState::default();
        let old = state.begin().unwrap();
        assert!(state.finish(old, Err(ConfigurationFailure::Unavailable)));
        assert_eq!(
            state.phase(),
            RegistrationPhase::Failed(ConfigurationFailure::Unavailable)
        );
        let current = state.begin().unwrap();
        let checking = state;
        assert!(!state.finish(
            old,
            Ok(Configuration {
                login: LoginMethod::Oidc,
                registration: Registration::Unavailable {
                    reason: RegistrationUnavailableReason::ProviderUnsupported,
                },
            })
        ));
        assert_eq!(state, checking);
        assert!(state.finish(current, Err(ConfigurationFailure::Protocol)));
        assert_eq!(
            state.phase(),
            RegistrationPhase::Failed(ConfigurationFailure::Protocol)
        );
    }
}
