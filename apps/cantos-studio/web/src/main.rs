#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount::mount_to_body(cantos_studio::view::Studio);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
