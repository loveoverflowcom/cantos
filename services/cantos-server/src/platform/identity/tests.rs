use std::{collections::BTreeMap, sync::Mutex};

use proptest::prelude::*;

use super::{
    domain::{
        decide_link, decide_registration, ActorId, ActorRecord, ExternalIdentityKey, IdentityError,
        LinkOutcome, Registration,
    },
    IdentityRegistry, IdentityService, RegistryError,
};

fn actor(id: &str) -> ActorId {
    ActorId::new(id).unwrap()
}

fn key(issuer: &str, subject: &str) -> ExternalIdentityKey {
    ExternalIdentityKey::new(issuer, subject).unwrap()
}

#[test]
fn legacy_actor_spelling_and_wire_barriers_are_preserved() {
    for value in [
        "alice",
        "creator-01",
        "a_b",
        "5b7b7e4e-0a1d-4a0e-9d44-3f3f2d6d1a01",
    ] {
        assert_eq!(actor(value).as_str(), value);
        assert_eq!(
            serde_json::from_str::<ActorId>(&format!("\"{value}\"")).unwrap(),
            actor(value)
        );
    }
    for value in ["", "Alice", "người", "a b", "a\0", &"a".repeat(65)] {
        assert_eq!(ActorId::new(value), Err(IdentityError::InvalidActorId));
        assert!(serde_json::from_value::<ActorId>(serde_json::json!(value))
            .unwrap_err()
            .to_string()
            .contains("invalid actor ID"));
    }
    assert_eq!(actor(&"x".repeat(64)).as_str().len(), 64);
}

#[test]
fn external_parts_reject_empty_controls_and_byte_overflow_without_normalizing() {
    for value in ["", " \t", "\u{2003}", "a\0b", "a\nb", "a\u{0085}b"] {
        assert_eq!(
            ExternalIdentityKey::new(value, "sub"),
            Err(IdentityError::InvalidIssuer)
        );
        assert_eq!(
            ExternalIdentityKey::new("issuer", value),
            Err(IdentityError::InvalidSubject)
        );
    }
    assert_eq!(
        ExternalIdentityKey::new("x".repeat(1025), "sub"),
        Err(IdentityError::InvalidIssuer)
    );
    assert_eq!(
        ExternalIdentityKey::new("issuer", "Đ".repeat(257)),
        Err(IdentityError::InvalidSubject)
    );
    assert_eq!(
        key(&"x".repeat(1024), &"Đ".repeat(256)).subject().len(),
        512
    );
    let original = key(" https://id.example/ ", "Người");
    assert_eq!(original.issuer(), " https://id.example/ ");
    assert_eq!(original.subject(), "Người");
    assert_ne!(original, key(" https://id.example/ ", "Người"));
    assert_eq!(
        serde_json::from_value::<ExternalIdentityKey>(serde_json::to_value(&original).unwrap())
            .unwrap(),
        original
    );
    for value in [
        serde_json::json!({"issuer":"", "subject":"s"}),
        serde_json::json!({"issuer":"i", "subject":"s", "email":"same@example.test"}),
    ] {
        assert!(serde_json::from_value::<ExternalIdentityKey>(value).is_err());
    }
}

#[test]
fn registration_partition_has_exact_outcomes() {
    let candidate = actor("new");
    let existing = ActorRecord {
        id: actor("old"),
        active: true,
    };
    for candidate_exists in [false, true] {
        assert_eq!(
            decide_registration(Some(&existing), &candidate, candidate_exists),
            Ok(Registration::Existing(actor("old")))
        );
        assert_eq!(
            decide_registration(
                Some(&ActorRecord {
                    active: false,
                    ..existing.clone()
                }),
                &candidate,
                candidate_exists
            ),
            Err(IdentityError::ActorInactive)
        );
    }
    assert_eq!(
        decide_registration(None, &candidate, false),
        Ok(Registration::Created(candidate.clone()))
    );
    assert_eq!(
        decide_registration(None, &candidate, true),
        Err(IdentityError::ActorIdCollision)
    );
}

#[test]
fn linking_partition_never_transfers_or_reactivates_a_binding() {
    for active in [false, true] {
        let target = ActorRecord {
            id: actor("a"),
            active,
        };
        for (binding, expected) in [
            (None, LinkOutcome::Linked),
            (Some(actor("a")), LinkOutcome::Existing),
        ] {
            let outcome = decide_link(Some(&target), binding.as_ref());
            assert_eq!(
                outcome,
                if active {
                    Ok(expected)
                } else {
                    Err(IdentityError::ActorInactive)
                }
            );
        }
        assert_eq!(
            decide_link(Some(&target), Some(&actor("b"))),
            Err(if active {
                IdentityError::IdentityAlreadyBound
            } else {
                IdentityError::ActorInactive
            })
        );
    }
    assert_eq!(decide_link(None, None), Err(IdentityError::ActorMissing));
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
struct State {
    actors: BTreeMap<ActorId, bool>,
    bindings: BTreeMap<ExternalIdentityKey, ActorId>,
}

#[derive(Default)]
struct FakeRegistry(Mutex<State>);

impl IdentityRegistry for &FakeRegistry {
    async fn lookup(
        &self,
        key: &ExternalIdentityKey,
    ) -> Result<Option<ActorRecord>, RegistryError> {
        let state = self.0.lock().unwrap();
        Ok(state.bindings.get(key).map(|id| ActorRecord {
            id: id.clone(),
            active: state.actors[id],
        }))
    }

    async fn register(
        &self,
        key: &ExternalIdentityKey,
        candidate: &ActorId,
    ) -> Result<Registration, RegistryError> {
        let mut state = self.0.lock().unwrap();
        let binding = state.bindings.get(key).map(|id| ActorRecord {
            id: id.clone(),
            active: state.actors[id],
        });
        let result = decide_registration(
            binding.as_ref(),
            candidate,
            state.actors.contains_key(candidate),
        )?;
        if let Registration::Created(id) = &result {
            state.actors.insert(id.clone(), true);
            state.bindings.insert(key.clone(), id.clone());
        }
        Ok(result)
    }

    async fn link(
        &self,
        key: &ExternalIdentityKey,
        target: &ActorId,
    ) -> Result<LinkOutcome, RegistryError> {
        let mut state = self.0.lock().unwrap();
        let actor = state.actors.get(target).map(|active| ActorRecord {
            id: target.clone(),
            active: *active,
        });
        let result = decide_link(actor.as_ref(), state.bindings.get(key))?;
        if result == LinkOutcome::Linked {
            state.bindings.insert(key.clone(), target.clone());
        }
        Ok(result)
    }
}

#[tokio::test]
async fn application_replay_and_explicit_linking_preserve_the_winner_and_legacy_actor() {
    let fake = FakeRegistry::default();
    fake.0.lock().unwrap().actors.insert(actor("legacy"), true);
    let service = IdentityService::new(&fake);
    let first = key("issuer-a", "same-email@example.test");
    assert_eq!(
        service.register(&first, &actor("alice")).await,
        Ok(Registration::Created(actor("alice")))
    );
    assert_eq!(
        service.register(&first, &actor("loser")).await,
        Ok(Registration::Existing(actor("alice")))
    );
    assert_eq!(
        service
            .register(&key("issuer-b", first.subject()), &actor("bob"))
            .await,
        Ok(Registration::Created(actor("bob")))
    );
    let second = key("issuer-a", "AnotherCase");
    assert_eq!(
        service.link(&second, &actor("legacy")).await,
        Ok(LinkOutcome::Linked)
    );
    assert_eq!(
        service.link(&second, &actor("legacy")).await,
        Ok(LinkOutcome::Existing)
    );
    let before = fake.0.lock().unwrap().clone();
    assert_eq!(
        service.link(&first, &actor("legacy")).await,
        Err(IdentityError::IdentityAlreadyBound.into())
    );
    assert_eq!(
        service
            .register(&key("issuer-a", "new"), &actor("legacy"))
            .await,
        Err(IdentityError::ActorIdCollision.into())
    );
    assert_eq!(*fake.0.lock().unwrap(), before);
    assert_eq!(
        service.lookup(&second).await.unwrap().unwrap().id,
        actor("legacy")
    );
    assert!(!before.actors.contains_key(&actor("loser")));
}

#[tokio::test]
async fn application_duplicate_registration_creates_one_actor_and_inactive_retry_is_rejected() {
    let fake = FakeRegistry::default();
    let service = IdentityService::new(&fake);
    let key = key("issuer", "subject");
    let a_id = actor("a");
    let b_id = actor("b");
    let (a, b) = tokio::join!(service.register(&key, &a_id), service.register(&key, &b_id));
    assert_eq!(a.unwrap().actor_id(), b.unwrap().actor_id());
    assert_eq!(fake.0.lock().unwrap().actors.len(), 1);
    let id = service.lookup(&key).await.unwrap().unwrap().id;
    fake.0.lock().unwrap().actors.insert(id, false);
    assert_eq!(
        service.register(&key, &actor("new")).await,
        Err(IdentityError::ActorInactive.into())
    );
    assert_eq!(fake.0.lock().unwrap().actors.len(), 1);
}

struct UnavailableRegistry;
impl IdentityRegistry for UnavailableRegistry {
    async fn lookup(&self, _: &ExternalIdentityKey) -> Result<Option<ActorRecord>, RegistryError> {
        Err(RegistryError::Unavailable)
    }
    async fn register(
        &self,
        _: &ExternalIdentityKey,
        _: &ActorId,
    ) -> Result<Registration, RegistryError> {
        Err(RegistryError::Unavailable)
    }
    async fn link(
        &self,
        _: &ExternalIdentityKey,
        _: &ActorId,
    ) -> Result<LinkOutcome, RegistryError> {
        Err(RegistryError::Unavailable)
    }
}

#[tokio::test]
async fn application_never_replaces_unavailable_registry_with_success() {
    let service = IdentityService::new(UnavailableRegistry);
    let key = key("issuer", "sub");
    assert_eq!(service.lookup(&key).await, Err(RegistryError::Unavailable));
    assert_eq!(
        service.register(&key, &actor("a")).await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(
        service.link(&key, &actor("a")).await,
        Err(RegistryError::Unavailable)
    );
}

proptest! {
    #[test]
    fn keys_preserve_exact_case_and_issuer_isolation(subject in "[a-z]{1,30}") {
        let lower = key("issuer-a", &subject);
        prop_assert_ne!(&lower, &key("issuer-a", &subject.to_uppercase()));
        prop_assert_ne!(&lower, &key("issuer-b", &subject));
        prop_assert_eq!(lower.subject(), subject);
    }

    #[test]
    fn every_registration_replay_keeps_original_actor(candidate in "[a-z0-9_-]{1,64}", winner in "[a-z0-9_-]{1,64}", occupied in any::<bool>()) {
        let record = ActorRecord { id:actor(&winner), active:true };
        prop_assert_eq!(decide_registration(Some(&record), &actor(&candidate), occupied), Ok(Registration::Existing(actor(&winner))));
        prop_assert_eq!(decide_link(Some(&record), Some(&record.id)), Ok(LinkOutcome::Existing));
    }
}
