use crate::{
    app::{context::TheatreContext, routes::Route},
    application::catalog::Scenario,
    features::{
        account::Account,
        common::*,
        discovery::Discover,
        library::{History, Library},
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
    Effect::new(move |_| platform::set_document_language(ctx.locale.get().lang()));

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
                </aside>
                <div class="theatre-content">
                    <header class="theatre-topbar">
                        <p>"Cantos"<span aria-hidden="true">" / "</span>{move || ctx.text(messages::route_key(ctx.route.get()))}</p>
                        <div class="theatre-topbar-actions">
                            <span class="theatre-demo-label">{move || ctx.text(Key::DemoBadge)}</span>
                            <a class="theatre-avatar" href="#account" aria-label=move || ctx.text(Key::Account)
                                aria-current=move || (ctx.route.get() == Route::Account).then_some("page")>
                                <PersonGlyph/>
                            </a>
                        </div>
                    </header>
                    <Container width=ContainerWidth::Wide>
                        <main id="theatre-main" node_ref=main tabindex="-1" class="theatre-main">
                            <p class="theatre-demo-notice" id="theatre-demo-notice">{move || ctx.text(Key::DemoNotice)}</p>
                            {move || match ctx.route.get() {
                                Route::Discover | Route::Explore => view! { <Discover ctx=ctx/> }.into_any(),
                                Route::Work(id) => view! { <WorkDetail ctx=ctx id=id/> }.into_any(),
                                Route::Library => view! { <Library ctx=ctx/> }.into_any(),
                                Route::History => view! { <History ctx=ctx/> }.into_any(),
                                Route::Player => view! { <Player ctx=ctx/> }.into_any(),
                                Route::Queue => view! { <Queue ctx=ctx/> }.into_any(),
                                Route::Settings => view! { <Settings ctx=ctx/> }.into_any(),
                                Route::Account => view! { <Account ctx=ctx/> }.into_any(),
                                // Live public IDs never resolve against mock fixtures.
                                Route::PublicWork(_) | Route::Auth(_) | Route::Membership(_) | Route::NotFound => view! { <Recovery ctx=ctx title=Key::NotFound help=Key::NotFoundHelp/> }.into_any(),
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
        (Route::Library, Key::Library, "library_books"),
    ]
    .into_iter()
    .map(move |(route, label, asset)| {
        view! {
            <a class="theatre-nav-link" href=route.hash()
                aria-current=move || (ctx.route.get().primary_navigation() == Some(route)).then_some("page")>
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
                    on_input=Callback::new(move |value: String| { if let Some(selected) = Locale::from_lang(&value) { ctx.locale.set(selected); } })>
                    {Locale::ALL.into_iter().map(|option| view! { <option value=option.lang() lang=option.lang()>{option.autonym()}</option> }).collect_view()}
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
                <button type="button" class="theatre-action" on:click=move |_| ctx.reset_demo()><span class="cui-control-label">{move || ctx.text(Key::Reset)}</span></button>
                <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.clear_library()><span class="cui-control-label">{move || ctx.text(Key::ClearLibrary)}</span></button>
                <a class="theatre-text-link" href="#discover">{move || ctx.text(Key::BackDiscover)}</a>
            </div>
        </section>
    }
}

#[component]
pub(crate) fn Recovery(ctx: TheatreContext, title: Key, help: Key) -> impl IntoView {
    view! {
        {move || view! {
            <EmptyState title=messages::text(title, ctx.locale.get()).to_owned()>
                <p>{move || ctx.text(help)}</p>
                <a class="theatre-text-link" href="#discover">{move || ctx.text(Key::BackDiscover)}</a>
            </EmptyState>
        }}
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
            assert!(html.contains("class=\"theatre-avatar\""));
            assert!(html.contains("href=\"#account\""));
            assert!(!html.contains("Cantos Theatre"));
            for forbidden in ["<audio", "<video", "/api/v1/", "session-token", "/studio"] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        });
    }
    #[test]
    fn main_navigation_has_only_discovery_and_my_library() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            for current in [Route::Discover, Route::History, Route::Work(WorkId::River)] {
                ctx.route.set(current);
                let html = view! { <Navigation ctx=ctx/> }.to_html();
                assert_eq!(html.matches("class=\"theatre-nav-link\"").count(), 2);
                assert!(html.contains("href=\"#discover\""));
                assert!(html.contains("href=\"#library\""));
                assert!(html.contains("aria-current=\"page\""));
                for secondary in ["#explore", "#account", "#settings", "#player", "#queue"] {
                    assert!(!html.contains(secondary), "{secondary}");
                }
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
            assert_eq!(ctx.player.snapshot().unwrap().position_ms(), 30000);
            assert_eq!(
                ctx.player.state.get_untracked().status(),
                crate::core::playback::Status::Playing
            );
        });
    }

    #[test]
    fn live_shell_mounts_account_and_library_without_mock_media_or_studio_authority() {
        Owner::new().with(|| {
            let html = view! { <LiveBoundary/> }.to_html();
            assert!(html.contains("theatre-public-home"));
            assert!(html.contains("href=\"#account\""));
            assert!(!html.contains("theatre-live-account-heading"));
            assert!(html.contains("role=\"status\""));
            for forbidden in [
                "Cantos Theatre",
                "<audio",
                "<video",
                "session-token",
                "/studio",
                "Người giữ tiếng chuông",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        });
    }
    #[test]
    fn locale_projection_keeps_player_snapshot_and_saved_work() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.open_episode(EpisodeId {
                work: WorkId::River,
                index: 0,
            });
            ctx.playback(crate::core::playback::Event::SeekRequested {
                position_ms: 30_000,
            });
            ctx.set_saved(WorkId::River, true);
            let before_player = ctx.player.snapshot().unwrap();
            let before_library = ctx.library.get_untracked();
            for (locale, action, recovery) in [
                (Locale::Vietnamese, "Phát mô phỏng", "Chưa chọn tập nghe"),
                (Locale::English, "Simulate play", "No episode selected"),
            ] {
                ctx.locale.set(locale);
                let html = view! { <TransportAction ctx=ctx/> }.to_html();
                let empty =
                    view! { <Recovery ctx title=Key::NoPlayer help=Key::NoPlayerHelp/> }.to_html();
                assert!(html.contains(action) && empty.contains(recovery));
                assert_eq!(ctx.player.snapshot().unwrap(), before_player);
                assert_eq!(ctx.library.get_untracked(), before_library);
            }
        });
    }
}

mod live_shell;
pub use live_shell::{AuthPreview, LiveBoundary};

// Keep the existing shared foundation adoption at this root composition site.
#[component]
fn LiveFrame(theme: RwSignal<Theme>, children: Children) -> impl IntoView {
    view! { <UiRoot theme density=Density::Comfortable class="theatre-shell">{children()}</UiRoot> }
}
#[component]
fn LiveContainer(children: Children) -> impl IntoView {
    view! { <Container width=ContainerWidth::Wide>{children()}</Container> }
}

#[component]
pub fn AuthorizationComplete() -> impl IntoView {
    let locale = RwSignal::new(crate::messages::Locale::Vietnamese);
    Effect::new(move |_| platform::set_document_language(locale.get().lang()));
    view! {<main class="theatre-main"><h1>{move ||crate::features::auth_copy::text(crate::features::auth_copy::CopyKey::LoginHeading,locale.get())}</h1><p role="status">{move ||crate::features::auth_copy::text(crate::features::auth_copy::CopyKey::AuthorizationComplete,locale.get())}</p>
        <Select id="authorization-language" label=Signal::derive(move ||messages::text(Key::Locale,locale.get()).to_owned()) value=Signal::derive(move ||locale.get().lang().to_owned()) on_input=Callback::new(move |value:String|{if let Some(value)=Locale::from_lang(&value){locale.set(value);}})>
            {Locale::ALL.into_iter().map(|value|view!{<option value=value.lang() lang=value.lang()>{value.autonym()}</option>}).collect_view()}
        </Select>
    </main>}
}
