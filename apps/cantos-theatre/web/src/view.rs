use crate::{
    app::{context::TheatreContext, routes::Route},
    application::catalog::Scenario,
    features::{
        common::*,
        discovery::Discover,
        explore::Explore,
        library::Library,
        player::{MiniPlayer, Player},
        queue::Queue,
        work_detail::WorkDetail,
    },
    messages::{self, Key, Locale},
    platform,
};
use cantos_ui::{
    Container, ContainerWidth, Density, EmptyState, Select, Space, Stack, Surface, SurfaceTone,
    Theme, UiRoot,
};
use leptos::prelude::*;

#[component]
pub fn Theatre() -> impl IntoView {
    let main = NodeRef::<leptos::html::Main>::new();
    let ctx = TheatreContext::new(main);
    provide_context(ctx.player);
    platform::bind_navigation(ctx.route, main);
    Effect::new(move |_| {
        let route = ctx.route.get();
        let title = match route {
            Route::Work(id) => {
                ctx.format(Key::PageTitle, &[("page", ctx.work(id).title.to_owned())])
            }
            route => messages::page_title(ctx.locale.get(), route),
        };
        platform::set_title(&title);
    });

    view! {
        <UiRoot theme=ctx.theme density=Density::Comfortable class="theatre-shell">
            <div class="theatre-app" lang=move || ctx.locale.get().lang()>
                <a class="theatre-skip" href="#theatre-main" on:click=move |event| { event.prevent_default(); platform::focus_main(main); }>{move || ctx.text(Key::SkipContent)}</a>
                <aside class="theatre-rail">
                    <a class="theatre-brand" href="#discover">
                        <span class="theatre-brand-name">"Cantos"</span>
                        <small>{move || ctx.text(Key::BrandCaption)}</small>
                    </a>
                    <nav class="theatre-nav" aria-label=move || ctx.text(Key::MainNavigation)>
                        <Navigation ctx=ctx/>
                    </nav>
                    <p class="theatre-quote">{move || ctx.text(Key::Quote)}</p>
                    <p class="theatre-rail-note">{move || ctx.text(Key::DemoBadge)}</p>
                </aside>
                <div class="theatre-content">
                    <header class="theatre-topbar">
                        <p>"Cantos"<span aria-hidden="true">" / "</span>{move || ctx.text(messages::route_key(ctx.route.get()))}</p>
                        <span class="theatre-demo-label">{move || ctx.text(Key::DemoBadge)}</span>
                    </header>
                    <Container width=ContainerWidth::Wide>
                        <main id="theatre-main" node_ref=main tabindex="-1" class="theatre-main">
                            <p class="theatre-demo-notice" id="theatre-demo-notice">{move || ctx.text(Key::DemoNotice)}</p>
                            {move || match ctx.route.get() {
                                Route::Discover => view! { <Discover ctx=ctx/> }.into_any(),
                                Route::Explore => view! { <Explore ctx=ctx/> }.into_any(),
                                Route::Work(id) => view! { <WorkDetail ctx=ctx id=id/> }.into_any(),
                                Route::Library => view! { <Library ctx=ctx/> }.into_any(),
                                Route::Player => view! { <Player ctx=ctx/> }.into_any(),
                                Route::Queue => view! { <Queue ctx=ctx/> }.into_any(),
                                Route::Settings => view! { <Settings ctx=ctx/> }.into_any(),
                                Route::Account => view! { <Account ctx=ctx/> }.into_any(),
                                Route::NotFound => view! { <Recovery ctx=ctx title=Key::NotFound help=Key::NotFoundHelp/> }.into_any(),
                            }}
                            <footer class="theatre-footer">{move || ctx.text(Key::Footer)}</footer>
                        </main>
                    </Container>
                </div>
                <MiniPlayer ctx=ctx/>
            </div>
        </UiRoot>
    }
}

#[component]
fn Navigation(ctx: TheatreContext) -> impl IntoView {
    [
        (Route::Discover, Key::Discover, "home"),
        (Route::Explore, Key::Explore, "search"),
        (Route::Library, Key::Library, "library_books"),
        (Route::Player, Key::Player, "headphones"),
        (Route::Settings, Key::Settings, "settings"),
        (Route::Account, Key::Account, "smartphone"),
    ]
    .into_iter()
    .map(move |(route, label, asset)| {
        view! {
            <a class="theatre-nav-link" href=route.hash()
                aria-current=move || (ctx.route.get() == route).then_some("page")>
                {icon(asset)}<span>{move || ctx.text(label)}</span>
            </a>
        }
    })
    .collect_view()
}

#[component]
fn Settings(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Settings deck=Key::Preferences/>
        <Surface tone=SurfaceTone::Container inset=Space::Lg>
            <Stack gap=Space::Lg>
                <Select id="theatre-theme" label=Signal::derive(move || ctx.text(Key::Theme).to_owned())
                    value=Signal::derive(move || ctx.theme.get().as_str().to_owned())
                    on_input=Callback::new(move |value: String| { if let Some(theme) = Theme::from_attribute(&value) { ctx.theme.set(theme); } })>
                    <option value="system">{move || ctx.text(Key::System)}</option>
                    <option value="light">{move || ctx.text(Key::Light)}</option>
                    <option value="dark">{move || ctx.text(Key::Dark)}</option>
                </Select>
                <Select id="theatre-locale" label=Signal::derive(move || ctx.text(Key::Locale).to_owned())
                    value=Signal::derive(move || ctx.locale.get().lang().to_owned())
                    on_input=Callback::new(move |value: String| { match value.as_str() { "vi-VN" => ctx.locale.set(Locale::Vietnamese), "en" => ctx.locale.set(Locale::English), _ => {} } })>
                    <option value="vi-VN">"Tiếng Việt"</option><option value="en">"English"</option>
                </Select>
                <Select id="theatre-speed" label=Signal::derive(move || ctx.text(Key::Speed).to_owned())
                    value=Signal::derive(move || ctx.player.view.get().map(|player| player.speed).unwrap_or(100).to_string())
                    help=Signal::derive(move || Some(ctx.text(Key::SpeedHelp).to_owned()))
                    on_input=Callback::new(move |value: String| { if let Ok(speed) = value.parse() { ctx.set_speed(speed); } })>
                    <option value="75">{move || messages::speed_label(ctx.locale.get(), 75)}</option><option value="100">{move || messages::speed_label(ctx.locale.get(), 100)}</option><option value="125">{move || messages::speed_label(ctx.locale.get(), 125)}</option><option value="150">{move || messages::speed_label(ctx.locale.get(), 150)}</option><option value="200">{move || messages::speed_label(ctx.locale.get(), 200)}</option>
                </Select>
            </Stack>
        </Surface>
        <section class="theatre-demo-tools">
            <h2 class="theatre-section-heading">{move || ctx.text(Key::DemoTools)}</h2>
            <Select id="theatre-scenario" label=Signal::derive(move || ctx.text(Key::Scenario).to_owned())
                value=Signal::derive(move || ctx.catalog.scenario().value().to_owned())
                on_input=Callback::new(move |value: String| { if let Some(scenario) = Scenario::from_value(&value) { ctx.catalog.load(scenario); } })>
                <option value="ready">{move || ctx.text(Key::Ready)}</option><option value="loading">{move || ctx.text(Key::Loading)}</option>
                <option value="empty">{move || ctx.text(Key::Empty)}</option><option value="error">{move || ctx.text(Key::Error)}</option>
            </Select>
            <p class="theatre-muted">{move || ctx.text(Key::ResetHelp)}</p>
            <div class="theatre-actions">
                <button type="button" class="theatre-action" on:click=move |_| ctx.reset_demo()>{move || ctx.text(Key::Reset)}</button>
                <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.clear_library()>{move || ctx.text(Key::ClearLibrary)}</button>
                <a class="theatre-text-link" href="#discover">{move || ctx.text(Key::BackDiscover)}</a>
            </div>
        </section>
    }
}

#[component]
fn Account(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Account/>
        <Surface tone=SurfaceTone::Container inset=Space::Lg>
            <p>{move || ctx.text(Key::AccountHelp)}</p>
            <UnsupportedAction ctx=ctx label=Key::SignIn/>
            <UnsupportedAction ctx=ctx label=Key::Sync/>
            <a class="theatre-text-link" href="#library">{move || ctx.text(Key::Library)}</a>
        </Surface>
    }
}

#[component]
pub(crate) fn Recovery(ctx: TheatreContext, title: Key, help: Key) -> impl IntoView {
    view! {
        <EmptyState title=ctx.text(title).to_owned()>
            <p>{move || ctx.text(help)}</p>
            <a class="theatre-text-link" href="#discover">{move || ctx.text(Key::BackDiscover)}</a>
        </EmptyState>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            ids::{EpisodeId, WorkId},
            listening::library::Bookmark,
        },
        features::player::{position_label, TransportAction},
        messages::Locale,
    };

    #[test]
    fn listener_title_is_the_user_facing_name() {
        let index = include_str!("../index.html");
        assert!(index.contains("<title>Cantos</title>"));
        assert!(!index.contains("Cantos Theatre"));
        assert!(index.contains("data-bin=\"cantos-theatre\""));
        assert!(index.contains("src/styles/theatre.css"));
    }
    #[test]
    fn mock_shell_is_explicit_and_never_mounts_media_or_studio_authority() {
        Owner::new().with(|| {
            let html = view! { <Theatre/> }.to_html();
            assert!(html.contains("data-density=\"comfortable\""));
            assert!(html.contains("theatre-demo-notice"));
            assert!(html.contains("không có âm thanh") || html.contains("Không có âm thanh"));
            assert!(html.contains("aria-label=\"Điều hướng chính\""));
            assert!(html.contains("aria-current=\"page\""));
            assert!(!html.contains("Cantos Theatre"));
            for forbidden in ["<audio", "<video", "/api/v1/", "session-token", "/studio"] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        });
    }
    #[test]
    fn player_screen_and_navigation_keep_the_same_root_session() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.locale.set(Locale::English);
            ctx.add_bookmark();
            ctx.open_episode(EpisodeId {
                work: WorkId::River,
                index: 0,
            });
            assert_eq!(ctx.route.get_untracked(), Route::Player);
            let html = view! { <Player ctx=ctx/> }.to_html();
            assert!(html.contains("Ngọn đèn đầu ngõ"));
            assert!(html.contains("lang=\"vi-VN\""));
            assert!(html.contains("Mock position"));
            assert!(html.contains("theatre-position"));
            assert!(html.contains("Not integrated in this mock"));
            ctx.resume_bookmark(Bookmark {
                episode: EpisodeId {
                    work: WorkId::Mountain,
                    index: 2,
                },
                position_ms: 768000,
            });
            assert_eq!(position_label(ctx), "12:48 / 28:42");
            ctx.route.set(Route::Discover);
            let identity = ctx.player.state.get_untracked().identity();
            let _ = view! { <Discover ctx=ctx/> }.to_html();
            ctx.route.set(Route::Library);
            let _ = view! { <Library ctx=ctx/> }.to_html();
            assert_eq!(ctx.player.state.get_untracked().identity(), identity);
        });
    }
    #[test]
    fn ended_then_seek_back_labels_play_and_resumes_the_chosen_position() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.locale.set(Locale::English);
            ctx.mock_ended();
            let end = view! { <TransportAction ctx=ctx/> }.to_html();
            assert!(end.contains("Simulate from the beginning"));
            ctx.playback(crate::core::playback::Event::SeekRequested { position_ms: 30000 });
            let sought = view! { <TransportAction ctx=ctx/> }.to_html();
            assert!(sought.contains("Simulate play"));
            assert!(!sought.contains("Simulate from the beginning"));
            ctx.toggle_transport();
            assert_eq!(ctx.player.snapshot().unwrap().position_ms, 30000);
            assert_eq!(
                ctx.player.state.get_untracked().status(),
                crate::core::playback::Status::Simulating
            );
        });
    }
}

/// The live boundary never creates the mock catalog or fake root media engine.
#[component]
pub fn LiveBoundary() -> impl IntoView {
    use crate::live::{Failure, ProbePhase, ProbeState};
    let state = RwSignal::new(ProbeState::default());
    let english = RwSignal::new(false);
    #[cfg(target_arch = "wasm32")]
    let abort = StoredValue::new_local(None::<web_sys::AbortController>);
    #[cfg(target_arch = "wasm32")]
    on_cleanup(move || {
        abort.update_value(|c| {
            if let Some(c) = c.take() {
                c.abort();
            }
        })
    });
    view! {<UiRoot class="theatre-shell"><Container width=ContainerWidth::Reading>
        <main class="theatre-main" lang=move ||if english.get(){"en"}else{"vi-VN"}>
            <h1>"Cantos"</h1><button class="cui-button" type="button" on:click=move |_|english.update(|e| *e = !*e)>"Tiếng Việt / English"</button>
            <p>{move ||if english.get(){"Published audio and listener identity are not available on this API yet."}else{"API hiện chưa có âm thanh đã phát hành và phiên người nghe."}}</p>
            <p role="status" aria-live="polite">{move ||match state.get().phase {
                ProbePhase::Idle=>if english.get(){"Live mode: no request sent."}else{"Chế độ thật: chưa gửi yêu cầu."},
                ProbePhase::Pending=>if english.get(){"Checking the listener boundary…"}else{"Đang kiểm tra ranh giới người nghe…"},
                ProbePhase::Unsupported(_)=>if english.get(){"Unsupported: listener API not implemented. Playback, downloads and progress remain unavailable."}else{"Chưa hỗ trợ: API người nghe chưa triển khai. Phát, tải và đồng bộ tiến độ chưa khả dụng."},
                ProbePhase::Failed(Failure::Unauthenticated|Failure::Forbidden)=>if english.get(){"Listener access refused. A Studio session cannot authorize listening."}else{"Quyền người nghe bị từ chối. Phiên Studio không cấp quyền nghe."},
                ProbePhase::Failed(_)=>if english.get(){"Cannot verify the API boundary. No demo results were substituted."}else{"Chưa xác minh được ranh giới API. Không thay bằng dữ liệu minh họa."},
            }}</p>
            <button class="cui-button" type="button" on:click=move |_| {
                #[cfg(target_arch="wasm32")]
                {abort.update_value(|c| {if let Some(c)=c.take() {c.abort();}});
                if let (Some(next),Ok(controller))=(state.get_untracked().begin(),web_sys::AbortController::new()) {
                    abort.set_value(Some(controller.clone()));state.set(next);
                    leptos::task::spawn_local(async move {let phase=crate::live::browser::probe(controller).await;let _=state.try_update(|s|*s=s.finish(next.generation,phase));});
                }}
            }>{move ||if english.get(){"Check API availability"}else{"Kiểm tra khả năng API"}}</button>
            <button class="cui-button" type="button" on:click=move |_| {
                #[cfg(target_arch="wasm32")]
                abort.update_value(|c| {if let Some(c)=c.take() {c.abort();}});
                state.update(|s| {if let Some(next)=s.cancel() {*s=next;}});
            }>{move ||if english.get(){"Cancel"}else{"Hủy"}}</button>
            <p><a href="?mode=mock">{move ||if english.get(){"Open isolated mock preview"}else{"Mở bản xem thử minh họa riêng"}}</a></p>
        </main>
    </Container></UiRoot>}
}
