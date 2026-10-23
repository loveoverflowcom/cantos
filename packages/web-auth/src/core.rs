//! Pure Identity v1 Web session decisions. No credential exists here: the cookie is HttpOnly and
//! owned by the browser. This module admits nonsecret session facts, fences intent and decides
//! when to use, renew or re-read the session. Server lifetimes (idle/absolute) stay authoritative.
use crate::wire::Session;
use crate::{Failure, Surface};

/// Renew the cookie's idle deadline once this few seconds remain (contract: renew before idle).
pub const RENEW_MARGIN: u64 = 90;
const DEFAULT_RETRY_AFTER: u32 = 30;
const MAX_RETRY_AFTER: u32 = 3600;

fn canonical_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value && !id.is_nil())
}

/// Nonsecret facts of one admitted Web session. `session` is the fence value sent as
/// `x-cantos-{surface}-session`; it is never a credential.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionView {
    pub actor: String,
    pub session: String,
    /// Idle deadline of the cookie (UNIX seconds).
    pub idle: u64,
    /// Absolute deadline; cannot be extended.
    pub absolute: u64,
    pub authenticated_at: u64,
}
impl std::fmt::Debug for SessionView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionView([redacted])")
    }
}
impl SessionView {
    /// Structural admission only. Deadlines are never compared with the device clock here: a
    /// skewed clock must not turn a valid server session into a protocol failure.
    pub fn admit(wire: Session) -> Result<Self, Failure> {
        let Some(authenticated_at) = wire.authenticated_at else {
            // Identity v1 never treats a missing authentication time as recent authentication.
            return Err(Failure::Protocol);
        };
        if wire.actor_id.is_empty()
            || wire.actor_id.len() > 128
            || wire.actor_id.chars().any(char::is_control)
            || !canonical_id(&wire.session_id)
            || wire.expires_at == 0
            || wire.absolute_expires_at < wire.expires_at
            || authenticated_at == 0
            || authenticated_at >= wire.expires_at
        {
            return Err(Failure::Protocol);
        }
        Ok(Self {
            actor: wire.actor_id,
            session: wire.session_id,
            idle: wire.expires_at,
            absolute: wire.absolute_expires_at,
            authenticated_at,
        })
    }
    fn same_family(&self, other: &Self) -> bool {
        self.actor == other.actor
            && self.session == other.session
            && self.absolute == other.absolute
            && self.authenticated_at == other.authenticated_at
    }
    /// A renewal keeps the same cookie value, never shortens idle and never passes absolute.
    pub fn renewal_of(&self, old: &Self) -> bool {
        self.same_family(old) && self.idle >= old.idle && self.idle <= self.absolute
    }
}

/// Wall-clock seconds with a monotonic floor: server UNIX deadlines are kept, but the device
/// clock moving backwards can never extend the remaining time of a session.
#[derive(Default)]
pub struct Clock {
    started: bool,
    time: u64,
    elapsed: u64,
}
impl Clock {
    pub fn now(&mut self, wall: u64, elapsed: u64) -> u64 {
        if !self.started {
            self.started = true;
            self.time = wall;
            self.elapsed = elapsed;
            return wall;
        }
        let elapsed = elapsed.max(self.elapsed);
        let advanced = self.time.saturating_add(elapsed - self.elapsed);
        self.time = wall.max(advanced);
        self.elapsed = elapsed;
        self.time
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessAction {
    Use,
    Renew,
    JoinRenew,
    /// Local deadlines say the session may be over: ask the API instead of concluding.
    Reconcile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Adopted,
    Refreshed,
    /// A different actor/session replaced the local one; every old lease is fenced.
    Switched,
}

#[derive(Default)]
pub struct Lifecycle {
    intent: u64,
    session: Option<SessionView>,
}
impl Lifecycle {
    pub fn access_action(
        &self,
        expected: Option<&str>,
        now: u64,
        cleanup: bool,
        authorizing: bool,
        renewing: bool,
    ) -> Result<AccessAction, Failure> {
        if cleanup {
            return Err(Failure::CleanupPending);
        }
        if authorizing {
            return Err(Failure::Changed);
        }
        let session = self.session.as_ref().ok_or(Failure::SignedOut)?;
        if expected.is_some_and(|expected| expected != session.session) {
            return Err(Failure::Changed);
        }
        Ok(if renewing {
            AccessAction::JoinRenew
        } else if session.idle <= now || session.absolute <= now {
            AccessAction::Reconcile
        } else if session.idle < session.absolute && session.idle - now <= RENEW_MARGIN {
            AccessAction::Renew
        } else {
            AccessAction::Use
        })
    }
    pub fn permits_logout(&self, expected: Option<&str>, authorizing: bool) -> bool {
        !authorizing
            && self
                .session
                .as_ref()
                .is_none_or(|session| Some(session.session.as_str()) == expected)
    }
    pub fn ticket(&self) -> Ticket {
        Ticket(self.intent)
    }
    pub fn accepts(&self, issued: Ticket) -> bool {
        self.ticket() == issued
    }
    pub fn session(&self) -> Option<&SessionView> {
        self.session.as_ref()
    }
    /// Invalidate before asynchronous cleanup. A late success OR denial must not change the new intent.
    pub fn invalidate(&mut self) -> Result<Option<SessionView>, Failure> {
        self.intent = self.intent.checked_add(1).ok_or(Failure::Changed)?;
        Ok(self.session.take())
    }
    /// Adopt a session the API just confirmed (login or an authoritative read).
    pub fn observe(
        &mut self,
        issued: Ticket,
        view: SessionView,
        expected_actor: Option<&str>,
    ) -> Result<Observation, Failure> {
        if !self.accepts(issued) {
            return Err(Failure::Changed);
        }
        if expected_actor.is_some_and(|actor| actor != view.actor) {
            return Err(Failure::AccountMismatch);
        }
        match self.session.as_mut() {
            None => {
                self.session = Some(view);
                Ok(Observation::Adopted)
            }
            Some(old) if old.same_family(&view) => {
                old.idle = old.idle.max(view.idle);
                Ok(Observation::Refreshed)
            }
            Some(_) => {
                self.intent = self.intent.checked_add(1).ok_or(Failure::Changed)?;
                self.session = Some(view);
                Ok(Observation::Switched)
            }
        }
    }
    /// A renewal is the same cookie value with a later idle deadline; in-flight requests of the
    /// same session stay valid, so renewing never changes the ticket. The reply is validated
    /// against the snapshot the renewal was **dispatched** with (an authoritative read may have
    /// confirmed a later idle deadline meanwhile), then merged into the current family keeping
    /// the largest idle deadline the API has confirmed.
    pub fn renew(
        &mut self,
        issued: Ticket,
        dispatched: &SessionView,
        view: SessionView,
    ) -> Result<(), Failure> {
        if !self.accepts(issued) {
            return Err(Failure::Changed);
        }
        if !view.renewal_of(dispatched) {
            return Err(Failure::Protocol);
        }
        match self.session.as_mut() {
            Some(current) if current.same_family(&view) => {
                current.idle = current.idle.max(view.idle);
                Ok(())
            }
            _ => Err(Failure::Changed),
        }
    }
}

/// 429 gate: seconds before another request of this surface may leave the document. A later,
/// shorter answer never shortens a hold the API already asked for.
#[derive(Default)]
pub struct RateGate {
    until: u64,
}
impl RateGate {
    pub fn arm(&mut self, now: u64, seconds: u32) {
        self.until = self.until.max(now.saturating_add(u64::from(seconds)));
    }
    pub fn check(&self, now: u64) -> Result<(), Failure> {
        if self.until > now {
            return Err(Failure::RateLimited(
                (self.until - now).min(u64::from(u32::MAX)) as u32,
            ));
        }
        Ok(())
    }
}

/// Header carrying the session id the client believes it holds (a fence, never a credential).
pub const fn session_header(surface: Surface) -> &'static str {
    match surface {
        Surface::Studio => "x-cantos-studio-session",
        Surface::Theatre => "x-cantos-theatre-session",
    }
}

/// Bounded delta-seconds Retry-After; an absent or unreadable header (HTTP-date, or a
/// cross-origin header the API did not expose) gets a conservative default, never an instant retry.
pub fn retry_after(header: Option<&str>) -> u32 {
    header
        .and_then(|value| value.trim().parse::<u32>().ok())
        .map_or(DEFAULT_RETRY_AFTER, |seconds| {
            seconds.clamp(1, MAX_RETRY_AFTER)
        })
}

/// Status projection for session and protected calls.
pub fn classify(status: u16, retry_after_header: Option<&str>) -> Failure {
    match status {
        400 => Failure::Invalid,
        401 => Failure::SignedOut,
        403 => Failure::Forbidden,
        404 => Failure::NotFound,
        409 => Failure::Changed,
        429 => Failure::RateLimited(retry_after(retry_after_header)),
        _ => Failure::Unavailable,
    }
}
/// Login differs only where the API answers a wrong password: 401 is a credential failure, not
/// a lost session.
pub fn classify_login(status: u16, retry_after_header: Option<&str>) -> Failure {
    match status {
        401 => Failure::InvalidCredentials,
        409 => Failure::Protocol,
        other => classify(other, retry_after_header),
    }
}

/// A credentialed request or upload may only reach this surface of the configured API origin
/// (or, with no configured origin, the page's own origin). This is the only place an arbitrary
/// URL is accepted for an ambient cookie.
pub fn credentialed_target(
    target: &str,
    page_origin: &str,
    api_origin: Option<&str>,
    surface: Surface,
) -> Result<String, Failure> {
    let page = url::Url::parse(page_origin).map_err(|_| Failure::Protocol)?;
    if page.origin().ascii_serialization() != page_origin {
        return Err(Failure::Protocol);
    }
    let expected = match api_origin {
        Some(origin) => url::Url::parse(origin).map_err(|_| Failure::Protocol)?,
        None => page.clone(),
    };
    let resolved = page.join(target).map_err(|_| Failure::Protocol)?;
    let base = surface.base();
    if !matches!(resolved.scheme(), "http" | "https")
        || !resolved.username().is_empty()
        || resolved.password().is_some()
        || resolved.fragment().is_some()
        || resolved.origin() != expected.origin()
        || !resolved.path().starts_with(&format!("{base}/"))
        || resolved.path().contains('\\')
        || resolved.path().contains('%')
    {
        return Err(Failure::Protocol);
    }
    Ok(resolved.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(actor: &str, session: &str, idle: u64, absolute: u64, auth: Option<u64>) -> Session {
        Session {
            actor_id: actor.into(),
            session_id: session.into(),
            expires_at: idle,
            absolute_expires_at: absolute,
            authenticated_at: auth,
        }
    }
    const S1: &str = "00000000-0000-4000-8000-000000000001";
    const S2: &str = "00000000-0000-4000-8000-000000000002";
    fn view(actor: &str, session: &str, idle: u64) -> SessionView {
        SessionView::admit(wire(actor, session, idle, 900, Some(10))).unwrap()
    }

    #[test]
    fn admission_is_structural_and_requires_a_real_authentication_time() {
        assert!(SessionView::admit(wire("a", S1, 600, 900, Some(10))).is_ok());
        for bad in [
            wire("a", S1, 600, 900, None),
            wire("a", S1, 600, 900, Some(0)),
            wire("a", S1, 600, 900, Some(600)),
            wire("a", S1, 600, 500, Some(10)),
            wire("a", S1, 0, 900, Some(10)),
            wire("", S1, 600, 900, Some(10)),
            wire("a\n", S1, 600, 900, Some(10)),
            wire(&"a".repeat(129), S1, 600, 900, Some(10)),
            wire("a", "not-a-uuid", 600, 900, Some(10)),
            wire(
                "a",
                "00000000-0000-0000-0000-000000000000",
                600,
                900,
                Some(10),
            ),
            wire(
                "a",
                "00000000-0000-4000-8000-00000000000A",
                600,
                900,
                Some(10),
            ),
        ] {
            assert_eq!(SessionView::admit(bad).unwrap_err(), Failure::Protocol);
        }
        assert_eq!(
            format!("{:?}", view("a", S1, 600)),
            "SessionView([redacted])"
        );
    }

    #[test]
    fn clock_skew_never_rejects_a_server_session_and_never_runs_backwards() {
        // A device clock one hour ahead still admits; the decision to re-read is separate.
        assert!(SessionView::admit(wire("a", S1, 100, 400, Some(10))).is_ok());
        let mut clock = Clock::default();
        assert_eq!(clock.now(1_000, 0), 1_000);
        assert_eq!(clock.now(1_010, 10), 1_010);
        // The wall clock jumps back 500 s; the monotonic floor keeps time moving forward.
        assert_eq!(clock.now(510, 20), 1_020);
        assert_eq!(clock.now(510, 20), 1_020);
        // A forward jump is followed; a regressing elapsed value is ignored.
        assert_eq!(clock.now(2_000, 25), 2_000);
        assert_eq!(clock.now(1_500, 5), 2_000);
    }

    #[test]
    fn use_renew_reconcile_and_barrier_decisions_have_exact_edges() {
        let mut state = Lifecycle::default();
        let t = state.ticket();
        state.observe(t, view("a", S1, 600), None).unwrap();
        let action = |now, cleanup, authorizing, renewing| {
            state.access_action(Some(S1), now, cleanup, authorizing, renewing)
        };
        assert_eq!(action(509, false, false, false), Ok(AccessAction::Use));
        assert_eq!(action(510, false, false, false), Ok(AccessAction::Renew));
        assert_eq!(action(599, false, false, false), Ok(AccessAction::Renew));
        assert_eq!(action(510, false, false, true), Ok(AccessAction::JoinRenew));
        // Idle or absolute passed locally: ask the API, never conclude signed-out from a clock.
        assert_eq!(
            action(600, false, false, false),
            Ok(AccessAction::Reconcile)
        );
        assert_eq!(
            action(900, false, false, false),
            Ok(AccessAction::Reconcile)
        );
        for authorizing in [false, true] {
            for renewing in [false, true] {
                assert_eq!(
                    action(100, true, authorizing, renewing),
                    Err(Failure::CleanupPending)
                );
            }
        }
        assert_eq!(action(100, false, true, false), Err(Failure::Changed));
        assert_eq!(
            state.access_action(Some(S2), 100, false, false, false),
            Err(Failure::Changed)
        );
        assert_eq!(
            Lifecycle::default().access_action(None, 100, false, false, false),
            Err(Failure::SignedOut)
        );
    }

    #[test]
    fn a_session_capped_at_its_absolute_deadline_is_not_renewed() {
        let mut state = Lifecycle::default();
        let t = state.ticket();
        let capped = SessionView::admit(wire("a", S1, 900, 900, Some(10))).unwrap();
        state.observe(t, capped, None).unwrap();
        assert_eq!(
            state.access_action(None, 850, false, false, false),
            Ok(AccessAction::Use)
        );
        assert_eq!(
            state.access_action(None, 900, false, false, false),
            Ok(AccessAction::Reconcile)
        );
    }

    #[test]
    fn logout_and_account_switch_fence_both_late_success_and_late_denial() {
        let mut state = Lifecycle::default();
        let a = state.ticket();
        state.observe(a, view("a", S1, 600), None).unwrap();
        state.invalidate().unwrap();
        let b = state.ticket();
        state.observe(b, view("b", S2, 600), None).unwrap();
        assert_eq!(
            state.observe(a, view("a", S1, 600), None),
            Err(Failure::Changed)
        );
        assert!(!state.accepts(a));
        assert!(state.accepts(b));
        assert_eq!(state.session().unwrap().actor, "b");
    }

    #[test]
    fn reauthentication_cannot_rebind_retained_account_a_drafts_to_b() {
        let mut state = Lifecycle::default();
        assert_eq!(
            state.observe(state.ticket(), view("b", S2, 600), Some("a")),
            Err(Failure::AccountMismatch)
        );
        assert!(state.session().is_none());
    }

    #[test]
    fn a_read_of_another_session_switches_intent_and_fences_old_leases() {
        let mut state = Lifecycle::default();
        let first = state.ticket();
        state.observe(first, view("a", S1, 600), None).unwrap();
        assert_eq!(
            state.observe(first, view("a", S1, 700), None),
            Ok(Observation::Refreshed)
        );
        assert!(state.accepts(first), "same session keeps in-flight leases");
        assert_eq!(state.session().unwrap().idle, 700);
        // Another tab logged in: same actor, new session id, or another actor entirely.
        for next in [view("a", S2, 600), view("b", S2, 600)] {
            let mut state = Lifecycle::default();
            let ticket = state.ticket();
            state.observe(ticket, view("a", S1, 600), None).unwrap();
            assert_eq!(
                state.observe(ticket, next.clone(), None),
                Ok(Observation::Switched)
            );
            assert!(!state.accepts(ticket));
            assert_eq!(state.session(), Some(&next));
        }
    }

    #[test]
    fn renewal_keeps_the_family_never_shortens_and_never_changes_the_ticket() {
        let mut state = Lifecycle::default();
        let issued = state.ticket();
        state.observe(issued, view("a", S1, 600), None).unwrap();
        let dispatched = view("a", S1, 600);
        for bad in [
            view("a", S2, 700),
            view("b", S1, 700),
            view("a", S1, 500),
            SessionView::admit(wire("a", S1, 700, 901, Some(10))).unwrap(),
            SessionView::admit(wire("a", S1, 700, 900, Some(11))).unwrap(),
        ] {
            assert_eq!(
                state.renew(issued, &dispatched, bad),
                Err(Failure::Protocol)
            );
        }
        state
            .renew(issued, &dispatched, view("a", S1, 700))
            .unwrap();
        assert!(state.accepts(issued));
        assert_eq!(state.session().unwrap().idle, 700);
        state.invalidate().unwrap();
        assert_eq!(
            state.renew(issued, &dispatched, view("a", S1, 800)),
            Err(Failure::Changed)
        );
        assert!(state.session().is_none());
    }

    #[test]
    fn a_renewal_reply_older_than_a_confirmed_read_keeps_the_larger_idle_and_the_session() {
        let mut state = Lifecycle::default();
        let issued = state.ticket();
        state.observe(issued, view("a", S1, 600), None).unwrap();
        // Dispatched at idle 600; an authoritative read then confirms 800.
        let dispatched = state.session().cloned().unwrap();
        state.observe(issued, view("a", S1, 800), None).unwrap();
        assert_eq!(state.session().unwrap().idle, 800);
        // The older renewal answers 700: valid against its dispatch snapshot, never a regression.
        state
            .renew(issued, &dispatched, view("a", S1, 700))
            .unwrap();
        assert_eq!(state.session().unwrap().idle, 800);
        assert!(state.accepts(issued));
        // 500 is still below what this renewal was dispatched with.
        assert_eq!(
            state.renew(issued, &dispatched, view("a", S1, 500)),
            Err(Failure::Protocol)
        );
        assert_eq!(state.session().unwrap().idle, 800);
        // A reply capped above the absolute deadline is refused even when idle only grows.
        assert_eq!(
            state.renew(
                issued,
                &dispatched,
                SessionView {
                    idle: 901,
                    ..view("a", S1, 700)
                }
            ),
            Err(Failure::Protocol)
        );
    }

    #[test]
    fn a_renewal_of_another_family_or_intent_is_never_published() {
        let mut state = Lifecycle::default();
        let issued = state.ticket();
        state.observe(issued, view("a", S1, 600), None).unwrap();
        let dispatched = state.session().cloned().unwrap();
        // Another tab switched the session: the intent moved on, the old reply cannot publish.
        state.observe(issued, view("a", S2, 600), None).unwrap();
        assert_eq!(
            state.renew(issued, &dispatched, view("a", S1, 700)),
            Err(Failure::Changed)
        );
        assert_eq!(state.session().unwrap().session, S2);
        assert_eq!(state.session().unwrap().idle, 600);
        // A reply of a foreign family for the current intent is a protocol fault, not published.
        let current = state.ticket();
        let dispatched = state.session().cloned().unwrap();
        assert_eq!(
            state.renew(current, &dispatched, view("b", S1, 700)),
            Err(Failure::Protocol)
        );
        assert_eq!(state.session().unwrap().idle, 600);
    }

    #[test]
    fn the_rate_gate_holds_until_the_deadline_and_is_never_shortened() {
        let mut gate = RateGate::default();
        assert_eq!(gate.check(0), Ok(()));
        gate.arm(100, 30);
        assert_eq!(gate.check(100), Err(Failure::RateLimited(30)));
        assert_eq!(gate.check(129), Err(Failure::RateLimited(1)));
        assert_eq!(gate.check(130), Ok(()));
        gate.arm(110, 5);
        assert_eq!(gate.check(115), Err(Failure::RateLimited(15)));
        gate.arm(120, 60);
        assert_eq!(gate.check(121), Err(Failure::RateLimited(59)));
    }

    #[test]
    fn stale_logout_cannot_cancel_a_new_login_or_close_account_b() {
        let mut state = Lifecycle::default();
        state
            .observe(state.ticket(), view("b", S2, 600), None)
            .unwrap();
        assert!(!state.permits_logout(None, false));
        assert!(!state.permits_logout(Some(S1), false));
        assert!(state.permits_logout(Some(S2), false));
        assert!(!state.permits_logout(Some(S2), true));
        state.invalidate().unwrap();
        assert!(!state.permits_logout(None, true));
        assert!(state.permits_logout(None, false));
    }

    #[test]
    fn status_projection_keeps_unknown_effect_distinct_from_signed_out() {
        assert_eq!(classify(401, None), Failure::SignedOut);
        assert_eq!(classify(409, None), Failure::Changed);
        assert_eq!(classify(403, None), Failure::Forbidden);
        assert_eq!(classify(404, None), Failure::NotFound);
        assert_eq!(classify(400, None), Failure::Invalid);
        for status in [500, 502, 503, 504, 418] {
            assert_eq!(classify(status, None), Failure::Unavailable);
        }
        assert_eq!(classify(429, Some("7")), Failure::RateLimited(7));
        assert_eq!(classify_login(401, None), Failure::InvalidCredentials);
        assert_eq!(classify_login(403, None), Failure::Forbidden);
        assert_eq!(classify_login(503, None), Failure::Unavailable);
        assert_eq!(classify_login(409, None), Failure::Protocol);
    }

    #[test]
    fn retry_after_is_bounded_and_never_zero() {
        assert_eq!(retry_after(Some("7")), 7);
        assert_eq!(retry_after(Some(" 12 ")), 12);
        assert_eq!(retry_after(Some("0")), 1);
        assert_eq!(retry_after(Some("999999")), 3600);
        for unreadable in [
            None,
            Some(""),
            Some("-1"),
            Some("Wed, 21 Oct 2026 07:28:00 GMT"),
        ] {
            assert_eq!(retry_after(unreadable), 30);
        }
    }

    #[test]
    fn session_headers_are_surface_specific() {
        assert_eq!(session_header(Surface::Theatre), "x-cantos-theatre-session");
        assert_eq!(session_header(Surface::Studio), "x-cantos-studio-session");
    }

    #[test]
    fn credentials_only_ever_target_this_surface_of_the_api_origin() {
        let page = "https://studio.example";
        let studio = Surface::Studio;
        // Same-origin proxy (no configured API origin).
        assert_eq!(
            credentialed_target("/api/v1/studio/uploads/id", page, None, studio),
            Ok("https://studio.example/api/v1/studio/uploads/id".into())
        );
        // Configured API origin: only that origin.
        let api = Some("https://api.example");
        assert_eq!(
            credentialed_target("https://api.example/api/v1/studio/x", page, api, studio),
            Ok("https://api.example/api/v1/studio/x".into())
        );
        for target in [
            "/api/v1/studio/x", // resolves to the page origin, not the API origin
            "https://evil.example/api/v1/studio/x",
            "https://api.example:444/api/v1/studio/x",
            "https://api.example/api/v1/theatre/x",
            "https://api.example/api/v1/studio",
            "https://api.example/api/v1/studio/../theatre/x",
            "https://api.example/api/v1/studio/%2e%2e/theatre",
            "https://user@api.example/api/v1/studio/x",
            "https://api.example/api/v1/studio/x#fragment",
            "javascript:alert(1)",
        ] {
            assert_eq!(
                credentialed_target(target, page, api, studio),
                Err(Failure::Protocol),
                "{target}"
            );
        }
        assert_eq!(
            credentialed_target(
                "https://studio.example/api/v1/theatre/x",
                page,
                None,
                studio
            ),
            Err(Failure::Protocol)
        );
        assert_eq!(
            credentialed_target("/api/v1/studio/x", "https://studio.example/", None, studio),
            Err(Failure::Protocol)
        );
    }
}
