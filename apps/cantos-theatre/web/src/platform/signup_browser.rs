//! Capture email proof synchronously, remove it from history, retain it only in application RAM.
use crate::application::signup::VerificationToken;

#[cfg(any(target_arch = "wasm32", test))]
mod navigation_guard {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum EventKind {
        PopState,
        HashChange,
    }

    /// History traversal dispatches popstate before hashchange. The first handler can scrub
    /// the proof URL before the companion event runs. Keep only this nonsecret event fact;
    /// never suppress a later popstate, even if Back has the same sanitized destination.
    #[derive(Default)]
    pub(super) struct ProofNavigationGuard {
        proof_hashchange_pending: bool,
    }

    impl ProofNavigationGuard {
        pub(super) fn should_process(
            &mut self,
            kind: EventKind,
            event_targeted_proof: bool,
            location_contains_proof: bool,
        ) -> bool {
            let scrubbed_companion = self.proof_hashchange_pending
                && kind == EventKind::HashChange
                && event_targeted_proof
                && !location_contains_proof;
            self.proof_hashchange_pending = false;
            !scrubbed_companion
        }

        pub(super) fn proof_scrubbed(&mut self, kind: EventKind) {
            self.proof_hashchange_pending = kind == EventKind::PopState;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn history_event_pair_keeps_the_captured_proof_after_its_url_is_scrubbed() {
            let mut guard = ProofNavigationGuard::default();
            assert!(guard.should_process(EventKind::PopState, false, true));
            guard.proof_scrubbed(EventKind::PopState);
            assert!(!guard.should_process(EventKind::HashChange, true, false));
            assert!(guard.should_process(EventKind::HashChange, true, false));
        }

        #[test]
        fn actual_back_always_processes_even_to_the_same_sanitized_url() {
            for targeted_proof in [false, true] {
                let mut guard = ProofNavigationGuard::default();
                guard.proof_scrubbed(EventKind::PopState);
                assert!(guard.should_process(EventKind::PopState, targeted_proof, false));
                assert!(guard.should_process(EventKind::HashChange, true, false));
            }
        }

        #[test]
        fn hash_only_navigation_new_links_and_unrelated_events_are_never_debounced() {
            for first in [EventKind::PopState, EventKind::HashChange] {
                for (targeted_proof, contains_proof) in
                    [(false, false), (false, true), (true, true)]
                {
                    let mut guard = ProofNavigationGuard::default();
                    guard.proof_scrubbed(first);
                    assert!(guard.should_process(
                        EventKind::HashChange,
                        targeted_proof,
                        contains_proof
                    ));
                    assert!(guard.should_process(EventKind::PopState, false, false));
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaptureFailure {
    #[cfg(target_arch = "wasm32")]
    InvalidLink,
    HistoryUnavailable,
}

pub(crate) fn capture_link() -> Result<Option<VerificationToken>, CaptureFailure> {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return Ok(None);
        };
        let hash = window
            .location()
            .hash()
            .map_err(|_| CaptureFailure::HistoryUnavailable)?;
        if !hash.starts_with("#register-verify") {
            return Ok(None);
        }
        let token = VerificationToken::from_fragment(&hash);
        // This happens before navigation construction or any session/configuration/discovery IO.
        // Neither history state nor the replacement URL includes the proof or applicant email.
        window
            .history()
            .map_err(|_| CaptureFailure::HistoryUnavailable)?
            .replace_state_with_url(
                &web_sys::wasm_bindgen::JsValue::NULL,
                "",
                Some("#register?from=home"),
            )
            .map_err(|_| CaptureFailure::HistoryUnavailable)?;
        token.map(Some).ok_or(CaptureFailure::InvalidLink)
    }
    #[cfg(not(target_arch = "wasm32"))]
    Ok(None)
}

/// Explicit recovery from a failed history scrub. No proof is retained or dispatched here.
pub(crate) fn discard_link_and_reload() -> Result<(), CaptureFailure> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().ok_or(CaptureFailure::HistoryUnavailable)?;
        window
            .history()
            .map_err(|_| CaptureFailure::HistoryUnavailable)?
            .replace_state_with_url(
                &web_sys::wasm_bindgen::JsValue::NULL,
                "",
                Some("#register?from=home"),
            )
            .map_err(|_| CaptureFailure::HistoryUnavailable)?;
        // A fresh safe mount reinstalls the ordinary account focus/revalidation lifecycle.
        // Keep the current safety gate closed until navigation actually replaces this owner.
        window
            .location()
            .reload()
            .map_err(|_| CaptureFailure::HistoryUnavailable)?;
    }
    Ok(())
}

/// Register before LiveNavigation's listeners: an incoming same-tab email link is scrubbed
/// before another listener reads navigation state or starts any route-driven request.
pub(crate) fn bind_capture(
    on_capture: leptos::prelude::Callback<Result<Option<VerificationToken>, CaptureFailure>>,
    on_leave: leptos::prelude::Callback<()>,
    on_restore: leptos::prelude::Callback<()>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::{leptos_dom::helpers::window_event_listener_untyped, prelude::*};
        use navigation_guard::{EventKind, ProofNavigationGuard};
        use web_sys::wasm_bindgen::JsCast;
        let guard = StoredValue::new(ProofNavigationGuard::default());
        let read = move |kind, event_targeted_proof| {
            let contains_proof = super::auth_browser::hash().starts_with("#register-verify");
            let mut process = false;
            guard.update_value(|current| {
                process = current.should_process(kind, event_targeted_proof, contains_proof);
            });
            if !process {
                return;
            }
            match capture_link() {
                Ok(None) => on_leave.run(()),
                captured => {
                    if captured != Err(CaptureFailure::HistoryUnavailable) {
                        guard.update_value(|current| current.proof_scrubbed(kind));
                    }
                    on_capture.run(captured);
                }
            }
        };
        let hash = window_event_listener_untyped("hashchange", move |event| {
            // Read event metadata transiently. The original URL is never stored or logged.
            let targeted_proof = event
                .dyn_ref::<web_sys::HashChangeEvent>()
                .is_some_and(|event| {
                    event
                        .new_url()
                        .split_once('#')
                        .is_some_and(|(_, fragment)| fragment.starts_with("register-verify"))
                });
            read(EventKind::HashChange, targeted_proof);
        });
        let pop =
            window_event_listener_untyped("popstate", move |_| read(EventKind::PopState, false));
        let page = window_event_listener_untyped("pageshow", move |event| {
            if event
                .dyn_ref::<web_sys::PageTransitionEvent>()
                .is_some_and(|event| event.persisted())
            {
                on_restore.run(());
            }
        });
        let escape = window_event_listener_untyped("keydown", move |event| {
            if event
                .dyn_ref::<web_sys::KeyboardEvent>()
                .is_some_and(|event| event.key() == "Escape")
            {
                on_leave.run(());
            }
        });
        on_cleanup(move || {
            hash.remove();
            pop.remove();
            page.remove();
            escape.remove();
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (on_capture, on_leave, on_restore);
}
