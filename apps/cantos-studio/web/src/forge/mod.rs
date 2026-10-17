//! Studio-owned Forge mock. These are internal mock contracts, never HTTP/provider DTOs.
pub mod mock;
pub mod model;
#[cfg(target_arch = "wasm32")]
mod platform;
pub mod view;

#[cfg(test)]
mod tests;
