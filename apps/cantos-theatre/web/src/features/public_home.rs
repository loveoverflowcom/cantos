//! Public Home composition. Session lookup and auth are separate, explicit user intents.
use crate::{
    app::routes::Route,
    application::public_discovery::{
        PublicDiscoveryFailure, PublicDiscoveryPhase, PublicDiscoveryState, PUBLIC_WORK_PAGE_LIMIT,
    },
    core::public_ids::PublicWorkId,
    features::auth_copy::{self, CopyKey},
    messages::{self, text, Key, Locale},
};
use cantos_ui::{Button, ButtonVariant};
use leptos::prelude::*;

/// Stable focus target for returning from that Work's detail.
pub(crate) fn work_link_id(work: PublicWorkId) -> String {
    format!("theatre-public-work-link-{work}")
}

/// Plain primary activation is routed in-app; modified clicks keep native new-tab behavior.
fn in_app_activation(event: &leptos::ev::MouseEvent) -> bool {
    event.button() == 0
        && !event.default_prevented()
        && !(event.ctrl_key() || event.meta_key() || event.shift_key() || event.alt_key())
}

#[component]
pub(crate) fn PublicHome(
    state: RwSignal<PublicDiscoveryState>,
    locale: RwSignal<Locale>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_refresh: Callback<()>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    view! {
        <section class="theatre-public-home" aria-labelledby="theatre-public-home-heading">
            <header class="theatre-public-hero theatre-page-heading">
                <p class="theatre-eyebrow">{move || text(Key::PublicHomeEyebrow, locale.get())}</p>
                <h1 id="theatre-public-home-heading">{move || text(Key::HomeHeading, locale.get())}</h1>
                <p class="theatre-muted">{move || text(Key::PublicHomeIntroduction, locale.get())}</p>
                <div class="theatre-home-auth-actions">
                    <Button id="theatre-home-login" variant=ButtonVariant::Tonal
                        focus_on_press=true on_press=Callback::new(move |_| on_login.run(()))>
                        {move || text(Key::SignIn, locale.get())}
                    </Button>
                    <Button id="theatre-home-register" variant=ButtonVariant::Text
                        focus_on_press=true on_press=Callback::new(move |_| on_register.run(()))>
                        {move || auth_copy::text(CopyKey::CreateAccount, locale.get())}
                    </Button>
                </div>
            </header>
            <section class="theatre-public-catalog" aria-labelledby="theatre-public-shelf-heading"
                aria-busy=move || matches!(state.get().phase(), PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading)>
                <h2 id="theatre-public-shelf-heading">{move || text(Key::StoryShelf, locale.get())}</h2>
                {move || match state.get().phase().clone() {
                    PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading => view! {
                        <p class="theatre-public-state" role="status">{move || text(Key::PublicHomeLoading, locale.get())}</p>
                    }.into_any(),
                    PublicDiscoveryPhase::Ready(page) => view! {
                        <div class="theatre-public-work-grid">
                            {page.items.into_iter().map(|work| view! {
                                <article class="theatre-public-work theatre-panel" data-work-id=work.work_id.clone()>
                                    <p class="theatre-eyebrow">{move || text(messages::work_kind_key(work.kind), locale.get())}</p>
                                    <h3 lang=work.language.clone()>{match PublicWorkId::parse(&work.work_id) {
                                        Some(id) => view! {
                                            <a id=work_link_id(id) class="theatre-public-work-link" href=Route::PublicWork(id).hash()
                                                on:click=move |event| if in_app_activation(&event) {
                                                    event.prevent_default();
                                                    on_open_work.run(id);
                                                }>{work.title}</a>
                                        }.into_any(),
                                        // The decoder admits only canonical IDs; never route a guessed one.
                                        None => work.title.into_any(),
                                    }}</h3>
                                    {(!work.description.trim().is_empty()).then(|| view! {
                                        <p lang=work.language class="theatre-muted">{work.description}</p>
                                    })}
                                </article>
                            }).collect_view()}
                        </div>
                        {page.next_cursor.is_some().then(|| view! {
                            <p class="theatre-muted">{move || messages::format(Key::PublicHomeMore, locale.get(), &[("limit", PUBLIC_WORK_PAGE_LIMIT.to_string())])}</p>
                        })}
                    }.into_any(),
                    PublicDiscoveryPhase::Empty => view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h3>{move || text(Key::PublicHomeEmpty, locale.get())}</h3>
                            <p>{move || text(Key::PublicHomeEmptyHelp, locale.get())}</p>
                            <Button id="theatre-home-refresh" variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |_| on_refresh.run(()))>
                                {move || text(Key::PublicHomeRefresh, locale.get())}
                            </Button>
                        </div>
                    }.into_any(),
                    PublicDiscoveryPhase::Failed(failure) => {
                        let (heading, help) = match failure {
                            PublicDiscoveryFailure::Unavailable => (Key::PublicHomeUnavailable, Key::PublicConnectionUnavailableHelp),
                            PublicDiscoveryFailure::Protocol => (Key::PublicHomeProtocol, Key::PublicHomeProtocolHelp),
                        };
                        view! {
                            <div class="theatre-public-state theatre-panel" role="status">
                                <h3>{move || text(heading, locale.get())}</h3>
                                <p>{move || text(help, locale.get())}</p>
                                <Button id="theatre-home-retry" variant=ButtonVariant::Tonal
                                    on_press=Callback::new(move |_| on_refresh.run(()))>
                                    {move || text(Key::PublicRetry, locale.get())}
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
    use cantos_contracts::theatre::{WorkKind, WorkPage, WorkSummary};

    fn render(state: PublicDiscoveryState, locale: Locale) -> String {
        Owner::new().with(|| {
            view! {
                <PublicHome state=RwSignal::new(state) locale=RwSignal::new(locale)
                    on_login=Callback::new(|()| ()) on_register=Callback::new(|()| ())
                    on_refresh=Callback::new(|()| ()) on_open_work=Callback::new(|_| ()) />
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
        assert!(html.contains("href=\"#work/00000000-0000-0000-0000-000000000001\""));
        assert!(
            html.contains("id=\"theatre-public-work-link-00000000-0000-0000-0000-000000000001\"")
        );
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

    #[test]
    fn public_home_renders_each_typed_state_in_both_locales() {
        for (locale, loading, empty, unavailable, protocol, retry, refresh) in [
            (
                Locale::Vietnamese,
                "Đang mở những câu chuyện đã công bố…",
                "Những câu chuyện đang được chuẩn bị.",
                "Chưa mở được danh mục công khai.",
                "Chưa đọc được danh mục công khai.",
                "Thử lại",
                "Cập nhật danh mục",
            ),
            (
                Locale::English,
                "Opening published stories…",
                "Stories are being prepared.",
                "The public catalog could not be opened.",
                "The public catalog response could not be read.",
                "Try again",
                "Refresh catalog",
            ),
        ] {
            assert!(render(PublicDiscoveryState::default(), locale).contains(loading));
            let mut empty_state = PublicDiscoveryState::default();
            let issued = empty_state.begin_refresh().unwrap();
            empty_state.finish(
                issued,
                Ok(WorkPage {
                    items: vec![],
                    next_cursor: None,
                }),
            );
            let html = render(empty_state, locale);
            assert!(html.contains(empty) && html.contains(refresh), "{html}");
            for (failure, heading) in [
                (PublicDiscoveryFailure::Unavailable, unavailable),
                (PublicDiscoveryFailure::Protocol, protocol),
            ] {
                let mut failed = PublicDiscoveryState::default();
                let issued = failed.begin_refresh().unwrap();
                failed.finish(issued, Err(failure));
                let html = render(failed, locale);
                assert!(html.contains(heading) && html.contains(retry), "{html}");
                assert!(html.contains("role=\"status\""));
            }
        }
    }

    #[test]
    fn locale_projection_keeps_public_content_language_and_page_identity() {
        Owner::new().with(|| {
            let locale = RwSignal::new(Locale::Vietnamese);
            let mut initial = PublicDiscoveryState::default();
            let issued = initial.begin_refresh().unwrap();
            initial.finish(issued, Ok(WorkPage { items: vec![WorkSummary {
                work_id: "00000000-0000-0000-0000-000000000001".into(), kind: WorkKind::Story,
                title: "Lời hẹn {time}".into(), description: "Giữ nguyên tiếng Việt.".into(),
                language: "vi-VN".into(), first_published_at_ms: 1,
            }], next_cursor: Some("opaque-page".into()) }));
            let state = RwSignal::new(initial.clone());
            for (next, label, notice) in [
                (Locale::Vietnamese, "Đăng nhập", "Đang hiển thị tối đa 20"),
                (Locale::English, "Sign in", "Showing up to 20"),
            ] {
                locale.set(next);
                let html = view! { <PublicHome state locale on_login=Callback::new(|()| ()) on_register=Callback::new(|()| ()) on_refresh=Callback::new(|()| ()) on_open_work=Callback::new(|_| ())/ > }.to_html();
                assert!(html.contains(label) && html.contains(notice), "{html}");
                assert!(html.contains("Lời hẹn {time}") && html.contains("lang=\"vi-VN\""));
                assert_eq!(state.get_untracked(), initial);
            }
        });
    }
}
