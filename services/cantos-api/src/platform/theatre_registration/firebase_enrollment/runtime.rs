//! Selected Firebase enrollment capability with the same attempt boundary and peer policy.
use super::application::FirebaseEnrollmentService;
use crate::platform::theatre_registration::{
    domain::{NetworkKey, PeerPolicy, RegistrationError},
    runtime::{AttemptPolicy, RegistrationAdmission},
};
use axum::http::HeaderMap;
use std::{net::IpAddr, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;

pub struct FirebaseEnrollmentRuntime {
    service: Arc<FirebaseEnrollmentService>,
    admission: RegistrationAdmission,
    native_enabled: bool,
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
            native_enabled: false,
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
    /// Explicit development/activation boundary, independent of Web registration enablement.
    pub fn with_native_enabled(mut self, enabled: bool) -> Self {
        self.native_enabled = enabled;
        self
    }
    pub fn native_enabled(&self) -> bool {
        self.native_enabled
    }
    /// Replace the default direct-peer quota identity with an explicit, validated policy.
    pub fn with_peer_policy(mut self, policy: PeerPolicy) -> Self {
        self.admission.set_peer_policy(policy);
        self
    }
    pub fn network_key(
        &self,
        peer: IpAddr,
        headers: &HeaderMap,
    ) -> Result<NetworkKey, RegistrationError> {
        self.admission.network_key(peer, headers)
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
