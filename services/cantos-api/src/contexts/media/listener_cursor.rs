//! Bounded process-local opaque cursor vault. Random references contain no query or private ID.
use super::domain::listening::DiscoveryQuery;
use super::listener_application::ListenerError;
use std::{collections::BTreeMap, sync::Mutex};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Keyset {
    Works {
        time: i64,
        id: String,
    },
    Units {
        work: String,
        position: u32,
        id: String,
    },
}
#[derive(Clone)]
struct Entry {
    query: DiscoveryQuery,
    key: Keyset,
    expires: i64,
}
#[derive(Default)]
pub(super) struct CursorVault(Mutex<BTreeMap<String, Entry>>);
impl CursorVault {
    pub fn read(
        &self,
        token: &str,
        query: &DiscoveryQuery,
        now: i64,
    ) -> Result<Keyset, ListenerError> {
        if token.len() != 68
            || !token.starts_with("lc1_")
            || !token[4..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let entries = self.0.lock().map_err(|_| ListenerError::Unavailable)?;
        let entry = entries.get(token).ok_or(ListenerError::InvalidCursor)?;
        if entry.query != *query || now < 0 || now >= entry.expires {
            return Err(ListenerError::InvalidCursor);
        }
        Ok(entry.key.clone())
    }
    pub fn issue(
        &self,
        query: &DiscoveryQuery,
        key: Keyset,
        now: i64,
    ) -> Result<String, ListenerError> {
        let expires = now.checked_add(900_000).ok_or(ListenerError::Unavailable)?;
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| ListenerError::Unavailable)?;
        let token = format!(
            "lc1_{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        let mut entries = self.0.lock().map_err(|_| ListenerError::Unavailable)?;
        entries.retain(|_, entry| entry.expires > now);
        if entries.len() >= 4096 || entries.contains_key(&token) {
            return Err(ListenerError::Unavailable);
        }
        entries.insert(
            token.clone(),
            Entry {
                query: query.clone(),
                key,
                expires,
            },
        );
        Ok(token)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_tokens_bind_query_expire_and_never_encode_internal_values() {
        let v = CursorVault::default();
        let q = DiscoveryQuery::new("riêng tư", None, None, 1).unwrap();
        let key = Keyset::Works {
            time: 5000,
            id: "hidden-internal-id".into(),
        };
        let token = v.issue(&q, key.clone(), 1000).unwrap();
        assert_eq!(token.len(), 68);
        assert!(!token.contains("hidden"));
        assert_eq!(v.read(&token, &q, 1000).unwrap(), key);
        assert_eq!(
            v.read(&token, &q, 901000),
            Err(ListenerError::InvalidCursor)
        );
        assert_eq!(
            v.read(
                &token,
                &DiscoveryQuery::new("", None, None, 1).unwrap(),
                1000
            ),
            Err(ListenerError::InvalidCursor)
        );
        let mut tampered = token;
        tampered.replace_range(10..11, "z");
        assert_eq!(
            v.read(&tampered, &q, 1000),
            Err(ListenerError::InvalidCursor)
        );
    }
}
