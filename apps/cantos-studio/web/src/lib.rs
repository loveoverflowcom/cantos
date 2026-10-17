pub mod adaptation;
#[cfg(target_arch = "wasm32")]
mod api_endpoint;
pub mod authoring;
pub mod editor;
pub mod entry;
pub mod import;
pub mod inspector;
pub mod messages;
pub mod production;
pub mod session;
pub mod ui;
pub mod wire;

// An explicit debug composition only; never a production fallback or a service client.
#[cfg(debug_assertions)]
pub mod forge;

#[cfg(target_arch = "wasm32")]
pub mod api;
#[cfg(target_arch = "wasm32")]
pub mod authoring_view;
#[cfg(target_arch = "wasm32")]
mod navigation;
#[cfg(target_arch = "wasm32")]
pub mod view;

pub mod live;
