//! Studio presentation adapters. Values, permissions and effects remain app-owned.
use cantos_ui::{
    Button, ButtonState, ButtonVariant, Checkbox, ChoiceState, Dialog, DialogError, DisabledReason,
    ErrorState, Expression, FieldDensity, IconName, LinkHref, NavItem, Select,
};
use leptos::{ev::MouseEvent, prelude::*};

use crate::messages::{
    self,
    workspace::{text, Key},
};

/// Studio shares Theatre's Expressive palette, weights and control geometry (#559); density,
/// layout and reading intensity stay Studio's own. One constant so every Studio root agrees.
pub const STUDIO_EXPRESSION: Expression = Expression::Theatre;

/// Authenticated forms (works, units, rights, import) sit in a dense writing desk, so their
/// Floating fields take the compact shared density; sign-in and the account menu keep the
/// expression's comfortable auth geometry. One constant, never a per-field override (#559).
pub const EDITOR_FIELD_DENSITY: FieldDensity = FieldDensity::Compact;

mod workspace_navigation;
pub use workspace_navigation::WorkspaceNavigationShell;

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
            let locale = RwSignal::new(crate::localization::Locale::Vietnamese);
            for en in crate::localization::Locale::supported() {
                locale.set(en);
                let html = view! {
                    <h3>{move ||crate::messages::copy(locale.get()).theme}</h3>
                    <ContentText language=Signal::derive(move ||Some(baseline.get().metadata().language))
                        text=Signal::derive(move ||baseline.get().metadata().work_title)/>
                    <ContentText language=Signal::derive(move ||Some(document.get().metadata().language))
                        text=Signal::derive(move ||document.get().metadata().work_title)/>
                }.to_html();
                assert!(html.contains("<span lang=\"en\">Lời hẹn {time}</span>"), "{html}");
                assert!(html.contains("<span lang=\"vi-VN\">Ánh đèn cuối sân khấu</span>"), "{html}");
                assert!(html.contains(if en == crate::localization::Locale::English { "Appearance" } else { "Giao diện" }), "{html}");
                assert_eq!(document.get_untracked().to_json(), source);
            }
        });
    }
}

/// App-owned options share the exact value used by the CreateWork DTO.
#[component]
pub fn WorkKindOptions(
    value: RwSignal<String>,
    locale: Signal<crate::localization::Locale>,
) -> impl IntoView {
    view! { <option value="novel" selected=move || value.get()=="novel">{move || crate::live::copy::work_kind("novel", locale.get())}</option><option value="story" selected=move || value.get()=="story">{move || crate::live::copy::work_kind("story", locale.get())}</option><option value="podcast" selected=move || value.get()=="podcast">{move || crate::live::copy::work_kind("podcast", locale.get())}</option> }
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
                view! { <WorkKindOptions value=kind locale=Signal::derive(|| crate::localization::Locale::Vietnamese)/> }.to_html();
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
                let locale = RwSignal::new(crate::localization::Locale::Vietnamese);
                for (choice, expected) in [(crate::localization::Locale::Vietnamese, ["Tiểu thuyết", "Truyện", "Podcast"]), (crate::localization::Locale::English, ["Novel", "Story", "Podcast"])] {
                    locale.set(choice);
                    let html = view! { <WorkKindOptions value=kind locale=Signal::derive(move || locale.get())/> }.to_html();
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

    pub fn label(self, locale: crate::localization::Locale) -> &'static str {
        match self {
            Self::Dashboard => {
                crate::messages::entry::text(crate::messages::entry::Key::Dashboard, locale)
            }
            Self::Content => crate::live::copy::text(crate::live::copy::Key::Pane, locale),
            Self::Import => text(Key::Import, locale),
            Self::Proposal => text(Key::Proposal, locale),
            Self::Revisions => text(Key::Revisions, locale),
            Self::Production => messages::production::text(messages::production::Key::Pane, locale),
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

/// Presentation groups never replace the six pane identities or their mounted owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceGroup {
    Desk,
    Editing,
    Production,
}

impl WorkspaceGroup {
    pub const ALL: [Self; 3] = [Self::Desk, Self::Editing, Self::Production];

    pub const fn panes(self) -> &'static [WorkspacePane] {
        match self {
            Self::Desk => &[WorkspacePane::Dashboard, WorkspacePane::Content],
            Self::Editing => &[
                WorkspacePane::Import,
                WorkspacePane::Proposal,
                WorkspacePane::Revisions,
            ],
            Self::Production => &[WorkspacePane::Production],
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Desk => "desk",
            Self::Editing => "editing",
            Self::Production => "production",
        }
    }

    fn heading_id(self, scope: &str) -> String {
        format!("workspace-{scope}-{}-heading", self.id())
    }

    fn label(self, locale: crate::localization::Locale) -> &'static str {
        text(
            match self {
                Self::Desk => Key::DeskGroup,
                Self::Editing => Key::EditingGroup,
                Self::Production => Key::ProductionGroup,
            },
            locale,
        )
    }
}

/// The same grouped links serve the rail and drawer; only IDs are scoped per presentation.
#[component]
pub fn WorkspaceNavigation(
    panel: RwSignal<WorkspacePane>,
    locale: Signal<crate::localization::Locale>,
    #[prop(default = "rail")] scope: &'static str,
    #[prop(optional)] on_select: Option<Callback<()>>,
    #[prop(optional_no_strip)] footer: Option<Callback<&'static str, AnyView>>,
) -> impl IntoView {
    view! {
        <nav class="workspace-nav" aria-label=move || text(Key::Workspace, locale.get())>
            <div class="workspace-nav-list">
            {WorkspaceGroup::ALL.into_iter().map(|group| {
                let heading_id = group.heading_id(scope);
                view! {
                <section class="workspace-nav-group" aria-labelledby=heading_id.clone()
                    data-current=move || group.panes().contains(&panel.get()).to_string()>
                <h3 id=heading_id.clone()>{move || group.label(locale.get())}</h3>
                {group.panes().iter().copied().filter_map(|pane| pane_href(pane).map(|href| view! {
                <NavItem id=if scope == "rail" { format!("{}-control", pane.id()) } else { format!("{}-{scope}-control", pane.id()) } href=href
                    icon=pane.icon() current=Signal::derive(move || panel.get() == pane)
                    on_press=Callback::new(move |event: MouseEvent| {
                        if switches_in_place(event.button(), event.ctrl_key(), event.meta_key(), event.shift_key(), event.alt_key()) {
                            event.prevent_default();
                            pane_selection(panel, pane).run(true);
                            if let Some(selected) = on_select { selected.run(()); }
                        }
                    })>
                    {move || if pane == WorkspacePane::Dashboard { text(Key::Overview, locale.get()) } else { pane.label(locale.get()) }}
                </NavItem>
                })).collect_view()}
                </section>
                }
            }).collect_view()}
            </div>
            <div class="workspace-nav-footer">{footer.map(|render| render.run(scope))}</div>
        </nav>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    Live(crate::live::copy::Key),
    Metadata(crate::localization::generated::live::Key),
    #[cfg(debug_assertions)]
    Forge(crate::messages::forge::Key),
}

impl BlockReason {
    pub fn text(self, locale: crate::localization::Locale) -> &'static str {
        let copy = messages::copy(locale);
        match self {
            #[cfg(debug_assertions)]
            Self::Forge(key) => crate::messages::forge::text(key, locale),
            Self::Live(key) => crate::live::copy::text(key, locale),
            Self::Metadata(key) => crate::localization::generated::live::text(key, locale),
            Self::Artwork(key) => crate::live::artwork::copy::text(key, locale),
            Self::Busy => copy.busy,
            Self::ContextReady => text(Key::ControlBlocked, locale),
            Self::PageNavigation => {
                crate::live::copy::text(crate::live::copy::Key::PageUnavailable, locale)
            }
            Self::SaveReview => text(Key::ValidateFirst, locale),
            Self::Pending => copy.pending,
            Self::ReadOnly => text(Key::ControlReadOnly, locale),
            Self::ScriptReadOnly => messages::authoring::authoring_copy(locale).readonly_choice,
            Self::ProductionReview => {
                messages::production::text(messages::production::Key::ValidationHelp, locale)
            }
        }
    }
}

/// Validate every generated locale at mount. Invalid copy never enables the control.
#[derive(Clone)]
pub(crate) struct LocalizedReason {
    values: crate::localization::generated::LocaleMap<DisabledReason>,
}

impl LocalizedReason {
    pub(crate) fn new(reason: BlockReason) -> Result<Self, cantos_ui::EmptyDisabledReason> {
        Ok(Self {
            values: crate::localization::generated::LocaleMap::try_new(|locale| {
                DisabledReason::new(reason.text(locale))
            })?,
        })
    }

    pub(crate) fn get(&self, locale: crate::localization::Locale) -> DisabledReason {
        self.values.get(locale).clone()
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

/// Static or reactive blocked reason for [`ActionButton`].
#[derive(Clone, Copy)]
pub enum ActionReason {
    Static(BlockReason),
    Reactive(Signal<BlockReason>),
}

impl ActionReason {
    fn get(self) -> BlockReason {
        match self {
            Self::Static(reason) => reason,
            Self::Reactive(reason) => reason.get(),
        }
    }

    fn get_untracked(self) -> BlockReason {
        match self {
            Self::Static(reason) => reason,
            Self::Reactive(reason) => reason.get_untracked(),
        }
    }
}

impl From<BlockReason> for ActionReason {
    fn from(reason: BlockReason) -> Self {
        Self::Static(reason)
    }
}

impl From<Signal<BlockReason>> for ActionReason {
    fn from(reason: Signal<BlockReason>) -> Self {
        Self::Reactive(reason)
    }
}

impl From<Memo<BlockReason>> for ActionReason {
    fn from(reason: Memo<BlockReason>) -> Self {
        Self::Reactive(reason.into())
    }
}

impl<F> From<F> for ActionReason
where
    F: Fn() -> BlockReason + Send + Sync + 'static,
{
    fn from(reason: F) -> Self {
        Self::Reactive(Signal::derive(reason))
    }
}

/// Map existing app guards to the reusable action contract without inferring permissions.
#[component]
pub fn ActionButton(
    #[prop(into)] id: String,
    locale: Signal<crate::localization::Locale>,
    children: Children,
    on_press: Callback<MouseEvent>,
    #[prop(optional)] focus_on_press: bool,
    #[prop(into, default = ButtonVariant::Primary.into())] variant: Signal<ButtonVariant>,
    #[prop(optional)] icon: Option<IconName>,
    #[prop(into, default = false.into())] blocked: Signal<bool>,
    #[prop(into, default = false.into())] busy: Signal<bool>,
    #[prop(into, default = ActionReason::Static(BlockReason::ContextReady))] reason: ActionReason,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
) -> impl IntoView {
    let (Ok(initial), Ok(loading), Ok(fallback)) = (
        LocalizedReason::new(reason.get_untracked()),
        LocalizedReason::new(BlockReason::Busy),
        LocalizedReason::new(BlockReason::ContextReady),
    ) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, locale.get_untracked())><p>{text(Key::PermissionHelp, locale.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reasons = StoredValue::new((initial, loading, fallback));
    view! {
        <Button id=id variant=variant nostrip:icon=icon described_by=described_by on_press=on_press focus_on_press=focus_on_press
            state=Signal::derive(move || {
                let busy = busy.get();
                let current = reason.get();
                let locale = locale.get();
                let reason = reasons.with_value(|(initial, loading, fallback)| {
                    if busy {
                        loading.get(locale)
                    } else if matches!(reason, ActionReason::Static(_)) {
                        initial.get(locale)
                    } else {
                        LocalizedReason::new(current)
                            .map(|localized| localized.get(locale))
                            .unwrap_or_else(|_| fallback.get(locale))
                    }
                });
                action_state(blocked.get(), busy, reason)
            })>{children()}</Button>
    }.into_any()
}

/// Translate the existing Studio boolean gate into the shared checkbox state. The caller
/// retains the exact review/consent transition; this adapter never grants approval.
#[component]
pub fn ReviewCheckbox(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] checked: Signal<bool>,
    on_change: Callback<bool>,
    locale: Signal<crate::localization::Locale>,
    #[prop(into, default = false.into())] blocked: Signal<bool>,
    #[prop(default = BlockReason::SaveReview)] reason: BlockReason,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(reason) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, locale.get_untracked())><p>{text(Key::PermissionHelp,locale.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reason = StoredValue::new(reason);
    view! {
        <Checkbox id label checked on_change described_by class="review-confirmation"
            state=Signal::derive(move || if blocked.get() {
                ChoiceState::Disabled(reason.with_value(|reason|reason.get(locale.get())))
            } else { ChoiceState::Enabled })/>
    }
    .into_any()
}

/// Real form submission stays native (including Enter); the form owns the submit guard/effect.
/// The optional decorative slot takes a caller-owned view; adopting Icon at a new product site
/// still requires its two-renderer qualification. No interactive content belongs in this slot.
/// Tonal by default; the form that is the screen's current task passes `Primary`.
#[component]
pub fn FormSubmit(
    #[prop(into)] id: String,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    children: Children,
    #[prop(optional)] icon: Option<ViewFn>,
    #[prop(into, default = false.into())] busy: Signal<bool>,
    /// Sign-in already names progress in its button; retain a single offscreen announcement.
    #[prop(optional)]
    busy_label_only: bool,
    #[prop(default = ButtonVariant::Tonal)] variant: ButtonVariant,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(BlockReason::ContextReady) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, locale.get_untracked())><p>{text(Key::PermissionHelp, locale.get_untracked())}</p></ErrorState> }.into_any();
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
                    Some(view! { <span class="cui-button-icon" aria-hidden="true"><span class="cui-loading-indicator"></span></span> }.into_any())
                } else { icon.clone().map(|icon| view! { <span class="cui-button-icon" aria-hidden="true">{icon.run()}</span> }.into_any()) }}
                <span class="cui-control-label">{children()}</span>
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || !blocked.get() && !busy.get()
                class:cui-sr-only=move || busy_label_only && busy.get()
                role=move || busy.get().then_some("status")>{move || if busy.get() {
                    BlockReason::Busy.text(locale.get()).to_owned()
                } else { reason.with_value(|r| r.get(locale.get()).as_str().to_owned()) }}</span>
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
    locale: Signal<crate::localization::Locale>,
    details: Signal<String>,
    on_cancel: Callback<()>,
    on_confirm: Callback<MouseEvent>,
    on_error: Callback<DialogError>,
) -> impl IntoView {
    use crate::messages::forge::{text as copy, Key as ForgeKey};
    move || {
        let language = locale.get();
        view! {
            <Dialog id="forge-sync-dialog" title=copy(ForgeKey::Plan, language)
                close_label=copy(ForgeKey::Close, language) open=open
                on_close_request=Callback::new(move |_| on_cancel.run(())) on_error=on_error
                description=copy(ForgeKey::PlanHelp, language)
                actions=move || view! {
                    <ActionButton id="forge-sync-cancel" locale=locale variant=ButtonVariant::Text
                        icon=IconName::Close on_press=Callback::new(move |_| on_cancel.run(()))>
                        {move || copy(ForgeKey::Cancel, locale.get())}
                    </ActionButton>
                    <ActionButton id="forge-sync-confirm" locale=locale icon=IconName::CloudSync
                        on_press=on_confirm>{move || copy(ForgeKey::Confirm, locale.get())}</ActionButton>
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
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    on_confirm: Callback<(), bool>,
    on_error: Callback<DialogError>,
    #[prop(into, default = DiscardTarget::Draft.into())] target: Signal<DiscardTarget>,
) -> impl IntoView {
    // Structural locale/target changes remount chrome, never app draft state.
    move || {
        use crate::live::artwork::copy::{text as artwork_text, Key as ArtworkKey};
        let language = locale.get();
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
            <ActionButton id=confirm_id locale=locale icon=IconName::DeleteOutline variant=ButtonVariant::Danger blocked=blocked reason=reason
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
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(BlockReason::ReadOnly) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, locale.get_untracked())><p>{text(Key::PermissionHelp, locale.get_untracked())}</p></ErrorState> }.into_any();
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
            state=Signal::derive(move || if readonly.get() { ChoiceState::Disabled(reason.with_value(|r| r.get(locale.get()))) } else { ChoiceState::Enabled })>
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
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    details: Signal<String>,
    on_confirm: Callback<MouseEvent>,
    on_error: Callback<DialogError>,
) -> impl IntoView {
    use crate::live::delegation::copy::{text, Key};
    move || {
        let language = locale.get();
        view! {
            <Dialog id="delegation-revoke-dialog" title=text(Key::DialogTitle, language)
                close_label=text(Key::DialogClose, language) open=open
                on_close_request=Callback::new(move |_| open.set(false)) on_error=on_error
                actions=move || view! {
                    <Button id="delegation-revoke-keep" variant=ButtonVariant::Secondary
                        on_press=Callback::new(move |_| open.set(false))>
                        {move || text(Key::DialogKeep, locale.get())}
                    </Button>
                    <ActionButton id="delegation-revoke-confirm" locale=locale icon=IconName::Close variant=ButtonVariant::Danger
                        blocked=blocked on_press=on_confirm>
                        {move || text(Key::DialogConfirm, locale.get())}
                    </ActionButton>
                }>
                <p>{move || details.get()}</p>
                <p>{move || text(Key::DialogBody, locale.get())}</p>
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
            for locale in crate::localization::Locale::supported() {
                let label = if locale == crate::localization::Locale::English { "Sign in to Cantos Studio" } else { "Đăng nhập Cantos Studio" };
                for blocked in [false, true] {
                    let html = view! {
                        <form>
                            <FormSubmit id="login-fixture" locale=Signal::stored(locale)
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
                        <ActionButton id="action-fixture" locale=Signal::stored(locale) icon=IconName::Save
                            busy=Signal::stored(busy) on_press=Callback::new(move |_| calls.update(|value| *value += 1))>{label}</ActionButton>
                    }.to_html();
                    assert_eq!(html.contains("aria-busy=\"true\""), busy, "{html}");
                    assert_eq!(html.contains("cui-loading-indicator"), busy, "{html}");
                    assert_eq!(html.contains("<svg"), !busy, "forward the optional glyph: {html}");
                    assert!(html.contains("aria-hidden=\"true\""), "{html}");
                    assert_eq!(calls.get_untracked(), 0, "render cannot dispatch");
                }
            }
        });
    }

    #[test]
    fn busy_and_blocked_actions_never_dispatch_and_keep_localized_reason() {
        for locale in crate::localization::Locale::supported() {
            for block in [
                BlockReason::Busy,
                BlockReason::ContextReady,
                BlockReason::SaveReview,
                BlockReason::Pending,
                BlockReason::ReadOnly,
                BlockReason::ProductionReview,
            ] {
                let reason = LocalizedReason::new(block).unwrap().get(locale);
                for (blocked, busy) in [(true, false), (false, true), (true, true)] {
                    let state = action_state(blocked, busy, reason.clone());
                    assert!(!state.can_press());
                    assert_eq!(state.is_busy(), busy);
                    assert_eq!(state.reason().unwrap().as_str(), block.text(locale));
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
            let locale = Signal::stored(crate::localization::Locale::Vietnamese);
            let html = view! { <WorkspaceNavigation panel=panel locale=locale/> }.to_html();
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
    fn groups_partition_all_six_stable_panes_without_changing_hashes() {
        let grouped: Vec<_> = WorkspaceGroup::ALL
            .into_iter()
            .flat_map(|group| group.panes().iter().copied())
            .collect();
        assert_eq!(grouped.len(), 6);
        for pane in WorkspacePane::ALL {
            assert_eq!(grouped.iter().filter(|item| **item == pane).count(), 1);
            assert_eq!(WorkspacePane::from_hash(&format!("#{}", pane.id())), pane);
        }
        assert_eq!(
            WorkspaceGroup::Desk.panes(),
            &[WorkspacePane::Dashboard, WorkspacePane::Content]
        );
        assert_eq!(
            WorkspaceGroup::Editing.panes(),
            &[
                WorkspacePane::Import,
                WorkspacePane::Proposal,
                WorkspacePane::Revisions
            ]
        );
        assert_eq!(
            WorkspaceGroup::Production.panes(),
            &[WorkspacePane::Production]
        );
    }

    #[test]
    fn drawer_links_have_unique_ids_and_the_same_destinations_and_current_group() {
        let owner = Owner::new();
        owner.with(|| {
            let panel = RwSignal::new(WorkspacePane::Content);
            for language in crate::localization::Locale::supported() {
                let locale = Signal::stored(language);
                let html = view! { <WorkspaceNavigation panel locale scope="drawer"/> }.to_html();
                assert_eq!(html.matches("data-current=\"true\"").count(), 1);
                assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
                assert_eq!(html.matches("<section ").count(), 3);
                for pane in WorkspacePane::ALL {
                    assert!(html.contains(&format!("id=\"{}-drawer-control\"", pane.id())));
                    assert!(html.contains(&format!("href=\"#{}\"", pane.id())));
                }
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
                on_change=Callback::new(move |raw| events.update(|events| events.push(raw))) locale=Signal::stored(crate::localization::Locale::Vietnamese) readonly=Signal::derive(move || readonly) error=Signal::derive(|| Some("Giá trị 0 được giữ · chưa có trong profile".to_owned()))/> }.to_html();
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
            let html = view! { <ActionButton id="revision-test-save" locale=Signal::stored(crate::localization::Locale::Vietnamese) blocked=true reason=BlockReason::SaveReview
                on_press=Callback::new(move |_| events.update(|count| *count += 1))>"Lưu bản sửa đổi bất biến mới"</ActionButton> }.to_html();
            assert!(html.contains("type=\"button\""));
            assert!(html.contains(" disabled"));
            assert!(html.contains("revision-test-save-reason"));
            assert!(html.contains(text(Key::ValidateFirst, crate::localization::Locale::Vietnamese)));
            assert_eq!(events.get_untracked(), 0);
        });
    }

    #[test]
    fn reactive_action_reason_projects_the_current_blocking_condition() {
        use crate::localization::generated::live;
        let owner = Owner::new();
        owner.with(|| {
            let reason = RwSignal::new(BlockReason::Metadata(live::Key::FieldEmptyPatch));
            let render = || {
                view! {
                    <ActionButton id="live-patch-work" locale=Signal::stored(crate::localization::Locale::Vietnamese)
                        blocked=true reason=Signal::from(reason) on_press=Callback::new(|_| {})>"Lưu tiêu đề và mô tả"</ActionButton>
                }
                .to_html()
            };
            let clean = render();
            assert!(clean.contains(live::text(live::Key::FieldEmptyPatch, crate::localization::Locale::Vietnamese)));
            assert!(!clean.contains(text(Key::ControlBlocked, crate::localization::Locale::Vietnamese)));
            reason.set(BlockReason::Metadata(live::Key::ReasonWorkArchived));
            let archived = render();
            assert!(archived.contains(live::text(live::Key::ReasonWorkArchived, crate::localization::Locale::Vietnamese)));
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
        for locale in crate::localization::Locale::supported() {
            let owner = Owner::new();
            owner.with(|| {
                let calls = RwSignal::new(0usize);
                let html = view! { <DiscardDraftDialog open=RwSignal::new(true) locale=Signal::stored(locale) blocked=Signal::derive(|| false)
                    on_confirm=Callback::new(move |()| { calls.update(|n| *n += 1); true }) on_error=Callback::new(|_| {})/> }.to_html();
                assert!(html.contains("<dialog"));
                assert!(!html.split("<dialog").nth(1).unwrap().split('>').next().unwrap().contains(" open"));
                assert!(html.contains("autofocus"));
                assert!(html.contains(text(Key::Cancel, locale)));
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
                let html = view! { <FormSubmit id="session-submit" locale=Signal::stored(crate::localization::Locale::Vietnamese) blocked=Signal::derive(move || blocked)>"Đăng nhập"</FormSubmit> }.to_html();
                assert!(html.contains("type=\"submit\""));
                assert!(html.contains("data-variant=\"tonal\""), "secondary forms stay tonal: {html}");
                assert_eq!(html.contains(" disabled"), blocked);
                assert_eq!(html.contains("aria-describedby=\"session-submit-reason\""), blocked);
                assert!(html.contains(text(Key::ControlBlocked, crate::localization::Locale::Vietnamese)));
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

#[cfg(test)]
mod review_checkbox_tests {
    use super::*;

    #[test]
    fn confirmation_projects_checked_disabled_reason_and_localized_name_without_mutation() {
        Owner::new().with(|| {
            let checked = RwSignal::new(true);
            let blocked = RwSignal::new(false);
            let locale = RwSignal::new(crate::localization::Locale::Vietnamese);
            let calls = RwSignal::new(0);
            for (disabled, en) in [(false,crate::localization::Locale::Vietnamese),(true,crate::localization::Locale::Vietnamese),(true,crate::localization::Locale::English),(false,crate::localization::Locale::English)] {
                blocked.set(disabled);
                locale.set(en);
                let html = view! {
                    <ReviewCheckbox id="review-test" locale=locale.into()
                        label=Signal::derive(move ||text(Key::ReviewBeforeSave,locale.get()).to_owned())
                        checked blocked described_by=Some("review-help".to_owned())
                        on_change=Callback::new(move |value|{checked.set(value);calls.update(|n|*n+=1);})/>
                }.to_html();
                assert!(html.contains("cui-choice-label"),"{html}");
                assert!(html.contains("cui-choice-control"),"{html}");
                assert!(html.contains(r#"for="review-test""#),"{html}");
                assert!(html.contains(r#"type="checkbox""#),"{html}");
                assert!(html.contains(" checked"),"{html}");
                assert_eq!(html.contains(" disabled"), disabled,"{html}");
                let descriptions = if disabled {"review-help review-test-reason"} else {"review-help"};
                assert!(html.contains(&format!("aria-describedby=\"{descriptions}\"")),"{html}");
                assert!(html.contains(text(Key::ReviewBeforeSave,en)),"{html}");
                assert!(checked.get_untracked());
                assert_eq!(calls.get_untracked(),0);
            }
        });
    }
}

#[cfg(test)]
mod expression_tests {
    use super::STUDIO_EXPRESSION;

    #[test]
    fn studio_selects_a_generated_expression_on_every_root() {
        let attribute = STUDIO_EXPRESSION
            .attribute()
            .expect("Studio names an expression");
        assert!(
            cantos_ui::tokens::EXPRESSIONS.contains(&attribute),
            "{attribute}"
        );
        let view = include_str!("view.rs");
        let roots = view.matches("<UiRoot ").count();
        let expressive = view
            .matches("density=Density::Comfortable expression=STUDIO_EXPRESSION>")
            .count();
        assert_eq!(
            (roots, expressive),
            (4, 4),
            "a Studio root fell back to Standard"
        );
    }

    #[test]
    fn authenticated_floating_fields_are_compact_and_sign_in_keeps_the_expression_geometry() {
        use super::{FieldDensity, EDITOR_FIELD_DENSITY};
        assert_eq!(EDITOR_FIELD_DENSITY, FieldDensity::Compact);
        // Every authenticated Floating field names the one constant; no per-field size override.
        for (name, source) in [
            ("live/workspace.rs", include_str!("live/workspace.rs")),
            (
                "live/catalog_controls.rs",
                include_str!("live/catalog_controls.rs"),
            ),
            ("live/publication.rs", include_str!("live/publication.rs")),
            ("import/fields.rs", include_str!("import/fields.rs")),
        ] {
            let floating = source.matches("TextFieldVariant::Floating").count();
            let compact = source
                .matches("TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY")
                .count();
            assert!(
                floating > 0 && floating == compact,
                "{name}: {floating} floating, {compact} compact"
            );
        }
        // Sign-in is the auth surface: comfortable, i.e. no density request at all.
        assert!(!include_str!("signin_form.rs").contains("density"));
        // Studio CSS must not shrink a shared field locally.
        let css = include_str!("../studio.css");
        assert!(
            !css.contains("--cantos-web-field-"),
            "local field geometry override"
        );
        // A composition may select a semantic fill, but never change shared shell geometry.
        // The existing label rule only excludes shared labels from Studio's global margins.
        for (selector, declarations) in css
            .split('}')
            .filter_map(|rule| rule.rsplit_once('{'))
            .filter(|(selector, _)| {
                selector.contains(".cui-field-shell") && !selector.contains(":where(label)")
            })
        {
            assert!(
                selector.contains("#live-audio-brief-stage"),
                "field fill escaped its composition: {selector}"
            );
            for declaration in declarations
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                assert!(
                    declaration.starts_with("background:")
                        && declaration.contains("var(--cantos-color-surface-container-high)"),
                    "local field shell geometry or nonsemantic fill: {declaration}"
                );
            }
        }
    }

    #[test]
    fn studio_entry_art_is_the_hero_surface_not_a_selection_ink_block() {
        let css = include_str!("../studio.css");
        let rule = css
            .lines()
            .find(|line| line.starts_with(".studio-shell .entry-artwork {"))
            .unwrap();
        assert!(
            rule.contains("background: var(--cantos-color-theatre-hero)"),
            "{rule}"
        );
        assert!(
            rule.contains("color: var(--cantos-color-theatre-on-hero)"),
            "{rule}"
        );
        assert!(!rule.contains("primary-container"), "{rule}");
    }
}
