//! Studio presentation adapters. Values, permissions and effects remain app-owned.
use cantos_ui::{
    Button, ButtonState, ButtonVariant, ChoiceState, Dialog, DialogError, DisabledReason,
    ErrorState, IconName, LinkHref, NavItem, Select,
};
use leptos::{ev::MouseEvent, prelude::*};

use crate::messages::{
    self,
    workspace::{text, Key},
};

/// Authored content keeps its declared language; this adapter has no interface-locale input.
#[component]
pub fn ContentText(language: Signal<Option<String>>, text: Signal<String>) -> impl IntoView {
    view! { <span lang=move ||language.get()>{move ||text.get()}</span> }
}

#[cfg(test)]
mod content_text_tests {
    use super::*;
    use crate::authoring::{Document, Field, Target};

    #[test]
    fn english_document_keeps_vietnamese_title_and_its_tag_when_interface_locale_changes() {
        let owner = Owner::new();
        owner.with(|| {
            let baseline = Document::parse(include_str!(
                "../../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json"
            )).unwrap();
            let document = baseline.clone()
                .set(&Target::Adaptation, Field::Language, "en").unwrap()
                .set(&Target::Work, Field::Title, "Lời hẹn {time}").unwrap();
            let source = document.to_json();
            let baseline = RwSignal::new(baseline);
            let document = RwSignal::new(document);
            let english = RwSignal::new(false);
            for en in [false, true] {
                english.set(en);
                let html = view! {
                    <h3>{move ||crate::messages::copy(english.get()).theme}</h3>
                    <ContentText language=Signal::derive(move ||Some(baseline.get().metadata().language))
                        text=Signal::derive(move ||baseline.get().metadata().work_title)/>
                    <ContentText language=Signal::derive(move ||Some(document.get().metadata().language))
                        text=Signal::derive(move ||document.get().metadata().work_title)/>
                }.to_html();
                assert!(html.contains("<span lang=\"en\">Lời hẹn {time}</span>"), "{html}");
                assert!(html.contains("<span lang=\"vi-VN\">Ánh đèn cuối sân khấu</span>"), "{html}");
                assert!(html.contains(if en { "Appearance" } else { "Giao diện" }), "{html}");
                assert_eq!(document.get_untracked().to_json(), source);
            }
        });
    }
}

/// App-owned options share the exact value used by the CreateWork DTO.
#[component]
pub fn WorkKindOptions(value: RwSignal<String>, english: Signal<bool>) -> impl IntoView {
    view! { <option value="novel" selected=move || value.get()=="novel">{move || crate::live::copy::work_kind("novel", english.get())}</option><option value="story" selected=move || value.get()=="story">{move || crate::live::copy::work_kind("story", english.get())}</option><option value="podcast" selected=move || value.get()=="podcast">{move || crate::live::copy::work_kind("podcast", english.get())}</option> }
}

#[cfg(test)]
mod work_kind_tests {
    use super::*;
    #[test]
    fn work_kind_options_display_vietnamese_labels_instead_of_protocol_values() {
        let owner = Owner::new();
        owner.with(|| {
            let kind = RwSignal::new(crate::live::DEFAULT_WORK_KIND.to_owned());
            let html =
                view! { <WorkKindOptions value=kind english=Signal::derive(|| false)/> }.to_html();
            assert!(html.contains(">Tiểu thuyết</option>"), "{html}");
            assert!(html.contains(">Truyện</option>"), "{html}");
        });
    }

    #[test]
    fn work_kind_display_and_payload_agree_for_default_changes_and_reset() {
        let owner = Owner::new();
        owner.with(|| {
            let kind = RwSignal::new(crate::live::DEFAULT_WORK_KIND.to_owned());
            for value in [
                crate::live::DEFAULT_WORK_KIND,
                "novel",
                "podcast",
                "story",
                crate::live::DEFAULT_WORK_KIND,
            ] {
                kind.set(value.to_owned());
                let english = RwSignal::new(false);
                for (locale, expected) in [(false, ["Tiểu thuyết", "Truyện", "Podcast"]), (true, ["Novel", "Story", "Podcast"])] {
                    english.set(locale);
                    let html = view! { <WorkKindOptions value=kind english=Signal::derive(move || english.get())/> }.to_html();
                    let chosen = format!("value=\"{value}\" selected");
                    assert!(html.contains(&chosen), "{html}");
                    assert_eq!(html.matches(" selected").count(), 1, "{html}");
                    for label in expected { assert!(html.contains(label), "{html}"); }
                    assert_eq!(kind.get_untracked(), value);
                }
                assert_eq!(kind.get_untracked(), value);
                let mut request: crate::contract::CreateWorkRequest =
                    serde_json::from_str(include_str!(
                        "../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
                    ))
                    .unwrap();
                request.kind = kind.get_untracked();
                let payload = crate::live::Command::CreateWork(request)
                    .json()
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&payload).unwrap()["kind"],
                    value
                );
            }
        });
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkspacePane {
    #[default]
    Dashboard,
    Import,
    Proposal,
    Revisions,
    Production,
    Content,
}

impl WorkspacePane {
    pub const ALL: [Self; 6] = [
        Self::Dashboard,
        Self::Import,
        Self::Proposal,
        Self::Revisions,
        Self::Production,
        Self::Content,
    ];

    pub fn from_hash(hash: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|pane| format!("#{}", pane.id()) == hash)
            .unwrap_or(Self::Dashboard)
    }
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "workspace-dashboard",
            Self::Import => "workspace-import",
            Self::Proposal => "workspace-proposal",
            Self::Revisions => "workspace-revisions",
            Self::Production => "workspace-production",
            Self::Content => "workspace-content",
        }
    }

    pub const fn icon(self) -> IconName {
        match self {
            Self::Dashboard => IconName::Home,
            Self::Content => IconName::MenuBook,
            Self::Import => IconName::Upload,
            Self::Proposal => IconName::Transform,
            Self::Revisions => IconName::Edit,
            Self::Production => IconName::Mic,
        }
    }

    pub fn label(self, english: bool) -> &'static str {
        match self {
            Self::Dashboard => {
                crate::messages::entry::text(crate::messages::entry::Key::Dashboard, english)
            }
            Self::Content => crate::live::copy::text(crate::live::copy::Key::Pane, english),
            Self::Import => text(Key::Import, english),
            Self::Proposal => text(Key::Proposal, english),
            Self::Revisions => text(Key::Revisions, english),
            Self::Production => {
                messages::production::text(messages::production::Key::Pane, english)
            }
        }
    }
}

fn pane_selection(panel: RwSignal<WorkspacePane>, pane: WorkspacePane) -> Callback<bool> {
    Callback::new(move |_| panel.set(pane))
}

/// Only a plain primary click switches the mounted pane in place. A modified click keeps the
/// browser's native open-in-new-tab behaviour through the pane's hash `href`.
fn switches_in_place(button: i16, ctrl: bool, meta: bool, shift: bool, alt: bool) -> bool {
    button == 0 && !(ctrl || meta || shift || alt)
}

fn pane_href(pane: WorkspacePane) -> Option<LinkHref> {
    LinkHref::new(format!("#{}", pane.id())).ok()
}

/// This changes visibility only. All four app panels stay mounted with their draft state.
/// One set of links serves every width: a column in the rail, a wrapped row below 64rem and a
/// horizontally scrollable strip below 45rem, always with icon and label.
#[component]
pub fn WorkspaceNavigation(panel: RwSignal<WorkspacePane>, english: Signal<bool>) -> impl IntoView {
    view! {
        <nav class="workspace-nav" aria-label=move || text(Key::Workspace, english.get())>
            <div class="workspace-nav-list">
            {WorkspacePane::ALL.into_iter().filter_map(|pane| pane_href(pane).map(|href| view! {
                <NavItem id=format!("{}-control", pane.id()) href=href
                    icon=pane.icon() current=Signal::derive(move || panel.get() == pane)
                    on_press=Callback::new(move |event: MouseEvent| {
                        if switches_in_place(event.button(), event.ctrl_key(), event.meta_key(), event.shift_key(), event.alt_key()) {
                            event.prevent_default();
                            pane_selection(panel, pane).run(true);
                        }
                    })>
                    {move || pane.label(english.get())}
                </NavItem>
            })).collect_view()}
            </div>
        </nav>
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum BlockReason {
    #[default]
    Busy,
    ContextReady,
    PageNavigation,
    SaveReview,
    Pending,
    ReadOnly,
    /// A structured script choice while the record or actor cannot edit.
    ScriptReadOnly,
    ProductionReview,
    Artwork(crate::live::artwork::copy::Key),
    #[cfg(debug_assertions)]
    Forge(crate::messages::forge::Key),
}

impl BlockReason {
    pub fn text(self, english: bool) -> &'static str {
        let copy = messages::copy(english);
        match self {
            #[cfg(debug_assertions)]
            Self::Forge(key) => crate::messages::forge::text(key, english),
            Self::Artwork(key) => crate::live::artwork::copy::text(key, english),
            Self::Busy => copy.busy,
            Self::ContextReady => text(Key::ControlBlocked, english),
            Self::PageNavigation => {
                crate::live::copy::text(crate::live::copy::Key::PageUnavailable, english)
            }
            Self::SaveReview => text(Key::ValidateFirst, english),
            Self::Pending => copy.pending,
            Self::ReadOnly => text(Key::ControlReadOnly, english),
            Self::ScriptReadOnly => messages::authoring::authoring_copy(english).readonly_choice,
            Self::ProductionReview => {
                messages::production::text(messages::production::Key::ValidationHelp, english)
            }
        }
    }
}

/// Validate both locales at mount. Invalid catalog text removes the control, never enables it.
#[derive(Clone)]
pub(crate) struct LocalizedReason {
    vi: DisabledReason,
    en: DisabledReason,
}

impl LocalizedReason {
    pub(crate) fn new(reason: BlockReason) -> Result<Self, cantos_ui::EmptyDisabledReason> {
        Ok(Self {
            vi: DisabledReason::new(reason.text(false))?,
            en: DisabledReason::new(reason.text(true))?,
        })
    }

    pub(crate) fn get(&self, english: bool) -> DisabledReason {
        if english {
            self.en.clone()
        } else {
            self.vi.clone()
        }
    }
}

pub(crate) fn action_state(blocked: bool, busy: bool, reason: DisabledReason) -> ButtonState {
    if busy {
        ButtonState::Loading(reason)
    } else if blocked {
        ButtonState::Disabled(reason)
    } else {
        ButtonState::Enabled
    }
}

/// Map existing app guards to the reusable action contract without inferring permissions.
#[component]
pub fn ActionButton(
    #[prop(into)] id: String,
    english: Signal<bool>,
    children: Children,
    on_press: Callback<MouseEvent>,
    #[prop(optional)] focus_on_press: bool,
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] icon: Option<IconName>,
    #[prop(into, default = false.into())] blocked: Signal<bool>,
    #[prop(into, default = false.into())] busy: Signal<bool>,
    #[prop(default = BlockReason::ContextReady)] reason: BlockReason,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
) -> impl IntoView {
    let (Ok(reason), Ok(loading)) = (
        LocalizedReason::new(reason),
        LocalizedReason::new(BlockReason::Busy),
    ) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, english.get_untracked())><p>{text(Key::PermissionHelp, english.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reasons = StoredValue::new((reason, loading));
    view! {
        <Button id=id variant=variant nostrip:icon=icon described_by=described_by on_press=on_press focus_on_press=focus_on_press
            state=Signal::derive(move || {
                let busy = busy.get();
                let reason = reasons.with_value(|(blocked, loading)| if busy { loading } else { blocked }.get(english.get()));
                action_state(blocked.get(), busy, reason)
            })>{children()}</Button>
    }.into_any()
}

/// Real form submission stays native (including Enter); the form owns the submit guard/effect.
/// The optional decorative slot takes a caller-owned view; adopting Icon at a new product site
/// still requires its two-renderer qualification. No interactive content belongs in this slot.
/// Tonal by default; the form that is the screen's current task passes `Primary`.
#[component]
pub fn FormSubmit(
    #[prop(into)] id: String,
    english: Signal<bool>,
    blocked: Signal<bool>,
    children: Children,
    #[prop(optional)] icon: Option<ViewFn>,
    #[prop(into, default = false.into())] busy: Signal<bool>,
    #[prop(default = ButtonVariant::Tonal)] variant: ButtonVariant,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(BlockReason::ContextReady) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, english.get_untracked())><p>{text(Key::PermissionHelp, english.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reason = StoredValue::new(reason);
    let reason_id = format!("{id}-reason");
    let described_by = reason_id.clone();
    view! {
        <div class="cui-button-field">
            <button id=id class="cui-button" data-variant=variant.as_str() type="submit" disabled=move || blocked.get() || busy.get()
                aria-busy=move || busy.get().then_some("true")
                aria-describedby=move || (blocked.get() || busy.get()).then(|| described_by.clone())>
                {move || if busy.get() {
                    Some(view! { <span class="cui-button-icon" aria-hidden="true"><span class="cui-action-progress"></span></span> }.into_any())
                } else { icon.clone().map(|icon| view! { <span class="cui-button-icon" aria-hidden="true">{icon.run()}</span> }.into_any()) }}
                <span class="cui-control-label">{children()}</span>
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || !blocked.get() && !busy.get()
                role=move || busy.get().then_some("status")>{move || if busy.get() {
                    BlockReason::Busy.text(english.get()).to_owned()
                } else { reason.with_value(|r| r.get(english.get()).as_str().to_owned()) }}</span>
        </div>
    }.into_any()
}

fn confirm_discard_request(blocked: bool, open: RwSignal<bool>, confirm: Callback<(), bool>) {
    if open.get_untracked() && !blocked && confirm.run(()) {
        open.set(false);
    }
}

/// Debug composition only. Indirect use here supplies no shared admission or live authority.
#[cfg(debug_assertions)]
#[component]
pub fn ForgeSyncDialog(
    open: Signal<bool>,
    english: Signal<bool>,
    details: Signal<String>,
    on_cancel: Callback<()>,
    on_confirm: Callback<MouseEvent>,
    on_error: Callback<DialogError>,
) -> impl IntoView {
    use crate::messages::forge::{text as copy, Key as ForgeKey};
    move || {
        let language = english.get();
        view! {
            <Dialog id="forge-sync-dialog" title=copy(ForgeKey::Plan, language)
                close_label=copy(ForgeKey::Close, language) open=open
                on_close_request=Callback::new(move |_| on_cancel.run(())) on_error=on_error
                description=copy(ForgeKey::PlanHelp, language)
                actions=move || view! {
                    <ActionButton id="forge-sync-cancel" english=english variant=ButtonVariant::Text
                        icon=IconName::Close on_press=Callback::new(move |_| on_cancel.run(()))>
                        {move || copy(ForgeKey::Cancel, english.get())}
                    </ActionButton>
                    <ActionButton id="forge-sync-confirm" english=english icon=IconName::CloudSync
                        on_press=on_confirm>{move || copy(ForgeKey::Confirm, english.get())}</ActionButton>
                }>
                <p class="forge-plan-identity">{move || details.get()}</p>
            </Dialog>
        }
    }
}

/// App-owned confirmation targets share the established modal/focus owner. Shared Dialog
/// imports stay at this existing boundary; no new shared component qualification is inferred.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DiscardTarget {
    #[default]
    Draft,
    Artwork(String),
}
/// Only explicit confirmation dispatches the caller's action. Escape/cancel preserve its draft.
#[component]
pub fn DiscardDraftDialog(
    open: RwSignal<bool>,
    english: Signal<bool>,
    blocked: Signal<bool>,
    on_confirm: Callback<(), bool>,
    on_error: Callback<DialogError>,
    #[prop(into, default = DiscardTarget::Draft.into())] target: Signal<DiscardTarget>,
) -> impl IntoView {
    // Structural locale/target changes remount chrome, never app draft state.
    move || {
        use crate::live::artwork::copy::{text as artwork_text, Key as ArtworkKey};
        let language = english.get();
        let (id, cancel_id, confirm_id, title, cancel, confirm, description, context, reason) =
            match target.get() {
                DiscardTarget::Draft => (
                    "revision-discard-dialog",
                    "revision-discard-cancel",
                    "revision-discard-confirm",
                    text(Key::Discard, language),
                    text(Key::Cancel, language),
                    text(Key::DiscardNow, language),
                    text(Key::DiscardConfirm, language),
                    text(Key::PermissionHelp, language).to_owned(),
                    BlockReason::ContextReady,
                ),
                DiscardTarget::Artwork(context) => (
                    "artwork-clear-dialog",
                    "artwork-clear-cancel",
                    "artwork-confirm-clear",
                    artwork_text(ArtworkKey::Clear, language),
                    artwork_text(ArtworkKey::Cancel, language),
                    artwork_text(ArtworkKey::Confirm, language),
                    artwork_text(ArtworkKey::ClearHelp, language),
                    context,
                    BlockReason::Artwork(ArtworkKey::NeedCurrent),
                ),
            };
        view! { <Dialog id=id title=title close_label=cancel description=description open=open
        on_close_request=Callback::new(move |_| open.set(false)) on_error=on_error
        actions=move || view! {
            <Button id=cancel_id variant=ButtonVariant::Secondary on_press=Callback::new(move |_| open.set(false))>{cancel}</Button>
            <ActionButton id=confirm_id english=english icon=IconName::DeleteOutline variant=ButtonVariant::Danger blocked=blocked reason=reason
                on_press=Callback::new(move |_| confirm_discard_request(blocked.get_untracked(), open, on_confirm))>{confirm}</ActionButton>
        }><p>{context}</p></Dialog> }
    }
}

/// Native context choices preserve a selected value even before a catalog arrives.
#[component]
pub fn ContextSelect(
    #[prop(into)] id: String,
    label: Signal<String>,
    value: Signal<String>,
    options: Signal<Vec<(String, String)>>,
    unknown_label: Signal<String>,
    on_change: Callback<String>,
    english: Signal<bool>,
    readonly: Signal<bool>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(BlockReason::ReadOnly) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, english.get_untracked())><p>{text(Key::PermissionHelp, english.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reason = StoredValue::new(reason);
    let value = Memo::new(move |_| value.get());
    let choices = Memo::new(move |_| {
        let mut choices = options.get();
        let current = value.get();
        if !current.is_empty() && !choices.iter().any(|(choice, _)| choice == &current) {
            choices.push((
                current.clone(),
                format!("{} · {current}", unknown_label.get()),
            ));
        }
        choices
    });
    view! {
        <Select id=id label=label value=Signal::derive(move || value.get()) on_input=on_change error=error class="production-field"
            state=Signal::derive(move || if readonly.get() { ChoiceState::Disabled(reason.with_value(|r| r.get(english.get()))) } else { ChoiceState::Enabled })>
            {move || {
                choices.get().into_iter().map(|(choice, label)| {
                    let choice = StoredValue::new(choice);
                    view! { <option value=choice.get_value() selected=move || value.get() == choice.get_value() prop:selected=move || value.get() == choice.get_value()>{label}</option> }
                }).collect_view()
            }}
        </Select>
    }.into_any()
}

/// Work delegation confirmation reuses the established Studio dialog adapter site.
/// The caller supplies presentation text and owns the owner-authorized revoke effect.
#[component]
pub fn DelegationRevokeDialog(
    open: RwSignal<bool>,
    english: Signal<bool>,
    blocked: Signal<bool>,
    details: Signal<String>,
    on_confirm: Callback<MouseEvent>,
    on_error: Callback<DialogError>,
) -> impl IntoView {
    use crate::live::delegation::copy::{text, Key};
    move || {
        let language = english.get();
        view! {
            <Dialog id="delegation-revoke-dialog" title=text(Key::DialogTitle, language)
                close_label=text(Key::DialogClose, language) open=open
                on_close_request=Callback::new(move |_| open.set(false)) on_error=on_error
                actions=move || view! {
                    <Button id="delegation-revoke-keep" variant=ButtonVariant::Secondary
                        on_press=Callback::new(move |_| open.set(false))>
                        {move || text(Key::DialogKeep, english.get())}
                    </Button>
                    <ActionButton id="delegation-revoke-confirm" english=english icon=IconName::Close variant=ButtonVariant::Danger
                        blocked=blocked on_press=on_confirm>
                        {move || text(Key::DialogConfirm, english.get())}
                    </ActionButton>
                }>
                <p>{move || details.get()}</p>
                <p>{move || text(Key::DialogBody, english.get())}</p>
            </Dialog>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_submit_and_action_adapters_keep_one_label_and_decorative_icon_without_render_effects()
    {
        let owner = Owner::new();
        owner.with(|| {
            let calls = RwSignal::new(0);
            for english in [false, true] {
                let label = if english { "Sign in to Cantos Studio" } else { "Đăng nhập Cantos Studio" };
                for blocked in [false, true] {
                    let html = view! {
                        <form>
                            <FormSubmit id="login-fixture" english=Signal::stored(english)
                                blocked=Signal::stored(blocked)
                                icon=ViewFn::from(|| view! { <span data-fixture="decorative-slot"></span> })>{label}</FormSubmit>
                        </form>
                    }.to_html();
                    assert_eq!(html.matches("<button ").count(), 1);
                    assert!(html.contains("type=\"submit\""), "{html}");
                    assert!(html.contains("aria-hidden=\"true\""), "{html}");
                    assert!(html.contains("cui-control-label"), "{html}");
                    assert_eq!(html.matches(label).count(), 1, "{html}");
                    assert_eq!(html.contains(" disabled"), blocked, "{html}");
                    assert!(!html.contains("aria-label="), "{html}");
                }
                for busy in [false, true] {
                    let html = view! {
                        <ActionButton id="action-fixture" english=Signal::stored(english) icon=IconName::Save
                            busy=Signal::stored(busy) on_press=Callback::new(move |_| calls.update(|value| *value += 1))>{label}</ActionButton>
                    }.to_html();
                    assert_eq!(html.contains("aria-busy=\"true\""), busy, "{html}");
                    assert_eq!(html.contains("cui-action-progress"), busy, "{html}");
                    assert_eq!(html.contains("<svg"), !busy, "forward the optional glyph: {html}");
                    assert!(html.contains("aria-hidden=\"true\""), "{html}");
                    assert_eq!(calls.get_untracked(), 0, "render cannot dispatch");
                }
            }
        });
    }

    #[test]
    fn busy_and_blocked_actions_never_dispatch_and_keep_localized_reason() {
        for english in [false, true] {
            for block in [
                BlockReason::Busy,
                BlockReason::ContextReady,
                BlockReason::SaveReview,
                BlockReason::Pending,
                BlockReason::ReadOnly,
                BlockReason::ProductionReview,
            ] {
                let reason = LocalizedReason::new(block).unwrap().get(english);
                for (blocked, busy) in [(true, false), (false, true), (true, true)] {
                    let state = action_state(blocked, busy, reason.clone());
                    assert!(!state.can_press());
                    assert_eq!(state.is_busy(), busy);
                    assert_eq!(state.reason().unwrap().as_str(), block.text(english));
                }
                assert_eq!(action_state(false, false, reason), ButtonState::Enabled);
            }
        }
    }

    #[test]
    fn pane_control_is_a_current_page_link_with_an_icon_and_reselection_keeps_current_pane() {
        let owner = Owner::new();
        owner.with(|| {
            let panel = RwSignal::new(WorkspacePane::Proposal);
            let english = Signal::stored(false);
            let html = view! { <WorkspaceNavigation panel=panel english=english/> }.to_html();
            assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
            assert_eq!(html.matches("<a ").count(), WorkspacePane::ALL.len());
            assert_eq!(html.matches("cui-icon").count(), WorkspacePane::ALL.len());
            assert!(!html.contains("aria-pressed"));
            assert!(!html.contains("cui-chip-check"));
            assert!(!html.contains("<select"));
            assert!(!html.contains("role=\"tab"));
            for pane in WorkspacePane::ALL {
                assert!(html.contains(&format!("id=\"{}-control\"", pane.id())));
                assert!(html.contains(&format!("href=\"#{}\"", pane.id())));
                pane_selection(panel, pane).run(true);
                assert_eq!(panel.get_untracked(), pane);
                pane_selection(panel, pane).run(false);
                assert_eq!(panel.get_untracked(), pane);
            }
        });
    }

    #[test]
    fn every_pane_has_a_distinct_icon() {
        let mut icons: Vec<_> = WorkspacePane::ALL
            .iter()
            .map(|pane| pane.icon().as_str())
            .collect();
        icons.sort_unstable();
        icons.dedup();
        assert_eq!(icons.len(), WorkspacePane::ALL.len());
    }

    #[test]
    fn only_a_plain_primary_click_switches_in_place() {
        assert!(switches_in_place(0, false, false, false, false));
        for (button, ctrl, meta, shift, alt) in [
            (1, false, false, false, false),
            (2, false, false, false, false),
            (0, true, false, false, false),
            (0, false, true, false, false),
            (0, false, false, true, false),
            (0, false, false, false, true),
        ] {
            assert!(!switches_in_place(button, ctrl, meta, shift, alt));
        }
    }

    #[test]
    fn catalog_choices_keep_zero_and_unknown_nonfirst_selection_without_render_dispatch() {
        let owner = Owner::new();
        owner.with(|| {
            let value = RwSignal::new("0".to_owned());
            let options = RwSignal::new(vec![("first".into(), "Lựa chọn đầu tiên".into())]);
            let events = RwSignal::new(Vec::<String>::new());
            let render = |readonly| view! { <ContextSelect id="casting-voice" label=Signal::derive(|| "Giọng người dẫn chuyện".to_owned())
                value=value.into() options=options.into() unknown_label=Signal::derive(|| "Chưa có trong catalog".to_owned())
                on_change=Callback::new(move |raw| events.update(|events| events.push(raw))) english=Signal::stored(false) readonly=Signal::derive(move || readonly) error=Signal::derive(|| Some("Giá trị 0 được giữ · chưa có trong profile".to_owned()))/> }.to_html();
            let missing = render(false);
            assert!(missing.contains("value=\"0\" selected"));
            assert!(missing.contains("Chưa có trong catalog · 0"));
            assert!(missing.contains("aria-errormessage=\"casting-voice-error\""));
            let label = missing.split("<label").nth(1).unwrap().split("</label>").next().unwrap();
            assert!(label.contains("Giọng người dẫn chuyện"));
            assert!(!label.contains("Giá trị 0"));
            options.set(vec![("first".into(), "Giọng đầu".into()), ("0".into(), "Giọng trung tính".into())]);
            let known = render(false);
            assert!(known.contains("value=\"0\" selected"));
            assert!(!known.contains("Chưa có trong catalog · 0"));
            let disabled = render(true);
            assert!(disabled.contains("aria-describedby=\"casting-voice-error casting-voice-reason\""));
            assert!(disabled.contains("data-state=\"disabled\""));
            assert!(disabled.contains(" disabled"));
            assert!(events.get_untracked().is_empty());
        });
    }

    #[test]
    fn revision_action_keeps_reason_submit_safety_and_no_render_side_effect() {
        let owner = Owner::new();
        owner.with(|| {
            let events = RwSignal::new(0usize);
            let html = view! { <ActionButton id="revision-test-save" english=Signal::stored(false) blocked=true reason=BlockReason::SaveReview
                on_press=Callback::new(move |_| events.update(|count| *count += 1))>"Lưu bản sửa đổi bất biến mới"</ActionButton> }.to_html();
            assert!(html.contains("type=\"button\""));
            assert!(html.contains(" disabled"));
            assert!(html.contains("revision-test-save-reason"));
            assert!(html.contains(text(Key::ValidateFirst, false)));
            assert_eq!(events.get_untracked(), 0);
        });
    }
    #[test]
    fn blocked_or_rejected_discard_never_closes_the_confirmation_or_dispatches_twice() {
        let owner = Owner::new();
        owner.with(|| {
            let open = RwSignal::new(true);
            let events = RwSignal::new(0usize);
            let rejected = Callback::new(move |()| {
                events.update(|n| *n += 1);
                false
            });
            confirm_discard_request(true, open, rejected);
            assert_eq!(events.get_untracked(), 0);
            assert!(open.get_untracked());
            confirm_discard_request(false, open, rejected);
            assert_eq!(events.get_untracked(), 1);
            assert!(open.get_untracked());
            let accepted = Callback::new(move |()| {
                events.update(|n| *n += 1);
                true
            });
            confirm_discard_request(false, open, accepted);
            assert_eq!(events.get_untracked(), 2);
            assert!(!open.get_untracked());
            confirm_discard_request(false, open, accepted);
            assert_eq!(events.get_untracked(), 2);
        });
    }

    #[test]
    fn discard_dialog_has_safe_cancel_and_separate_confirmation_without_render_effects() {
        for english in [false, true] {
            let owner = Owner::new();
            owner.with(|| {
                let calls = RwSignal::new(0usize);
                let html = view! { <DiscardDraftDialog open=RwSignal::new(true) english=Signal::stored(english) blocked=Signal::derive(|| false)
                    on_confirm=Callback::new(move |()| { calls.update(|n| *n += 1); true }) on_error=Callback::new(|_| {})/> }.to_html();
                assert!(html.contains("<dialog"));
                assert!(!html.split("<dialog").nth(1).unwrap().split('>').next().unwrap().contains(" open"));
                assert!(html.contains("autofocus"));
                assert!(html.contains(text(Key::Cancel, english)));
                assert!(html.contains("revision-discard-confirm"));
                assert!(html.contains("cui-dialog-actions"));
                assert_eq!(calls.get_untracked(), 0);
            });
        }
    }
    #[test]
    fn native_form_submit_exposes_only_the_current_blocked_reason() {
        for blocked in [false, true] {
            let owner = Owner::new();
            owner.with(|| {
                let html = view! { <FormSubmit id="session-submit" english=Signal::stored(false) blocked=Signal::derive(move || blocked)>"Đăng nhập"</FormSubmit> }.to_html();
                assert!(html.contains("type=\"submit\""));
                assert!(html.contains("data-variant=\"tonal\""), "secondary forms stay tonal: {html}");
                assert_eq!(html.contains(" disabled"), blocked);
                assert_eq!(html.contains("aria-describedby=\"session-submit-reason\""), blocked);
                assert!(html.contains(text(Key::ControlBlocked, false)));
            });
        }
    }
}

#[cfg(test)]
mod pane_return_tests {
    use super::*;
    #[test]
    fn reload_restores_only_canonical_nonsecret_workspace_routes() {
        for pane in WorkspacePane::ALL {
            assert_eq!(WorkspacePane::from_hash(&format!("#{}", pane.id())), pane);
        }
        for malformed in [
            "#workspace-import?actor=b",
            "//evil.example",
            "#session/complete",
            "#unknown",
        ] {
            assert_eq!(
                WorkspacePane::from_hash(malformed),
                WorkspacePane::Dashboard
            );
        }
    }
}
