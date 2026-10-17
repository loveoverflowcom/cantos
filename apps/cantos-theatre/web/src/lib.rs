// PR96 generated Leptos view layouts exceed Rust 1.87's default query depth.
#![recursion_limit = "256"]

//! Cantos listener Web: a same-origin account boundary and an explicitly isolated media mock.
#[cfg(target_arch = "wasm32")]
mod api_endpoint;
pub mod app;
pub mod application;
pub mod core;
mod features;
mod infrastructure;
pub mod membership_preview;
pub mod messages;
mod platform;
pub mod view;

pub mod live;
