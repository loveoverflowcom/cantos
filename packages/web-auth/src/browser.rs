//! Browser effects for Identity v1 cookie sessions. The HttpOnly credential is owned by the
//! browser: this module never reads, stores or forwards it, and keeps only nonsecret session
//! facts (actor, session id, deadlines) in this page's RAM. Every request here carries
//! `credentials: include` and the session fence; the browser adds `Origin` itself.
//!
//! All decisions live in [`crate::owner`]; this module is the browser [`Port`]: clock, one
//! `fetch` attempt with a deadline, the cross-tab Web Lock that orders cookie-changing
//! exchanges, a `BroadcastChannel` that tells other tabs a cookie changed, and the view event.
use crate::{
    core::session_header,
    owner::{self, Method, Port, Reply, Request, MAX_BODY},
    Failure, Surface,
};
use cantos_contracts::web_api::api_url;
use gloo_net::http::{Request as Fetch, RequestBuilder};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, BroadcastChannel, RequestCredentials, RequestRedirect, Window};

const HTTP_DEADLINE_MS: i32 = 15_000;

fn window() -> Result<Window, Failure> {
    web_sys::window().ok_or(Failure::Unavailable)
}
pub fn invalidation_event(surface: Surface) -> &'static str {
    match surface {
        Surface::Studio => "cantos-studio-session-invalidated",
        Surface::Theatre => "cantos-theatre-session-invalidated",
    }
}
fn channel_name(surface: Surface) -> &'static str {
    match surface {
        Surface::Studio => "cantos-studio-identity-cookie",
        Surface::Theatre => "cantos-theatre-identity-cookie",
    }
}

pub struct BrowserPort {
    channel: Option<BroadcastChannel>,
}

/// What the browser's lock callback and the waiting owner share. Whichever of "granted" and
/// "dropped" happens second releases the lock, so a waiter cancelled between the grant and its
/// next poll cannot keep the cross-tab lock for the rest of the document's life.
#[derive(Default)]
struct Grant {
    release: Option<js_sys::Function>,
    dropped: bool,
}

/// Held while this tab owns the cookie exchange; the Web Lock is released on drop.
pub struct TabLock(Rc<RefCell<Grant>>);
impl Drop for TabLock {
    fn drop(&mut self) {
        let release = {
            let mut grant = self.0.borrow_mut();
            grant.dropped = true;
            grant.release.take()
        };
        if let Some(release) = release {
            let _ = release.call0(&JsValue::UNDEFINED);
        }
    }
}

impl Port for BrowserPort {
    type TabLock = TabLock;

    fn clock(&self) -> (u64, u64) {
        let wall = (js_sys::Date::now() / 1000.0) as u64;
        let elapsed = web_sys::window()
            .and_then(|window| window.performance())
            .map_or(0, |performance| (performance.now() / 1000.0) as u64);
        (wall, elapsed)
    }

    async fn send(&self, surface: Surface, request: Request) -> Result<Reply, Failure> {
        let deadline = Deadline::new()?;
        let builder = match request.method {
            Method::Get => Fetch::get(&request.url),
            Method::Post => Fetch::post(&request.url),
            Method::Delete => Fetch::delete(&request.url),
        }
        .credentials(RequestCredentials::Include)
        .redirect(RequestRedirect::Error)
        .header("Accept", "application/json")
        .abort_signal(Some(&deadline.controller.signal()));
        let builder = match request.fence.as_deref() {
            Some(fence) => builder.header(session_header(surface), fence),
            None => builder,
        };
        let built = match request.body {
            Some(body) => builder
                .header("Content-Type", "application/json")
                .body(body),
            None => builder.build(),
        }
        .map_err(|_| Failure::Protocol)?;
        let response = built.send().await.map_err(|_| Failure::Unavailable)?;
        let status = response.status();
        let content_type = response.headers().get("content-type");
        // Readable cross-origin only when the API exposes it; the owner keeps a conservative
        // default otherwise.
        let retry_after = response.headers().get("retry-after");
        // Only a session answer has a body this owner reads; error bodies stay unread.
        let body = if status == 200 {
            let bytes = response.binary().await.map_err(|_| Failure::Unavailable)?;
            if bytes.len() > MAX_BODY {
                return Err(Failure::Protocol);
            }
            bytes
        } else {
            Vec::new()
        };
        drop(deadline);
        Ok(Reply {
            status,
            content_type,
            retry_after,
            body,
        })
    }

    async fn tab_lock(&self, surface: Surface, origin: Option<&str>) -> Result<TabLock, Failure> {
        let name = format!("{}:{}", channel_name(surface), origin.unwrap_or(""));
        acquire_tab_lock(&name).await
    }

    async fn pause(&self, millis: u32) -> Result<(), Failure> {
        let window = window()?;
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            if window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &resolve,
                    millis.min(i32::MAX as u32) as i32,
                )
                .is_err()
            {
                let _ = reject.call0(&JsValue::NULL);
            }
        });
        JsFuture::from(promise)
            .await
            .map(|_| ())
            .map_err(|_| Failure::Unavailable)
    }

    fn lost(&self, surface: Surface) {
        if let (Ok(window), Ok(event)) =
            (window(), web_sys::Event::new(invalidation_event(surface)))
        {
            let _ = window.dispatch_event(&event);
        }
    }

    fn announce(&self, _surface: Surface) {
        if let Some(channel) = &self.channel {
            let _ = channel.post_message(&JsValue::from_str("cookie-changed"));
        }
    }
}

/// `navigator.locks.request(name, {mode: "exclusive", signal}, callback)` through reflection
/// (the typed binding is unstable in `web-sys`). A browser without Web Locks, or one that
/// throws while we set the request up, yields a no-op lock: ordering then holds inside this
/// document only. A lock another tab never releases within the deadline is `Unavailable`.
/// Dropping the returned future before the grant aborts the request (the deadline's drop); a
/// grant that lands after the drop is released at once (see [`Grant`]).
async fn acquire_tab_lock(name: &str) -> Result<TabLock, Failure> {
    let none = || Ok(TabLock(Rc::default()));
    let Some(window) = web_sys::window() else {
        return none();
    };
    let get = |target: &JsValue, key: &str| {
        js_sys::Reflect::get(target, &JsValue::from_str(key))
            .ok()
            .filter(|value| !value.is_undefined() && !value.is_null())
    };
    let navigator = JsValue::from(window.navigator());
    let Some(locks) = get(&navigator, "locks") else {
        return none();
    };
    let Some(request) = get(&locks, "request").and_then(|f| f.dyn_into::<js_sys::Function>().ok())
    else {
        return none();
    };
    let deadline = Deadline::new()?;
    let options = js_sys::Object::new();
    let mut ready = true;
    ready &= js_sys::Reflect::set(
        &options,
        &JsValue::from_str("mode"),
        &JsValue::from_str("exclusive"),
    )
    .is_ok();
    ready &= js_sys::Reflect::set(
        &options,
        &JsValue::from_str("signal"),
        &deadline.controller.signal(),
    )
    .is_ok();
    if !ready {
        return none();
    }
    // Owned before the first await: dropping this future drops it, and with it any grant.
    let lock = TabLock(Rc::default());
    let mut failed_setup = false;
    let granted = js_sys::Promise::new(&mut |resolve, reject| {
        let slot = lock.0.clone();
        let on_lock = Closure::once_into_js(move |_lock: JsValue| -> js_sys::Promise {
            // The returned promise keeps the lock until `release` is called.
            let held = js_sys::Promise::new(&mut |release_fn, _| {
                let abandoned = {
                    let mut grant = slot.borrow_mut();
                    if !grant.dropped {
                        grant.release = Some(release_fn.clone());
                    }
                    grant.dropped
                };
                if abandoned {
                    let _ = release_fn.call0(&JsValue::UNDEFINED);
                }
            });
            let _ = resolve.call0(&JsValue::UNDEFINED);
            held
        });
        match request.call3(&locks, &JsValue::from_str(name), &options, &on_lock) {
            Ok(pending) => {
                // An abort (deadline) rejects the request promise before the lock is granted.
                if let Ok(pending) = pending.dyn_into::<js_sys::Promise>() {
                    let on_reject = Closure::once(move |error: JsValue| {
                        let _ = reject.call1(&JsValue::UNDEFINED, &error);
                    });
                    let _ = pending.catch(&on_reject);
                    on_reject.forget();
                }
            }
            Err(_) => failed_setup = true,
        }
    });
    if failed_setup {
        return none();
    }
    JsFuture::from(granted)
        .await
        .map_err(|_| Failure::Unavailable)?;
    drop(deadline);
    Ok(lock)
}

pub type Client = owner::Client<BrowserPort>;
pub type Lease = owner::Lease<BrowserPort>;

impl Lease {
    /// The one place a protected fetch gets its credential policy: ambient cookie, the
    /// session fence, no redirects. Never use this for public, producer or foreign-origin URLs.
    pub fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .credentials(RequestCredentials::Include)
            .redirect(RequestRedirect::Error)
            .header(session_header(self.surface()), self.session())
    }
    /// [`owner::Lease::response`] for a `fetch` answer: this package reads the status,
    /// `Retry-After` and this surface's session echo itself, so a caller cannot pass the wrong
    /// header or forget one. Call it before the body can change any state.
    pub fn receive(&self, response: &gloo_net::http::Response) -> Result<(), Failure> {
        let headers = response.headers();
        self.response(
            response.status(),
            headers.get("retry-after").as_deref(),
            headers.get(session_header(self.surface())).as_deref(),
        )
    }
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
            if client.origin() != origin {
                return Err(Failure::Protocol);
            }
            return Ok(client.clone());
        }
        // One channel object both posts and listens: a channel never hears its own posts, while a
        // second object of the same name in this document would, and withdraw its own session.
        let channel = BroadcastChannel::new(channel_name(surface)).ok();
        let client = Client::new(
            BrowserPort {
                channel: channel.clone(),
            },
            surface,
            origin,
        )?;
        if let Some(channel) = channel {
            listen(&client, &channel);
        }
        *slot = Some(client.clone());
        Ok(client)
    };
    match surface {
        Surface::Studio => STUDIO.with(get),
        Surface::Theatre => THEATRE.with(get),
    }
}

/// Another tab's cookie change is a signal to withdraw local authority, nothing more: the
/// views then re-read the session from the API.
fn listen(client: &Client, channel: &BroadcastChannel) {
    let owner = client.clone();
    let on_message = Closure::wrap(Box::new(move |_: web_sys::MessageEvent| {
        owner.external_change();
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    channel.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // Document-lifetime singleton per surface.
    on_message.forget();
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
