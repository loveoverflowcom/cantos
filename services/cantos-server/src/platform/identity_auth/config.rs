//! Deployment inputs for the Identity-backed password sessions. Every value is explicit: there
//! is no default socket, peer uid or proxy policy, and an orphan input never half-enables a
//! feature.
use super::decision::Subject;
use super::peer::{PeerPolicy, TrustedProxies};
use std::path::PathBuf;

pub const SOCKET: &str = "CANTOS_IDENTITY_SOCKET";
pub const PEER_UID: &str = "CANTOS_IDENTITY_PEER_UID";
pub const PEER_POLICY: &str = "CANTOS_IDENTITY_PEER_POLICY";
pub const TRUSTED_PROXIES: &str = "CANTOS_IDENTITY_TRUSTED_PROXIES";
pub const STUDIO_SUBJECTS: &str = "CANTOS_IDENTITY_STUDIO_SUBJECTS";
/// A Studio roster is a deliberate, short operator list, never a directory.
pub const MAX_STUDIO_SUBJECTS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError;

#[derive(Clone, Debug)]
pub struct IdentityAuthConfig {
    pub socket: PathBuf,
    pub peer_uid: u32,
    pub peer_policy: PeerPolicy,
    /// Exact Identity subjects permitted to enter Studio. Empty: Studio login is unavailable.
    pub studio_subjects: Vec<Subject>,
}

/// `Ok(None)` when no Identity input is present. A partial set is a configuration error.
pub fn from_lookup(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<IdentityAuthConfig>, ConfigError> {
    let Some(socket) = lookup(SOCKET) else {
        return if [PEER_UID, PEER_POLICY, TRUSTED_PROXIES, STUDIO_SUBJECTS]
            .iter()
            .any(|name| lookup(name).is_some())
        {
            Err(ConfigError)
        } else {
            Ok(None)
        };
    };
    let socket = PathBuf::from(socket);
    if !socket.is_absolute() || socket.as_os_str().len() > 100 {
        return Err(ConfigError);
    }
    let peer_uid = lookup(PEER_UID)
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or(ConfigError)?;
    let peer_policy = match (lookup(PEER_POLICY).as_deref(), lookup(TRUSTED_PROXIES)) {
        (Some("direct_peer"), None) => PeerPolicy::DirectPeer,
        (Some("trusted_proxy"), Some(list)) => {
            PeerPolicy::TrustedProxy(TrustedProxies::parse(&list).map_err(|_| ConfigError)?)
        }
        _ => return Err(ConfigError),
    };
    let studio_subjects = match lookup(STUDIO_SUBJECTS) {
        None => Vec::new(),
        Some(raw) => {
            let values: Vec<String> = serde_json::from_str(&raw).map_err(|_| ConfigError)?;
            let mut subjects = values
                .iter()
                .map(|value| Subject::from_hex(value).ok_or(ConfigError))
                .collect::<Result<Vec<_>, _>>()?;
            let count = subjects.len();
            subjects.sort_by_key(|subject| subject.bytes());
            subjects.dedup();
            if count == 0 || subjects.len() != count || count > MAX_STUDIO_SUBJECTS {
                return Err(ConfigError);
            }
            subjects
        }
    };
    Ok(Some(IdentityAuthConfig {
        socket,
        peer_uid,
        peer_policy,
        studio_subjects,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    const BASE: [(&str, &str); 3] = [
        (SOCKET, "/run/cantos/identity.sock"),
        (PEER_UID, "1001"),
        (PEER_POLICY, "direct_peer"),
    ];

    #[test]
    fn absence_is_disabled_and_every_partial_set_is_refused() {
        assert!(from_lookup(lookup(&[])).unwrap().is_none());
        for orphan in [
            (PEER_UID, "1"),
            (PEER_POLICY, "direct_peer"),
            (TRUSTED_PROXIES, "10.0.0.0/8"),
            (STUDIO_SUBJECTS, "[]"),
        ] {
            assert!(from_lookup(lookup(&[orphan])).is_err(), "{orphan:?}");
        }
        for missing in [PEER_UID, PEER_POLICY] {
            let pairs: Vec<_> = BASE
                .iter()
                .copied()
                .filter(|(k, _)| *k != missing)
                .collect();
            assert!(from_lookup(lookup(&pairs)).is_err(), "{missing}");
        }
    }

    #[test]
    fn socket_uid_and_policy_are_strict() {
        let ok = from_lookup(lookup(&BASE)).unwrap().unwrap();
        assert_eq!((ok.peer_uid, ok.studio_subjects.len()), (1001, 0));
        for (name, value) in [
            (SOCKET, "relative.sock"),
            (SOCKET, ""),
            (PEER_UID, "-1"),
            (PEER_UID, ""),
            (PEER_UID, "4294967296"),
            (PEER_POLICY, "none"),
            (PEER_POLICY, "trusted_proxy"),
        ] {
            let pairs: Vec<_> = BASE
                .iter()
                .map(|&(k, v)| if k == name { (k, value) } else { (k, v) })
                .collect();
            assert!(from_lookup(lookup(&pairs)).is_err(), "{name}={value}");
        }
        let mut proxied: Vec<_> = BASE
            .iter()
            .map(|&(k, v)| {
                if k == PEER_POLICY {
                    (k, "trusted_proxy")
                } else {
                    (k, v)
                }
            })
            .collect();
        assert!(from_lookup(lookup(&proxied)).is_err());
        proxied.push((TRUSTED_PROXIES, "0.0.0.0/0"));
        assert!(from_lookup(lookup(&proxied)).is_err());
        proxied.pop();
        proxied.push((TRUSTED_PROXIES, "10.89.0.0/24"));
        assert!(matches!(
            from_lookup(lookup(&proxied)).unwrap().unwrap().peer_policy,
            PeerPolicy::TrustedProxy(_)
        ));
        let mut direct_with_list = BASE.to_vec();
        direct_with_list.push((TRUSTED_PROXIES, "10.89.0.0/24"));
        assert!(from_lookup(lookup(&direct_with_list)).is_err());
    }

    #[test]
    fn the_studio_roster_is_exact_distinct_bounded_hex_subjects() {
        let a = "aa".repeat(16);
        let b = "bb".repeat(16);
        let with = |roster: &str| {
            let mut pairs = BASE.to_vec();
            pairs.push((STUDIO_SUBJECTS, roster));
            from_lookup(lookup(&pairs))
        };
        assert_eq!(
            with(&format!("[\"{a}\",\"{b}\"]"))
                .unwrap()
                .unwrap()
                .studio_subjects
                .len(),
            2
        );
        for bad in [
            "[]".to_owned(),
            "not json".to_owned(),
            format!("[\"{a}\",\"{a}\"]"),
            format!("[\"{}\"]", "AA".repeat(16)),
            format!("[\"{}\"]", "00".repeat(16)),
            format!("[\"{}\"]", "aa".repeat(15)),
            format!(
                "[{}]",
                (0..9)
                    .map(|n| format!("\"{:02x}{}\"", n + 1, "00".repeat(15)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            "[1]".to_owned(),
        ] {
            assert!(with(&bad).is_err(), "{bad}");
        }
    }
}
