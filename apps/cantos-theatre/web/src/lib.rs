#![recursion_limit = "256"]

//! Session-only Cantos listener mock: no API, accounts, real media or persistent state.
pub mod app;
pub mod application;
pub mod core;
mod features;
mod infrastructure;
pub mod messages;
mod platform;
pub mod view;

pub mod live;
