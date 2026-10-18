use crate::{Failure, Surface};
use cantos_contracts::bearer::{TokenPair, TokenType};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Grant {
    pub(crate) access: String,
    pub(crate) refresh: String,
    pub actor: String,
    pub session: String,
    pub expires: u64,
    pub refresh_expires: u64,
    pub absolute: u64,
    pub authenticated_at: Option<u64>,
}
impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Grant([redacted])")
    }
}
fn secret(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub fn entropy(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
}
fn canonical_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
impl Grant {
    pub fn admit(surface: Surface, pair: TokenPair, now: u64) -> Result<Self, Failure> {
        let prefix = match surface {
            Surface::Studio => "ca1_s_",
            Surface::Theatre => "ca1_t_",
        };
        if pair.token_type != TokenType::Bearer
            || !secret(&pair.access_token, prefix)
            || !secret(&pair.refresh_token, "cr1_")
            || pair.actor_id.is_empty()
            || pair.actor_id.len() > 128
            || pair.actor_id.chars().any(char::is_control)
            || !canonical_id(&pair.session_id)
            || pair.expires_at <= now
            || pair.expires_at > pair.refresh_expires_at
            || pair.refresh_expires_at > pair.absolute_expires_at
            || pair.expires_at > now.saturating_add(120)
            || pair.authenticated_at.is_some_and(|time| time > now)
        {
            return Err(Failure::Protocol);
        }
        Ok(Self {
            access: pair.access_token,
            refresh: pair.refresh_token,
            actor: pair.actor_id,
            session: pair.session_id,
            expires: pair.expires_at,
            refresh_expires: pair.refresh_expires_at,
            absolute: pair.absolute_expires_at,
            authenticated_at: pair.authenticated_at,
        })
    }
    pub fn rotation_of(&self, old: &Self) -> bool {
        self.actor == old.actor
            && self.session == old.session
            && self.absolute == old.absolute
            && self.authenticated_at == old.authenticated_at
            && self.access != old.access
            && self.refresh != old.refresh
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub(crate) intent: u64,
    pub(crate) rotation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessAction {
    Use,
    Refresh,
    JoinRefresh,
}
#[derive(Default)]
pub struct Lifecycle {
    intent: u64,
    rotation: u64,
    grant: Option<Grant>,
}
impl Lifecycle {
    pub fn access_action(
        &self,
        expected: Option<&str>,
        now: u64,
        cleanup: bool,
        authorizing: bool,
        refreshing: bool,
    ) -> Result<AccessAction, Failure> {
        if cleanup {
            return Err(Failure::CleanupPending);
        }
        if authorizing {
            return Err(Failure::Changed);
        }
        let grant = self.grant.as_ref().ok_or(Failure::SignedOut)?;
        if expected.is_some_and(|expected| expected != grant.session) {
            return Err(Failure::Changed);
        }
        if grant.refresh_expires <= now || grant.absolute <= now {
            return Err(Failure::SignedOut);
        }
        Ok(if refreshing {
            AccessAction::JoinRefresh
        } else if grant.expires <= now.saturating_add(20) {
            AccessAction::Refresh
        } else {
            AccessAction::Use
        })
    }
    pub fn permits_logout(&self, expected: Option<&str>, authorizing: bool) -> bool {
        !authorizing
            && self
                .grant
                .as_ref()
                .is_none_or(|grant| Some(grant.session.as_str()) == expected)
    }
    pub fn ticket(&self) -> Ticket {
        Ticket {
            intent: self.intent,
            rotation: self.rotation,
        }
    }
    pub fn accepts(&self, issued: Ticket) -> bool {
        self.ticket() == issued
    }
    pub fn grant(&self) -> Option<&Grant> {
        self.grant.as_ref()
    }
    /// Invalidate before asynchronous cleanup. A late success OR denial must not change the new intent.
    pub fn invalidate(&mut self) -> Result<Option<Grant>, Failure> {
        self.intent = self.intent.checked_add(1).ok_or(Failure::Changed)?;
        self.rotation = 0;
        Ok(self.grant.take())
    }
    pub fn adopt(
        &mut self,
        issued: Ticket,
        grant: Grant,
        expected_actor: Option<&str>,
    ) -> Result<(), Failure> {
        if !self.accepts(issued) {
            return Err(Failure::Changed);
        }
        if self.grant.is_some() {
            return Err(Failure::Changed);
        }
        if expected_actor.is_some_and(|actor| actor != grant.actor) {
            return Err(Failure::AccountMismatch);
        }
        self.grant = Some(grant);
        Ok(())
    }
    pub fn rotate(&mut self, issued: Ticket, grant: Grant) -> Result<(), Failure> {
        if !self.accepts(issued) {
            return Err(Failure::Changed);
        }
        if !self
            .grant
            .as_ref()
            .is_some_and(|old| grant.rotation_of(old))
        {
            return Err(Failure::Protocol);
        }
        self.rotation = self.rotation.checked_add(1).ok_or(Failure::Changed)?;
        self.grant = Some(grant);
        Ok(())
    }
}

/// Only short-lived authorization protocol artifacts. Never an access/refresh token or manuscript.
/// Stored only in the reserved popup's tab, removed before emitting its result.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingFlow {
    pub flow: String,
    pub state: String,
    pub verifier: String,
    pub created: u64,
    pub origin: String,
    pub surface: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub flow_id: String,
    pub state: String,
    pub code: Option<String>,
    pub error: Option<String>,
}
impl PendingFlow {
    pub fn complete(
        &self,
        query: &str,
        origin: &str,
        surface: &str,
        now: u64,
    ) -> Result<Completion, Failure> {
        if origin != self.origin
            || surface != self.surface
            || now < self.created
            || now.saturating_sub(self.created) >= 300
            || !canonical_id(&self.flow)
            || !entropy(&self.state)
            || !entropy(&self.verifier)
        {
            return Err(Failure::Protocol);
        }
        let mut entries = std::collections::BTreeMap::new();
        for (key, value) in url::form_urlencoded::parse(query.trim_start_matches('?').as_bytes()) {
            if !matches!(key.as_ref(), "flow_id" | "state" | "code" | "error")
                || entries
                    .insert(key.into_owned(), value.into_owned())
                    .is_some()
            {
                return Err(Failure::Protocol);
            }
        }
        if entries.get("flow_id") != Some(&self.flow)
            || entries.get("state") != Some(&self.state)
            || entries.contains_key("code") == entries.contains_key("error")
        {
            return Err(Failure::Protocol);
        }
        let code = entries.remove("code");
        let error = entries.remove("error");
        if code.as_ref().is_some_and(|code| !entropy(code))
            || error
                .as_deref()
                .is_some_and(|error| error != "authorization_failed")
        {
            return Err(Failure::Protocol);
        }
        Ok(Completion {
            flow_id: self.flow.clone(),
            state: self.state.clone(),
            code,
            error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn grant(actor: &str, secret: &str) -> Grant {
        Grant {
            access: format!("ca1_t_{secret}"),
            refresh: format!("cr1_{secret}"),
            actor: actor.into(),
            session: "00000000-0000-0000-0000-000000000001".into(),
            expires: 200,
            refresh_expires: 500,
            absolute: 900,
            authenticated_at: Some(90),
        }
    }
    #[test]
    fn logout_and_account_switch_fence_both_late_success_and_late_denial() {
        let mut state = Lifecycle::default();
        let a = state.ticket();
        state.adopt(a, grant("a", "a"), None).unwrap();
        state.invalidate().unwrap();
        let b = state.ticket();
        state.adopt(b, grant("b", "b"), None).unwrap();
        assert_eq!(state.adopt(a, grant("a", "c"), None), Err(Failure::Changed));
        assert!(!state.accepts(a));
        assert!(state.accepts(b));
        assert_eq!(state.grant().unwrap().actor, "b");
    }
    #[test]
    fn reauthentication_cannot_rebind_retained_account_a_drafts_to_b() {
        let mut state = Lifecycle::default();
        assert_eq!(
            state.adopt(state.ticket(), grant("b", "b"), Some("a")),
            Err(Failure::AccountMismatch)
        );
        assert!(state.grant().is_none());
    }
    #[test]
    fn rotation_preserves_original_proof_and_family_and_fences_old_access_results() {
        let mut state = Lifecycle::default();
        let issued = state.ticket();
        state.adopt(issued, grant("a", "a"), None).unwrap();
        let mut wrong = grant("a", "b");
        wrong.authenticated_at = Some(100);
        assert_eq!(state.rotate(issued, wrong), Err(Failure::Protocol));
        state.rotate(issued, grant("a", "b")).unwrap();
        assert!(!state.accepts(issued));
        assert_eq!(state.grant().unwrap().authenticated_at, Some(90));
    }
    #[test]
    fn pending_cleanup_and_refresh_flights_are_admission_barriers_with_exact_expiry_edges() {
        let mut state = Lifecycle::default();
        state.adopt(state.ticket(), grant("a", "a"), None).unwrap();
        let session = state.grant().unwrap().session.clone();
        assert_eq!(
            state.access_action(Some(&session), 179, false, false, false),
            Ok(AccessAction::Use)
        );
        assert_eq!(
            state.access_action(Some(&session), 180, false, false, false),
            Ok(AccessAction::Refresh)
        );
        assert_eq!(
            state.access_action(Some(&session), 180, false, false, true),
            Ok(AccessAction::JoinRefresh)
        );
        assert_eq!(
            state.access_action(Some(&session), 499, false, false, false),
            Ok(AccessAction::Refresh)
        );
        assert_eq!(
            state.access_action(Some(&session), 500, false, false, false),
            Err(Failure::SignedOut)
        );
        assert_eq!(
            state.access_action(Some("session-b"), 100, false, false, false),
            Err(Failure::Changed)
        );
        for authorizing in [false, true] {
            for refreshing in [false, true] {
                assert_eq!(
                    state.access_action(Some(&session), 100, true, authorizing, refreshing),
                    Err(Failure::CleanupPending)
                );
            }
        }
        assert_eq!(
            state.access_action(Some(&session), 100, false, true, false),
            Err(Failure::Changed)
        );
    }
    #[test]
    fn stale_logout_cannot_cancel_a_new_authorization_or_revoke_account_b() {
        let mut state = Lifecycle::default();
        state.adopt(state.ticket(), grant("b", "b"), None).unwrap();
        let session = state.grant().unwrap().session.clone();
        assert!(!state.permits_logout(None, false));
        assert!(!state.permits_logout(Some("a"), false));
        assert!(state.permits_logout(Some(&session), false));
        assert!(!state.permits_logout(Some(&session), true));
        state.invalidate().unwrap();
        assert!(!state.permits_logout(None, true));
        assert!(state.permits_logout(None, false));
    }
    #[test]
    fn unknown_refresh_discards_admission_and_never_promotes_a_late_rotated_pair() {
        let mut state = Lifecycle::default();
        let issued = state.ticket();
        state.adopt(issued, grant("a", "a"), None).unwrap();
        let discarded = state.invalidate().unwrap().unwrap();
        assert_eq!(discarded.session, "00000000-0000-0000-0000-000000000001");
        assert_eq!(
            state.access_action(None, 100, false, false, false),
            Err(Failure::SignedOut)
        );
        assert_eq!(state.rotate(issued, grant("a", "b")), Err(Failure::Changed));
        assert!(state.grant().is_none());
    }
    #[test]
    fn callback_requires_exact_live_consume_once_protocol_binding_never_loose_query_parsing() {
        let flow = PendingFlow {
            flow: "00000000-0000-0000-0000-000000000001".into(),
            state: "s".repeat(43),
            verifier: "v".repeat(43),
            created: 100,
            origin: "https://web.example".into(),
            surface: "theatre".into(),
        };
        let query = format!(
            "?flow_id={}&state={}&code={}",
            flow.flow,
            flow.state,
            "c".repeat(43)
        );
        assert!(flow.complete(&query, &flow.origin, "theatre", 399).is_ok());
        for (q, o, s, t) in [
            (query.clone(), flow.origin.as_str(), "theatre", 400),
            (query.clone(), "https://evil.example", "theatre", 101),
            (query.clone(), flow.origin.as_str(), "studio", 101),
            (
                format!("{query}&state={}", flow.state),
                flow.origin.as_str(),
                "theatre",
                101,
            ),
            (
                format!("{query}&error=authorization_failed"),
                flow.origin.as_str(),
                "theatre",
                101,
            ),
            (
                format!("{query}&access_token=x"),
                flow.origin.as_str(),
                "theatre",
                101,
            ),
        ] {
            assert!(matches!(flow.complete(&q, o, s, t), Err(Failure::Protocol)));
        }
        let record = serde_json::to_string(&flow).unwrap();
        assert!(!record.contains("refresh_token") && !record.contains("access_token"));
    }
    #[test]
    fn fixture_and_surface_lifetime_checks_reject_foreign_purpose_and_inconsistent_expiry() {
        let body = include_str!("../../contracts/fixtures/bearer/v1/token-pair.json");
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let now = value["expires_at"].as_u64().unwrap() - 100;
        let prefix = value["access_token"].as_str().unwrap();
        let surface = if prefix.starts_with("ca1_s_") {
            Surface::Studio
        } else {
            Surface::Theatre
        };
        assert!(Grant::admit(surface, serde_json::from_value(value.clone()).unwrap(), now).is_ok());
        let other = if surface == Surface::Studio {
            Surface::Theatre
        } else {
            Surface::Studio
        };
        assert_eq!(
            Grant::admit(other, serde_json::from_value(value.clone()).unwrap(), now).unwrap_err(),
            Failure::Protocol
        );
        let mut bad = value;
        bad["refresh_expires_at"] = serde_json::json!(now);
        assert_eq!(
            Grant::admit(surface, serde_json::from_value(bad).unwrap(), now).unwrap_err(),
            Failure::Protocol
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadTransport {
    FetchOmit,
    CrossOriginXhr,
}
/// XHR cannot omit ambient cookies for same-origin URLs, even with withCredentials=false.
pub fn upload_transport(target: &str, page_origin: &str) -> Result<UploadTransport, Failure> {
    let page = url::Url::parse(page_origin).map_err(|_| Failure::Protocol)?;
    if page.origin().ascii_serialization() != page_origin {
        return Err(Failure::Protocol);
    }
    let target = page.join(target).map_err(|_| Failure::Protocol)?;
    if !matches!(target.scheme(), "http" | "https")
        || !target.username().is_empty()
        || target.password().is_some()
    {
        return Err(Failure::Protocol);
    }
    Ok(if target.origin() == page.origin() {
        UploadTransport::FetchOmit
    } else {
        UploadTransport::CrossOriginXhr
    })
}
#[cfg(test)]
mod upload_transport_tests {
    use super::*;
    #[test]
    fn same_origin_xhr_cookie_loophole_is_closed_without_losing_cross_origin_progress() {
        for target in [
            "/api/v1/studio/uploads/id",
            "https://studio.example/api/v1/studio/uploads/id",
        ] {
            assert_eq!(
                upload_transport(target, "https://studio.example"),
                Ok(UploadTransport::FetchOmit)
            );
        }
        for target in [
            "https://api.example/api/v1/studio/uploads/id",
            "https://studio.example:444/api/v1/studio/uploads/id",
            "http://127.0.0.1:8080/api/v1/studio/uploads/id",
        ] {
            assert_eq!(
                upload_transport(target, "https://studio.example"),
                Ok(UploadTransport::CrossOriginXhr)
            );
        }
        assert_eq!(
            upload_transport("javascript:alert(1)", "https://studio.example"),
            Err(Failure::Protocol)
        );
    }
}
