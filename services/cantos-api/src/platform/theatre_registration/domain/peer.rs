//! Which address owns an anonymous registration attempt's quota.
//!
//! Pure decision over plain facts: the socket peer and the raw `X-Forwarded-For` lines. The
//! header is believed only when the socket peer is an explicitly configured proxy, so a client
//! that reaches the API directly cannot choose its own quota identity, and users behind the
//! edge are not merged into the proxy's address.
use super::RegistrationError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// One configured proxy network. The prefix is validated; host bits are masked at parse time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cidr {
    network: IpAddr,
    prefix: u8,
}

impl Cidr {
    fn parse(value: &str) -> Option<Self> {
        let (address, prefix) = value.split_once('/')?;
        let address = normalize(address.parse().ok()?);
        let prefix: u8 = prefix.parse().ok()?;
        let limit = match address {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        // A /0 network would trust the whole Internet as a proxy.
        if prefix == 0 || prefix > limit || prefix.to_string() != value.split_once('/')?.1 {
            return None;
        }
        Some(Self {
            network: mask(address, prefix),
            prefix,
        })
    }

    fn contains(&self, candidate: IpAddr) -> bool {
        let candidate = normalize(candidate);
        std::mem::discriminant(&candidate) == std::mem::discriminant(&self.network)
            && mask(candidate, self.prefix) == self.network
    }
}

fn normalize(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or(IpAddr::V6(ip), IpAddr::V4),
        other => other,
    }
}

fn mask(address: IpAddr, prefix: u8) -> IpAddr {
    match address {
        IpAddr::V4(ip) => {
            let bits = u32::from(ip) & u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            IpAddr::V4(Ipv4Addr::from(bits))
        }
        IpAddr::V6(ip) => {
            let bits = u128::from(ip) & u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            IpAddr::V6(Ipv6Addr::from(bits))
        }
    }
}

/// A non-empty set of proxy networks the operator vouches for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedProxies(Vec<Cidr>);

impl TrustedProxies {
    /// Comma-separated `address/prefix` list. No hostnames, no `/0`, at most 16 entries.
    pub fn parse(value: &str) -> Result<Self, RegistrationError> {
        let networks = value
            .split(',')
            .map(|part| Cidr::parse(part.trim()))
            .collect::<Option<Vec<_>>>()
            .filter(|networks| (1..=16).contains(&networks.len()))
            .ok_or(RegistrationError::Configuration)?;
        Ok(Self(networks))
    }

    fn contains(&self, address: IpAddr) -> bool {
        self.0.iter().any(|network| network.contains(address))
    }
}

/// Selected by deployment configuration; there is no default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerPolicy {
    /// The socket peer owns the quota (no proxy in front of the API).
    DirectPeer,
    /// A configured edge proxy sets exactly the client address in `X-Forwarded-For`.
    TrustedProxy(TrustedProxies),
}

impl PeerPolicy {
    /// The address that owns the attempt. `forwarded` holds every `X-Forwarded-For` header line.
    ///
    /// A peer outside the trusted proxies is itself the client and its headers are ignored.
    /// A trusted proxy without a usable header fails closed instead of pooling every user under
    /// the proxy address. Entries are read right to left and trusted hops are skipped, so only an
    /// address a trusted hop appended can be selected, never one the client wrote on the left.
    pub fn client_ip(&self, peer: IpAddr, forwarded: &[&str]) -> Result<IpAddr, RegistrationError> {
        let Self::TrustedProxy(proxies) = self else {
            return Ok(peer);
        };
        if !proxies.contains(peer) {
            return Ok(peer);
        }
        let [line] = forwarded else {
            return Err(RegistrationError::Unavailable);
        };
        for entry in line.rsplit(',') {
            let address: IpAddr = entry
                .trim()
                .parse()
                .map_err(|_| RegistrationError::Unavailable)?;
            if !proxies.contains(address) {
                return Ok(address);
            }
        }
        Err(RegistrationError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PeerPolicy {
        PeerPolicy::TrustedProxy(TrustedProxies::parse("10.89.0.0/24, fd00:1::/64").unwrap())
    }
    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn direct_peer_ignores_forwarded_headers() {
        assert_eq!(
            PeerPolicy::DirectPeer.client_ip(ip("192.0.2.9"), &["198.51.100.1"]),
            Ok(ip("192.0.2.9"))
        );
    }

    #[test]
    fn untrusted_peer_cannot_spoof_a_quota_identity() {
        assert_eq!(
            policy().client_ip(ip("192.0.2.9"), &["198.51.100.1"]),
            Ok(ip("192.0.2.9"))
        );
        assert_eq!(
            policy().client_ip(ip("10.89.1.5"), &["198.51.100.1"]),
            Ok(ip("10.89.1.5")),
            "a neighbouring network is not the configured proxy"
        );
    }

    #[test]
    fn trusted_proxy_yields_each_distinct_client() {
        let policy = policy();
        let one = policy
            .client_ip(ip("10.89.0.4"), &["198.51.100.1"])
            .unwrap();
        let two = policy.client_ip(ip("10.89.0.4"), &["203.0.113.8"]).unwrap();
        assert_ne!(one, two);
        assert_eq!(one, ip("198.51.100.1"));
        assert_eq!(
            policy.client_ip(ip("::ffff:10.89.0.4"), &["2001:db8::7"]),
            Ok(ip("2001:db8::7"))
        );
        assert_eq!(
            policy.client_ip(ip("fd00:1::2"), &["198.51.100.1"]),
            Ok(ip("198.51.100.1"))
        );
    }

    #[test]
    fn client_written_left_entries_never_win() {
        // The client sent "1.2.3.4"; the edge appended the real peer on the right.
        assert_eq!(
            policy().client_ip(ip("10.89.0.4"), &["1.2.3.4, 198.51.100.1"]),
            Ok(ip("198.51.100.1"))
        );
        // A second trusted hop is skipped; the next untrusted address is the client.
        assert_eq!(
            policy().client_ip(ip("10.89.0.4"), &["198.51.100.1, 10.89.0.9"]),
            Ok(ip("198.51.100.1"))
        );
    }

    #[test]
    fn trusted_proxy_without_one_usable_header_fails_closed() {
        let policy = policy();
        for forwarded in [
            vec![],
            vec!["198.51.100.1", "203.0.113.8"],
            vec![""],
            vec!["not-an-address"],
            vec!["198.51.100.1, garbage"],
            vec!["10.89.0.9"],
            vec!["198.51.100.1:443"],
        ] {
            assert_eq!(
                policy.client_ip(ip("10.89.0.4"), &forwarded),
                Err(RegistrationError::Unavailable),
                "{forwarded:?}"
            );
        }
    }

    #[test]
    fn proxy_list_is_strict() {
        for invalid in [
            "",
            "10.0.0.1",
            "10.0.0.0/0",
            "10.0.0.0/33",
            "10.0.0.0/08",
            "::/0",
            "::/129",
            "proxy.local/24",
            "10.0.0.0/24,",
            "10.0.0.0/24,bad",
        ] {
            assert_eq!(
                TrustedProxies::parse(invalid),
                Err(RegistrationError::Configuration),
                "{invalid:?}"
            );
        }
        let too_many = (0..17)
            .map(|n| format!("10.{n}.0.0/16"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(TrustedProxies::parse(&too_many).is_err());
        let host_bits = TrustedProxies::parse("10.89.0.77/24").unwrap();
        assert!(host_bits.contains(ip("10.89.0.1")));
        assert!(!host_bits.contains(ip("10.89.1.1")));
    }
}
