use cantos_ui::{
    Container, ContainerWidth, Density, EmptyState, Select, Space, Stack, Surface, SurfaceTone,
    Theme, UiRoot,
};
use leptos::prelude::*;

/// Shared presentation only. No catalog, listener port, account or audio is mounted.
#[component]
pub fn Theatre() -> impl IntoView {
    let theme = RwSignal::new(Theme::System);
    view! {
        <UiRoot theme=theme density=Density::Comfortable class="theatre-shell">
            <Container width=ContainerWidth::Reading>
                <main class="theatre-main">
                    <header class="theatre-header">
                        <div><p class="theatre-wordmark">"Vọng Đài · Theatre of Voices"</p><h1>"Cantos"</h1></div>
                        <Select id="theatre-theme" label="Giao diện".to_string()
                            value=Signal::derive(move || theme.get().as_str().to_owned())
                            on_input=Callback::new(move |value: String| { if let Some(selected) = Theme::from_attribute(&value) { theme.set(selected); } })>
                            <option value="system" selected=move || theme.get() == Theme::System>"Theo hệ thống"</option>
                            <option value="light" selected=move || theme.get() == Theme::Light>"Sáng"</option>
                            <option value="dark" selected=move || theme.get() == Theme::Dark>"Tối"</option>
                        </Select>
                    </header>
                    <Surface tone=SurfaceTone::Container inset=Space::Lg>
                        <EmptyState title="Khung ứng dụng nghe web">
                            <Stack gap=Space::Sm>
                                <p lang="vi-VN">"Chưa có danh mục, tài khoản hoặc trình phát. Trải nghiệm nghe chỉ được mở khi có bản xuất bản và âm thanh đã được phê duyệt."</p>
                                <p lang="en">"Web skeleton only: no catalog, account or playback exists yet."</p>
                            </Stack>
                        </EmptyState>
                    </Surface>
                </main>
            </Container>
        </UiRoot>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_page_title_is_the_user_facing_name_not_the_technical_one() {
        let index = include_str!("../index.html");
        assert!(index.contains("<title>Cantos</title>"));
        assert!(!index.contains("Cantos Theatre"));
        assert!(index.contains("data-bin=\"cantos-theatre\""));
    }

    #[test]
    fn theatre_shared_shell_stays_explicitly_unavailable_and_separate_from_studio() {
        let owner = Owner::new();
        owner.with(|| {
            let html = view! { <Theatre/> }.to_html();
            assert!(html.contains("data-density=\"comfortable\""));
            assert!(html.contains("cui-empty-state"));
            assert!(html.contains("no catalog, account or playback exists yet"));
            assert!(html.contains("for=\"theatre-theme\""));
            assert!(html.contains("<h1>Cantos</h1>"));
            assert!(!html.contains("Cantos Theatre"));
            assert!(!html.contains("<audio"));
            assert!(!html.contains("<button"));
            assert!(!html.contains("/studio"));
            assert!(!html.contains("session-token"));
        });
    }
}
