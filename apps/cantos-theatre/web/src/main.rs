#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount::mount_to_body(cantos_theatre::view::Theatre);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
