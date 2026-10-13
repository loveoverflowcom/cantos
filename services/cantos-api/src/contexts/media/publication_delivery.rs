//! Narrow provider-neutral publication observation. It cannot expose or promote staging bytes.
use super::domain::publication::{DeliveryObservation, Sha256};
use std::{future::Future, pin::Pin};

#[derive(Clone, Debug)]
pub struct PublicationObject {
    pub rendition_id: String,
    pub sha256: Sha256,
    pub byte_length: u64,
    pub profile_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationDependencyError {
    Unavailable,
    InvalidObservation,
}
pub type PublicationDeliveryFuture = Pin<
    Box<
        dyn Future<Output = Result<DeliveryObservation, PublicationDependencyError>>
            + Send
            + 'static,
    >,
>;
/// Concrete consumer: the publication preparation/use case. Production uses UnavailableDelivery;
/// tests use named deterministic delivery fakes. Provider integration must implement this port
/// after an approved publication-class object has actually been verified via its delivery path.
pub trait PublicationDelivery: Send + Sync {
    fn observe(&self, object: PublicationObject) -> PublicationDeliveryFuture;
}
pub(crate) struct UnavailableDelivery;
impl PublicationDelivery for UnavailableDelivery {
    fn observe(&self, _object: PublicationObject) -> PublicationDeliveryFuture {
        Box::pin(async { Ok(DeliveryObservation::Unavailable) })
    }
}
