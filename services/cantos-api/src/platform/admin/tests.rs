use super::domain::{authorize, map_groups, Action, AdminError, Role, RoleEvidence};
use crate::platform::identity::domain::{ActorId, ExternalIdentityKey};

fn evidence(role: Role) -> RoleEvidence {
    RoleEvidence::verified(
        ExternalIdentityKey::new("issuer", "subject").unwrap(),
        role,
        100,
        105,
    )
}

#[test]
fn only_allowlisted_groups_map_to_closed_platform_roles() {
    assert_eq!(map_groups(&[]), Err(AdminError::MissingRole));
    assert_eq!(map_groups(&["admin".into()]), Err(AdminError::UnknownRole));
    assert_eq!(
        map_groups(&["cantos_system_auditors".into()]),
        Ok(Role::Auditor)
    );
    assert_eq!(
        map_groups(&["cantos_identity_operators".into()]),
        Ok(Role::IdentityOperator)
    );
    assert_eq!(
        map_groups(&["cantos_identity_operators".into(), "superadmin".into()]),
        Err(AdminError::UnknownRole)
    );
}

#[test]
fn complete_small_role_action_activity_revocation_time_matrix_is_fail_closed() {
    let actor = ActorId::new("minh").unwrap();
    for role in [Role::Auditor, Role::IdentityOperator] {
        for action in [Action::Capabilities, Action::RevokeAccess] {
            for active in [false, true] {
                for revoked in [false, true] {
                    for now in [99, 100, 104, 105, 106] {
                        let result =
                            authorize(&evidence(role), &actor, active, revoked, action, now);
                        let expected = if !(100..105).contains(&now) {
                            Err(AdminError::StaleRole)
                        } else if !active {
                            Err(AdminError::InactiveActor)
                        } else if revoked {
                            Err(AdminError::Revoked)
                        } else if role == Role::Auditor && action == Action::RevokeAccess {
                            Err(AdminError::Forbidden)
                        } else {
                            Ok(())
                        };
                        assert_eq!(result.map(|_| ()), expected);
                    }
                }
            }
        }
    }
}

proptest::proptest! {
    #[test]
    fn any_unrecognized_group_fails_even_beside_an_operator_group(group in ".{0,128}") {
        proptest::prop_assume!(group!="cantos_identity_operators" && group!="cantos_system_auditors");
        proptest::prop_assert_eq!(map_groups(&["cantos_identity_operators".into(),group]),Err(AdminError::UnknownRole));
    }
}
