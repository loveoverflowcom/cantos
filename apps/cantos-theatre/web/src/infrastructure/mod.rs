pub mod fake_media;
#[cfg(any(test, not(target_arch = "wasm32")))]
pub mod inert_media;
pub mod mock_catalog;
