pub mod account_menu;
pub mod adaptation;
#[cfg(target_arch = "wasm32")]
mod api_endpoint;
pub mod authoring;
pub mod contract;
pub mod editor;
pub mod entry;
pub mod import;
pub mod inspector;
pub mod messages;
pub mod production;
pub mod session;
pub mod signin;
pub mod signin_form;
pub mod ui;
pub mod wire;

// An explicit debug composition only; never a production fallback or a service client.
#[cfg(debug_assertions)]
pub mod forge;

#[cfg(target_arch = "wasm32")]
pub mod api;
#[cfg(all(target_arch = "wasm32", debug_assertions))]
mod authoring_preview;
pub mod authoring_view;
#[cfg(target_arch = "wasm32")]
mod download;
#[cfg(target_arch = "wasm32")]
mod navigation;
#[cfg(target_arch = "wasm32")]
pub mod view;

pub mod live;
pub mod localization;
