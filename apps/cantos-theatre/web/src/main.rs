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
            leptos::mount::mount_to_body(|| leptos::view! { <cantos_theatre::view::LiveBoundary/> })
        }
        cantos_theatre::live::Mode::AuthPreview => {
            leptos::mount::mount_to_body(cantos_theatre::view::AuthPreview)
        }
        cantos_theatre::live::Mode::MembershipPreview => {
            leptos::mount::mount_to_body(cantos_theatre::membership_preview::MembershipPreview)
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
