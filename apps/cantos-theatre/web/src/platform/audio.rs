//! The one browser audio element and the thin shell around it. Every decision (grants, retry,
//! which element fact becomes which player event) lives in the pure [`MediaDriver`]; this file
//! only performs its commands and reports what happened. There is exactly one
//! `HTMLAudioElement`, created on the first attachment and kept for the page's life, so
//! navigation, mini-player and full-player share a single source. A replaced source drops its
//! listeners, aborts its requests and clears its timers, and facts carry the attachment they
//! came from, so a callback of a cancelled source cannot change the current one.
use crate::{
    application::{
        media_driver::{
            AttachId, Command, ElementEvent, Input, MediaDriver, MediaFault, PlayRejection, Ticket,
        },
        player_session::PlayerSession,
        ports::MediaPort,
    },
    core::playback::{Effect, Event},
    live::public_media,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use web_sys::{
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    AbortController, HtmlAudioElement,
};

const ELEMENT_ID: &str = "theatre-audio-engine";
/// The element is moved only when it is farther than this from the player's position.
const SYNC_TOLERANCE_MS: f64 = 1_000.0;
const FACTS: [&str; 8] = [
    "loadedmetadata",
    "playing",
    "pause",
    "waiting",
    "timeupdate",
    "seeked",
    "ended",
    "error",
];

fn now_ms() -> i64 {
    web_sys::js_sys::Date::now() as i64
}

struct Listener {
    name: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
}

/// One URL attached to the element, with the listeners that report on it.
struct Binding {
    element: HtmlAudioElement,
    listeners: Vec<Listener>,
}

impl Drop for Binding {
    fn drop(&mut self) {
        for listener in &self.listeners {
            let _ = self.element.remove_event_listener_with_callback(
                listener.name,
                listener.closure.as_ref().unchecked_ref(),
            );
        }
    }
}

struct Timer {
    ticket: Ticket,
    handle: i32,
    fired: Rc<Cell<bool>>,
    // Kept alive until the next prune: a closure must not be dropped while it runs.
    _closure: Closure<dyn FnMut()>,
}

#[derive(Default)]
struct Resources {
    binding: Option<Binding>,
    current: Option<AttachId>,
    requests: Vec<(Ticket, AbortController)>,
    timers: Vec<Timer>,
}

struct Shell {
    session: PlayerSession,
    driver: RefCell<MediaDriver>,
    element: RefCell<Option<HtmlAudioElement>>,
    resources: RefCell<Resources>,
}

pub struct BrowserAudio {
    shell: Rc<Shell>,
}

impl BrowserAudio {
    pub fn new(session: PlayerSession) -> Self {
        Self {
            shell: Rc::new(Shell {
                session,
                driver: RefCell::new(MediaDriver::new()),
                element: RefCell::new(None),
                resources: RefCell::new(Resources::default()),
            }),
        }
    }
}

impl MediaPort for BrowserAudio {
    fn execute(&mut self, effect: &Effect) -> Vec<Event> {
        self.shell.input(Input::Engine(Box::new(effect.clone())))
    }
}

impl Drop for BrowserAudio {
    fn drop(&mut self) {
        self.shell.release();
        if let Some(element) = self.shell.element.borrow_mut().take() {
            element.remove();
        }
    }
}

fn fault_of(element: &HtmlAudioElement) -> Option<MediaFault> {
    // MediaError codes: 1 aborted, 2 network, 3 decode, 4 source not supported.
    match element.error().map(|error| error.code()) {
        Some(2) => Some(MediaFault::Network),
        Some(3) => Some(MediaFault::Decode),
        Some(4) | None => Some(MediaFault::Unsupported),
        Some(_) => None,
    }
}

fn position_of(element: &HtmlAudioElement) -> u32 {
    let seconds = element.current_time();
    if seconds.is_finite() && seconds > 0.0 {
        (seconds * 1000.0) as u32
    } else {
        0
    }
}

fn rejection_of(error: &JsValue) -> PlayRejection {
    match error
        .dyn_ref::<web_sys::DomException>()
        .map(web_sys::DomException::name)
        .as_deref()
    {
        Some("NotAllowedError") => PlayRejection::NotAllowed,
        Some("AbortError") => PlayRejection::Aborted,
        Some("NotSupportedError") => PlayRejection::NotSupported,
        _ => PlayRejection::Other,
    }
}

impl Shell {
    /// Step the driver, run its commands and return the player events it produced.
    fn input(self: &Rc<Self>, input: Input) -> Vec<Event> {
        let commands = self.driver.borrow_mut().step(now_ms(), input);
        let mut events = Vec::new();
        for command in commands {
            match command {
                Command::Emit(event) => events.push(event),
                other => self.run(other),
            }
        }
        events
    }

    /// Feed an asynchronous outcome back and deliver the events it produced.
    fn deliver(self: &Rc<Self>, input: Input) {
        for event in self.input(input) {
            self.session.dispatch(event);
        }
    }

    fn element(&self) -> Option<HtmlAudioElement> {
        if let Some(element) = self.element.borrow().clone() {
            return Some(element);
        }
        let document = web_sys::window()?.document()?;
        let element = match document.get_element_by_id(ELEMENT_ID) {
            Some(existing) => existing.dyn_into::<HtmlAudioElement>().ok()?,
            None => {
                let created = document
                    .create_element("audio")
                    .ok()?
                    .dyn_into::<HtmlAudioElement>()
                    .ok()?;
                created.set_id(ELEMENT_ID);
                created.set_hidden(true);
                let _ = created.set_attribute("aria-hidden", "true");
                document.body()?.append_child(&created).ok()?;
                created
            }
        };
        element.set_preload("auto");
        *self.element.borrow_mut() = Some(element.clone());
        Some(element)
    }

    fn run(self: &Rc<Self>, command: Command) {
        match command {
            Command::Authorize {
                ticket,
                publication,
            } => self.authorize(ticket, publication),
            Command::Wait { ticket, delay_ms } => self.wait(ticket, delay_ms),
            Command::Attach {
                attach,
                url,
                start_ms,
                play,
                speed_permille,
            } => self.attach(attach, url.as_str(), start_ms, play, speed_permille),
            Command::Play { from_ms } => {
                if let Some(element) = self.element.borrow().clone() {
                    if (element.current_time() * 1000.0 - f64::from(from_ms)).abs()
                        > SYNC_TOLERANCE_MS
                    {
                        element.set_current_time(f64::from(from_ms) / 1000.0);
                    }
                    self.play(&element);
                }
            }
            Command::Pause => {
                if let Some(element) = self.element.borrow().as_ref() {
                    let _ = element.pause();
                }
            }
            Command::Seek { to_ms } => {
                if let Some(element) = self.element.borrow().as_ref() {
                    element.set_current_time(f64::from(to_ms) / 1000.0);
                }
            }
            Command::Speed { permille } => {
                if let Some(element) = self.element.borrow().as_ref() {
                    let rate = f64::from(permille) / 1000.0;
                    element.set_default_playback_rate(rate);
                    element.set_playback_rate(rate);
                }
            }
            Command::Release => self.release(),
            Command::Emit(_) => {}
        }
    }

    fn authorize(self: &Rc<Self>, ticket: Ticket, publication: uuid::Uuid) {
        let Ok(controller) = AbortController::new() else {
            let shell = self.clone();
            leptos::task::spawn_local(async move {
                shell.deliver(Input::Authorized {
                    ticket,
                    result: Err(crate::application::media_driver::AuthorizationError::Unavailable),
                });
            });
            return;
        };
        self.resources
            .borrow_mut()
            .requests
            .push((ticket, controller.clone()));
        let weak = Rc::downgrade(self);
        leptos::task::spawn_local(async move {
            let result = public_media::authorize(publication, controller).await;
            let Some(shell) = weak.upgrade() else {
                return;
            };
            shell
                .resources
                .borrow_mut()
                .requests
                .retain(|(pending, _)| *pending != ticket);
            shell.deliver(Input::Authorized { ticket, result });
        });
    }

    fn wait(self: &Rc<Self>, ticket: Ticket, delay_ms: u32) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let fired = Rc::new(Cell::new(false));
        let weak = Rc::downgrade(self);
        let flag = fired.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            flag.set(true);
            if let Some(shell) = weak.upgrade() {
                shell.deliver(Input::Timer(ticket));
            }
        });
        let Ok(handle) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            i32::try_from(delay_ms).unwrap_or(i32::MAX),
        ) else {
            return;
        };
        let mut resources = self.resources.borrow_mut();
        resources.timers.retain(|timer| !timer.fired.get());
        resources.timers.push(Timer {
            ticket,
            handle,
            fired,
            _closure: closure,
        });
    }

    fn attach(
        self: &Rc<Self>,
        attach: AttachId,
        url: &str,
        start_ms: u32,
        play: bool,
        speed_permille: u16,
    ) {
        // Drop the previous source's listeners before the new one can raise any fact.
        self.resources.borrow_mut().binding = None;
        let Some(element) = self.element() else {
            let shell = self.clone();
            leptos::task::spawn_local(async move {
                shell.deliver(Input::Element {
                    attach,
                    event: ElementEvent::Error(MediaFault::Decode),
                });
            });
            return;
        };
        let listeners = FACTS
            .into_iter()
            .map(|name| {
                let weak = Rc::downgrade(self);
                let source = element.clone();
                let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                    let Some(shell) = weak.upgrade() else {
                        return;
                    };
                    let event = match name {
                        "loadedmetadata" => Some(ElementEvent::Loaded),
                        "playing" => Some(ElementEvent::Playing),
                        "pause" => Some(ElementEvent::Paused),
                        "waiting" => Some(ElementEvent::Waiting),
                        "timeupdate" => Some(ElementEvent::Position(position_of(&source))),
                        "seeked" => Some(ElementEvent::Seeked(position_of(&source))),
                        "ended" => Some(ElementEvent::Ended),
                        _ => fault_of(&source).map(ElementEvent::Error),
                    };
                    if let Some(event) = event {
                        shell.deliver(Input::Element { attach, event });
                    }
                });
                let _ = element
                    .add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
                Listener { name, closure }
            })
            .collect();
        {
            let mut resources = self.resources.borrow_mut();
            resources.current = Some(attach);
            resources.binding = Some(Binding {
                element: element.clone(),
                listeners,
            });
        }
        // A browser without Ogg/Opus support is a format failure, not something to retry.
        if element.can_play_type("audio/ogg; codecs=opus").is_empty() {
            let shell = self.clone();
            leptos::task::spawn_local(async move {
                shell.deliver(Input::Element {
                    attach,
                    event: ElementEvent::Error(MediaFault::Decode),
                });
            });
            return;
        }
        let rate = f64::from(speed_permille) / 1000.0;
        element.set_src(url);
        element.set_default_playback_rate(rate);
        element.set_playback_rate(rate);
        if start_ms > 0 {
            // Before metadata this records the default start position the load will honour.
            element.set_current_time(f64::from(start_ms) / 1000.0);
        }
        if play {
            self.play(&element);
        }
    }

    fn play(self: &Rc<Self>, element: &HtmlAudioElement) {
        let Some(attach) = self.resources.borrow().current else {
            return;
        };
        let weak = Rc::downgrade(self);
        let reject = move |rejection| {
            if let Some(shell) = weak.upgrade() {
                shell.deliver(Input::Element {
                    attach,
                    event: ElementEvent::PlayRejected(rejection),
                });
            }
        };
        match element.play() {
            Ok(promise) => {
                let on_error = Closure::<dyn FnMut(JsValue)>::new(move |error: JsValue| {
                    reject(rejection_of(&error));
                });
                let _ = promise.catch(&on_error);
                // The promise may settle after any later source change; the closure must outlive
                // it. It holds only a weak shell handle and the attachment it belongs to.
                on_error.forget();
            }
            Err(error) => reject(rejection_of(&error)),
        }
    }

    /// Stop the element and drop every source, listener, request and timer.
    fn release(&self) {
        let mut resources = self.resources.borrow_mut();
        resources.binding = None;
        resources.current = None;
        for (_, controller) in resources.requests.drain(..) {
            controller.abort();
        }
        if let Some(window) = web_sys::window() {
            for timer in resources.timers.drain(..) {
                window.clear_timeout_with_handle(timer.handle);
                let _ = timer.ticket;
            }
        }
        drop(resources);
        if let Some(element) = self.element.borrow().as_ref() {
            let _ = element.pause();
            let _ = element.remove_attribute("src");
            element.load();
        }
    }
}
