use super::*;
use crate::contexts::media::domain::publication::tests::{fixture, Fixture};
struct VerifiedDelivery;
impl PublicationDelivery for VerifiedDelivery {
    fn observe(&self, object: PublicationObject) -> PublicationDeliveryFuture {
        Box::pin(async move {
            Ok(DeliveryObservation::Verified(
                DeliveryEvidence::new(
                    "publication-fixture".into(),
                    object.sha256,
                    object.byte_length,
                    4800,
                    9000,
                )
                .map_err(|_| PublicationDependencyError::InvalidObservation)?,
            ))
        })
    }
}
struct FailingDelivery;
impl PublicationDelivery for FailingDelivery {
    fn observe(&self, _object: PublicationObject) -> PublicationDeliveryFuture {
        Box::pin(async { Err(PublicationDependencyError::Unavailable) })
    }
}
fn request(f: &Fixture) -> dto::PublishAudioReleaseRequest {
    dto::PublishAudioReleaseRequest {
        operation_id: "80000000-0000-4000-8000-000000000001".into(),
        candidate_digest: f.candidate.document().digest.clone(),
        expected_pointer_version: 0,
        expected_active_release_id: None,
    }
}
fn id() -> ReleaseId {
    ReleaseId::parse("90000000-0000-4000-8000-000000000001").unwrap()
}
#[tokio::test]
async fn real_delivery_observation_and_preparation_modules_compose_with_a_named_fake() {
    let f = fixture();
    let delivery = observe_delivery(&VerifiedDelivery, &f.r).await.unwrap();
    let mut facts = f.facts();
    facts.delivery = &delivery;
    let pointer = sql::Pointer {
        version: 0,
        active: None,
    };
    let (release, next) = prepare(&facts, &pointer, &request(&f), id()).unwrap();
    assert_eq!(next, 1);
    assert_eq!(
        release.document().manifest.delivery_object_id,
        "publication-fixture"
    );
    assert_eq!(
        pointer,
        sql::Pointer {
            version: 0,
            active: None
        }
    );
}
#[tokio::test]
async fn unavailable_production_delivery_never_proposes_activation() {
    let f = fixture();
    let delivery = observe_delivery(&UnavailableDelivery, &f.r).await.unwrap();
    let mut facts = f.facts();
    facts.delivery = &delivery;
    let pointer = sql::Pointer {
        version: 0,
        active: None,
    };
    assert!(
        matches!(prepare(&facts,&pointer,&request(&f),id()),Err(PublicationStoreError::Blocked(b)) if b==vec![dto::PublicationBlocker::DeliveryUnavailable])
    );
    assert_eq!(
        pointer,
        sql::Pointer {
            version: 0,
            active: None
        }
    );
}
#[tokio::test]
async fn dependency_failures_are_typed_and_do_not_become_a_success_observation() {
    let f = fixture();
    assert!(matches!(
        observe_delivery(&FailingDelivery, &f.r).await,
        Err(PublicationStoreError::Dependency(
            PublicationDependencyError::Unavailable
        ))
    ));
}
#[test]
fn a_second_publisher_using_the_first_expected_pointer_conflicts() {
    let f = fixture();
    let first = sql::Pointer {
        version: 0,
        active: None,
    };
    let (release, next) = prepare(&f.facts(), &first, &request(&f), id()).unwrap();
    let after = sql::Pointer {
        version: next,
        active: Some(release.document().id.clone()),
    };
    assert!(
        matches!(prepare(&f.facts(),&after,&request(&f),id()),Err(PublicationStoreError::Conflict{version:1,active_release_id:Some(id)}) if id==release.document().id)
    );
    assert_eq!(after.version, 1);
}
#[test]
fn failed_replacement_has_no_pointer_effect_and_preserves_prior_manifest() {
    let f = fixture();
    let (prior, version) = prepare(
        &f.facts(),
        &sql::Pointer {
            version: 0,
            active: None,
        },
        &request(&f),
        id(),
    )
    .unwrap();
    let pointer = sql::Pointer {
        version,
        active: Some(prior.document().id.clone()),
    };
    let snapshot = prior.document().clone();
    let mut request = request(&f);
    request.expected_pointer_version = version;
    request.expected_active_release_id = pointer.active.clone();
    let mut facts = f.facts();
    facts.object = &ObjectObservation::Missing;
    assert!(
        matches!(prepare(&facts,&pointer,&request,id()),Err(PublicationStoreError::Blocked(b)) if b==vec![dto::PublicationBlocker::ObjectMissing])
    );
    assert_eq!(prior.document(), &snapshot);
    assert_eq!(pointer.version, 1);
}
