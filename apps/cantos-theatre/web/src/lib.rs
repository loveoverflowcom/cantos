// PR96 generated Leptos view layouts exceed Rust 1.87's default query depth.
#![recursion_limit = "256"]

//! Cantos listener Web: a same-origin account boundary and an explicitly isolated media mock.
//!
//! Two build lanes. The default **production** lane composes only the live runtime. The opt-in
//! `preview` feature adds the isolated development previews (`?mode=mock`, `?mode=auth-preview`,
//! `?mode=membership-preview`) and the sample data behind them; release and deploy builds never
//! enable it. The unit tests exercise the preview code, so they need the feature too.
#[cfg(all(test, not(feature = "preview")))]
compile_error!(
    "cantos-theatre tests cover the isolated previews: run them with `--features preview` \
     (for the workspace: `--features cantos-theatre/preview`), as `cargo xtask verify` does"
);
#[cfg(target_arch = "wasm32")]
mod api_endpoint;
pub mod app;
pub mod application;
pub mod core;
mod features;
mod infrastructure;
#[cfg(feature = "preview")]
pub mod membership_preview;
pub mod messages;
mod platform;
pub mod view;

pub mod live;
