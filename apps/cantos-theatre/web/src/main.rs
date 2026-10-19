#[cfg(target_arch = "wasm32")]
fn main() {
    if cantos_web_auth::browser::complete_popup(
        cantos_web_auth::Surface::Theatre,
        option_env!("CANTOS_API_ORIGIN"),
    ) {
        leptos::mount::mount_to_body(cantos_theatre::view::AuthorizationComplete);
        return;
    }

    let query = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    match cantos_theatre::live::Mode::from_query(&query) {
        #[cfg(feature = "preview")]
        cantos_theatre::live::Mode::Mock => {
            leptos::mount::mount_to_body(cantos_theatre::view::Theatre)
        }
        cantos_theatre::live::Mode::Live => {
            leptos::mount::mount_to_body(|| leptos::view! { <cantos_theatre::view::LiveBoundary/> })
        }
        #[cfg(feature = "preview")]
        cantos_theatre::live::Mode::AuthPreview => {
            leptos::mount::mount_to_body(cantos_theatre::view::AuthPreview)
        }
        #[cfg(feature = "preview")]
        cantos_theatre::live::Mode::MembershipPreview => {
            leptos::mount::mount_to_body(cantos_theatre::membership_preview::MembershipPreview)
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
