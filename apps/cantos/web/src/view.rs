use leptos::prelude::*;

/// Placeholder root. It states what is missing instead of imitating a catalog or player.
#[component]
pub fn Theatre() -> impl IntoView {
    view! {
        <main>
            <h1>"Cantos Theatre"</h1>
            <p lang="vi-VN">"Khung ứng dụng nghe web. Chưa có danh mục, tài khoản hoặc trình phát."</p>
            <p lang="en">"Web skeleton only: no catalog, account or playback exists yet."</p>
        </main>
    }
}
