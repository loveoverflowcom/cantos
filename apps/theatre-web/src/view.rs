use crate::{
    demo::{matching_works, WORKS},
    messages::{route_key, state_key, text, Key},
    navigation::{DemoState, Route},
};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

fn current_route() -> Route {
    web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .map_or(Route::Discover, |hash| Route::from_hash(&hash))
}

fn focus_title() {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("page-title"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}

#[component]
fn WorkList(english: RwSignal<bool>, query: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="work-grid">
            {move || matching_works(&query.get()).into_iter().enumerate().map(|(index, work)| view! {
                <article class="work-card">
                    <div class="cover-placeholder" role="img" aria-label=move || text(Key::MissingArt, english.get())>
                        <span class="edition">{format!("{:02}", index + 1)}</span><span>{work.title}</span>
                    </div>
                    <h3>{work.title}</h3>
                    <p>{move || if english.get() { work.genre_en } else { work.genre_vi }}</p>
                    {if index == 0 && work.title == WORKS[0].title {
                        view! { <a href=Route::Work.hash()>{move || text(Key::Detail, english.get())}</a> }.into_any()
                    } else {
                        view! { <p class="muted">{move || text(Key::NotImplemented, english.get())}</p> }.into_any()
                    }}
                </article>
            }).collect_view()}
        </div>
        <Show when=move || matching_works(&query.get()).is_empty()>
            <p role="status">{move || text(Key::Empty, english.get())}</p>
        </Show>
    }
}

#[component]
fn Player(english: RwSignal<bool>) -> impl IntoView {
    view! {
        <section class="player-panel" aria-labelledby="episode-title">
            <div class="cover-placeholder player-cover" role="img" aria-label=move || text(Key::MissingArt, english.get())><span>"Sơn Hà Ký"</span></div>
            <div>
                <p class="eyebrow">"Sơn Hà Ký · 01"</p>
                <h2 id="episode-title">{move || text(Key::Episode, english.get())}</h2>
                <p id="playback-reason">{move || text(Key::PlaybackUnavailable, english.get())}</p>
                <div class="transport">
                    <button disabled aria-describedby="playback-reason">{move || text(Key::Play, english.get())}</button>
                    <button disabled aria-describedby="playback-reason">{move || text(Key::Save, english.get())}</button>
                    <button disabled aria-describedby="playback-reason">{move || text(Key::Download, english.get())}</button>
                </div>
            </div>
        </section>
    }
}

#[component]
pub fn Theatre() -> impl IntoView {
    let route = RwSignal::new(current_route());
    let english = RwSignal::new(false);
    let theme = RwSignal::new(String::from("system"));
    let state = RwSignal::new(DemoState::Ready);
    let query = RwSignal::new(String::new());
    let discovery_query = RwSignal::new(String::new());
    let navigation =
        leptos::leptos_dom::helpers::window_event_listener_untyped("hashchange", move |_| {
            route.set(current_route());
            state.set(DemoState::Ready);
            focus_title();
        });
    on_cleanup(move || navigation.remove());
    view! {
        <div class="theatre-shell" data-theme=move || theme.get() lang=move || if english.get() { "en" } else { "vi-VN" }>
            <a class="skip" href="#main-content" on:click=move |event| {
                event.prevent_default();
                focus_title();
            }>{move || text(Key::Skip, english.get())}</a>
            <aside class="sidebar">
                <a class="brand" href=Route::Discover.hash()>"cantos"<small>"VỌNG ĐÀI · THEATRE"</small></a>
                <nav aria-label=move || text(Key::Navigation, english.get())>
                    {[Route::Discover, Route::Explore, Route::Library, Route::Player].into_iter().map(|item| view! {
                        <a href=item.hash() aria-current=move || if route.get() == item { Some("page") } else { None }>
                            {move || text(route_key(item), english.get())}
                        </a>
                    }).collect_view()}
                </nav>
                <p class="side-note">"Theatre of Voices"</p>
            </aside>
            <div class="theatre-body">
                <header class="topbar">
                    <span>"Cantos Theatre"</span>
                    <div class="settings">
                        <button on:click=move |_| english.update(|value| *value = !*value)>{move || if english.get() { "Tiếng Việt" } else { "English" }}</button>
                        <label for="theme">{move || text(Key::Theme, english.get())}</label>
                        <select id="theme" prop:value=move || theme.get() on:change=move |event| theme.set(event_target_value(&event))>
                            <option value="system">{move || text(Key::System, english.get())}</option>
                            <option value="light">{move || text(Key::Light, english.get())}</option>
                            <option value="dark">{move || text(Key::Dark, english.get())}</option>
                        </select>
                    </div>
                </header>
                <main id="main-content" tabindex="-1">
                    <p class="eyebrow">"VỌNG ĐÀI · THEATRE OF VOICES"</p>
                    <h1 id="page-title" tabindex="-1">{move || if route.get() == Route::Discover { text(Key::Tagline, english.get()) } else { text(route_key(route.get()), english.get()) }}</h1>
                    <p class="demo-notice">{move || text(Key::Demo, english.get())}</p>
                    <details class="demo-controls">
                        <summary>{move || text(Key::DemoStates, english.get())}</summary>
                        <label for="demo-state">{move || text(Key::DemoStates, english.get())}</label>
                        <select id="demo-state" prop:value=move || state.get().control_value() on:change=move |event| state.set(DemoState::from_control(&event_target_value(&event)))>
                            <option value="ready">{move || text(Key::Ready, english.get())}</option>
                            <option value="loading">{move || text(Key::Loading, english.get())}</option>
                            <option value="empty">{move || text(Key::Empty, english.get())}</option>
                            <option value="error">{move || text(Key::Error, english.get())}</option>
                            <option value="not-implemented">{move || text(Key::NotImplemented, english.get())}</option>
                        </select>
                    </details>
                    {move || if state.get() != DemoState::Ready {
                        view! {
                            <section class="state-panel" aria-live="polite">
                                <h2>{text(state_key(state.get()), english.get())}</h2>
                                <button on:click=move |_| state.set(DemoState::Ready)>{move || text(Key::Reset, english.get())}</button>
                            </section>
                        }.into_any()
                    } else { match route.get() {
                        Route::Discover => view! {
                            <section class="feature" aria-labelledby="featured-title">
                                <div><p class="eyebrow">{move || text(Key::Featured, english.get())}</p>
                                    <h2 id="featured-title">"Sơn Hà Ký"</h2><p class="synopsis">{move || text(Key::Synopsis, english.get())}</p>
                                    <a class="primary-link" href=Route::Work.hash()>{move || text(Key::Detail, english.get())}</a>
                                </div>
                                <div class="cover-placeholder" role="img" aria-label=move || text(Key::MissingArt, english.get())><span>"SƠN HÀ KÝ"</span><small>{move || text(Key::MissingArt, english.get())}</small></div>
                            </section>
                            <section><p class="eyebrow">{move || text(Key::Genre, english.get())}</p><h2>{move || text(Key::Stories, english.get())}</h2><WorkList english=english query=discovery_query /></section>
                        }.into_any(),
                        Route::Explore => view! {
                            <label for="search">{move || text(Key::Search, english.get())}</label>
                            <input id="search" type="search" aria-describedby="search-hint" prop:value=move || query.get() on:input=move |event| query.set(event_target_value(&event)) />
                            <p id="search-hint" class="muted">{move || text(Key::SearchHint, english.get())}</p>
                            <WorkList english=english query=query />
                        }.into_any(),
                        Route::Library => view! { <section class="state-panel"><p>{move || text(Key::LibraryUnavailable, english.get())}</p><a href=Route::Discover.hash()>{move || text(Key::Back, english.get())}</a></section> }.into_any(),
                        Route::Work => view! {
                            <section><h2>"Sơn Hà Ký"</h2><p class="synopsis">{move || text(Key::Synopsis, english.get())}</p>
                                <h3>{move || text(Key::Episodes, english.get())}</h3>
                                <div class="episode-row"><span>"01 · "{move || text(Key::Episode, english.get())}</span><a class="primary-link" href=Route::Player.hash()>{move || text(Key::PlayerOpen, english.get())}</a></div>
                            </section>
                        }.into_any(),
                        Route::Player => view! { <Player english=english /> }.into_any(),
                        Route::NotFound => view! { <section class="state-panel"><a href=Route::Discover.hash()>{move || text(Key::Back, english.get())}</a></section> }.into_any(),
                    }} }
                </main>
                <footer class="mini-player" aria-label=move || text(Key::Mini, english.get())>
                    <a href=Route::Player.hash()>{move || text(Key::Mini, english.get())}</a>
                    <button disabled aria-describedby="mini-reason">{move || text(Key::Play, english.get())}</button>
                    <small id="mini-reason">{move || text(Key::Demo, english.get())}</small>
                </footer>
            </div>
        </div>
    }
}
