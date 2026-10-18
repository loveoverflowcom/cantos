//! Shared browser credential owner. Studio and Theatre have independent instances/purposes.
//! Pure lifecycle decisions are host-testable; browser effects never persist account credentials.
#[cfg(target_arch = "wasm32")]
pub mod browser;
pub mod core;
#[cfg(any(target_arch = "wasm32", test))]
mod refresh;
pub use cantos_contracts::web_api::WebSurface as Surface;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    SignedOut,
    Changed,
    Unavailable,
    Protocol,
    Cancelled,
    PopupBlocked,
    CleanupPending,
    AccountMismatch,
}
