//! Public Home composition. Session lookup and auth are separate, explicit user intents.
use crate::{
    application::public_discovery::{
        PublicDiscoveryFailure, PublicDiscoveryPhase, PublicDiscoveryState,
    },
    messages::Locale,
};
use cantos_contracts::theatre::WorkKind;
use cantos_ui::{Button, ButtonVariant};
use leptos::prelude::*;

#[derive(Clone, Copy, Debug)]
enum Copy {
    Eyebrow,
    Heading,
    Introduction,
    SignIn,
    Register,
    Shelf,
    Loading,
    Empty,
    EmptyHelp,
    Unavailable,
    UnavailableHelp,
    Protocol,
    ProtocolHelp,
    Retry,
    Refresh,
    More,
    Novel,
    Story,
    Podcast,
}

fn text(key: Copy, locale: Locale) -> &'static str {
    match (key, locale) {
        (Copy::Eyebrow, Locale::Vietnamese) => "VỌNG ĐÀI · NHỮNG MIỀN THANH ÂM",
        (Copy::Eyebrow, Locale::English) => "VỌNG ĐÀI · WORLDS OF SOUND",
        (Copy::Heading, Locale::Vietnamese) => "Mở một miền tưởng tượng.",
        (Copy::Heading, Locale::English) => "Open a world of imagination.",
        (Copy::Introduction, Locale::Vietnamese) => "Một câu chuyện hay không cần đợi bạn đăng nhập. Khám phá nội dung công khai ngay tại đây.",
        (Copy::Introduction, Locale::English) => "A good story does not need to wait for sign-in. Explore public stories here.",
        (Copy::SignIn, Locale::Vietnamese) => "Đăng nhập",
        (Copy::SignIn, Locale::English) => "Sign in",
        (Copy::Register, Locale::Vietnamese) => "Tạo tài khoản Cantos",
        (Copy::Register, Locale::English) => "Create a Cantos account",
        (Copy::Shelf, Locale::Vietnamese) => "Mỗi câu chuyện, một thế giới",
        (Copy::Shelf, Locale::English) => "Every story, a world",
        (Copy::Loading, Locale::Vietnamese) => "Đang mở những câu chuyện đã công bố…",
        (Copy::Loading, Locale::English) => "Opening published stories…",
        (Copy::Empty, Locale::Vietnamese) => "Những câu chuyện đang được chuẩn bị.",
        (Copy::Empty, Locale::English) => "Stories are being prepared.",
        (Copy::EmptyHelp, Locale::Vietnamese) => "Danh mục công khai chưa có tác phẩm. Bạn có thể quay lại hoặc cập nhật sau.",
        (Copy::EmptyHelp, Locale::English) => "The public catalog has no stories yet. You can return or refresh later.",
        (Copy::Unavailable, Locale::Vietnamese) => "Chưa mở được danh mục công khai.",
        (Copy::Unavailable, Locale::English) => "The public catalog could not be opened.",
        (Copy::UnavailableHelp, Locale::Vietnamese) => "Kết nối hiện chưa sẵn sàng. Hãy thử lại; bạn không cần đăng nhập để xem nội dung công khai.",
        (Copy::UnavailableHelp, Locale::English) => "The connection is unavailable. Try again; public stories do not require sign-in.",
        (Copy::Protocol, Locale::Vietnamese) => "Chưa đọc được danh mục công khai.",
        (Copy::Protocol, Locale::English) => "The public catalog response could not be read.",
        (Copy::ProtocolHelp, Locale::Vietnamese) => "Phản hồi chưa hợp lệ. Hãy thử lại để tải danh mục từ Cantos.",
        (Copy::ProtocolHelp, Locale::English) => "The response was invalid. Try loading the Cantos catalog again.",
        (Copy::Retry, Locale::Vietnamese) => "Thử lại",
        (Copy::Retry, Locale::English) => "Try again",
        (Copy::Refresh, Locale::Vietnamese) => "Cập nhật danh mục",
        (Copy::Refresh, Locale::English) => "Refresh catalog",
        (Copy::More, Locale::Vietnamese) => "Đang hiển thị tối đa 20 tác phẩm đã công bố; còn nội dung ngoài trang này.",
        (Copy::More, Locale::English) => "Showing up to 20 published stories; more content exists beyond this page.",
        (Copy::Novel, Locale::Vietnamese) => "Tiểu thuyết",
        (Copy::Novel, Locale::English) => "Novel",
        (Copy::Story, Locale::Vietnamese) => "Truyện",
        (Copy::Story, Locale::English) => "Story",
        (Copy::Podcast, _) => "Podcast",
    }
}

fn kind_key(kind: WorkKind) -> Copy {
    match kind {
        WorkKind::Novel => Copy::Novel,
        WorkKind::Story => Copy::Story,
        WorkKind::Podcast => Copy::Podcast,
    }
}

#[component]
pub(crate) fn PublicHome(
    state: RwSignal<PublicDiscoveryState>,
    locale: RwSignal<Locale>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_refresh: Callback<()>,
) -> impl IntoView {
    view! {
        <section class="theatre-public-home" aria-labelledby="theatre-public-home-heading">
            <header class="theatre-public-hero theatre-page-heading">
                <p class="theatre-eyebrow">{move || text(Copy::Eyebrow, locale.get())}</p>
                <h1 id="theatre-public-home-heading">{move || text(Copy::Heading, locale.get())}</h1>
                <p class="theatre-muted">{move || text(Copy::Introduction, locale.get())}</p>
                <div class="theatre-home-auth-actions">
                    <Button id="theatre-home-login" variant=ButtonVariant::Tonal
                        focus_on_press=true on_press=Callback::new(move |_| on_login.run(()))>
                        {move || text(Copy::SignIn, locale.get())}
                    </Button>
                    <Button id="theatre-home-register" variant=ButtonVariant::Text
                        focus_on_press=true on_press=Callback::new(move |_| on_register.run(()))>
                        {move || text(Copy::Register, locale.get())}
                    </Button>
                </div>
            </header>
            <section class="theatre-public-catalog" aria-labelledby="theatre-public-shelf-heading"
                aria-busy=move || matches!(state.get().phase(), PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading)>
                <h2 id="theatre-public-shelf-heading">{move || text(Copy::Shelf, locale.get())}</h2>
                {move || match state.get().phase().clone() {
                    PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading => view! {
                        <p class="theatre-public-state" role="status">{move || text(Copy::Loading, locale.get())}</p>
                    }.into_any(),
                    PublicDiscoveryPhase::Ready(page) => view! {
                        <div class="theatre-public-work-grid">
                            {page.items.into_iter().map(|work| view! {
                                <article class="theatre-public-work theatre-panel" data-work-id=work.work_id>
                                    <p class="theatre-eyebrow">{move || text(kind_key(work.kind), locale.get())}</p>
                                    <h3 lang=work.language.clone()>{work.title}</h3>
                                    {(!work.description.trim().is_empty()).then(|| view! {
                                        <p lang=work.language class="theatre-muted">{work.description}</p>
                                    })}
                                </article>
                            }).collect_view()}
                        </div>
                        {page.next_cursor.is_some().then(|| view! {
                            <p class="theatre-muted">{move || text(Copy::More, locale.get())}</p>
                        })}
                    }.into_any(),
                    PublicDiscoveryPhase::Empty => view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h3>{move || text(Copy::Empty, locale.get())}</h3>
                            <p>{move || text(Copy::EmptyHelp, locale.get())}</p>
                            <Button id="theatre-home-refresh" variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |_| on_refresh.run(()))>
                                {move || text(Copy::Refresh, locale.get())}
                            </Button>
                        </div>
                    }.into_any(),
                    PublicDiscoveryPhase::Failed(failure) => {
                        let (heading, help) = match failure {
                            PublicDiscoveryFailure::Unavailable => (Copy::Unavailable, Copy::UnavailableHelp),
                            PublicDiscoveryFailure::Protocol => (Copy::Protocol, Copy::ProtocolHelp),
                        };
                        view! {
                            <div class="theatre-public-state theatre-panel" role="status">
                                <h3>{move || text(heading, locale.get())}</h3>
                                <p>{move || text(help, locale.get())}</p>
                                <Button id="theatre-home-retry" variant=ButtonVariant::Tonal
                                    on_press=Callback::new(move |_| on_refresh.run(()))>
                                    {move || text(Copy::Retry, locale.get())}
                                </Button>
                            </div>
                        }.into_any()
                    }
                }}
            </section>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_contracts::theatre::{WorkPage, WorkSummary};

    fn render(state: PublicDiscoveryState, locale: Locale) -> String {
        Owner::new().with(|| {
            view! {
                <PublicHome state=RwSignal::new(state) locale=RwSignal::new(locale)
                    on_login=Callback::new(|()| ()) on_register=Callback::new(|()| ())
                    on_refresh=Callback::new(|()| ()) />
            }
            .to_html()
        })
    }

    #[test]
    fn outage_keeps_public_home_and_auth_intents_visible_without_mock_fallback() {
        let mut state = PublicDiscoveryState::default();
        let issued = state.begin_refresh().unwrap();
        state.finish(issued, Err(PublicDiscoveryFailure::Unavailable));
        let html = render(state, Locale::Vietnamese);
        for expected in [
            "Mở một miền tưởng tượng.",
            "không cần đợi bạn đăng nhập",
            "theatre-home-login",
            "theatre-home-register",
            "theatre-home-retry",
        ] {
            assert!(html.contains(expected), "{expected}");
        }
        for absent in [
            "Sơn Hà Ký",
            "Minh An",
            "Đang hiển thị tối đa",
            "access_token",
            "<audio",
        ] {
            assert!(!html.contains(absent), "{absent}");
        }
    }

    #[test]
    fn published_story_uses_server_text_safely_and_never_invents_art_or_playback() {
        let mut state = PublicDiscoveryState::default();
        let issued = state.begin_refresh().unwrap();
        state.finish(
            issued,
            Ok(WorkPage {
                items: vec![WorkSummary {
                    work_id: "00000000-0000-0000-0000-000000000001".into(),
                    kind: WorkKind::Story,
                    title: "Ánh đèn <cuối> sân khấu".into(),
                    description: "Ngày mai, mình có diễn tiếp không?".into(),
                    language: "vi-VN".into(),
                    first_published_at_ms: 1,
                }],
                next_cursor: Some("opaque-page".into()),
            }),
        );
        let html = render(state, Locale::English);
        assert!(html.contains("Ánh đèn &lt;cuối&gt; sân khấu"));
        assert!(html.contains("Ngày mai, mình có diễn tiếp không?"));
        assert!(html.contains("Every story, a world") && html.contains("Showing up to 20"));
        for absent in [
            "theatre-home-retry",
            "<img",
            "<audio",
            "publication_id",
            "opaque-page",
        ] {
            assert!(!html.contains(absent), "{absent}");
        }
    }
}
