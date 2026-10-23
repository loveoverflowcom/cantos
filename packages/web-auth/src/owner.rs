//! The Identity v1 session owner, independent of the browser. Everything here is the production
//! decision path: the browser supplies only a [`Port`] (clock, one HTTP attempt, a cross-tab
//! lock and signals), so host tests drive this exact owner with a scripted port.
//!
//! The HttpOnly credential is owned by the browser and never appears here. The owner keeps
//! nonsecret session facts, fences intent, and serializes the exchanges that change the cookie.
use crate::{
    core::{
        classify, classify_login, AccessAction, Clock, Lifecycle, RateGate, SessionView, Ticket,
    },
    refresh::RefreshFlight,
    serial::Serial,
    Failure, Surface,
};
use cantos_contracts::{bearer::Session, identity_auth::Login, web_api::api_url};
use futures_util::FutureExt;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    rc::Rc,
};

/// Largest session/error body this owner reads.
pub const MAX_BODY: usize = 16_384;
/// Cleanup waits at most this many 50 ms pauses for the old login/cleanup owner to settle.
const SETTLE_PAUSES: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
}

/// One credentialed identity attempt. `fence` is the session id the caller believes it holds.
#[derive(Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub body: Option<String>,
    pub fence: Option<String>,
}

/// A fully received answer (headers and a bounded body).
#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub content_type: Option<String>,
    pub retry_after: Option<String>,
    pub body: Vec<u8>,
}

/// Platform seam. Implementations hold no session state.
pub trait Port: 'static {
    /// Released when dropped; excludes other tabs of this browser profile.
    type TabLock;
    /// Wall-clock seconds and monotonic elapsed seconds.
    fn clock(&self) -> (u64, u64);
    /// One attempt with its own deadline. A deadline, network failure or abort is `Unavailable`
    /// (an unknown effect for anything that changes state).
    fn send(
        &self,
        surface: Surface,
        request: Request,
    ) -> impl Future<Output = Result<Reply, Failure>>;
    /// Serialize cookie-changing exchanges across tabs. A browser without the capability
    /// returns a no-op lock: ordering then holds inside one document only.
    fn tab_lock(
        &self,
        surface: Surface,
        origin: Option<&str>,
    ) -> impl Future<Output = Result<Self::TabLock, Failure>>;
    fn pause(&self, millis: u32) -> impl Future<Output = Result<(), Failure>>;
    /// The views of this document must stop showing the account (the API said it is gone).
    fn lost(&self, surface: Surface);
    /// Tell other tabs that this document changed the cookie. A signal, never authentication.
    fn announce(&self, surface: Surface);
}

struct Owner<P: Port> {
    port: P,
    surface: Surface,
    origin: Option<String>,
    lifecycle: RefCell<Lifecycle>,
    clock: RefCell<Clock>,
    renew: RefreshFlight,
    /// Sessions whose DELETE is not yet acknowledged. Blocks new admission until retried.
    cleanup: RefCell<Vec<String>>,
    cleaning: Cell<bool>,
    authorizing: Cell<bool>,
    gate: RefCell<RateGate>,
    /// Cookie-changing exchanges of this document, one at a time.
    serial: Serial,
}

pub struct Client<P: Port>(Rc<Owner<P>>);
impl<P: Port> Clone for Client<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// A fenced, admitted session. Apply it to every protected request of its surface.
pub struct Lease<P: Port> {
    client: Client<P>,
    ticket: Ticket,
    view: SessionView,
}
impl<P: Port> Clone for Lease<P> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            ticket: self.ticket,
            view: self.view.clone(),
        }
    }
}
impl<P: Port> Lease<P> {
    pub fn actor(&self) -> &str {
        &self.view.actor
    }
    pub fn session(&self) -> &str {
        &self.view.session
    }
    pub fn surface(&self) -> Surface {
        self.client.0.surface
    }
    /// Header name and value for transports that are not `fetch` (XHR sets
    /// `withCredentials` itself). The value is a nonsecret session id.
    pub fn fence(&self) -> (&'static str, &str) {
        (
            crate::core::session_header(self.client.0.surface),
            &self.view.session,
        )
    }
    pub fn check(&self) -> Result<(), Failure> {
        if self.client.0.lifecycle.borrow().accepts(self.ticket) {
            Ok(())
        } else {
            Err(Failure::Changed)
        }
    }
    /// Call with the status (and `Retry-After`, when the origin exposes it) of every answer to a
    /// request made with this lease, before its body can change any state. A 429 arms the
    /// surface-wide gate even for a stale lease: it is a fact about the API, not the account.
    /// A stale lease is `Changed`; a stale 401 cannot sign out a switched or new account.
    /// 403 is left to the caller: it is `Forbidden`, never an expired session.
    pub fn response(&self, status: u16, retry_after: Option<&str>) -> Result<(), Failure> {
        let limited = (status == 429).then(|| self.client.limited(retry_after));
        self.check()?;
        if let Some(failure) = limited {
            return Err(failure);
        }
        if status == 401 {
            self.client.lose(self.ticket)?;
            return Err(Failure::SignedOut);
        }
        Ok(())
    }
    /// The API answered 409 with the session-fence code: another tab changed the account. The
    /// local session is dropped (no event) so the owner must re-read before any further request.
    pub fn fence_mismatch(&self) -> Failure {
        match self.client.forget(self.ticket) {
            Ok(_) => Failure::Changed,
            Err(error) => error,
        }
    }
}

/// How an exchange ended: it never left (`Err` of the outer result) or it was dispatched.
type Exchange = Result<Result<Reply, Failure>, Failure>;

impl<P: Port> Client<P> {
    pub fn new(port: P, surface: Surface, origin: Option<&str>) -> Result<Self, Failure> {
        api_url(
            origin,
            surface,
            &format!("{}/identity/session", surface.base()),
        )
        .map_err(|_| Failure::Protocol)?;
        Ok(Self(Rc::new(Owner {
            port,
            surface,
            origin: origin.map(str::to_owned),
            lifecycle: RefCell::new(Lifecycle::default()),
            clock: RefCell::new(Clock::default()),
            renew: RefreshFlight::default(),
            cleanup: RefCell::new(Vec::new()),
            cleaning: Cell::new(false),
            authorizing: Cell::new(false),
            gate: RefCell::new(RateGate::default()),
            serial: Serial::default(),
        })))
    }
    pub fn origin(&self) -> Option<&str> {
        self.0.origin.as_deref()
    }
    pub fn surface(&self) -> Surface {
        self.0.surface
    }
    fn now(&self) -> u64 {
        let (wall, elapsed) = self.0.port.clock();
        self.0.clock.borrow_mut().now(wall, elapsed)
    }
    fn endpoint(&self, suffix: &str) -> Result<String, Failure> {
        api_url(
            self.0.origin.as_deref(),
            self.0.surface,
            &format!("{}/identity{suffix}", self.0.surface.base()),
        )
        .map_err(|_| Failure::Protocol)
    }
    /// 429 gate: nothing leaves this surface of the document before the hold ends.
    pub fn gate(&self) -> Result<(), Failure> {
        let now = self.now();
        self.0.gate.borrow().check(now)
    }
    fn limited(&self, retry_after_header: Option<&str>) -> Failure {
        let failure = classify(429, retry_after_header);
        if let Failure::RateLimited(seconds) = failure {
            let now = self.now();
            self.0.gate.borrow_mut().arm(now, seconds);
        }
        failure
    }
    /// Project a non-success answer and arm the 429 gate: no dense retries.
    fn refusal(&self, reply: &Reply, login: bool) -> Failure {
        let header = reply.retry_after.as_deref();
        let failure = if login {
            classify_login(reply.status, header)
        } else {
            classify(reply.status, header)
        };
        if matches!(failure, Failure::RateLimited(_)) {
            return self.limited(header);
        }
        failure
    }
    fn remember(&self, session: &str) {
        let mut queue = self.0.cleanup.borrow_mut();
        if !queue.iter().any(|queued| queued == session) {
            queue.push(session.to_owned());
        }
    }
    fn abandon(&self, ticket: Ticket) -> Result<(), Failure> {
        if self.0.lifecycle.borrow().accepts(ticket) {
            let previous = self.0.lifecycle.borrow_mut().invalidate()?;
            self.0.renew.detach();
            if let Some(session) = previous {
                self.remember(&session.session);
            }
        }
        Ok(())
    }
    /// Drop the local session without queueing a revoke: the API already said it is gone (401)
    /// or that the cookie now belongs to another session (409). Returns whether one was dropped.
    fn forget(&self, ticket: Ticket) -> Result<bool, Failure> {
        let current = {
            let state = self.0.lifecycle.borrow();
            state.accepts(ticket) && state.session().is_some()
        };
        if current {
            self.0.lifecycle.borrow_mut().invalidate()?;
            self.0.renew.detach();
        }
        Ok(current)
    }
    /// 401: forget and tell the owning view, so it cannot keep showing the account.
    fn lose(&self, ticket: Ticket) -> Result<(), Failure> {
        if self.forget(ticket)? {
            self.0.port.lost(self.0.surface);
        }
        Ok(())
    }
    /// Another tab of this browser changed the cookie (login, logout or a revoked login). That
    /// signal proves nothing about who the cookie now names: it only withdraws this document's
    /// authority, fences every lease and lets the views re-read the session from the API.
    pub fn external_change(&self) {
        // Even a guest may have a session read or login in flight. Fence that answer before
        // notifying views, and wake guests too: a peer may just have logged in. Do not queue
        // a revoke, since the ambient cookie belongs to the peer's current intent.
        let invalidated = self.0.lifecycle.borrow_mut().invalidate().is_ok();
        if invalidated {
            self.0.renew.detach();
            self.0.port.lost(self.0.surface);
        }
    }
    pub fn has_session(&self) -> bool {
        self.0.lifecycle.borrow().session().is_some()
    }
    pub fn actor(&self) -> Option<String> {
        self.0
            .lifecycle
            .borrow()
            .session()
            .map(|session| session.actor.clone())
    }
    pub fn session(&self) -> Option<SessionView> {
        self.0.lifecycle.borrow().session().cloned()
    }
    fn ticket_current(&self, issued: Ticket) -> Result<(), Failure> {
        if self.0.lifecycle.borrow().accepts(issued) {
            Ok(())
        } else {
            Err(Failure::Changed)
        }
    }
    fn admission(&self) -> Result<(), Failure> {
        if !self.0.cleanup.borrow().is_empty() {
            return Err(Failure::CleanupPending);
        }
        if self.0.authorizing.get() {
            return Err(Failure::Changed);
        }
        Ok(())
    }

    /// One attempt that does not change the cookie.
    async fn dispatch(
        &self,
        method: Method,
        suffix: &str,
        fence: Option<&str>,
    ) -> Result<Reply, Failure> {
        let request = Request {
            method,
            url: self.endpoint(suffix)?,
            body: None,
            fence: fence.map(str::to_owned),
        };
        self.0.port.send(self.0.surface, request).await
    }
    /// One cookie-changing attempt (login, renew, logout). Exchanges of this surface leave one
    /// at a time, here and in every tab, so the browser applies their `Set-Cookie` in dispatch
    /// order. `recheck` runs once the exchange owns the lock and before anything is sent: a
    /// login cancelled while it waited is never dispatched.
    async fn exchange(
        &self,
        method: Method,
        suffix: &str,
        body: Option<String>,
        fence: Option<&str>,
        recheck: impl FnOnce() -> Result<(), Failure>,
    ) -> Exchange {
        let request = Request {
            method,
            url: self.endpoint(suffix)?,
            body,
            fence: fence.map(str::to_owned),
        };
        let _serial = self.0.serial.acquire().await;
        let _tabs = self
            .0
            .port
            .tab_lock(self.0.surface, self.0.origin.as_deref())
            .await?;
        recheck()?;
        Ok(self.0.port.send(self.0.surface, request).await)
    }

    /// Authoritative read of the cookie session (`GET identity/session`, no fence: "who am I
    /// now?"). `Ok(None)` is a genuine 401. A different session than the local one switches
    /// intent and fences every old lease. Use at startup, after an account change in another
    /// tab, or whenever local deadlines cannot be trusted.
    pub async fn read_session(&self) -> Result<Option<SessionView>, Failure> {
        self.admission()?;
        let issued = self.0.lifecycle.borrow().ticket();
        self.gate()?;
        let reply = self.dispatch(Method::Get, "/session", None).await?;
        self.ticket_current(issued)?;
        match reply.status {
            200 => {
                let view = read_view(&reply)?;
                self.ticket_current(issued)?;
                self.0.lifecycle.borrow_mut().observe(issued, view, None)?;
                Ok(self.session())
            }
            401 => {
                self.forget(issued)?;
                Ok(None)
            }
            _ => Err(self.refusal(&reply, false)),
        }
    }

    /// Email/password sign-in. One attempt, never retried transparently. A definite refusal
    /// leaves the previous local state untouched; an unknown outcome (the cookie may have been
    /// replaced) drops it so only an authoritative read can re-admit a session.
    ///
    /// The authorization intent is fixed before the first await, so a cancel (or logout, or a
    /// newer intent) while old cleanup or another tab's exchange is pending wins: it is checked
    /// again after that wait and before the POST, so a cancelled login sends nothing. After the
    /// POST was dispatched its effect is unknown: a late answer is revoked, never adopted.
    pub async fn login(
        &self,
        email: String,
        password: String,
        expected_actor: Option<String>,
    ) -> Result<SessionView, Failure> {
        if self.0.authorizing.replace(true) {
            return Err(Failure::Changed);
        }
        struct Reset<'a>(&'a Cell<bool>);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _reset = Reset(&self.0.authorizing);
        let issued = self.0.lifecycle.borrow().ticket();
        if !self.0.cleanup.borrow().is_empty() {
            self.cleanup().await?;
        }
        self.ticket_current(issued)?;
        let body =
            serde_json::to_string(&Login { email, password }).map_err(|_| Failure::Protocol)?;
        let sent = self
            .exchange(Method::Post, "/login", Some(body), None, || {
                self.ticket_current(issued)?;
                self.gate()
            })
            .await?;
        let reply = match sent {
            Ok(reply) => reply,
            Err(error) => {
                let _ = self.abandon(issued);
                return Err(error);
            }
        };
        if reply.status != 200 {
            let failure = self.refusal(&reply, true);
            if failure == Failure::Unavailable {
                let _ = self.abandon(issued);
            } else if self.ticket_current(issued).is_err() {
                return Err(Failure::Changed);
            }
            return Err(failure);
        }
        let view = match read_view(&reply) {
            Ok(view) => view,
            Err(error) => {
                let _ = self.abandon(issued);
                return Err(error);
            }
        };
        let session = view.session.clone();
        let observed =
            self.0
                .lifecycle
                .borrow_mut()
                .observe(issued, view, expected_actor.as_deref());
        match observed {
            Ok(_) => {
                self.0.renew.detach();
                self.0.port.announce(self.0.surface);
                self.session().ok_or(Failure::Protocol)
            }
            Err(error) => {
                // The API already set this cookie. A refused or superseded login must not
                // leave an ambient session the application never admitted.
                self.remember(&session);
                let _ = self.cleanup().await;
                Err(error)
            }
        }
    }

    /// Admit a session for one protected request: use it, renew its idle deadline (one shared
    /// flight) or re-read when local deadlines say it may be over. Local time never decides
    /// signed-out by itself. A 429 hold applies even to a session that is still valid: nothing
    /// is dispatched before it ends.
    pub async fn access(&self, expected_session: Option<&str>) -> Result<Lease<P>, Failure> {
        let issued = self.0.lifecycle.borrow().ticket();
        let action = self.0.lifecycle.borrow().access_action(
            expected_session,
            self.now(),
            !self.0.cleanup.borrow().is_empty(),
            self.0.authorizing.get(),
            self.0.renew.is_pending(),
        )?;
        self.gate()?;
        match action {
            AccessAction::Use => {}
            AccessAction::Renew | AccessAction::JoinRenew => {
                let session = self
                    .0
                    .lifecycle
                    .borrow()
                    .session()
                    .cloned()
                    .ok_or(Failure::SignedOut)?;
                let client = self.clone();
                let future = self
                    .0
                    .renew
                    .acquire(|| async move { client.renew(issued, session).await }.boxed_local());
                let result = future.clone().await;
                self.0.renew.finish(&future);
                result?;
            }
            AccessAction::Reconcile => {
                if self.read_session().await?.is_none() {
                    return Err(Failure::SignedOut);
                }
            }
        }
        let state = self.0.lifecycle.borrow();
        if state.ticket() != issued {
            return Err(Failure::Changed);
        }
        let view = state.session().cloned().ok_or(Failure::SignedOut)?;
        if expected_session.is_some_and(|expected| expected != view.session) {
            return Err(Failure::Changed);
        }
        Ok(Lease {
            client: self.clone(),
            ticket: state.ticket(),
            view,
        })
    }
    /// Renewal keeps the same cookie value, so an unknown outcome is safe to retry later and
    /// never discards the session. The reply is judged against `dispatched`, the snapshot the
    /// request was sent for, because an authoritative read may have raised the local idle
    /// deadline while it was in flight.
    async fn renew(&self, issued: Ticket, dispatched: SessionView) -> Result<(), Failure> {
        let sent = self
            .exchange(
                Method::Post,
                "/session/renew",
                None,
                Some(&dispatched.session),
                || {
                    self.ticket_current(issued)?;
                    self.gate()
                },
            )
            .await?;
        let reply = sent?;
        self.ticket_current(issued)?;
        match reply.status {
            200 => {
                let view = read_view(&reply)?;
                self.ticket_current(issued)?;
                let renewed = self
                    .0
                    .lifecycle
                    .borrow_mut()
                    .renew(issued, &dispatched, view);
                if renewed == Err(Failure::Protocol) {
                    self.lose(issued)?;
                }
                renewed
            }
            401 => {
                self.lose(issued)?;
                Err(Failure::SignedOut)
            }
            409 => {
                self.forget(issued)?;
                Err(Failure::Changed)
            }
            _ => Err(self.refusal(&reply, false)),
        }
    }
    /// Synchronous fence before the first await; all old effects lose admission immediately.
    pub fn invalidate(&self) -> Result<(), Failure> {
        let issued = self.0.lifecycle.borrow().ticket();
        self.abandon(issued)
    }
    /// Fence an in-flight login: it is never dispatched when it has not left yet, and its late
    /// response is revoked instead of adopted when it has.
    pub fn cancel_authorization(&self) -> Result<(), Failure> {
        self.invalidate()
    }
    pub fn begin_logout(&self, expected: Option<&str>) -> Result<(), Failure> {
        if !self
            .0
            .lifecycle
            .borrow()
            .permits_logout(expected, self.0.authorizing.get())
        {
            return Err(Failure::Changed);
        }
        self.invalidate()
    }
    pub async fn logout(&self, expected: Option<&str>) -> Result<(), Failure> {
        self.begin_logout(expected)?;
        self.cleanup().await
    }
    /// Cancellation includes the old login/cleanup owner settling.
    pub async fn settled_cleanup(&self) -> Result<(), Failure> {
        let mut pauses = 0;
        while self.0.authorizing.get() || self.0.cleaning.get() {
            pauses += 1;
            if pauses > SETTLE_PAUSES {
                return Err(Failure::CleanupPending);
            }
            self.0.port.pause(50).await?;
        }
        self.cleanup().await
    }
    /// `DELETE identity/session` fenced to each session id. 204 is done; 401 (already gone)
    /// and 409 (the cookie now belongs to another session, so this one is already retired) are
    /// terminal too. Any other outcome stays queued and blocks admission until retried.
    pub async fn cleanup(&self) -> Result<(), Failure> {
        if self.0.cleaning.replace(true) {
            return Err(Failure::CleanupPending);
        }
        struct Reset<'a>(&'a Cell<bool>);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _reset = Reset(&self.0.cleaning);
        loop {
            let next = self.0.cleanup.borrow().first().cloned();
            let Some(session) = next else {
                return Ok(());
            };
            self.gate()?;
            let sent = self
                .exchange(Method::Delete, "/session", None, Some(&session), || {
                    self.gate()
                })
                .await
                .map_err(|failure| match failure {
                    Failure::RateLimited(_) => failure,
                    _ => Failure::CleanupPending,
                })?;
            let reply = sent.map_err(|_| Failure::CleanupPending)?;
            match reply.status {
                // The cookie was cleared: other tabs must stop trusting their copy of it.
                204 => self.0.port.announce(self.0.surface),
                401 | 409 => {}
                _ => {
                    let failure = self.refusal(&reply, false);
                    return Err(if matches!(failure, Failure::RateLimited(_)) {
                        failure
                    } else {
                        Failure::CleanupPending
                    });
                }
            }
            self.0
                .cleanup
                .borrow_mut()
                .retain(|queued| queued != &session);
        }
    }
}

fn read_view(reply: &Reply) -> Result<SessionView, Failure> {
    let json = reply.content_type.as_deref().is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    });
    if !json || reply.body.len() > MAX_BODY {
        return Err(Failure::Protocol);
    }
    let wire: Session = serde_json::from_slice(&reply.body).map_err(|_| Failure::Protocol)?;
    SessionView::admit(wire)
}

#[cfg(test)]
mod tests;
