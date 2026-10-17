//! Trusted host boundary: bounded anonymous attempts and direct-peer abuse keys.
use super::{
    application::RegistrationService,
    domain::{NetworkKey, PeerPolicy, RegistrationError},
};
use axum::http::HeaderMap;
use std::{
    collections::{BTreeMap, VecDeque},
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone, Copy)]
pub struct AttemptPolicy {
    window: Duration,
    global: usize,
    network: u32,
}
impl AttemptPolicy {
    pub fn new(
        window_seconds: u32,
        max_global: u32,
        max_network: u32,
    ) -> Result<Self, RegistrationError> {
        if !(1..=86400).contains(&window_seconds)
            || !(1..=10000).contains(&max_global)
            || !(1..=max_global).contains(&max_network)
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            window: Duration::from_secs(window_seconds.into()),
            global: max_global as usize,
            network: max_network,
        })
    }
}
#[derive(Default)]
struct Attempts {
    receipts: VecDeque<(Instant, [u8; 32])>,
    networks: BTreeMap<[u8; 32], u32>,
}
impl Attempts {
    fn admit(&mut self, key: [u8; 32], policy: AttemptPolicy, now: Instant) -> bool {
        while self
            .receipts
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= policy.window)
        {
            let (_, old) = self.receipts.pop_front().expect("front checked");
            if let Some(count) = self.networks.get_mut(&old) {
                *count -= 1;
                if *count == 0 {
                    self.networks.remove(&old);
                }
            }
        }
        if self.receipts.len() >= policy.global
            || self.networks.get(&key).copied().unwrap_or(0) >= policy.network
        {
            return false;
        }
        self.receipts.push_back((now, key));
        *self.networks.entry(key).or_default() += 1;
        true
    }
}

pub struct RegistrationRuntime {
    service: Arc<RegistrationService>,
    admission: RegistrationAdmission,
}

/// Shared anonymous-attempt bounds for the selected registration provider.
pub(crate) struct RegistrationAdmission {
    web_origin: String,
    network_key: [u8; 32],
    peer_policy: PeerPolicy,
    slots: Arc<Semaphore>,
    attempts: Mutex<Attempts>,
    attempt_policy: AttemptPolicy,
}
impl RegistrationRuntime {
    pub fn new(
        service: Arc<RegistrationService>,
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
    pub fn web_origin(&self) -> &str {
        self.admission.web_origin()
    }
    pub fn service(&self) -> &Arc<RegistrationService> {
        &self.service
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

impl RegistrationAdmission {
    pub(crate) fn new(
        web_origin: &str,
        network_key: [u8; 32],
        max_inflight: u32,
        attempt_policy: AttemptPolicy,
    ) -> Result<Self, RegistrationError> {
        let url = url::Url::parse(web_origin).map_err(|_| RegistrationError::Configuration)?;
        let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if url.origin().ascii_serialization() != web_origin
            || (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
            || !(1..=32).contains(&max_inflight)
            || network_key == [0; 32]
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            web_origin: web_origin.into(),
            network_key,
            peer_policy: PeerPolicy::DirectPeer,
            slots: Arc::new(Semaphore::new(max_inflight as usize)),
            attempts: Mutex::new(Attempts::default()),
            attempt_policy,
        })
    }
    pub(crate) fn web_origin(&self) -> &str {
        &self.web_origin
    }
    pub(crate) fn set_peer_policy(&mut self, policy: PeerPolicy) {
        self.peer_policy = policy;
    }
    /// Quota identity of the caller. Header lines are only evidence under a trusted proxy.
    pub(crate) fn network_key(
        &self,
        peer: IpAddr,
        headers: &HeaderMap,
    ) -> Result<NetworkKey, RegistrationError> {
        let forwarded = headers
            .get_all("x-forwarded-for")
            .iter()
            .map(|line| line.to_str().map_err(|_| RegistrationError::Unavailable))
            .collect::<Result<Vec<_>, _>>();
        let client = match (&self.peer_policy, forwarded) {
            (PeerPolicy::DirectPeer, _) => peer,
            (policy, Ok(lines)) => policy.client_ip(peer, &lines)?,
            (_, Err(error)) => return Err(error),
        };
        NetworkKey::from_peer(client, &self.network_key)
    }
    pub(crate) fn admit(&self, key: NetworkKey) -> bool {
        self.attempts.lock().is_ok_and(|mut attempts| {
            attempts.admit(*key.as_bytes(), self.attempt_policy, Instant::now())
        })
    }
    pub(crate) fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.slots.clone().try_acquire_owned().ok()
    }
}

impl crate::composition::Store {
    pub fn with_theatre_registration(mut self, runtime: RegistrationRuntime) -> Self {
        self.theatre_firebase_enrollment = None;
        self.theatre_registration = Some(Arc::new(runtime));
        self
    }
    pub fn theatre_registration(&self) -> Option<&RegistrationRuntime> {
        self.theatre_registration.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attempts_bound_global_and_peer_and_expire_without_unbounded_keys() {
        let policy = AttemptPolicy::new(60, 3, 2).unwrap();
        let mut attempts = Attempts::default();
        let now = Instant::now();
        assert!(attempts.admit([1; 32], policy, now));
        assert!(attempts.admit([1; 32], policy, now));
        assert!(!attempts.admit([1; 32], policy, now));
        assert!(attempts.admit([2; 32], policy, now));
        assert!(!attempts.admit([3; 32], policy, now));
        assert_eq!(attempts.networks.len(), 2);
        assert!(attempts.admit([3; 32], policy, now + Duration::from_secs(60)));
        assert_eq!(attempts.networks.len(), 1);
    }
}
