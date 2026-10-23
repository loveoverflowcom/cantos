//! Shared browser session owner for Identity v1 cookies. Studio and Theatre have independent
//! instances/surfaces. Pure lifecycle decisions are host-testable; the browser never reads,
//! stores or forwards the HttpOnly credential and only keeps nonsecret session facts in RAM.
pub mod address;
#[cfg(target_arch = "wasm32")]
pub mod browser;
pub mod core;
#[cfg(any(target_arch = "wasm32", test))]
pub mod owner;
#[cfg(any(target_arch = "wasm32", test))]
mod refresh;
#[cfg(any(target_arch = "wasm32", test))]
mod serial;
pub mod wire;
pub use address::WebSurface as Surface;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No session, or the API answered 401 for a session/protected call.
    SignedOut,
    /// Local fence or API 409: another tab/intent changed account or session.
    Changed,
    /// 503, network, timeout or an unknown-effect outcome; never "signed out".
    Unavailable,
    /// 429 with a bounded Retry-After in seconds.
    RateLimited(u32),
    /// 403: wrong Origin or, for Studio login, a verified password without a roster seat.
    Forbidden,
    /// 404: Identity not configured (or Studio has no roster). No fallback exists.
    NotFound,
    /// 400: malformed request or password policy.
    Invalid,
    /// 401 on login: wrong or unknown credentials. The API never says which.
    InvalidCredentials,
    Protocol,
    CleanupPending,
    AccountMismatch,
}
