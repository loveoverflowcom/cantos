#[cfg(target_arch = "wasm32")]
fn main() {
    use cantos_ui::{Button, ButtonVariant, Icon, IconName, Theme, UiRoot};
    use leptos::prelude::*;

    leptos::mount::mount_to_body(|| {
        let theme = RwSignal::new(Theme::Light);
        let count = RwSignal::new(0u32);
        view! {
            <UiRoot theme=theme>
                <main class="cui-preview" lang="vi-VN">
                    <h1 class="cui-preview-title">"Cantos · Nền UI"</h1>
                    <p>"Sơn Hà Ký — Ánh đèn cuối sân khấu. Bản mẫu chỉ kiểm chứng nền UI, không tạo âm thanh."</p>
                    <div class="cui-preview-samples">
                        <Button id="theme" variant=ButtonVariant::Tonal on_press=Callback::new(move |_| theme.update(|theme| {
                            *theme = if *theme == Theme::Light { Theme::Dark } else { Theme::Light };
                        }))>
                            <Icon name=IconName::DarkMode/>"Đổi sáng / tối"
                        </Button>
                        <Button id="sample-action" on_press=Callback::new(move |_| count.update(|count| *count += 1))>
                            <Icon name=IconName::Add/>"Thêm lời thoại mẫu"
                        </Button>
                    </div>
                    <p role="status">{move || format!("Số thao tác mẫu: {}", count.get())}</p>
                </main>
            </UiRoot>
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
