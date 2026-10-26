//! Debug-only injected composition of the real structured editor. No HTTP, auth or storage.
//! Findings are synthetic fixtures with real rule codes; they are never validator output.
use crate::{
    authoring::{Document, Field, Target},
    authoring_view::StructuredEditor,
    contract::FieldIssue,
    messages::{authoring::preview_copy, Locale},
};
use cantos_ui::Theme;
use leptos::prelude::*;

const FIXTURE: &str = include_str!(
    "../../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json"
);
pub const RECIPES: [&str; 4] = ["editable", "invalid", "readonly", "long"];

pub fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|query| query == "?preview=studio-authoring")
}

fn issue(document: &Document, target: &Target, field: Field, rule: &str) -> Option<FieldIssue> {
    document
        .diagnostic_paths(target, field)
        .ok()
        .and_then(|paths| paths.into_iter().find(|path| path.starts_with('/')))
        .map(|path| FieldIssue {
            path,
            rule: rule.into(),
        })
}

/// One recipe is one fixed draft, finding set and permission; nothing is derived from a session.
fn fixture(recipe: &str) -> (String, Vec<FieldIssue>, bool) {
    let Ok(base) = Document::parse(FIXTURE) else {
        return (FIXTURE.into(), vec![], true);
    };
    let dialogue = Target::Dialogue("dialogue-02".into());
    let edits: &[(Target, Field, &str)] = match recipe {
        "invalid" => &[
            (Target::Work, Field::Title, ""),
            (Target::Dialogue("dialogue-02".into()), Field::Intensity, "1200"),
            (Target::Dialogue("dialogue-03".into()), Field::Speaker, "minh-cu"),
        ],
        "long" => &[
            (Target::Work, Field::Title, "Ánh đèn cuối sân khấu và những lời hẹn chưa kịp nói của đoàn kịch Vọng Đài"),
            (Target::Episode, Field::Title, "Một lời hẹn dưới mái hiên nhà hát cũ, khi cơn mưa đầu mùa vừa tạnh"),
            (Target::Dialogue("dialogue-02".into()), Field::Text, "Ngày mai, mình có diễn tiếp không? Nếu đèn sân khấu lại tắt giữa chừng, cậu có còn đứng đó đọc nốt lời thoại cuối cùng cho khán giả nghe không?"),
        ],
        _ => &[],
    };
    let document = edits.iter().fold(base, |document, (target, field, value)| {
        document.set(target, *field, value).unwrap_or(document)
    });
    let issues = if recipe == "invalid" {
        [
            issue(&document, &Target::Work, Field::Title, "empty_text"),
            issue(
                &document,
                &Target::Dialogue("dialogue-03".into()),
                Field::Speaker,
                "unknown_speaker",
            ),
            issue(&document, &dialogue, Field::Intensity, "intensity_range"),
        ]
        .into_iter()
        .flatten()
        .collect()
    } else {
        vec![]
    };
    (document.to_json(), issues, recipe == "readonly")
}

#[component]
pub fn Preview(theme: RwSignal<Theme>) -> impl IntoView {
    let locale_choice = RwSignal::new(Locale::default());
    let locale = Signal::derive(move || locale_choice.get());
    let recipe = RwSignal::new(RECIPES[0].to_owned());
    let commits = RwSignal::new(0_usize);
    let copy = move || preview_copy(locale.get());
    Effect::new(move |_| {
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            let _ = root.set_attribute("lang", locale.get().lang());
            let _ = root.set_attribute("dir", locale.get().dir());
        }
    });
    view! {
        <main class="studio-main" lang=move || locale.get().lang() dir=move || locale.get().dir()>
            <h1>{move || copy().title}</h1>
            <p class="help">{move || copy().help}</p>
            <div class="toolbar">
                <label for="preview-recipe">{move || copy().scenario}</label>
                <select id="preview-recipe" on:change=move |event| {
                    let value = event_target_value(&event);
                    if RECIPES.contains(&value.as_str()) { commits.set(0); recipe.set(value); }
                }>{RECIPES.map(|id| view! { <option value=id>{id}</option> })}</select>
                <label for="preview-theme">{move || copy().theme}</label>
                <select id="preview-theme" on:change=move |event| if let Some(selected) = Theme::from_attribute(&event_target_value(&event)) { theme.set(selected); }>
                    <option value="light">{move || copy().light}</option><option value="dark">{move || copy().dark}</option>
                </select>
                <label for="preview-locale">{move || copy().locale}</label>
                <select id="preview-locale" on:change=move |event| if let Some(selected) = Locale::from_lang(&event_target_value(&event)) { locale_choice.set(selected); }>
                    {Locale::supported().map(|locale| view! { <option value=locale.lang() lang=locale.lang()>{locale.autonym()}</option> }).collect_view()}
                </select>
                <label for="preview-commits">{move || copy().commits}</label><output id="preview-commits">{move || commits.get()}</output>
            </div>
            // A recipe change remounts the editor; theme and locale changes keep its mounted draft.
            {move || {
                let (source, issues, readonly) = fixture(&recipe.get());
                let draft = RwSignal::new(source.clone());
                let issues = RwSignal::new(issues);
                let activity = RwSignal::new(false);
                // Like the revision shell, an accepted edit clears findings in the same flush.
                view! {
                    <StructuredEditor draft=draft.into() locale=locale readonly=Signal::stored(readonly) namespace="preview"
                        baseline=Signal::stored(source) issues=issues.into() activity=activity
                        on_change=Callback::new(move |text: String| { draft.set(text); issues.set(vec![]); commits.update(|count| *count += 1); })>
                        <span></span>
                    </StructuredEditor>
                }
            }}
        </main>
    }
}
