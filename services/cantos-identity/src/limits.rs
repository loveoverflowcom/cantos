//! In-memory abuse budgets: fixed windows, bounded key count, process-local.
//!
//! State is lost on restart, so a restart resets every budget; the durable protections are the
//! proof lifetime, one-use proofs and the KDF cost, not these counters. Keys are salted hashes
//! of the budgeted value (per process), so the table holds no address or email. When the table
//! is full the oldest window is evicted first: a flood of distinct keys can age out other keys'
//! history, which weakens a budget but never denies a legitimate caller.

use std::{collections::HashMap, net::IpAddr, sync::Mutex};

use sha2::{Digest, Sha256};

use crate::policy::{Rule, LIMITER_MAX_KEYS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Throttled {
    pub retry_after_seconds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key([u8; 32]);

/// Derives budget keys. One salt per process, from the OS.
pub struct KeyFactory {
    salt: [u8; 16],
}

impl KeyFactory {
    pub fn new() -> Result<Self, getrandom::Error> {
        let mut salt = [0; 16];
        getrandom::fill(&mut salt)?;
        Ok(Self { salt })
    }

    pub fn text(&self, namespace: &str, value: &str) -> Key {
        self.derive(namespace, value.as_bytes(), &[])
    }

    pub fn address(&self, namespace: &str, ip: IpAddr) -> Key {
        // IPv4-mapped IPv6 and plain IPv4 are one client; an IPv6 /64 is one client because a
        // single host routinely controls the whole prefix.
        let canonical: Vec<u8> = match ip {
            IpAddr::V4(v4) => v4.octets().to_vec(),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => v4.octets().to_vec(),
                None => v6.octets()[..8].to_vec(),
            },
        };
        self.derive(namespace, &canonical, &[])
    }

    pub fn pair(&self, namespace: &str, first: &str, ip: IpAddr) -> Key {
        let address = self.address(namespace, ip);
        self.derive(namespace, first.as_bytes(), &address.0)
    }

    fn derive(&self, namespace: &str, a: &[u8], b: &[u8]) -> Key {
        let mut hash = Sha256::new();
        hash.update(b"cantos-identity-limit-key-v1\0");
        hash.update(self.salt);
        hash.update((namespace.len() as u32).to_be_bytes());
        hash.update(namespace.as_bytes());
        hash.update((a.len() as u32).to_be_bytes());
        hash.update(a);
        hash.update(b);
        Key(hash.finalize().into())
    }
}

struct Window {
    start: u64,
    count: u32,
}

/// One rule over many keys.
pub struct Limiter {
    rule: Rule,
    max_keys: usize,
    windows: Mutex<HashMap<Key, Window>>,
}

impl Limiter {
    pub fn new(rule: Rule) -> Self {
        Self::with_capacity(rule, LIMITER_MAX_KEYS)
    }

    pub fn with_capacity(rule: Rule, max_keys: usize) -> Self {
        Self {
            rule,
            max_keys: max_keys.max(1),
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Counts an attempt and refuses it once the window's limit is reached.
    pub fn admit(&self, key: Key, now: u64) -> Result<(), Throttled> {
        let mut windows = self.windows.lock().unwrap_or_else(|p| p.into_inner());
        let window = self.window(&mut windows, key, now);
        if window.count >= self.rule.limit {
            return Err(Throttled {
                retry_after_seconds: window.start + self.rule.window_seconds - now,
            });
        }
        window.count += 1;
        Ok(())
    }

    /// Whether the key is already at its limit, without counting.
    pub fn exceeded(&self, key: Key, now: u64) -> Option<Throttled> {
        let windows = self.windows.lock().unwrap_or_else(|p| p.into_inner());
        let window = windows.get(&key)?;
        (now < window.start + self.rule.window_seconds && window.count >= self.rule.limit).then(
            || Throttled {
                retry_after_seconds: window.start + self.rule.window_seconds - now,
            },
        )
    }

    /// Counts one failure without refusing.
    pub fn record(&self, key: Key, now: u64) {
        let mut windows = self.windows.lock().unwrap_or_else(|p| p.into_inner());
        let window = self.window(&mut windows, key, now);
        window.count = window.count.saturating_add(1);
    }

    pub fn clear(&self, key: Key) {
        self.windows
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&key);
    }

    pub fn len(&self) -> usize {
        self.windows.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn window<'a>(
        &self,
        windows: &'a mut HashMap<Key, Window>,
        key: Key,
        now: u64,
    ) -> &'a mut Window {
        let window_seconds = self.rule.window_seconds;
        if windows
            .get(&key)
            .is_some_and(|w| now >= w.start + window_seconds)
        {
            windows.remove(&key);
        }
        if !windows.contains_key(&key) && windows.len() >= self.max_keys {
            windows.retain(|_, w| now < w.start + window_seconds);
            if windows.len() >= self.max_keys {
                if let Some(oldest) = windows.iter().min_by_key(|(_, w)| w.start).map(|(k, _)| *k) {
                    windows.remove(&oldest);
                }
            }
        }
        windows.entry(key).or_insert(Window {
            start: now,
            count: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE: Rule = Rule {
        limit: 3,
        window_seconds: 100,
    };

    fn keys() -> KeyFactory {
        KeyFactory::new().unwrap()
    }

    #[test]
    fn attempts_are_admitted_up_to_the_limit_then_refused_with_a_retry_time() {
        let limiter = Limiter::new(RULE);
        let key = keys().text("n", "a");
        for _ in 0..3 {
            assert!(limiter.admit(key, 1_000).is_ok());
        }
        assert_eq!(
            limiter.admit(key, 1_040),
            Err(Throttled {
                retry_after_seconds: 60
            })
        );
        // The window ends exactly `window_seconds` after its first event.
        assert!(limiter.admit(key, 1_099).is_err());
        assert!(limiter.admit(key, 1_100).is_ok());
    }

    #[test]
    fn keys_are_independent_and_namespaced() {
        let factory = keys();
        let limiter = Limiter::new(RULE);
        let (a, b) = (factory.text("n", "a"), factory.text("n", "b"));
        for _ in 0..3 {
            limiter.admit(a, 0).unwrap();
        }
        assert!(limiter.admit(a, 0).is_err());
        assert!(limiter.admit(b, 0).is_ok());
        assert_ne!(factory.text("one", "x"), factory.text("two", "x"));
        assert_ne!(
            keys().text("n", "a"),
            keys().text("n", "a"),
            "salt is per factory"
        );
    }

    #[test]
    fn failures_count_without_refusing_and_a_success_clears_only_its_key() {
        let factory = keys();
        let limiter = Limiter::new(RULE);
        let (a, b) = (factory.text("n", "a"), factory.text("n", "b"));
        assert_eq!(limiter.exceeded(a, 0), None);
        for _ in 0..3 {
            limiter.record(a, 0);
        }
        limiter.record(b, 0);
        assert!(limiter.exceeded(a, 10).is_some());
        assert_eq!(limiter.exceeded(b, 10), None);
        assert_eq!(limiter.exceeded(a, 100), None, "expired window");
        limiter.clear(a);
        assert_eq!(limiter.exceeded(a, 10), None);
    }

    #[test]
    fn the_table_is_bounded_and_evicts_the_oldest_window_first() {
        let factory = keys();
        let limiter = Limiter::with_capacity(RULE, 3);
        let all: Vec<Key> = (0..5).map(|n| factory.text("n", &n.to_string())).collect();
        for (second, key) in all.iter().enumerate() {
            limiter.record(*key, second as u64);
            assert!(limiter.len() <= 3);
        }
        assert_eq!(limiter.len(), 3);
        // Keys 0 and 1 were evicted; their history is gone, never a refusal of a newcomer.
        for key in &all[..2] {
            limiter.record(*key, 5);
        }
        assert_eq!(limiter.len(), 3);
        assert!(limiter.admit(factory.text("n", "fresh"), 5).is_ok());
    }

    #[test]
    fn expired_windows_are_reclaimed_before_anything_live_is_evicted() {
        let factory = keys();
        let limiter = Limiter::with_capacity(RULE, 2);
        let (old, live) = (factory.text("n", "old"), factory.text("n", "live"));
        limiter.record(old, 0);
        limiter.record(live, 90);
        limiter.record(factory.text("n", "new"), 150);
        assert_eq!(limiter.len(), 2);
        limiter.record(live, 150);
        assert_eq!(
            limiter.exceeded(live, 150),
            None,
            "live window survived: only the expired one was reclaimed"
        );
        limiter.record(live, 150);
        limiter.record(live, 150);
        assert!(limiter.exceeded(live, 150).is_some());
    }

    #[test]
    fn address_keys_merge_ipv4_mapped_ipv6_and_whole_ipv6_prefixes() {
        let factory = keys();
        let v4: IpAddr = "203.0.113.9".parse().unwrap();
        let mapped: IpAddr = "::ffff:203.0.113.9".parse().unwrap();
        assert_eq!(factory.address("n", v4), factory.address("n", mapped));
        let one: IpAddr = "2001:db8:1:2:aaaa::1".parse().unwrap();
        let two: IpAddr = "2001:db8:1:2:bbbb::2".parse().unwrap();
        let other: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(factory.address("n", one), factory.address("n", two));
        assert_ne!(factory.address("n", one), factory.address("n", other));
        assert_ne!(
            factory.pair("n", "a", v4),
            factory.pair("n", "a", "203.0.113.10".parse().unwrap())
        );
        assert_ne!(factory.pair("n", "a", v4), factory.pair("n", "b", v4));
    }
}
