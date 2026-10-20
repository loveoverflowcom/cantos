//! Pure session lifetime and rotation decisions over server policy and clock/state facts.

/// Immutable seconds-based policy. Only checked construction can create a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionPolicy {
    access_ttl: u64,
    idle_ttl: u64,
    absolute_ttl: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidSessionPolicy {
    ZeroAccess,
    AccessExceedsIdle,
    IdleExceedsAbsolute,
}

impl SessionPolicy {
    pub const fn new(
        access_ttl: u64,
        idle_ttl: u64,
        absolute_ttl: u64,
    ) -> Result<Self, InvalidSessionPolicy> {
        if access_ttl == 0 {
            return Err(InvalidSessionPolicy::ZeroAccess);
        }
        if access_ttl > idle_ttl {
            return Err(InvalidSessionPolicy::AccessExceedsIdle);
        }
        if idle_ttl > absolute_ttl {
            return Err(InvalidSessionPolicy::IdleExceedsAbsolute);
        }
        Ok(Self {
            access_ttl,
            idle_ttl,
            absolute_ttl,
        })
    }

    pub const fn access_seconds(self) -> u64 {
        self.access_ttl
    }
    pub const fn idle_seconds(self) -> u64 {
        self.idle_ttl
    }
    pub const fn absolute_seconds(self) -> u64 {
        self.absolute_ttl
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifetime {
    pub absolute: u64,
    pub idle: u64,
    pub access: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Inactive,
    Expired,
}
pub fn issue(
    policy: SessionPolicy,
    now: u64,
    proof_expiry: u64,
    provider_expiry: u64,
) -> Result<Lifetime, Refusal> {
    let absolute = proof_expiry
        .min(provider_expiry)
        .min(now.saturating_add(policy.absolute_seconds()));
    renew(policy, now, absolute)
}
fn renew(policy: SessionPolicy, now: u64, absolute: u64) -> Result<Lifetime, Refusal> {
    if now >= absolute {
        return Err(Refusal::Expired);
    }
    Ok(Lifetime {
        absolute,
        idle: absolute.min(now.saturating_add(policy.idle_seconds())),
        access: absolute.min(now.saturating_add(policy.access_seconds())),
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Refuse(Refusal),
    RevokeFamily,
    Rotate(Lifetime),
}
pub fn rotate(
    policy: SessionPolicy,
    now: u64,
    absolute: u64,
    idle: u64,
    active: bool,
    used: bool,
) -> Rotation {
    if !active {
        return Rotation::Refuse(Refusal::Inactive);
    }
    // A spent secret identifies compromise even after its access/idle lifetime. Never resurrect.
    if used {
        return Rotation::RevokeFamily;
    }
    if now >= idle {
        return Rotation::Refuse(Refusal::Expired);
    }
    match renew(policy, now, absolute) {
        Ok(lifetime) => Rotation::Rotate(lifetime),
        Err(error) => Rotation::Refuse(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_construction_checks_zero_order_and_equal_boundaries() {
        use InvalidSessionPolicy::*;
        for (access, idle, absolute, expected) in [
            (0, 0, 0, ZeroAccess),
            (0, 300, 600, ZeroAccess),
            (1, 0, 600, AccessExceedsIdle),
            (301, 300, 600, AccessExceedsIdle),
            (120, 600, 599, IdleExceedsAbsolute),
            (1, 1, 0, IdleExceedsAbsolute),
        ] {
            assert_eq!(SessionPolicy::new(access, idle, absolute), Err(expected));
        }
        for (access, idle, absolute) in [
            (1, 1, 1),
            (120, 120, 600),
            (120, 600, 600),
            (120, 300, 600),
            (u64::MAX, u64::MAX, u64::MAX),
        ] {
            let policy = SessionPolicy::new(access, idle, absolute).unwrap();
            assert_eq!(policy.access_seconds(), access);
            assert_eq!(policy.idle_seconds(), idle);
            assert_eq!(policy.absolute_seconds(), absolute);
        }
    }

    #[test]
    fn issuance_has_independent_exact_deadline_examples() {
        // Literal expected deadlines are independent of the algorithm and registration mapping.
        for (ttls, now, proof, provider, expected) in [
            ((120, 300, 600), 100, 2000, 2000, (220, 400, 700)),
            ((120, 600, 900), 100, 2000, 2000, (220, 700, 1000)),
            ((120, 300, 600), 100, 350, 2000, (220, 350, 350)),
            ((120, 600, 900), 100, 2000, 230, (220, 230, 230)),
            ((120, 300, 600), 100, 219, 2000, (219, 219, 219)),
            ((120, 600, 900), 100, 2000, 101, (101, 101, 101)),
            ((2, 3, 5), 100, 2000, 2000, (102, 103, 105)),
            ((1, 1, 1), 0, 2000, 2000, (1, 1, 1)),
            (
                (120, 300, 600),
                u64::MAX - 60,
                u64::MAX,
                u64::MAX,
                (u64::MAX, u64::MAX, u64::MAX),
            ),
            (
                (u64::MAX, u64::MAX, u64::MAX),
                1,
                u64::MAX,
                u64::MAX,
                (u64::MAX, u64::MAX, u64::MAX),
            ),
        ] {
            let policy = SessionPolicy::new(ttls.0, ttls.1, ttls.2).unwrap();
            assert_eq!(
                issue(policy, now, proof, provider),
                Ok(Lifetime {
                    access: expected.0,
                    idle: expected.1,
                    absolute: expected.2,
                })
            );
        }
    }

    #[test]
    fn issuance_refuses_at_or_after_either_authority_deadline() {
        let policy = SessionPolicy::new(120, 300, 600).unwrap();
        for (now, proof, provider) in [
            (100, 100, 2000),
            (100, 2000, 100),
            (100, 99, 2000),
            (100, 2000, 99),
            (u64::MAX, u64::MAX, u64::MAX),
        ] {
            assert_eq!(issue(policy, now, proof, provider), Err(Refusal::Expired));
        }
    }

    #[test]
    fn inactive_precedes_replay_and_active_replay_precedes_expiry() {
        let policy = SessionPolicy::new(120, 300, 600).unwrap();
        for (now, absolute, idle) in [(100, 900, 600), (600, 900, 600), (900, 900, 600)] {
            for used in [false, true] {
                assert_eq!(
                    rotate(policy, now, absolute, idle, false, used),
                    Rotation::Refuse(Refusal::Inactive)
                );
            }
            assert_eq!(
                rotate(policy, now, absolute, idle, true, true),
                Rotation::RevokeFamily
            );
        }
        for (now, absolute, idle) in [
            (600, 900, 600),
            (601, 900, 600),
            (900, 900, 900),
            (900, 900, 901),
        ] {
            assert_eq!(
                rotate(policy, now, absolute, idle, true, false),
                Rotation::Refuse(Refusal::Expired)
            );
        }
    }

    #[test]
    fn rotation_uses_policy_and_never_rebases_the_absolute_deadline() {
        let policy = SessionPolicy::new(120, 300, 600).unwrap();
        let first = issue(policy, 100, 2000, 2000).unwrap();
        assert_eq!(
            first,
            Lifetime {
                access: 220,
                idle: 400,
                absolute: 700
            }
        );
        let next = rotate(policy, 399, first.absolute, first.idle, true, false);
        assert_eq!(
            next,
            Rotation::Rotate(Lifetime {
                access: 519,
                idle: 699,
                absolute: 700
            })
        );
        let Rotation::Rotate(next) = next else {
            panic!("rotation refused")
        };
        assert_eq!(
            rotate(policy, 698, next.absolute, next.idle, true, false),
            Rotation::Rotate(Lifetime {
                access: 700,
                idle: 700,
                absolute: 700
            })
        );
        assert_eq!(
            rotate(policy, 700, 700, 700, true, false),
            Rotation::Refuse(Refusal::Expired)
        );
        // A shorter original proof/provider cap is preserved as well.
        assert_eq!(
            rotate(policy, 249, 250, 250, true, false),
            Rotation::Rotate(Lifetime {
                access: 250,
                idle: 250,
                absolute: 250
            })
        );
        let small = SessionPolicy::new(2, 3, 5).unwrap();
        assert_eq!(
            rotate(small, 101, 105, 103, true, false),
            Rotation::Rotate(Lifetime {
                access: 103,
                idle: 104,
                absolute: 105
            })
        );
        assert_eq!(
            rotate(policy, u64::MAX - 1, u64::MAX, u64::MAX, true, false),
            Rotation::Rotate(Lifetime {
                access: u64::MAX,
                idle: u64::MAX,
                absolute: u64::MAX
            })
        );
    }

    #[test]
    fn valid_policies_preserve_deadline_order_over_bounded_time_and_state_facts() {
        for access in 1..=4 {
            for idle in access..=4 {
                for absolute in idle..=4 {
                    let policy = SessionPolicy::new(access, idle, absolute).unwrap();
                    for now in 0..=8 {
                        for proof in 0..=8 {
                            for provider in 0..=8 {
                                match issue(policy, now, proof, provider) {
                                    Err(error) => {
                                        assert_eq!(error, Refusal::Expired);
                                        assert!(now >= proof || now >= provider);
                                    }
                                    Ok(first) => {
                                        assert!(now < first.access && first.access <= first.idle);
                                        assert!(first.idle <= first.absolute);
                                        assert!(
                                            first.absolute <= proof && first.absolute <= provider
                                        );
                                        for later in now..=8 {
                                            match rotate(
                                                policy,
                                                later,
                                                first.absolute,
                                                first.idle,
                                                true,
                                                false,
                                            ) {
                                                Rotation::Rotate(next) => {
                                                    assert_eq!(next.absolute, first.absolute);
                                                    assert!(
                                                        later < next.access
                                                            && next.access <= next.idle
                                                    );
                                                    assert!(next.idle <= next.absolute);
                                                }
                                                Rotation::Refuse(error) => {
                                                    assert_eq!(error, Refusal::Expired);
                                                    assert!(
                                                        later >= first.idle
                                                            || later >= first.absolute
                                                    );
                                                }
                                                Rotation::RevokeFamily => {
                                                    panic!("unused refresh treated as replay")
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
