//! Browser effects for Identity v1 cookie sessions. The HttpOnly credential is owned by the
//! browser: this module never reads, stores or forwards it, and keeps only nonsecret session
//! facts (actor, session id, deadlines) in this page's RAM. Every request here carries
//! `credentials: include` and the session fence; the browser adds `Origin` itself.
use crate::{
    core::{
        classify, classify_login, session_header, AccessAction, Clock, Lifecycle, SessionView,
        Ticket,
    },
    refresh::RefreshFlight,
    Failure, Surface,
};
use cantos_contracts::{bearer::Session, identity_auth::Login, web_api::api_url};
use futures_util::FutureExt;
use gloo_net::http::{Request, RequestBuilder, Response};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, RequestCredentials, RequestRedirect, Window};

const HTTP_DEADLINE_MS: i32 = 15_000;
const MAX_BODY: usize = 16_384;

fn window() -> Result<Window, Failure> {
    web_sys::window().ok_or(Failure::Unavailable)
}
pub fn invalidation_event(surface: Surface) -> &'static str {
    match surface {
        Surface::Studio => "cantos-studio-session-invalidated",
        Surface::Theatre => "cantos-theatre-session-invalidated",
    }
}

#[derive(Clone)]
pub struct Client(Rc<Owner>);
struct Owner {
    surface: Surface,
    origin: Option<String>,
    lifecycle: RefCell<Lifecycle>,
    clock: RefCell<Clock>,
    renew: RefreshFlight,
    /// Sessions whose DELETE is not yet acknowledged. Blocks new admission until retried.
    cleanup: RefCell<Vec<String>>,
    cleaning: Cell<bool>,
    authorizing: Cell<bool>,
    /// 429 gate: monotonic-floored seconds before another request may leave this document.
    hold_until: Cell<u64>,
}
thread_local! {
    static STUDIO: RefCell<Option<Client>> = const { RefCell::new(None) };
    static THEATRE: RefCell<Option<Client>> = const { RefCell::new(None) };
}
/// One owner per surface and document. Reconfiguration cannot redirect a credentialed request.
pub fn client(surface: Surface, origin: Option<&str>) -> Result<Client, Failure> {
    api_url(
        origin,
        surface,
        &format!("{}/identity/session", surface.base()),
    )
    .map_err(|_| Failure::Protocol)?;
    let get = |slot: &RefCell<Option<Client>>| {
        let mut slot = slot.borrow_mut();
        if let Some(client) = slot.as_ref() {
            if client.0.origin.as_deref() != origin {
                return Err(Failure::Protocol);
            }
            return Ok(client.clone());
        }
        let client = Client(Rc::new(Owner {
            surface,
            origin: origin.map(str::to_owned),
            lifecycle: RefCell::new(Lifecycle::default()),
            clock: RefCell::new(Clock::default()),
            renew: RefreshFlight::default(),
            cleanup: RefCell::new(Vec::new()),
            cleaning: Cell::new(false),
            authorizing: Cell::new(false),
            hold_until: Cell::new(0),
        }));
        *slot = Some(client.clone());
        Ok(client)
    };
    match surface {
        Surface::Studio => STUDIO.with(get),
        Surface::Theatre => THEATRE.with(get),
    }
}

#[derive(Clone, Copy)]
enum Method {
    Get,
    Post,
    Delete,
}

/// A fenced, admitted session. Apply it to every protected request of its surface.
#[derive(Clone)]
pub struct Lease {
    client: Client,
    ticket: Ticket,
    view: SessionView,
}
impl Lease {
    pub fn actor(&self) -> &str {
        &self.view.actor
    }
    pub fn session(&self) -> &str {
        &self.view.session
    }
    /// The one place a protected fetch gets its credential policy: ambient cookie, the
    /// session fence, no redirects. Never use this for public, producer or foreign-origin URLs.
    pub fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .credentials(RequestCredentials::Include)
            .redirect(RequestRedirect::Error)
            .header(session_header(self.client.0.surface), &self.view.session)
    }
    /// Header name and value for transports that are not `fetch` (XHR sets
    /// `withCredentials` itself). The value is a nonsecret session id.
    pub fn fence(&self) -> (&'static str, &str) {
        (session_header(self.client.0.surface), &self.view.session)
    }
    pub fn check(&self) -> Result<(), Failure> {
        if self.client.0.lifecycle.borrow().accepts(self.ticket) {
            Ok(())
        } else {
            Err(Failure::Changed)
        }
    }
    /// Call again after consuming the body. A stale 401 cannot sign out a switched/new account.
    pub fn response(&self, status: u16) -> Result<(), Failure> {
        self.check()?;
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

impl Client {
    fn now(&self) -> u64 {
        let wall = (js_sys::Date::now() / 1000.0) as u64;
        let elapsed = web_sys::window()
            .and_then(|window| window.performance())
            .map_or(0, |performance| (performance.now() / 1000.0) as u64);
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
    fn gate(&self) -> Result<(), Failure> {
        let now = self.now();
        let until = self.0.hold_until.get();
        if until > now {
            return Err(Failure::RateLimited(
                (until - now).min(u32::MAX as u64) as u32
            ));
        }
        Ok(())
    }
    /// Project a non-success status and arm the 429 gate: no dense retries.
    fn refusal(&self, response: &Response, login: bool) -> Failure {
        let header = response.headers().get("retry-after");
        let status = response.status();
        let failure = if login {
            classify_login(status, header.as_deref())
        } else {
            classify(status, header.as_deref())
        };
        if let Failure::RateLimited(seconds) = failure {
            self.0
                .hold_until
                .set(self.now().saturating_add(u64::from(seconds)));
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
            if let (Ok(window), Ok(event)) = (
                window(),
                web_sys::Event::new(invalidation_event(self.0.surface)),
            ) {
                let _ = window.dispatch_event(&event);
            }
        }
        Ok(())
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

    /// Authoritative read of the cookie session (`GET identity/session`, no fence: "who am I
    /// now?"). `Ok(None)` is a genuine 401. A different session than the local one switches
    /// intent and fences every old lease. Use at startup, after an account change in another
    /// tab, or whenever local deadlines cannot be trusted.
    pub async fn read_session(&self) -> Result<Option<SessionView>, Failure> {
        self.admission()?;
        let issued = self.0.lifecycle.borrow().ticket();
        self.gate()?;
        let deadline = Deadline::new()?;
        let response = self
            .send(Method::Get, "/session", None, None, &deadline)
            .await?;
        self.ticket_current(issued)?;
        match response.status() {
            200 => {
                let view = read_view(response).await?;
                self.ticket_current(issued)?;
                self.0.lifecycle.borrow_mut().observe(issued, view, None)?;
                Ok(self.session())
            }
            401 => {
                self.forget(issued)?;
                Ok(None)
            }
            _ => Err(self.refusal(&response, false)),
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

    /// Email/password sign-in. One attempt, never retried transparently. A definite refusal
    /// leaves the previous local state untouched; an unknown outcome (the cookie may have been
    /// replaced) drops it so only an authoritative read can re-admit a session.
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
        if !self.0.cleanup.borrow().is_empty() {
            self.cleanup().await?;
        }
        self.gate()?;
        let issued = self.0.lifecycle.borrow().ticket();
        let body =
            serde_json::to_string(&Login { email, password }).map_err(|_| Failure::Protocol)?;
        let deadline = Deadline::new()?;
        let sent = self
            .send(Method::Post, "/login", Some(body), None, &deadline)
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(error) => {
                let _ = self.abandon(issued);
                return Err(error);
            }
        };
        if response.status() != 200 {
            let failure = self.refusal(&response, true);
            if failure == Failure::Unavailable {
                let _ = self.abandon(issued);
            }
            return Err(failure);
        }
        let view = match read_view(response).await {
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
    /// signed-out by itself.
    pub async fn access(&self, expected_session: Option<&str>) -> Result<Lease, Failure> {
        let issued = self.0.lifecycle.borrow().ticket();
        let action = self.0.lifecycle.borrow().access_action(
            expected_session,
            self.now(),
            !self.0.cleanup.borrow().is_empty(),
            self.0.authorizing.get(),
            self.0.renew.is_pending(),
        )?;
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
    /// never discards the session.
    async fn renew(&self, issued: Ticket, session: SessionView) -> Result<(), Failure> {
        self.gate()?;
        let deadline = Deadline::new()?;
        let response = self
            .send(
                Method::Post,
                "/session/renew",
                None,
                Some(&session.session),
                &deadline,
            )
            .await?;
        self.ticket_current(issued)?;
        match response.status() {
            200 => {
                let view = read_view(response).await?;
                self.ticket_current(issued)?;
                let renewed = self.0.lifecycle.borrow_mut().renew(issued, view);
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
            _ => Err(self.refusal(&response, false)),
        }
    }
    /// Synchronous fence before the first await; all old effects lose admission immediately.
    pub fn invalidate(&self) -> Result<(), Failure> {
        let issued = self.0.lifecycle.borrow().ticket();
        self.abandon(issued)
    }
    /// Fence an in-flight login: its late response is revoked instead of adopted.
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
        let deadline = Deadline::new()?;
        while self.0.authorizing.get() || self.0.cleaning.get() {
            if deadline.controller.signal().aborted() {
                return Err(Failure::CleanupPending);
            }
            let window = window()?;
            let promise = js_sys::Promise::new(&mut |resolve, reject| {
                if window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
                    .is_err()
                {
                    let _ = reject.call0(&JsValue::NULL);
                }
            });
            JsFuture::from(promise)
                .await
                .map_err(|_| Failure::Unavailable)?;
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
            let deadline = Deadline::new()?;
            let response = self
                .send(Method::Delete, "/session", None, Some(&session), &deadline)
                .await
                .map_err(|_| Failure::CleanupPending)?;
            match response.status() {
                204 | 401 | 409 => {}
                _ => {
                    let failure = self.refusal(&response, false);
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

    async fn send(
        &self,
        method: Method,
        suffix: &str,
        body: Option<String>,
        fence: Option<&str>,
        deadline: &Deadline,
    ) -> Result<Response, Failure> {
        let url = self.endpoint(suffix)?;
        let mut request = match method {
            Method::Get => Request::get(&url),
            Method::Post => Request::post(&url),
            Method::Delete => Request::delete(&url),
        }
        .credentials(RequestCredentials::Include)
        .redirect(RequestRedirect::Error)
        .header("Accept", "application/json")
        .abort_signal(Some(&deadline.controller.signal()));
        if let Some(fence) = fence {
            request = request.header(session_header(self.0.surface), fence);
        }
        let request = match body {
            Some(body) => request
                .header("Content-Type", "application/json")
                .body(body),
            None => request.build(),
        }
        .map_err(|_| Failure::Protocol)?;
        request.send().await.map_err(|_| Failure::Unavailable)
    }
}

async fn read_view(response: Response) -> Result<SessionView, Failure> {
    let json = response.headers().get("content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    });
    if !json {
        return Err(Failure::Protocol);
    }
    let bytes = response.binary().await.map_err(|_| Failure::Unavailable)?;
    if bytes.len() > MAX_BODY {
        return Err(Failure::Protocol);
    }
    let wire: Session = serde_json::from_slice(&bytes).map_err(|_| Failure::Protocol)?;
    SessionView::admit(wire)
}

struct Deadline {
    window: Window,
    controller: AbortController,
    timer: i32,
    _callback: Closure<dyn FnMut()>,
}
impl Deadline {
    fn new() -> Result<Self, Failure> {
        let window = window()?;
        let controller = AbortController::new().map_err(|_| Failure::Unavailable)?;
        let abort = controller.clone();
        let callback = Closure::wrap(Box::new(move || abort.abort()) as Box<dyn FnMut()>);
        let timer = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                HTTP_DEADLINE_MS,
            )
            .map_err(|_| Failure::Unavailable)?;
        Ok(Self {
            window,
            controller,
            timer,
            _callback: callback,
        })
    }
}
impl Drop for Deadline {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}
