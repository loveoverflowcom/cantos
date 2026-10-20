//! Real Fetch only against the existing reserved group. No Studio or producer credential.
use super::{classify, Failure, ProbePhase};
use gloo_net::http::Request;
use web_sys::wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{AbortController, RequestCredentials};
pub(super) struct Deadline {
    window: web_sys::Window,
    pub(super) controller: AbortController,
    timer: i32,
    timed_out: std::rc::Rc<std::cell::Cell<bool>>,
    _callback: Closure<dyn FnMut()>,
}
impl Deadline {
    pub(super) fn failure_kind(&self) -> crate::application::errors::ErrorKind {
        if self.timed_out.get() {
            crate::application::errors::ErrorKind::Timeout
        } else {
            crate::application::errors::ErrorKind::Network
        }
    }
}
impl Drop for Deadline {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}
pub async fn probe(controller: AbortController) -> ProbePhase {
    let Ok(deadline) = deadline(controller) else {
        return ProbePhase::Failed(Failure::Transport);
    };
    let signal = deadline.controller.signal();
    let Ok(path) = crate::api_endpoint::endpoint(cantos_contracts::routes::THEATRE_BASE) else {
        return ProbePhase::Failed(Failure::Transport);
    };
    let request = Request::get(&path)
        .credentials(RequestCredentials::Omit)
        .header("Accept", "application/json")
        .abort_signal(Some(&signal));
    match request.send().await {
        Ok(response) => {
            if !response.headers().get("content-type").is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            }) {
                return ProbePhase::Failed(Failure::Protocol);
            }
            match response.binary().await {
                Ok(body) if body.len() <= 8192 => classify(response.status(), &body),
                _ => ProbePhase::Failed(Failure::Protocol),
            }
        }
        Err(_) => ProbePhase::Failed(Failure::Transport),
    }
}
pub(super) fn deadline(controller: AbortController) -> Result<Deadline, ()> {
    let window = web_sys::window().ok_or(())?;
    let abort = controller.clone();
    let timed_out = std::rc::Rc::new(std::cell::Cell::new(false));
    let expired = timed_out.clone();
    let callback = Closure::wrap(Box::new(move || {
        expired.set(true);
        abort.abort();
    }) as Box<dyn FnMut()>);
    let timer = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            15_000,
        )
        .map_err(|_| ())?;
    Ok(Deadline {
        window,
        controller,
        timer,
        timed_out,
        _callback: callback,
    })
}
