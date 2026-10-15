//! Bounded observations of trusted input on the real gallery specimen.
//!
//! This observer never dispatches an input, changes a class or drives an animation.

use leptos::{html, prelude::*};

/// Observe one mounted specimen. Native string rendering has no browser resources.
pub fn bind(frame: NodeRef<html::Section>, output: RwSignal<Vec<String>>, scope: Signal<String>) {
    #[cfg(target_arch = "wasm32")]
    browser::bind(frame, output, scope);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (frame, output, scope);
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use leptos::{
        wasm_bindgen::{closure::Closure, JsCast},
        web_sys::{self, Document, Element, Event, EventTarget, KeyboardEvent, Window},
    };
    use std::{cell::RefCell, rc::Rc};

    /// End at the first frame on or after this cap, including long CSS transitions.
    const MAX_SAMPLE_DURATION_MS: f64 = 600.0;
    // Sparse frame observations leave room for this run's start, middle and end in the bound.
    const PROFILE_FRAME_INTERVAL_MS: f64 = 33.0;
    const MAX_PROFILE_LINES: usize = 24;

    #[derive(Clone, Copy, Debug)]
    enum ProbeError {
        BrowserUnavailable,
        ListenerRejected,
        FrameRejected,
        StyleUnavailable,
    }

    impl std::fmt::Display for ProbeError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(match self {
                Self::BrowserUnavailable => "browser unavailable",
                Self::ListenerRejected => "browser rejected a diagnostic listener",
                Self::FrameRejected => "browser rejected the diagnostic frame request",
                Self::StyleUnavailable => "computed style unavailable",
            })
        }
    }

    struct Listener {
        target: EventTarget,
        name: &'static str,
        capture: bool,
        callback: Closure<dyn FnMut(Event)>,
    }

    impl Listener {
        fn new(
            target: &EventTarget,
            name: &'static str,
            capture: bool,
            callback: impl FnMut(Event) + 'static,
        ) -> Result<Self, ProbeError> {
            let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
            target
                .add_event_listener_with_callback_and_bool(
                    name,
                    callback.as_ref().unchecked_ref(),
                    capture,
                )
                .map_err(|_| ProbeError::ListenerRejected)?;
            Ok(Self {
                target: target.clone(),
                name,
                capture,
                callback,
            })
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = self.target.remove_event_listener_with_callback_and_bool(
                self.name,
                self.callback.as_ref().unchecked_ref(),
                self.capture,
            );
        }
    }

    struct Interaction {
        target: Element,
        input: &'static str,
        sequence: u64,
        started_at_ms: f64,
        duration_ms: f64,
        last_profile_at_ms: f64,
        middle_observed: bool,
    }

    struct Sampler {
        window: Window,
        document: Document,
        frame: Element,
        output: RwSignal<Vec<String>>,
        sequence: u64,
        active: Option<Interaction>,
        pending_frame: Option<i32>,
        callback: Option<Closure<dyn FnMut(f64)>>,
    }

    fn push_line(output: RwSignal<Vec<String>>, line: String) {
        let _ = output.try_update(|lines| {
            lines.push(line);
            if lines.len() > MAX_PROFILE_LINES {
                lines.drain(..lines.len() - MAX_PROFILE_LINES);
            }
        });
    }

    fn css_duration_ms(value: &str) -> f64 {
        value
            .split(',')
            .filter_map(|part| {
                let part = part.trim();
                if let Some(number) = part.strip_suffix("ms") {
                    number.parse::<f64>().ok()
                } else {
                    part.strip_suffix('s')
                        .and_then(|number| number.parse::<f64>().ok())
                        .map(|seconds| seconds * 1000.0)
                }
            })
            .filter(|duration| duration.is_finite() && *duration >= 0.0)
            .fold(0.0, f64::max)
    }

    impl Sampler {
        fn visible(&self, target: &Element) -> bool {
            if self.document.hidden() || !target.is_connected() || !self.frame.is_connected() {
                return false;
            }
            let bounds = target.get_bounding_client_rect();
            let width = self
                .window
                .inner_width()
                .ok()
                .and_then(|value| value.as_f64());
            let height = self
                .window
                .inner_height()
                .ok()
                .and_then(|value| value.as_f64());
            match (width, height) {
                (Some(width), Some(height)) => {
                    bounds.width() > 0.0
                        && bounds.height() > 0.0
                        && bounds.right() > 0.0
                        && bounds.bottom() > 0.0
                        && bounds.left() < width
                        && bounds.top() < height
                }
                _ => false,
            }
        }

        fn now(&self) -> f64 {
            self.window
                .performance()
                .map(|performance| performance.now())
                .unwrap_or(0.0)
        }

        fn record(&self, phase: &str, timestamp_ms: f64) {
            let Some(active) = &self.active else {
                return;
            };
            let elapsed = (timestamp_ms - active.started_at_ms).max(0.0);
            let bounds = active.target.get_bounding_client_rect();
            let style = self
                .window
                .get_computed_style(&active.target)
                .ok()
                .flatten();
            let property = |name: &str| {
                style
                    .as_ref()
                    .and_then(|style| style.get_property_value(name).ok())
                    .unwrap_or_else(|| "unavailable".to_owned())
            };
            push_line(self.output, format!(
                "#{} {} {phase}: t={timestamp_ms:.2}ms elapsed={elapsed:.2}ms css-window={:.2}ms target={} variant={} state={} aria-pressed={} bounds=({:.2},{:.2},{:.2},{:.2}) radius={} transform={} opacity={} background={} color={} transition-property={} transition-duration={} transition-delay={} transition-timing={}",
                active.sequence,
                active.input,
                active.duration_ms,
                active.target.id(),
                active.target.get_attribute("data-variant").unwrap_or_default(),
                active.target.get_attribute("data-state").unwrap_or_default(),
                active.target.get_attribute("aria-pressed").unwrap_or_default(),
                bounds.x(), bounds.y(), bounds.width(), bounds.height(),
                property("border-radius"), property("transform"), property("opacity"),
                property("background-color"), property("color"),
                property("transition-property"), property("transition-duration"),
                property("transition-delay"), property("transition-timing-function"),
            ));
        }

        fn cancel(&mut self, reason: &str) {
            if let Some(request) = self.pending_frame.take() {
                let _ = self.window.cancel_animation_frame(request);
            }
            if self.active.is_some() {
                self.record(reason, self.now());
                self.active = None;
            }
        }

        fn schedule(&mut self) -> Result<(), ProbeError> {
            let callback = self.callback.as_ref().ok_or(ProbeError::FrameRejected)?;
            let request = self
                .window
                .request_animation_frame(callback.as_ref().unchecked_ref())
                .map_err(|_| ProbeError::FrameRejected)?;
            self.pending_frame = Some(request);
            Ok(())
        }

        fn start(&mut self, target: Element, input: &'static str) -> Result<(), ProbeError> {
            self.cancel(&format!("interrupted-by-{input}"));
            if !self.visible(&target) {
                return Ok(());
            }
            let style = self
                .window
                .get_computed_style(&target)
                .map_err(|_| ProbeError::StyleUnavailable)?
                .ok_or(ProbeError::StyleUnavailable)?;
            let duration = style
                .get_property_value("transition-duration")
                .map_err(|_| ProbeError::StyleUnavailable)?;
            let delay = style
                .get_property_value("transition-delay")
                .map_err(|_| ProbeError::StyleUnavailable)?;
            self.sequence += 1;
            let timestamp = self.now();
            self.active = Some(Interaction {
                target,
                input,
                sequence: self.sequence,
                started_at_ms: timestamp,
                duration_ms: (css_duration_ms(&duration) + css_duration_ms(&delay))
                    .min(MAX_SAMPLE_DURATION_MS),
                last_profile_at_ms: 0.0,
                middle_observed: false,
            });
            self.record("start-trusted-input", timestamp);
            self.schedule()
        }

        fn tick(&mut self, _frame_timestamp_ms: f64) {
            // RAF supplies the frame origin, which can precede a newly registered input.
            // Attribute style observations to the actual sampling time instead.
            let timestamp_ms = self.now();
            self.pending_frame = None;
            let Some(active) = self.active.as_ref() else {
                return;
            };
            if !self.visible(&active.target) {
                self.cancel("cancelled-hidden-offscreen-or-unmounted");
                return;
            }
            let elapsed = (timestamp_ms - active.started_at_ms).max(0.0);
            let duration = active.duration_ms;
            if elapsed >= duration {
                // Instant transitions and a stalled first frame cannot establish a middle frame.
                if !active.middle_observed {
                    self.record("middle-unavailable-before-next-frame", timestamp_ms);
                }
                self.record("end", timestamp_ms);
                self.active = None;
                return;
            }
            if !active.middle_observed && elapsed >= duration / 2.0 {
                self.record("middle", timestamp_ms);
                if let Some(active) = self.active.as_mut() {
                    active.middle_observed = true;
                    active.last_profile_at_ms = elapsed;
                }
            } else if elapsed - active.last_profile_at_ms >= PROFILE_FRAME_INTERVAL_MS {
                self.record("frame", timestamp_ms);
                if let Some(active) = self.active.as_mut() {
                    active.last_profile_at_ms = elapsed;
                }
            }
            if let Err(error) = self.schedule() {
                push_line(self.output, format!("Motion observation stopped: {error}"));
                self.cancel("cancelled-frame-request");
            }
        }
    }

    impl Drop for Sampler {
        fn drop(&mut self) {
            if let Some(request) = self.pending_frame.take() {
                let _ = self.window.cancel_animation_frame(request);
            }
        }
    }

    struct Probe {
        sampler: Rc<RefCell<Sampler>>,
        listeners: Vec<Listener>,
    }

    impl Probe {
        fn new(frame: Element, output: RwSignal<Vec<String>>) -> Result<Self, ProbeError> {
            let window = web_sys::window().ok_or(ProbeError::BrowserUnavailable)?;
            let document = window.document().ok_or(ProbeError::BrowserUnavailable)?;
            let sampler = Rc::new(RefCell::new(Sampler {
                window: window.clone(),
                document: document.clone(),
                frame: frame.clone(),
                output,
                sequence: 0,
                active: None,
                pending_frame: None,
                callback: None,
            }));
            let weak = Rc::downgrade(&sampler);
            sampler.borrow_mut().callback = Some(Closure::wrap(Box::new(move |timestamp: f64| {
                if let Some(sampler) = weak.upgrade() {
                    sampler.borrow_mut().tick(timestamp);
                }
            })
                as Box<dyn FnMut(f64)>));

            let mut listeners = Vec::new();
            for input in [
                "pointerdown",
                "pointerup",
                "pointercancel",
                "keydown",
                "keyup",
            ] {
                let weak = Rc::downgrade(&sampler);
                listeners.push(Listener::new(frame.as_ref(), input, false, move |event| {
                    if !event.is_trusted() {
                        return;
                    }
                    let interaction_input = if input.starts_with("key") {
                        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                            return;
                        };
                        if key.is_composing()
                            || key.repeat()
                            || !matches!(key.key().as_str(), " " | "Enter")
                        {
                            return;
                        }
                        match (input, key.key().as_str()) {
                            ("keydown", " ") => "keydown-space",
                            ("keyup", " ") => "keyup-space",
                            ("keydown", _) => "keydown-enter",
                            _ => "keyup-enter",
                        }
                    } else {
                        input
                    };
                    let Some(target) = event
                        .target()
                        .and_then(|target| target.dyn_into::<Element>().ok())
                    else {
                        return;
                    };
                    let Some(target) = target
                        .closest(".cui-button, .cui-filter-chip")
                        .ok()
                        .flatten()
                    else {
                        return;
                    };
                    if let Some(sampler) = weak.upgrade() {
                        let mut sampler = sampler.borrow_mut();
                        if !sampler.frame.contains(Some(&target)) {
                            return;
                        }
                        if input == "pointercancel" {
                            sampler.cancel("cancelled-trusted-pointercancel");
                        } else if let Err(error) = sampler.start(target, interaction_input) {
                            push_line(output, format!("Motion observation unavailable: {error}"));
                            sampler.cancel("cancelled-start-failure");
                        }
                    }
                })?);
            }
            let weak = Rc::downgrade(&sampler);
            listeners.push(Listener::new(
                document.as_ref(),
                "visibilitychange",
                false,
                move |_| {
                    if let Some(sampler) = weak.upgrade() {
                        let mut sampler = sampler.borrow_mut();
                        if sampler.document.hidden() {
                            sampler.cancel("cancelled-document-hidden");
                        }
                    }
                },
            )?);
            for name in ["scroll", "resize"] {
                let weak = Rc::downgrade(&sampler);
                listeners.push(Listener::new(window.as_ref(), name, true, move |_| {
                    if let Some(sampler) = weak.upgrade() {
                        let mut sampler = sampler.borrow_mut();
                        if sampler
                            .active
                            .as_ref()
                            .is_some_and(|active| !sampler.visible(&active.target))
                        {
                            sampler.cancel("cancelled-offscreen");
                        }
                    }
                })?);
            }
            Ok(Self { sampler, listeners })
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            self.listeners.clear();
            self.sampler
                .borrow_mut()
                .cancel("cancelled-observer-unmounted");
        }
    }

    pub(super) fn bind(
        frame: NodeRef<html::Section>,
        output: RwSignal<Vec<String>>,
        scope: Signal<String>,
    ) {
        let binding = StoredValue::new_local(None::<Probe>);
        Effect::new(move |_| {
            scope.get();
            let Some(frame) = frame.get() else {
                return;
            };
            binding.update_value(|current| {
                *current = None;
                output.set(Vec::new());
                match Probe::new(frame.into(), output) {
                    Ok(probe) => *current = Some(probe),
                    Err(error) => {
                        push_line(output, format!("Motion observation unavailable: {error}"))
                    }
                }
            });
        });
        on_cleanup(move || {
            binding.try_update_value(|current| *current = None);
        });
    }
}
