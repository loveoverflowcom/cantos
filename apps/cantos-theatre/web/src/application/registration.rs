//! Registration availability is a server fact, never an account or session receipt.
use cantos_contracts::theatre_identity::{
    Configuration, LoginMethod, Registration, RegistrationUnavailableReason,
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
pub enum LoginPhase {
    #[default]
    Idle,
    Checking,
    Available,
    Unconfigured,
    Failed(ConfigurationFailure),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegistrationState {
    generation: u64,
    phase: RegistrationPhase,
    login: LoginPhase,
}

impl RegistrationState {
    pub fn phase(self) -> RegistrationPhase {
        self.phase
    }

    pub fn login(self) -> LoginPhase {
        self.login
    }

    pub fn begin(&mut self) -> Option<u64> {
        if self.phase == RegistrationPhase::Checking {
            return None;
        }
        self.generation = self.generation.checked_add(1)?;
        self.phase = RegistrationPhase::Checking;
        self.login = LoginPhase::Checking;
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
        self.login = match &result {
            Ok(Configuration {
                login: LoginMethod::Oidc,
                ..
            }) => LoginPhase::Available,
            Ok(Configuration {
                login: LoginMethod::Unconfigured,
                ..
            }) => LoginPhase::Unconfigured,
            Err(failure) => LoginPhase::Failed(*failure),
        };
        self.phase = match result {
            Ok(Configuration {
                registration:
                    Registration::EmailVerification {} | Registration::FirebaseEmailVerification {},
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
    fn only_explicit_mounted_email_capabilities_enable_the_form() {
        for registration in [
            Registration::EmailVerification {},
            Registration::FirebaseEmailVerification {},
        ] {
            let mut state = RegistrationState::default();
            let issued = state.begin().unwrap();
            assert!(state.finish(
                issued,
                Ok(Configuration {
                    login: LoginMethod::Oidc,
                    registration
                })
            ));
            assert_eq!(state.phase(), RegistrationPhase::Available);
        }
    }

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

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use crate::application::account_view::AccountViewState;
    #[test]
    fn login_and_registration_form_an_independent_capability_matrix() {
        let mut account = AccountViewState::default();
        let cleanup = account.begin_logout().unwrap();
        assert!(account.finish_logout(cleanup, Ok(())));
        for login in [LoginMethod::Oidc, LoginMethod::Unconfigured] {
            for registration in [
                Registration::EmailVerification {},
                Registration::FirebaseEmailVerification {},
                Registration::Unavailable {
                    reason: RegistrationUnavailableReason::RegistrationUnconfigured,
                },
            ] {
                let mut state = RegistrationState::default();
                assert!(!account.can_login(state));
                let issued = state.begin().unwrap();
                assert!(!account.can_login(state));
                let available = !matches!(registration, Registration::Unavailable { .. });
                state.finish(
                    issued,
                    Ok(Configuration {
                        login,
                        registration,
                    }),
                );
                assert_eq!(account.can_login(state), login == LoginMethod::Oidc);
                assert_eq!(state.phase() == RegistrationPhase::Available, available);
                assert!(account.can_authenticate());
                let retry = state.begin().unwrap();
                assert!(!account.can_login(state));
                state.finish(retry, Err(ConfigurationFailure::Unavailable));
                assert_eq!(
                    state.login(),
                    LoginPhase::Failed(ConfigurationFailure::Unavailable)
                );
                assert!(!account.can_login(state));
            }
        }
    }
}
