//! Browser effects for Bearer v1. Credentials only live in this page's RAM.
use crate::{
    core::{AccessAction, Completion, Grant, Lifecycle, PendingFlow, Ticket},
    Failure, Surface,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use cantos_contracts::{
    bearer::{CancelRequest, ExchangeRequest, Flow, FlowRequest, RefreshRequest, TokenPair},
    web_api::api_url,
};
use futures_util::FutureExt;
use gloo_net::http::{Request, Response};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, MessageEvent, RequestCredentials, RequestRedirect, Window};

const FLOW_KEY: &str = "cantos.bearer-popup.v1";
const COMPLETE_PATH: &str = "/auth/bearer/complete";
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}
fn window() -> Result<Window, Failure> {
    web_sys::window().ok_or(Failure::Unavailable)
}
pub fn invalidation_event(surface: Surface) -> &'static str {
    match surface {
        Surface::Studio => "cantos-studio-bearer-invalidated",
        Surface::Theatre => "cantos-theatre-bearer-invalidated",
    }
}
fn name(surface: Surface) -> &'static str {
    match surface {
        Surface::Studio => "studio",
        Surface::Theatre => "theatre",
    }
}

#[derive(Clone)]
pub struct Client(Rc<Owner>);
use crate::refresh::RefreshFlight;
struct Owner {
    surface: Surface,
    origin: Option<String>,
    lifecycle: RefCell<Lifecycle>,
    refresh: RefreshFlight,
    cleanup: RefCell<Vec<Cleanup>>,
    cleaning: Cell<bool>,
    authorizing: Cell<bool>,
    popup: RefCell<Option<Window>>,
}
#[derive(Clone)]
enum Cleanup {
    Family {
        refresh: String,
        session: String,
    },
    Flow {
        id: String,
        state: String,
        verifier: String,
    },
}
thread_local! {
    static STUDIO: RefCell<Option<Client>> = const { RefCell::new(None) };
    static THEATRE: RefCell<Option<Client>> = const { RefCell::new(None) };
}
/// One owner per surface and document. Reconfiguration cannot redirect a credential.
pub fn client(surface: Surface, origin: Option<&str>) -> Result<Client, Failure> {
    api_url(origin, surface, &format!("{}/session", surface.base()))
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
            refresh: RefreshFlight::default(),
            cleanup: RefCell::new(Vec::new()),
            cleaning: Cell::new(false),
            authorizing: Cell::new(false),
            popup: RefCell::new(None),
        }));
        *slot = Some(client.clone());
        Ok(client)
    };
    match surface {
        Surface::Studio => STUDIO.with(get),
        Surface::Theatre => THEATRE.with(get),
    }
}

#[derive(Clone)]
pub struct Lease {
    client: Client,
    ticket: Ticket,
    grant: Grant,
}
impl Lease {
    pub fn header(&self) -> String {
        format!("Bearer {}", self.grant.access)
    }
    pub fn actor(&self) -> &str {
        &self.grant.actor
    }
    pub fn session(&self) -> &str {
        &self.grant.session
    }
    pub fn check(&self) -> Result<(), Failure> {
        if self.client.0.lifecycle.borrow().accepts(self.ticket) {
            Ok(())
        } else {
            Err(Failure::Changed)
        }
    }
    /// Call again after consuming the body. A stale 401 cannot sign out a rotated/new account.
    pub fn response(&self, status: u16) -> Result<(), Failure> {
        self.check()?;
        if status == 401 {
            self.client.lose(self.ticket)?;
            return Err(Failure::SignedOut);
        }
        Ok(())
    }
}
impl Client {
    fn endpoint(&self, suffix: &str) -> Result<String, Failure> {
        api_url(
            self.0.origin.as_deref(),
            self.0.surface,
            &format!("{}/auth/bearer{suffix}", self.0.surface.base()),
        )
        .map_err(|_| Failure::Protocol)
    }
    fn remember(&self, grant: &Grant) {
        let mut queue = self.0.cleanup.borrow_mut();
        if !queue
            .iter()
            .any(|item| matches!(item,Cleanup::Family{session,..} if session==&grant.session))
        {
            queue.push(Cleanup::Family {
                refresh: grant.refresh.clone(),
                session: grant.session.clone(),
            });
        }
    }
    fn abandon(&self, ticket: Ticket) -> Result<(), Failure> {
        if self.0.lifecycle.borrow().accepts(ticket) {
            let previous = self.0.lifecycle.borrow_mut().invalidate()?;
            self.0.refresh.detach();
            if let Some(grant) = previous {
                self.remember(&grant);
            }
        }
        Ok(())
    }
    fn lose(&self, ticket: Ticket) -> Result<(), Failure> {
        let current = {
            let state = self.0.lifecycle.borrow();
            state.accepts(ticket) && state.grant().is_some()
        };
        self.abandon(ticket)?;
        if current {
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
        self.0.lifecycle.borrow().grant().is_some()
    }
    pub fn actor(&self) -> Option<String> {
        self.0.lifecycle.borrow().grant().map(|g| g.actor.clone())
    }
    pub async fn access(&self, expected_session: Option<&str>) -> Result<Lease, Failure> {
        let issued = self.0.lifecycle.borrow().ticket();
        let action = self.0.lifecycle.borrow().access_action(
            expected_session,
            now(),
            !self.0.cleanup.borrow().is_empty(),
            self.0.authorizing.get(),
            self.0.refresh.is_pending(),
        );
        let action = match action {
            Ok(action) => action,
            Err(error) => {
                if error == Failure::SignedOut {
                    self.lose(issued)?;
                }
                return Err(error);
            }
        };
        let grant = self
            .0
            .lifecycle
            .borrow()
            .grant()
            .cloned()
            .ok_or(Failure::SignedOut)?;
        let future = if action == AccessAction::Use {
            None
        } else {
            let client = self.clone();
            Some(self.0.refresh.acquire(|| {
                async move {
                    let result: Result<TokenPair, Failure> = client
                        .json(
                            "/refresh",
                            &RefreshRequest {
                                refresh_token: grant.refresh.clone(),
                                session_id: grant.session.clone(),
                            },
                        )
                        .await;
                    if !client.0.lifecycle.borrow().accepts(issued) {
                        return Err(Failure::Changed);
                    }
                    let result = result
                        .and_then(|pair| Grant::admit(client.0.surface, pair, now()))
                        .and_then(|next| client.0.lifecycle.borrow_mut().rotate(issued, next));
                    if result.is_err() {
                        client.lose(issued)?;
                    }
                    result
                }
                .boxed_local()
            }))
        };
        if let Some(future) = future {
            let result = future.clone().await;
            self.0.refresh.finish(&future);
            if let Err(error) = result {
                if error != Failure::Changed {
                    let _ = self.cleanup().await;
                }
                return Err(error);
            }
        }
        let state = self.0.lifecycle.borrow();
        if state.ticket().intent != issued.intent {
            return Err(Failure::Changed);
        }
        let grant = state.grant().cloned().ok_or(Failure::SignedOut)?;
        if expected_session.is_some_and(|expected| expected != grant.session) {
            return Err(Failure::Changed);
        }
        Ok(Lease {
            client: self.clone(),
            ticket: state.ticket(),
            grant,
        })
    }
    /// Synchronous fence before the first await; all old effects lose admission immediately.
    pub fn invalidate(&self) -> Result<(), Failure> {
        let issued = self.0.lifecycle.borrow().ticket();
        self.abandon(issued)
    }
    pub fn cancel_authorization(&self) -> Result<(), Failure> {
        if !self.0.authorizing.get() {
            return Ok(());
        }
        if let Some(popup) = self.0.popup.borrow_mut().take() {
            let _ = popup.close();
        }
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
        self.cancel_authorization()?;
        self.invalidate()
    }
    pub async fn logout(&self, expected: Option<&str>) -> Result<(), Failure> {
        self.begin_logout(expected)?;
        self.cleanup().await
    }
    /// Cancellation includes the old authorization/cleanup owner settling. It cannot launch
    /// another flow while an earlier, possibly committed response is still being reconciled.
    pub async fn settled_cleanup(&self) -> Result<(), Failure> {
        self.wait_cleanup(true).await
    }
    async fn wait_cleanup(&self, authorization: bool) -> Result<(), Failure> {
        let deadline = Deadline::new()?;
        while authorization && self.0.authorizing.get() || self.0.cleaning.get() {
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
            let Some(next) = next else {
                return Ok(());
            };
            match next {
                Cleanup::Family { refresh, session } => {
                    self.unit(
                        "/revoke",
                        &RefreshRequest {
                            refresh_token: refresh,
                            session_id: session,
                        },
                    )
                    .await?
                }
                Cleanup::Flow {
                    id,
                    state,
                    verifier,
                } => {
                    self.unit(
                        &format!("/flows/{id}/cancel"),
                        &CancelRequest {
                            state,
                            code_verifier: verifier,
                        },
                    )
                    .await?
                }
            }
            self.0.cleanup.borrow_mut().remove(0);
        }
    }
    async fn json<T: DeserializeOwned, P: Serialize>(
        &self,
        suffix: &str,
        body: &P,
    ) -> Result<T, Failure> {
        let deadline = Deadline::new()?;
        let response = self.post(suffix, body, &deadline).await?;
        if response.status() != 200 && response.status() != 201 {
            return Err(classify(response.status()));
        }
        if !json_content(&response) {
            return Err(Failure::Protocol);
        }
        let bytes = response.binary().await.map_err(|_| Failure::Unavailable)?;
        if bytes.len() > 16_384 {
            return Err(Failure::Protocol);
        }
        serde_json::from_slice(&bytes).map_err(|_| Failure::Protocol)
    }
    async fn unit<P: Serialize>(&self, suffix: &str, body: &P) -> Result<(), Failure> {
        let deadline = Deadline::new()?;
        let response = self.post(suffix, body, &deadline).await?;
        if response.status() == 204 {
            Ok(())
        } else {
            Err(Failure::CleanupPending)
        }
    }
    async fn post<P: Serialize>(
        &self,
        suffix: &str,
        body: &P,
        deadline: &Deadline,
    ) -> Result<Response, Failure> {
        Request::post(&self.endpoint(suffix)?)
            .credentials(RequestCredentials::Omit)
            .redirect(RequestRedirect::Error)
            .header("Accept", "application/json")
            .abort_signal(Some(&deadline.controller.signal()))
            .json(body)
            .map_err(|_| Failure::Protocol)?
            .send()
            .await
            .map_err(|_| Failure::Unavailable)
    }
    /// Reserve synchronously from a trusted click. No async work precedes window.open.
    /// The opener retains drafts, expected actor, verifier and intent in RAM through issuer navigation.
    pub fn reserve_login(
        &self,
        expected_actor: Option<String>,
        reauthenticate: bool,
    ) -> Result<Login, Failure> {
        if self.0.authorizing.get() {
            return Err(Failure::Changed);
        }
        let popup = window()?
            .open_with_url_and_target_and_features(
                "about:blank",
                "_blank",
                "popup,width=520,height=720",
            )
            .map_err(|_| Failure::PopupBlocked)?
            .ok_or(Failure::PopupBlocked)?;
        self.invalidate()?;
        self.0.authorizing.set(true);
        *self.0.popup.borrow_mut() = Some(popup.clone());
        Ok(Login {
            client: self.clone(),
            popup,
            ticket: self.0.lifecycle.borrow().ticket(),
            expected_actor,
            reauthenticate,
        })
    }
}
fn classify(status: u16) -> Failure {
    if status == 401 {
        Failure::SignedOut
    } else if status == 409 {
        Failure::Changed
    } else {
        Failure::Unavailable
    }
}
fn json_content(response: &Response) -> bool {
    response.headers().get("content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    })
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
                15_000,
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
fn random() -> Result<String, Failure> {
    let mut bytes = [0u8; 32];
    window()?
        .crypto()
        .map_err(|_| Failure::Unavailable)?
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| Failure::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
async fn challenge(verifier: &str) -> Result<String, Failure> {
    let promise = window()?
        .crypto()
        .map_err(|_| Failure::Unavailable)?
        .subtle()
        .digest_with_str_and_u8_array("SHA-256", verifier.as_bytes())
        .map_err(|_| Failure::Unavailable)?;
    let digest = JsFuture::from(promise)
        .await
        .map_err(|_| Failure::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(js_sys::Uint8Array::new(&digest).to_vec()))
}
pub struct Login {
    client: Client,
    popup: Window,
    ticket: Ticket,
    expected_actor: Option<String>,
    reauthenticate: bool,
}
impl Drop for Login {
    fn drop(&mut self) {
        self.client.0.authorizing.set(false);
        self.client.0.popup.borrow_mut().take();
        let _ = self.popup.close();
    }
}
impl Login {
    pub async fn run(self) -> Result<(), Failure> {
        let result = self.run_inner().await;
        if result.is_err() {
            let _ = self.client.cleanup().await;
        }
        result
    }
    async fn run_inner(&self) -> Result<(), Failure> {
        self.client.wait_cleanup(false).await?;
        if !self.client.0.lifecycle.borrow().accepts(self.ticket) {
            return Err(Failure::Changed);
        }
        let state = random()?;
        let verifier = random()?;
        let nonce = random()?;
        let flow: Flow = self
            .client
            .json(
                "/flows",
                &FlowRequest {
                    state: state.clone(),
                    nonce,
                    code_challenge: challenge(&verifier).await?,
                    reauthenticate: self.reauthenticate,
                },
            )
            .await?;
        if uuid::Uuid::parse_str(&flow.flow_id).is_err() {
            return Err(Failure::Protocol);
        }
        self.client.0.cleanup.borrow_mut().push(Cleanup::Flow {
            id: flow.flow_id.clone(),
            state: state.clone(),
            verifier: verifier.clone(),
        });
        let issuer = url::Url::parse(&flow.authorization_url).map_err(|_| Failure::Protocol)?;
        if !(issuer.scheme() == "https"
            || issuer.scheme() == "http"
                && matches!(issuer.host_str(), Some("localhost" | "127.0.0.1")))
            || !issuer.username().is_empty()
            || issuer.password().is_some()
            || issuer.fragment().is_some()
        {
            return Err(Failure::Protocol);
        }
        let origin = window()?
            .location()
            .origin()
            .map_err(|_| Failure::Protocol)?;
        let protocol = PendingFlow {
            flow: flow.flow_id.clone(),
            state: state.clone(),
            verifier: verifier.clone(),
            created: now(),
            origin: origin.clone(),
            surface: name(self.client.0.surface).into(),
        };
        let storage = self
            .popup
            .session_storage()
            .map_err(|_| Failure::Unavailable)?
            .ok_or(Failure::Unavailable)?;
        storage
            .set_item(
                FLOW_KEY,
                &serde_json::to_string(&protocol).map_err(|_| Failure::Protocol)?,
            )
            .map_err(|_| Failure::Unavailable)?;
        let completion = wait_for_popup(&self.popup, &origin, &flow.authorization_url).await?;
        if !self.client.0.lifecycle.borrow().accepts(self.ticket) {
            return Err(Failure::Changed);
        }
        if completion.flow_id != flow.flow_id || completion.state != state {
            return Err(Failure::Protocol);
        }
        let Some(code) = completion.code.filter(|code| crate::core::entropy(code)) else {
            return Err(Failure::Cancelled);
        };
        if completion.error.is_some() {
            return Err(Failure::Protocol);
        }
        let pair: TokenPair = self
            .client
            .json(
                &format!("/flows/{}/exchange", flow.flow_id),
                &ExchangeRequest {
                    state,
                    code,
                    code_verifier: verifier,
                },
            )
            .await?;
        let grant = Grant::admit(self.client.0.surface, pair, now())?;
        // Cancel remains queued until validated adoption; unknown exchange never retries its code.
        self.client.0.lifecycle.borrow_mut().adopt(
            self.ticket,
            grant,
            self.expected_actor.as_deref(),
        )?;
        self.client
            .0
            .cleanup
            .borrow_mut()
            .retain(|entry| !matches!(entry,Cleanup::Flow{id,..} if id==&flow.flow_id));
        Ok(())
    }
}
struct PopupWait {
    window: Window,
    message: Closure<dyn FnMut(MessageEvent)>,
    _tick: Closure<dyn FnMut()>,
    timer: i32,
}
impl Drop for PopupWait {
    fn drop(&mut self) {
        let _ = self
            .window
            .remove_event_listener_with_callback("message", self.message.as_ref().unchecked_ref());
        self.window.clear_interval_with_handle(self.timer);
    }
}
async fn wait_for_popup(popup: &Window, origin: &str, url: &str) -> Result<Completion, Failure> {
    let window = window()?;
    let expected_popup = popup.clone();
    let expected_origin = origin.to_owned();
    let done = Rc::new(Cell::new(false));
    let message_done = done.clone();
    let tick_done = done.clone();
    let mut resolver = None;
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        resolver = Some(resolve);
    });
    let resolve = resolver.ok_or(Failure::Unavailable)?;
    let message_resolve = resolve.clone();
    let message = Closure::wrap(Box::new(move |event: MessageEvent| {
        if message_done.get()
            || event.origin() != expected_origin
            || !event
                .source()
                .is_some_and(|source| js_sys::Object::is(source.as_ref(), expected_popup.as_ref()))
        {
            return;
        }
        if let Some(value) = event.data().as_string().filter(|s| s.len() <= 2048) {
            message_done.set(true);
            let _ = message_resolve.call1(&JsValue::NULL, &JsValue::from_str(&value));
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    window
        .add_event_listener_with_callback("message", message.as_ref().unchecked_ref())
        .map_err(|_| Failure::Unavailable)?;
    let observed = popup.clone();
    let started = now();
    let tick = Closure::wrap(Box::new(move || {
        if !tick_done.get()
            && (observed.closed().unwrap_or(true) || now().saturating_sub(started) >= 300)
        {
            tick_done.set(true);
            let _ = resolve.call1(&JsValue::NULL, &JsValue::NULL);
        }
    }) as Box<dyn FnMut()>);
    let timer = match window
        .set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 250)
    {
        Ok(timer) => timer,
        Err(_) => {
            let _ = window
                .remove_event_listener_with_callback("message", message.as_ref().unchecked_ref());
            return Err(Failure::Unavailable);
        }
    };
    let _guard = PopupWait {
        window,
        message,
        _tick: tick,
        timer,
    };
    popup
        .location()
        .set_href(url)
        .map_err(|_| Failure::Unavailable)?;
    let value = JsFuture::from(promise)
        .await
        .map_err(|_| Failure::Unavailable)?;
    let value = value.as_string().ok_or(Failure::Cancelled)?;
    serde_json::from_str(&value).map_err(|_| Failure::Protocol)
}
/// Run before either product mounts. Scrub the handoff from history before other app effects.
/// Only the matching popup's same-origin opener receives a one-time code; credentials stay there.
/// Returns true for the completion document, including malformed/orphaned returns (fail closed).
pub fn complete_popup(surface: Surface, api_origin: Option<&str>) -> bool {
    let Ok(window) = window() else {
        return false;
    };
    if window.location().pathname().ok().as_deref() != Some(COMPLETE_PATH) {
        return false;
    }
    let query = window.location().search().unwrap_or_default();
    let scrubbed = window
        .history()
        .and_then(|history| history.replace_state_with_url(&JsValue::NULL, "", Some(COMPLETE_PATH)))
        .is_ok();
    let recovery = RefCell::new(None::<PendingFlow>);
    let result = (|| {
        if !scrubbed {
            return Err(Failure::Protocol);
        }
        let storage = window
            .session_storage()
            .map_err(|_| Failure::Unavailable)?
            .ok_or(Failure::Unavailable)?;
        let value = storage
            .get_item(FLOW_KEY)
            .map_err(|_| Failure::Unavailable)?
            .ok_or(Failure::Protocol)?;
        storage
            .remove_item(FLOW_KEY)
            .map_err(|_| Failure::Unavailable)?;
        if value.len() > 2048 {
            return Err(Failure::Protocol);
        }
        let protocol: PendingFlow = serde_json::from_str(&value).map_err(|_| Failure::Protocol)?;
        let origin = window.location().origin().map_err(|_| Failure::Protocol)?;
        let completion = protocol.complete(&query, &origin, name(surface), now())?;
        *recovery.borrow_mut() = Some(protocol);
        let opener = window.opener().map_err(|_| Failure::Cancelled)?;
        if opener.is_null() || opener.is_undefined() {
            return Err(Failure::Cancelled);
        }
        // WebIDL supplies a WindowProxy. instanceof Window is false across popup realms.
        // Exact targetOrigin and the opener's message source/flow checks remain authoritative.
        let opener: Window = opener.unchecked_into();
        if opener.closed().unwrap_or(true) {
            return Err(Failure::Cancelled);
        }
        opener
            .post_message(
                &JsValue::from_str(
                    &serde_json::to_string(&completion).map_err(|_| Failure::Protocol)?,
                ),
                &origin,
            )
            .map_err(|_| Failure::Unavailable)?;
        Ok(())
    })();
    if result.is_ok() {
        let _ = window.close();
    } else if let Some(protocol) = recovery.into_inner() {
        if let Ok(client) = client(surface, api_origin) {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = client
                    .unit(
                        &format!("/flows/{}/cancel", protocol.flow),
                        &CancelRequest {
                            state: protocol.state,
                            code_verifier: protocol.verifier,
                        },
                    )
                    .await;
            });
        }
    }
    // The app's localized completion view remains if close is blocked or the opener was lost.
    true
}
