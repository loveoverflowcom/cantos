//! Trusted host boundary: bounded anonymous attempts and direct-peer abuse keys.
use super::{
    application::RegistrationService,
    domain::{NetworkKey, RegistrationError},
};
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
    web_origin: String,
    network_key: [u8; 32],
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
        let url = url::Url::parse(web_origin).map_err(|_| RegistrationError::Configuration)?;
        if url.origin().ascii_serialization() != web_origin
            || !(1..=32).contains(&max_inflight)
            || network_key == [0; 32]
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            service,
            web_origin: web_origin.into(),
            network_key,
            slots: Arc::new(Semaphore::new(max_inflight as usize)),
            attempts: Mutex::new(Attempts::default()),
            attempt_policy,
        })
    }
    pub fn web_origin(&self) -> &str {
        &self.web_origin
    }
    pub fn service(&self) -> &Arc<RegistrationService> {
        &self.service
    }
    pub fn network_key(&self, peer: IpAddr) -> Result<NetworkKey, RegistrationError> {
        NetworkKey::from_peer(peer, &self.network_key)
    }
    /// Process-local attempt bounds supplement durable mail budgets. Restart/multiple hosts do
    /// not share these counters; deployment-wide ingress limiting remains live acceptance.
    pub fn admit(&self, key: NetworkKey) -> bool {
        self.attempts.lock().is_ok_and(|mut attempts| {
            attempts.admit(*key.as_bytes(), self.attempt_policy, Instant::now())
        })
    }
    pub fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.slots.clone().try_acquire_owned().ok()
    }
}
impl crate::composition::Store {
    pub fn with_theatre_registration(mut self, runtime: RegistrationRuntime) -> Self {
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
