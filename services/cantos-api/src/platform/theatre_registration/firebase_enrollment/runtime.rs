//! Selected Firebase enrollment capability with the same direct-peer attempt boundary.
use super::application::FirebaseEnrollmentService;
use crate::platform::theatre_registration::{
    domain::{NetworkKey, RegistrationError},
    runtime::{AttemptPolicy, RegistrationAdmission},
};
use std::{net::IpAddr, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;

pub struct FirebaseEnrollmentRuntime {
    service: Arc<FirebaseEnrollmentService>,
    admission: RegistrationAdmission,
}
impl FirebaseEnrollmentRuntime {
    pub fn new(
        service: Arc<FirebaseEnrollmentService>,
        web_origin: &str,
        network_key: [u8; 32],
        max_inflight: u32,
        attempt_policy: AttemptPolicy,
    ) -> Result<Self, RegistrationError> {
        Ok(Self {
            service,
            admission: RegistrationAdmission::new(
                web_origin,
                network_key,
                max_inflight,
                attempt_policy,
            )?,
        })
    }
    pub fn service(&self) -> &Arc<FirebaseEnrollmentService> {
        &self.service
    }
    pub fn web_origin(&self) -> &str {
        self.admission.web_origin()
    }
    pub fn network_key(&self, peer: IpAddr) -> Result<NetworkKey, RegistrationError> {
        self.admission.network_key(peer)
    }
    pub fn admit(&self, key: NetworkKey) -> bool {
        self.admission.admit(key)
    }
    pub fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.admission.acquire()
    }
}
impl crate::composition::Store {
    pub fn with_theatre_firebase_enrollment(mut self, runtime: FirebaseEnrollmentRuntime) -> Self {
        self.theatre_registration = None;
        self.theatre_firebase_enrollment = Some(Arc::new(runtime));
        self
    }
    pub fn theatre_firebase_enrollment(&self) -> Option<&FirebaseEnrollmentRuntime> {
        self.theatre_firebase_enrollment.as_deref()
    }
}
