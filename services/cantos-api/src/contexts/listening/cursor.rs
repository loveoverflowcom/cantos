//! Opaque bounded account/route/limit/epoch-bound keyset vault. It grants no identity or access.
use super::{
    application::{LibraryError, PageKind},
    domain::ListenerId,
};
use std::{collections::BTreeMap, sync::Mutex};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PageAnchor {
    pub upper_order: u64,
    pub after_order: u64,
    pub after_work: String,
    pub history_epoch: u64,
}
struct Entry {
    listener: ListenerId,
    kind: PageKind,
    limit: u16,
    anchor: PageAnchor,
    expires: i64,
}
#[derive(Default)]
pub(super) struct CursorVault(Mutex<BTreeMap<String, Entry>>);
impl CursorVault {
    pub fn issue(
        &self,
        listener: &ListenerId,
        kind: PageKind,
        limit: u16,
        anchor: PageAnchor,
        now: i64,
    ) -> Result<String, LibraryError> {
        let expires = now.checked_add(900_000).ok_or(LibraryError::Unavailable)?;
        let mut entries = self.0.lock().map_err(|_| LibraryError::Corrupt)?;
        entries.retain(|_, v| v.expires > now);
        if entries.len() >= 4096 {
            return Err(LibraryError::Unavailable);
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| LibraryError::Unavailable)?;
        let key = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        if entries.contains_key(&key) {
            return Err(LibraryError::Unavailable);
        }
        entries.insert(
            key.clone(),
            Entry {
                listener: listener.clone(),
                kind,
                limit,
                anchor,
                expires,
            },
        );
        Ok(key)
    }
    pub fn read(
        &self,
        key: &str,
        listener: &ListenerId,
        kind: PageKind,
        limit: u16,
        now: i64,
    ) -> Result<PageAnchor, LibraryError> {
        if key.len() != 64
            || !key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(LibraryError::InvalidCursor);
        }
        let entries = self.0.lock().map_err(|_| LibraryError::Corrupt)?;
        let e = entries.get(key).ok_or(LibraryError::InvalidCursor)?;
        if e.listener != *listener || e.kind != kind || e.limit != limit || now >= e.expires {
            return Err(LibraryError::InvalidCursor);
        }
        Ok(e.anchor.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contexts::listening::auth::AuthenticatedListener, platform::identity::domain::ActorId,
    };
    #[test]
    fn cursors_cannot_cross_accounts_routes_limits_expiry_or_restart() {
        let a = AuthenticatedListener::from_verified_actor(ActorId::new("alice").unwrap());
        let b = AuthenticatedListener::from_verified_actor(ActorId::new("bob").unwrap());
        let vault = CursorVault::default();
        let anchor = PageAnchor {
            upper_order: 10,
            after_order: 9,
            after_work: "00000000-0000-4000-8000-000000000001".into(),
            history_epoch: 0,
        };
        let key = vault
            .issue(a.listener(), PageKind::History, 20, anchor.clone(), 1000)
            .unwrap();
        assert_eq!(key.len(), 64);
        assert!(!key.contains("alice"));
        assert_eq!(
            vault.read(&key, a.listener(), PageKind::History, 20, 1001),
            Ok(anchor)
        );
        for (who, kind, limit, time) in [
            (b.listener(), PageKind::History, 20, 1001),
            (a.listener(), PageKind::Saved, 20, 1001),
            (a.listener(), PageKind::History, 21, 1001),
            (a.listener(), PageKind::History, 20, 901000),
        ] {
            assert_eq!(
                vault.read(&key, who, kind, limit, time),
                Err(LibraryError::InvalidCursor)
            );
        }
        assert_eq!(
            CursorVault::default().read(&key, a.listener(), PageKind::History, 20, 1001),
            Err(LibraryError::InvalidCursor)
        );
    }
}
