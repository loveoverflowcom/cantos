//! Pure decision for the listeners and timer `runtime::mount` leaves running for the lifetime of
//! the shell. They are mounted once, but the shell can block the network afterwards: a same-tab
//! email link whose history scrub failed leaves an unscrubbed proof in the address bar, and until
//! it is cleared nothing may issue a request. Each trigger therefore asks here, with the live
//! signals, instead of acting on what was true at mount time.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// The window regained focus.
    Focus,
    /// The 60 s keep-alive tick.
    Interval { page_hidden: bool, signed_in: bool },
    /// The shared owner withdrew the session (401, another tab's cookie change).
    CredentialLoss,
    /// `visibilitychange`.
    Visibility { page_hidden: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Skip,
    /// Re-read the session from the API.
    Refresh,
    /// Flush the playback checkpoint (a protected write).
    FlushCheckpoint,
    /// Close the local account view and clean the dead session up.
    CredentialsLost,
}

/// `network_allowed` is the shell's live gate (configuration valid and no unscrubbed proof in the
/// URL); `rate_held` is the shared owner's 429 hold. A held session read would only blank a valid
/// account, so refreshes wait for the hold; cleanup and writes are gated by the owner itself.
pub fn decide(trigger: Trigger, network_allowed: bool, rate_held: bool) -> Action {
    if !network_allowed {
        return Action::Skip;
    }
    match trigger {
        Trigger::Focus if !rate_held => Action::Refresh,
        Trigger::Interval {
            page_hidden: false,
            signed_in: true,
        } if !rate_held => Action::Refresh,
        Trigger::CredentialLoss => Action::CredentialsLost,
        Trigger::Visibility { page_hidden: true } => Action::FlushCheckpoint,
        _ => Action::Skip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIGGERS: [Trigger; 8] = [
        Trigger::Focus,
        Trigger::Interval {
            page_hidden: false,
            signed_in: true,
        },
        Trigger::Interval {
            page_hidden: true,
            signed_in: true,
        },
        Trigger::Interval {
            page_hidden: false,
            signed_in: false,
        },
        Trigger::CredentialLoss,
        Trigger::Visibility { page_hidden: true },
        Trigger::Visibility { page_hidden: false },
        Trigger::Interval {
            page_hidden: true,
            signed_in: false,
        },
    ];

    #[test]
    fn an_unscrubbed_proof_in_the_url_silences_every_mounted_listener_and_timer() {
        for trigger in TRIGGERS {
            for held in [false, true] {
                assert_eq!(decide(trigger, false, held), Action::Skip, "{trigger:?}");
            }
        }
    }

    #[test]
    fn once_the_network_is_allowed_each_trigger_keeps_its_own_action() {
        assert_eq!(decide(Trigger::Focus, true, false), Action::Refresh);
        let tick = |page_hidden, signed_in| Trigger::Interval {
            page_hidden,
            signed_in,
        };
        assert_eq!(decide(tick(false, true), true, false), Action::Refresh);
        assert_eq!(decide(tick(true, true), true, false), Action::Skip);
        assert_eq!(decide(tick(false, false), true, false), Action::Skip);
        assert_eq!(
            decide(Trigger::CredentialLoss, true, false),
            Action::CredentialsLost
        );
        assert_eq!(
            decide(Trigger::Visibility { page_hidden: true }, true, false),
            Action::FlushCheckpoint
        );
        assert_eq!(
            decide(Trigger::Visibility { page_hidden: false }, true, false),
            Action::Skip
        );
    }

    #[test]
    fn a_429_hold_postpones_session_reads_but_never_the_local_loss_of_credentials() {
        assert_eq!(decide(Trigger::Focus, true, true), Action::Skip);
        assert_eq!(
            decide(
                Trigger::Interval {
                    page_hidden: false,
                    signed_in: true
                },
                true,
                true
            ),
            Action::Skip
        );
        assert_eq!(
            decide(Trigger::CredentialLoss, true, true),
            Action::CredentialsLost
        );
    }
}
