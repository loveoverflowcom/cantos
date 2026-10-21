//! The durable session port behind [`super::IdentityAuth`]. Records, the closed error and the
//! secret digest live here so the PostgreSQL adapter and the uncomposed Turso candidate (#333)
//! feed the same production decisions; neither adapter owns a second session engine.
//!
//! No method performs network I/O. Identity is consulted by the caller before and after a call,
//! never while an adapter holds a lock or the writer.
use super::{
    credential::{Channel, Secrets, Surface},
    decision::{ObservedSecurity, RevokeReason, Subject},
};
use crate::platform::bearer::core::Lifetime;
use sha2::{Digest, Sha256};
use std::future::Future;

/// Closed storage outcome. Mapped to the generic wire failures at the HTTP boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    /// Unknown, revoked, spent, wrong-channel or inactive-actor credential.
    Inactive,
    Expired,
    /// A spent refresh secret was presented: the whole family is now revoked.
    Replay,
    Unavailable,
    /// The COMMIT acknowledgement was lost: the write may be durable. Never retried here; a
    /// native refresh in particular may already have spent its secret.
    Unknown,
}

/// A live session as stored. Secret material never appears here.
#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: String,
    pub actor_id: String,
    pub channel: Channel,
    pub observed: ObservedSecurity,
    pub authenticated_at: u64,
    pub absolute_expires: u64,
    pub idle_expires: u64,
    /// Expiry of the presented access/cookie secret.
    pub token_expires: u64,
}

#[derive(Debug)]
pub struct Issued {
    pub actor_id: String,
    pub session_id: String,
    pub lifetime: Lifetime,
}

#[derive(Debug)]
pub struct Rotated {
    pub actor_id: String,
    pub lifetime: Lifetime,
    pub authenticated_at: u64,
}

/// The only issuer whose subjects these sessions bind, as spelled in the Actor Registry.
pub const ISSUER: &str = "cantos-identity";

/// SHA-256 of a secret; both adapters store only this digest, so a stored row never yields it.
pub fn digest(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

pub(crate) fn channel_of(stored: &str) -> Option<Channel> {
    match stored {
        "theatre-web" => Some(Channel::Web(Surface::Theatre)),
        "studio-web" => Some(Channel::Web(Surface::Studio)),
        "theatre-native" => Some(Channel::Native),
        _ => None,
    }
}

/// Web: the cookie lives until the idle deadline (renewal extends it). Native: the access
/// secret lives for the access window and the refresh secret carries the idle deadline.
pub(crate) fn token_expiry(channel: Channel, lifetime: Lifetime) -> u64 {
    match channel {
        Channel::Web(_) => lifetime.idle,
        Channel::Native => lifetime.access,
    }
}

/// Durable session operations. Implementations keep each operation one atomic unit, store only
/// [`digest`]s, never move a binding, authentication time, observed security or absolute
/// deadline, never shorten an idle deadline and never resurrect a committed revocation.
pub trait SessionStore: Send + Sync {
    /// Idempotent exact-subject → ActorId binding (never by email); no session is created.
    fn bind(&self, subject: Subject) -> impl Future<Output = Result<String, SessionError>> + Send;
    /// Binds the subject and writes the session and its first secret in one unit of work.
    /// `observed` is what a fresh `GetSecurityState` confirmed.
    fn issue(
        &self,
        channel: Channel,
        observed: &ObservedSecurity,
        authenticated_at: u64,
        secrets: &Secrets,
    ) -> impl Future<Output = Result<Issued, SessionError>> + Send;
    /// The live session behind an access/cookie secret of exactly `channel`.
    fn access(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl Future<Output = Result<SessionRecord, SessionError>> + Send;
    /// The nonsecret session id behind a secret of exactly `channel`, even once it expired or
    /// its session was revoked. A fence input only; knowing it authenticates nobody.
    fn session_id_of(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl Future<Output = Result<Option<String>, SessionError>> + Send;
    /// Durable, irreversible, idempotent revocation after a non-admit security decision.
    fn revoke(
        &self,
        session_id: &str,
        reason: RevokeReason,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;
    /// Logout by the secret itself, even expired or spent; wrong-channel secrets select nothing.
    fn logout(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;
    /// Refresh cleanup by the (possibly spent) refresh secret and its session id.
    fn revoke_refresh(
        &self,
        refresh: &str,
        session_id: &str,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;
    /// First leg of a native renewal: contain a replay at once (no Identity needed) and return
    /// what the session observed, so the caller compares it with a fresh read outside any unit.
    fn peek_refresh(
        &self,
        refresh: &str,
        session_id: &str,
    ) -> impl Future<Output = Result<SessionRecord, SessionError>> + Send;
    /// Second leg: spend the refresh secret exactly once and issue the next pair atomically.
    fn rotate(
        &self,
        refresh: &str,
        session_id: &str,
        secrets: &Secrets,
    ) -> impl Future<Output = Result<Rotated, SessionError>> + Send;
    /// Extends the idle deadline of the same Web cookie secret, never past the absolute one.
    fn renew_web(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl Future<Output = Result<(SessionRecord, Lifetime), SessionError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_adapters_store_the_same_secret_digest() {
        for secret in ["", "ci1_t_0123", "ci1_r_ffff", "tiếng Việt"] {
            assert_eq!(
                digest(secret).to_vec(),
                crate::platform::postgres::token_hash(secret)
            );
        }
        assert_ne!(digest("a"), digest("b"));
    }

    #[test]
    fn stored_channels_round_trip_and_nothing_else_decodes() {
        for channel in [
            Channel::Web(Surface::Theatre),
            Channel::Web(Surface::Studio),
            Channel::Native,
        ] {
            assert_eq!(channel_of(channel.stored()), Some(channel));
        }
        for stored in ["", "studio-native", "Theatre-web", "theatre-web "] {
            assert_eq!(channel_of(stored), None);
        }
    }
}
