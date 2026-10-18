#[cfg(target_arch = "wasm32")]
fn main() {
    if cantos_web_auth::browser::complete_popup(
        cantos_web_auth::Surface::Studio,
        option_env!("CANTOS_API_ORIGIN"),
    ) {
        leptos::mount::mount_to_body(cantos_studio::view::AuthorizationComplete);
        return;
    }

    leptos::mount::mount_to_body(cantos_studio::view::Studio);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
