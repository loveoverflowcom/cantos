#[cfg(target_arch = "wasm32")]
fn main() {
    let query = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    match cantos_theatre::live::Mode::from_query(&query) {
        cantos_theatre::live::Mode::Mock => {
            leptos::mount::mount_to_body(cantos_theatre::view::Theatre)
        }
        cantos_theatre::live::Mode::Live => {
            leptos::mount::mount_to_body(cantos_theatre::view::LiveBoundary)
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
