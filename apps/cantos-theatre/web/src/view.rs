use cantos_ui::{
    Container, ContainerWidth, Density, EmptyState, Select, Space, Stack, Surface, SurfaceTone,
    Theme, UiRoot,
};
use leptos::prelude::*;

use crate::{
    messages::{self, Key, Locale},
    model::{self, Bookmark, DemoState, EpisodeId, Event, Genre, LibraryTab, Route, Scenario, Transport, WorkId},
    platform,
};

/// App-local composition context. It owns only session presentation, never a listener session.
#[derive(Clone, Copy)]
struct DemoContext {
    state: RwSignal<DemoState>,
    route: RwSignal<Route>,
    locale: RwSignal<Locale>,
    theme: RwSignal<Theme>,
    main: NodeRef<leptos::html::Main>,
}

impl DemoContext {
    fn text(self, key: Key) -> &'static str {
        messages::text(key, self.locale.get())
    }

    fn format(self, key: Key, values: &[(&str, String)]) -> String {
        messages::format(key, self.locale.get(), values)
    }

    fn dispatch(self, event: Event) {
        self.state.update(move |state| *state = model::reduce(state.clone(), event));
    }

    fn open_episode(self, id: EpisodeId) {
        if model::episode(id).is_some_and(|episode| episode.available) {
            self.dispatch(Event::Select(id));
            platform::navigate(self.route, Route::Player);
        }
    }
}

/// Existing shared import site only. No new shared component is adopted or qualified here.
#[component]
pub fn Theatre() -> impl IntoView {
    let main = NodeRef::<leptos::html::Main>::new();
    let ctx = DemoContext {
        state: RwSignal::new(DemoState::default()),
        route: RwSignal::new(Route::Discover),
        locale: RwSignal::new(Locale::Vietnamese),
        theme: RwSignal::new(Theme::System),
        main,
    };
    platform::bind_navigation(ctx.route, main);
    Effect::new(move |_| platform::set_title(&messages::page_title(ctx.locale.get(), ctx.route.get())));

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
fn Navigation(ctx: DemoContext) -> impl IntoView {
    [
        (Route::Discover, Key::Discover, "home"),
        (Route::Explore, Key::Explore, "search"),
        (Route::Library, Key::Library, "library_books"),
        (Route::Player, Key::Player, "headphones"),
        (Route::Settings, Key::Settings, "settings"),
        (Route::Account, Key::Account, "smartphone"),
    ].into_iter().map(move |(route, label, asset)| {
        view! {
            <a class="theatre-nav-link" href=route.hash()
                aria-current=move || (ctx.route.get() == route).then_some("page")>
                {icon(asset)}<span>{move || ctx.text(label)}</span>
            </a>
        }
    }).collect_view()
}

// Official licensed static markup, never user content. This is app chrome, not a shared API.
fn icon(asset: &'static str) -> impl IntoView {
    let markup = match asset {
        "home" => include_str!("../assets/icons/home.svg"),
        "search" => include_str!("../assets/icons/search.svg"),
        "library_books" => include_str!("../assets/icons/library_books.svg"),
        "headphones" => include_str!("../assets/icons/headphones.svg"),
        "settings" => include_str!("../assets/icons/settings.svg"),
        "smartphone" => include_str!("../assets/icons/smartphone.svg"),
        "arrow_back" => include_str!("../assets/icons/arrow_back.svg"),
        "play_arrow" => include_str!("../assets/icons/play_arrow.svg"),
        "pause" => include_str!("../assets/icons/pause.svg"),
        "bookmark_border" => include_str!("../assets/icons/bookmark_border.svg"),
        "check" => include_str!("../assets/icons/check.svg"),
        "queue_music" => include_str!("../assets/icons/queue_music.svg"),
        "replay" => include_str!("../assets/icons/replay.svg"),
        "rotate_right" => include_str!("../assets/icons/rotate_right.svg"),
        "delete_outline" => include_str!("../assets/icons/delete_outline.svg"),
        "add" => include_str!("../assets/icons/add.svg"),
        _ => "",
    };
    view! { <span class="theatre-icon" aria-hidden="true" inner_html=markup></span> }
}

#[component]
fn PageHeading(ctx: DemoContext, title: Key, #[prop(optional)] deck: Option<Key>) -> impl IntoView {
    view! {
        <header class="theatre-page-heading">
            <p class="theatre-eyebrow">{move || ctx.text(Key::BrandCaption)}</p>
            <h1>{move || ctx.text(title)}</h1>
            {deck.map(|key| view! { <p class="theatre-muted">{move || ctx.text(key)}</p> })}
        </header>
    }
}

#[component]
fn Cover(ctx: DemoContext, id: WorkId) -> impl IntoView {
    view! {
        <div class="theatre-cover" data-work=id.slug() role="img"
            aria-label=move || format!("{}: {}", model::work(id).title, ctx.text(Key::CoverPlaceholder))>
            <span class="theatre-cover-moon" aria-hidden="true"></span>
            <span class="theatre-cover-index" aria-hidden="true">"C / "{match id { WorkId::Mountain => "01", WorkId::River => "02", WorkId::Dream => "03" }}</span>
            <strong lang="vi-VN" aria-hidden="true">{model::work(id).title}</strong>
            <small aria-hidden="true">{move || ctx.text(messages::genre_key(model::work(id).genre))}</small>
        </div>
    }
}

#[component]
fn SaveAction(ctx: DemoContext, id: WorkId) -> impl IntoView {
    view! {
        <button type="button" class="theatre-action theatre-action-tonal"
            aria-pressed=move || ctx.state.with(|state| state.saved.contains(&id)).to_string()
            aria-label=move || ctx.format(Key::SaveName, &[("title", model::work(id).title.to_owned())])
            on:click=move |_| { ctx.dispatch(Event::ToggleSaved(id)); if ctx.route.get_untracked() == Route::Library { platform::focus_main(ctx.main); } }>
            {icon("bookmark_border")}
            {move || ctx.text(Key::Save)}
        </button>
    }
}

#[component]
fn WorkCard(ctx: DemoContext, id: WorkId) -> impl IntoView {
    view! {
        <article class="theatre-work-card">
            <a class="theatre-cover-link" href=Route::Work(id).hash()><Cover ctx=ctx id=id/></a>
            <div class="theatre-work-meta">
                <h3 lang="vi-VN"><a href=Route::Work(id).hash()>{model::work(id).title}</a></h3>
                <p class="theatre-muted">
                    {move || ctx.text(messages::genre_key(model::work(id).genre))}
                    <span aria-hidden="true">" · "</span>
                    {move || messages::episode_count(ctx.locale.get(), model::work(id).episodes.len())}
                </p>
                <SaveAction ctx=ctx id=id/>
            </div>
        </article>
    }
}

#[component]
fn CatalogState(ctx: DemoContext) -> impl IntoView {
    view! {
        <section class="theatre-state" role="status" aria-atomic="true">
            <h2>{move || ctx.text(match ctx.state.get().scenario {
                Scenario::Ready => Key::Ready, Scenario::Loading => Key::Loading,
                Scenario::Empty => Key::EmptyCatalog, Scenario::Error => Key::CatalogError,
            })}</h2>
            <p>{move || ctx.text(match ctx.state.get().scenario {
                Scenario::Ready => Key::DemoNotice, Scenario::Loading => Key::LoadingHelp,
                Scenario::Empty => Key::EmptyCatalogHelp, Scenario::Error => Key::CatalogErrorHelp,
            })}</p>
            <button type="button" class="theatre-action" on:click=move |_| ctx.dispatch(Event::SetScenario(Scenario::Ready))>
                {move || if ctx.state.get().scenario == Scenario::Error { ctx.text(Key::Retry) } else { ctx.text(Key::LoadDemo) }}
            </button>
        </section>
    }
}

#[component]
fn Discover(ctx: DemoContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::HomeHeading deck=Key::HomeDeck/>
        <Show when=move || ctx.state.get().scenario == Scenario::Ready fallback=move || view! { <CatalogState ctx=ctx/> }>
            <section class="theatre-feature">
                <div class="theatre-feature-copy">
                    <p class="theatre-eyebrow">{move || ctx.text(Key::Featured)}</p>
                    <h2 lang="vi-VN">{model::work(WorkId::Mountain).title}</h2>
                    <p lang="vi-VN">{model::work(WorkId::Mountain).synopsis}</p>
                    <p class="theatre-muted">{move || ctx.text(Key::ContentLanguage)}</p>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action" on:click=move |_| ctx.open_episode(EpisodeId { work: WorkId::Mountain, index: 0 })>
                            {icon("play_arrow")}{move || ctx.text(Key::Begin)}
                        </button>
                        <a class="theatre-text-link" href=Route::Work(WorkId::Mountain).hash()>{move || ctx.text(Key::AboutWork)}</a>
                    </div>
                </div>
                <div class="theatre-feature-cover"><Cover ctx=ctx id=WorkId::Mountain/></div>
            </section>
            <ContinueCard ctx=ctx/>
            <section aria-labelledby="theatre-shelf-heading">
                <h2 id="theatre-shelf-heading" class="theatre-section-heading">{move || ctx.text(Key::StoryShelf)}</h2>
                <div class="theatre-shelf">{WorkId::ALL.into_iter().map(move |id| view! { <WorkCard ctx=ctx id=id/> }).collect_view()}</div>
            </section>
        </Show>
    }
}

#[component]
fn Explore(ctx: DemoContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Explore/>
        <div class="theatre-search-field">
            <label for="theatre-search">{move || ctx.text(Key::SearchLabel)}</label>
            <input id="theatre-search" type="search" class="theatre-input" aria-describedby="theatre-search-help"
                prop:value=move || ctx.state.get().query
                on:input=move |event| ctx.dispatch(Event::Search(event_target_value(&event)))/>
            <p id="theatre-search-help" class="theatre-muted">{move || ctx.text(Key::SearchHelp)}</p>
        </div>
        <div class="theatre-filters" role="group" aria-label=move || ctx.text(Key::Filters)>
            {[Genre::All, Genre::Fantasy, Genre::Memory, Genre::Dream].into_iter().map(move |genre| view! {
                <button type="button" class="theatre-chip" aria-pressed=move || (ctx.state.get().genre == genre).to_string()
                    on:click=move |_| ctx.dispatch(Event::Filter(genre))>{move || ctx.text(messages::genre_key(genre))}</button>
            }).collect_view()}
        </div>
        <Show when=move || ctx.state.get().scenario == Scenario::Ready fallback=move || view! { <CatalogState ctx=ctx/> }>
            <Show when=move || !ctx.state.with(model::matching_works).is_empty() fallback=move || view! {
                <section class="theatre-state">
                    <h2>{move || ctx.text(Key::NoResults)}</h2><p>{move || ctx.text(Key::NoResultsHelp)}</p>
                    <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| {
                        ctx.dispatch(Event::Search(String::new())); ctx.dispatch(Event::Filter(Genre::All));
                    }>{move || ctx.text(Key::ClearSearch)}</button>
                </section>
            }>
                <div class="theatre-shelf">
                    <For each=move || ctx.state.with(model::matching_works) key=|id| *id children=move |id| view! { <WorkCard ctx=ctx id=id/> }/>
                </div>
            </Show>
        </Show>
    }
}

#[component]
fn WorkDetail(ctx: DemoContext, id: WorkId) -> impl IntoView {
    view! {
        <a class="theatre-back" href="#discover">{icon("arrow_back")}{move || ctx.text(Key::BackDiscover)}</a>
        <Show when=move || ctx.state.get().scenario == Scenario::Ready fallback=move || view! { <CatalogState ctx=ctx/> }>
            <section class="theatre-work-intro">
                <Cover ctx=ctx id=id/>
                <div>
                    <p class="theatre-eyebrow">{move || ctx.text(messages::genre_key(model::work(id).genre))}</p>
                    <h1 lang="vi-VN">{model::work(id).title}</h1>
                    <p class="theatre-synopsis" lang="vi-VN">{model::work(id).synopsis}</p>
                    <p class="theatre-muted">{move || ctx.text(Key::ContentLanguage)}<span aria-hidden="true">" · "</span>
                        {move || messages::episode_count(ctx.locale.get(), model::work(id).episodes.len())}</p>
                    <SaveAction ctx=ctx id=id/>
                </div>
            </section>
            <section aria-labelledby="theatre-episodes-heading">
                <h2 id="theatre-episodes-heading" class="theatre-section-heading">{move || ctx.text(Key::Episodes)}</h2>
                <ol class="theatre-episode-list">
                    {(0..model::work(id).episodes.len()).map(move |index| view! { <EpisodeRow ctx=ctx id=EpisodeId { work: id, index }/> }).collect_view()}
                </ol>
            </section>
        </Show>
    }
}

#[component]
fn EpisodeRow(ctx: DemoContext, id: EpisodeId) -> impl IntoView {
    let Some(episode) = model::episode(id) else { return ().into_any(); };
    view! {
        <li class="theatre-episode-row" data-current=move || ctx.state.get().player.is_some_and(|player| player.episode == id)>
            <span class="theatre-episode-number">{format!("{:02}", id.index + 1)}</span>
            <div class="theatre-episode-copy">
                <h3 lang="vi-VN">{episode.title}</h3>
                <p class="theatre-muted">{if episode.available { model::clock(episode.duration) } else { String::new() }}
                    <Show when=move || ctx.state.get().player.is_some_and(|player| player.episode == id)>
                        <span>{move || ctx.text(Key::CurrentEpisode)}</span>
                    </Show>
                </p>
                {(!episode.available).then(|| view! { <p id=format!("unavailable-{}-{}", id.work.slug(), id.index)>{move || ctx.text(Key::UnavailableHelp)}</p> })}
            </div>
            <div class="theatre-episode-actions">
                <button type="button" class="theatre-action theatre-action-tonal" disabled=!episode.available
                    aria-label=move || ctx.format(Key::PlayName, &[("title", episode.title.to_owned())])
                    aria-describedby=(!episode.available).then(|| format!("unavailable-{}-{}", id.work.slug(), id.index))
                    on:click=move |_| ctx.open_episode(id)>
                    {icon("play_arrow")}{move || ctx.text(if episode.available { Key::Begin } else { Key::Unavailable })}
                </button>
                {episode.available.then(|| view! {
                    <div class="theatre-queue-action">
                        <button type="button" class="theatre-action theatre-action-text"
                            disabled=move || ctx.state.with(|state| state.queue.contains(&id) || state.player.as_ref().is_some_and(|player| player.episode == id))
                            aria-label=move || ctx.format(Key::QueueName, &[("title", episode.title.to_owned())])
                            aria-describedby=format!("queue-reason-{}-{}", id.work.slug(), id.index)
                            on:click=move |_| ctx.dispatch(Event::Enqueue(id))>
                            {icon("add")}{move || ctx.text(Key::QueueAdd)}
                        </button>
                        <p id=format!("queue-reason-{}-{}", id.work.slug(), id.index) class="theatre-muted"
                            hidden=move || ctx.state.with(|state| !state.queue.contains(&id) && state.player.as_ref().is_none_or(|player| player.episode != id))>
                            {move || if ctx.state.get().player.is_some_and(|player| player.episode == id) { ctx.text(Key::CurrentEpisode) } else { ctx.text(Key::QueueAdded) }}
                        </p>
                    </div>
                })}
            </div>
        </li>
    }.into_any()
}

#[component]
fn ContinueCard(ctx: DemoContext) -> impl IntoView {
    view! {
        <Show when=move || ctx.state.get().player.is_some()>
            <section class="theatre-continue">
                <div><p class="theatre-eyebrow">{move || ctx.text(Key::ContinueHeading)}</p>
                    <h2 lang="vi-VN">{move || ctx.state.get().player.and_then(|player| model::episode(player.episode).map(|episode| episode.title)).unwrap_or_default()}</h2>
                    <p class="theatre-muted" lang="vi-VN">{move || ctx.state.get().player.map(|player| model::work(player.episode.work).title).unwrap_or_default()}</p>
                </div>
                <span class="theatre-time">{move || position_label(ctx)}</span>
                <a class="theatre-action" href="#player">{move || ctx.text(Key::Continue)}</a>
            </section>
        </Show>
    }
}

#[component]
fn Library(ctx: DemoContext) -> impl IntoView {
    let tab = Memo::new(move |_| ctx.state.get().library_tab);
    view! {
        <PageHeading ctx=ctx title=Key::Library/>
        <div class="theatre-filters" role="group" aria-label=move || ctx.text(Key::Library)>
            {[(LibraryTab::Continue, Key::Continue), (LibraryTab::Saved, Key::Saved), (LibraryTab::Bookmarks, Key::Bookmarks)].into_iter().map(move |(tab, label)| view! {
                <button type="button" class="theatre-chip" aria-pressed=move || (ctx.state.get().library_tab == tab).to_string()
                    on:click=move |_| ctx.dispatch(Event::LibraryTab(tab))>{move || ctx.text(label)}</button>
            }).collect_view()}
        </div>
        {move || match tab.get() {
            LibraryTab::Continue => view! {
                <Show when=move || ctx.state.get().player.is_some() fallback=move || view! { <Recovery ctx=ctx title=Key::NoLibrary help=Key::NoLibraryHelp/> }>
                    <ContinueCard ctx=ctx/>
                </Show>
            }.into_any(),
            LibraryTab::Saved => view! {
                <Show when=move || !ctx.state.get().saved.is_empty() fallback=move || view! { <Recovery ctx=ctx title=Key::NoLibrary help=Key::NoLibraryHelp/> }>
                    <div class="theatre-shelf"><For each=move || ctx.state.get().saved.into_iter().collect::<Vec<_>>() key=|id| *id children=move |id| view! { <WorkCard ctx=ctx id=id/> }/></div>
                </Show>
            }.into_any(),
            LibraryTab::Bookmarks => view! {
                <Show when=move || !ctx.state.get().bookmarks.is_empty() fallback=move || view! { <Recovery ctx=ctx title=Key::NoLibrary help=Key::NoLibraryHelp/> }>
                    <ul class="theatre-bookmarks"><For each=move || ctx.state.get().bookmarks key=|mark| (mark.episode, mark.position) children=move |mark| view! { <BookmarkRow ctx=ctx mark=mark/> }/></ul>
                </Show>
            }.into_any(),
        }}
    }
}

#[component]
fn BookmarkRow(ctx: DemoContext, mark: Bookmark) -> impl IntoView {
    let title = model::episode(mark.episode).map(|episode| episode.title).unwrap_or_default();
    view! {
        <li class="theatre-bookmark-row">
            <div><h3 lang="vi-VN">{title}</h3><p>{model::clock(mark.position)}</p></div>
            <button type="button" class="theatre-action theatre-action-tonal"
                aria-label=move || ctx.format(Key::BookmarkName, &[("title", title.to_owned()), ("time", model::clock(mark.position))])
                on:click=move |_| { ctx.dispatch(Event::ResumeBookmark(mark)); platform::navigate(ctx.route, Route::Player); }>
                {icon("play_arrow")}{move || ctx.text(Key::Continue)}
            </button>
            <button type="button" class="theatre-action theatre-action-text"
                aria-label=move || ctx.format(Key::RemoveBookmarkName, &[("title", title.to_owned()), ("time", model::clock(mark.position))])
                on:click=move |_| { ctx.dispatch(Event::RemoveBookmark(mark)); platform::focus_main(ctx.main); }>{icon("delete_outline")}{move || ctx.text(Key::RemoveBookmark)}</button>
        </li>
    }
}

fn position_label(ctx: DemoContext) -> String {
    ctx.state.with(|state| state.player.as_ref().and_then(|player| model::episode(player.episode)
        .map(|episode| format!("{} / {}", model::clock(player.position), model::clock(episode.duration)))).unwrap_or_default())
}

#[component]
fn TransportAction(ctx: DemoContext, #[prop(optional)] compact: bool) -> impl IntoView {
    view! {
        <button type="button" class="theatre-action theatre-transport" data-compact=compact
            on:click=move |_| ctx.dispatch(Event::ToggleTransport) aria-describedby="theatre-demo-notice">
            {move || icon(if ctx.state.get().player.is_some_and(|player| player.transport == Transport::Simulating) { "pause" } else { "play_arrow" })}
            <span>{move || ctx.text(match ctx.state.get().player.map(|player| player.transport) {
                Some(Transport::Simulating) => Key::Pause, Some(Transport::Ended) => Key::Replay,
                Some(Transport::Paused) | None => Key::Play,
            })}</span>
        </button>
    }
}

#[component]
fn Player(ctx: DemoContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Player/>
        <Show when=move || ctx.state.get().player.is_some() fallback=move || view! { <Recovery ctx=ctx title=Key::NoPlayer help=Key::NoPlayerHelp/> }>
            <section class="theatre-player">
                <div class="theatre-player-cover">{move || ctx.state.get().player.map(|player| view! { <Cover ctx=ctx id=player.episode.work/> })}</div>
                <div class="theatre-player-body">
                    <p class="theatre-eyebrow">{move || ctx.text(Key::DemoBadge)}</p>
                    <h2 lang="vi-VN">{move || ctx.state.get().player.and_then(|player| model::episode(player.episode).map(|episode| episode.title)).unwrap_or_default()}</h2>
                    <a class="theatre-text-link" lang="vi-VN" href=move || ctx.state.get().player.map(|player| Route::Work(player.episode.work).hash()).unwrap_or_else(|| Route::Discover.hash())>
                        {move || ctx.state.get().player.map(|player| model::work(player.episode.work).title).unwrap_or_default()}
                    </a>
                    <p role="status" aria-atomic="true">{move || ctx.state.get().player.map(|player| ctx.text(messages::transport_key(player.transport))).unwrap_or_default()}</p>
                    <label class="theatre-range-field" for="theatre-position">{move || ctx.text(Key::Timeline)}
                        <input id="theatre-position" type="range" min="0" step="1"
                            max=move || ctx.state.get().player.and_then(|player| model::episode(player.episode).map(|episode| episode.duration)).unwrap_or(0).to_string()
                            prop:value=move || ctx.state.get().player.map(|player| player.position).unwrap_or(0).to_string()
                            aria-valuetext=move || ctx.state.get().player.map(|player| messages::position_description(ctx.locale.get(), player.position, model::episode(player.episode).map(|episode| episode.duration).unwrap_or(0))).unwrap_or_default()
                            aria-describedby="theatre-timeline-help"
                            on:input=move |event| { if let Ok(position) = event_target_value(&event).parse() { ctx.dispatch(Event::Seek(position)); } }/>
                    </label>
                    <p class="theatre-time">{move || position_label(ctx)}</p>
                    <p id="theatre-timeline-help" class="theatre-muted">{move || ctx.text(Key::TimelineHelp)}</p>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.dispatch(Event::Skip(-15))>{icon("replay")}{move || ctx.text(Key::Back15)}</button>
                        <TransportAction ctx=ctx/>
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.dispatch(Event::Skip(15))>{icon("rotate_right")}{move || ctx.text(Key::Forward15)}</button>
                    </div>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-text" on:click=move |_| ctx.dispatch(Event::AddBookmark)>
                            {icon("bookmark_border")}{move || if ctx.state.with(|state| state.player.as_ref().is_some_and(|player| state.bookmarks.contains(&Bookmark { episode: player.episode, position: player.position }))) { ctx.text(Key::BookmarkAdded) } else { ctx.text(Key::AddBookmark) }}
                        </button>
                        <a class="theatre-text-link" href="#queue">{icon("queue_music")}{move || ctx.text(Key::Queue)}</a>
                        <a class="theatre-text-link" href="#settings">{icon("settings")}{move || ctx.text(Key::Settings)}</a>
                    </div>
                    <dl class="theatre-player-facts"><div><dt>{move || ctx.text(Key::Speed)}</dt><dd>{move || messages::speed_label(ctx.locale.get(), ctx.state.get().speed)}</dd></div></dl>
                    <UnsupportedAction ctx=ctx label=Key::Download/>
                    <UnsupportedAction ctx=ctx label=Key::Timer/>
                </div>
            </section>
        </Show>
    }
}

#[component]
fn MiniPlayer(ctx: DemoContext) -> impl IntoView {
    view! {
        <Show when=move || ctx.state.get().player.is_some() && !matches!(ctx.route.get(), Route::Player | Route::Queue)>
            <aside class="theatre-mini-player" aria-label=move || ctx.text(Key::MiniPlayer)>
                <a class="theatre-mini-title" href="#player">
                    {icon("headphones")}<span><strong lang="vi-VN">{move || ctx.state.get().player.and_then(|player| model::episode(player.episode).map(|episode| episode.title)).unwrap_or_default()}</strong>
                    <small>{move || ctx.state.get().player.map(|player| ctx.text(messages::transport_key(player.transport))).unwrap_or_default()}</small></span>
                </a>
                <span class="theatre-time">{move || position_label(ctx)}</span>
                <TransportAction ctx=ctx compact=true/>
            </aside>
        </Show>
    }
}

#[component]
fn Queue(ctx: DemoContext) -> impl IntoView {
    view! {
        <a class="theatre-back" href="#player">{icon("arrow_back")}{move || ctx.text(Key::CloseQueue)}</a>
        <PageHeading ctx=ctx title=Key::Queue/>
        <p class="theatre-muted">{move || ctx.text(Key::QueueEmptyHelp)}</p>
        <button type="button" class="theatre-action" disabled=move || ctx.state.get().queue.is_empty()
            aria-describedby="theatre-next-reason" on:click=move |_| { ctx.dispatch(Event::Next); platform::navigate(ctx.route, Route::Player); }>
            {icon("play_arrow")}{move || ctx.text(Key::Next)}
        </button>
        <p id="theatre-next-reason" hidden=move || !ctx.state.get().queue.is_empty()>{move || ctx.text(Key::NoNext)}</p>
        <Show when=move || !ctx.state.get().queue.is_empty() fallback=move || view! { <Recovery ctx=ctx title=Key::QueueEmpty help=Key::QueueEmptyHelp/> }>
            <ol class="theatre-episode-list"><For each=move || ctx.state.get().queue key=|id| *id children=move |id| {
                let title = model::episode(id).map(|episode| episode.title).unwrap_or_default();
                view! {
                    <li class="theatre-bookmark-row"><div><h2 lang="vi-VN">{title}</h2><p lang="vi-VN">{model::work(id.work).title}</p></div>
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.open_episode(id)
                            aria-label=move || ctx.format(Key::PlayName, &[("title", title.to_owned())])>{icon("play_arrow")}{move || ctx.text(Key::Begin)}</button>
                        <button type="button" class="theatre-action theatre-action-text" on:click=move |_| { ctx.dispatch(Event::RemoveQueue(id)); platform::focus_main(ctx.main); }
                            aria-label=move || ctx.format(Key::QueueRemoveName, &[("title", title.to_owned())])>{icon("delete_outline")}{move || ctx.text(Key::QueueRemove)}</button>
                    </li>
                }
            }/></ol>
        </Show>
    }
}

#[component]
fn Settings(ctx: DemoContext) -> impl IntoView {
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
                    value=Signal::derive(move || ctx.state.get().speed.to_string())
                    help=Signal::derive(move || Some(ctx.text(Key::SpeedHelp).to_owned()))
                    on_input=Callback::new(move |value: String| { if let Ok(speed) = value.parse() { ctx.dispatch(Event::Speed(speed)); } })>
                    <option value="75">{move || messages::speed_label(ctx.locale.get(), 75)}</option><option value="100">{move || messages::speed_label(ctx.locale.get(), 100)}</option><option value="125">{move || messages::speed_label(ctx.locale.get(), 125)}</option><option value="150">{move || messages::speed_label(ctx.locale.get(), 150)}</option><option value="200">{move || messages::speed_label(ctx.locale.get(), 200)}</option>
                </Select>
            </Stack>
        </Surface>
        <section class="theatre-demo-tools">
            <h2 class="theatre-section-heading">{move || ctx.text(Key::DemoTools)}</h2>
            <Select id="theatre-scenario" label=Signal::derive(move || ctx.text(Key::Scenario).to_owned())
                value=Signal::derive(move || ctx.state.get().scenario.value().to_owned())
                on_input=Callback::new(move |value: String| { if let Some(scenario) = Scenario::from_value(&value) { ctx.dispatch(Event::SetScenario(scenario)); } })>
                <option value="ready">{move || ctx.text(Key::Ready)}</option><option value="loading">{move || ctx.text(Key::Loading)}</option>
                <option value="empty">{move || ctx.text(Key::Empty)}</option><option value="error">{move || ctx.text(Key::Error)}</option>
            </Select>
            <p class="theatre-muted">{move || ctx.text(Key::ResetHelp)}</p>
            <div class="theatre-actions">
                <button type="button" class="theatre-action" on:click=move |_| ctx.dispatch(Event::Reset)>{move || ctx.text(Key::Reset)}</button>
                <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.dispatch(Event::ClearLibrary)>{move || ctx.text(Key::ClearLibrary)}</button>
                <a class="theatre-text-link" href="#discover">{move || ctx.text(Key::BackDiscover)}</a>
            </div>
        </section>
    }
}

#[component]
fn UnsupportedAction(ctx: DemoContext, label: Key) -> impl IntoView {
    let reason_id = match label {
        Key::Download => "theatre-download-reason",
        Key::Timer => "theatre-timer-reason",
        Key::SignIn => "theatre-sign-in-reason",
        Key::Sync => "theatre-sync-reason",
        _ => "theatre-unsupported-reason",
    };
    view! {
        <div class="theatre-unsupported">
            <button type="button" class="theatre-action theatre-action-tonal" disabled aria-describedby=reason_id>{move || ctx.text(label)}</button>
            <p id=reason_id class="theatre-muted">{move || ctx.text(Key::Unsupported)}</p>
        </div>
    }
}

#[component]
fn Account(ctx: DemoContext) -> impl IntoView {
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
fn Recovery(ctx: DemoContext, title: Key, help: Key) -> impl IntoView {
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

    #[test]
    fn listener_title_is_the_user_facing_name() {
        let index = include_str!("../index.html");
        assert!(index.contains("<title>Cantos</title>"));
        assert!(!index.contains("Cantos Theatre"));
        assert!(index.contains("data-bin=\"cantos-theatre\""));
    }

    #[test]
    fn mock_shell_is_explicit_and_never_mounts_media_or_studio_authority() {
        let owner = Owner::new();
        owner.with(|| {
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
    fn presentation_flow_uses_the_same_mock_core_across_screens() {
        let owner = Owner::new();
        owner.with(|| {
            let ctx = DemoContext { state: RwSignal::new(DemoState::default()), route: RwSignal::new(Route::Discover), locale: RwSignal::new(Locale::English), theme: RwSignal::new(Theme::Light), main: NodeRef::new() };
            ctx.dispatch(Event::AddBookmark);
            ctx.open_episode(EpisodeId { work: WorkId::River, index: 0 });
            assert_eq!(ctx.route.get_untracked(), Route::Player);
            let html = view! { <Player ctx=ctx/> }.to_html();
            assert!(html.contains("Ngọn đèn đầu ngõ"));
            assert!(html.contains("lang=\"vi-VN\""));
            assert!(html.contains("Mock position"));
            assert!(html.contains("theatre-position"));
            assert!(html.contains("Not integrated in this mock"));
            ctx.dispatch(Event::ResumeBookmark(Bookmark { episode: EpisodeId { work: WorkId::Mountain, index: 2 }, position: 768 }));
            assert_eq!(position_label(ctx), "12:48 / 28:42");
            ctx.dispatch(Event::Reset);
            assert_eq!(ctx.state.get_untracked(), DemoState::default());
        });
    }
}
