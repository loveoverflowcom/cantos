//! Studio presentation adapters. Values, permissions and effects remain app-owned.
use cantos_ui::{
    Button, ButtonState, ButtonVariant, ChoiceState, Dialog, DialogError, DisabledReason,
    ErrorState, FilterChip, Select,
};
use leptos::{ev::MouseEvent, prelude::*};

use crate::messages::{
    self,
    workspace::{text, Key},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkspacePane {
    Import,
    #[default]
    Proposal,
    Revisions,
    Production,
}

impl WorkspacePane {
    pub const ALL: [Self; 4] = [
        Self::Import,
        Self::Proposal,
        Self::Revisions,
        Self::Production,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Import => "workspace-import",
            Self::Proposal => "workspace-proposal",
            Self::Revisions => "workspace-revisions",
            Self::Production => "workspace-production",
        }
    }

    pub fn label(self, english: bool) -> &'static str {
        match self {
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

/// This changes visibility only. All four app panels stay mounted with their draft state.
#[component]
pub fn WorkspaceNavigation(
    panel: RwSignal<WorkspacePane>,
    english: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <nav class="workspace-nav" aria-label=move || text(Key::Workspace, english.get())>
            {WorkspacePane::ALL.into_iter().map(|pane| view! {
                <FilterChip id=format!("{}-control", pane.id()) selected=Signal::derive(move || panel.get() == pane)
                    on_change=pane_selection(panel, pane)>
                    {move || pane.label(english.get())}
                </FilterChip>
            }).collect_view()}
        </nav>
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum BlockReason {
    #[default]
    Busy,
    ContextReady,
    SaveReview,
    Pending,
    ReadOnly,
    ProductionReview,
}

impl BlockReason {
    pub fn text(self, english: bool) -> &'static str {
        let copy = messages::copy(english);
        match self {
            Self::Busy => copy.busy,
            Self::ContextReady => text(Key::ControlBlocked, english),
            Self::SaveReview => text(Key::ValidateFirst, english),
            Self::Pending => copy.pending,
            Self::ReadOnly => text(Key::ControlReadOnly, english),
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
    english: RwSignal<bool>,
    children: Children,
    on_press: Callback<MouseEvent>,
    #[prop(optional)] variant: ButtonVariant,
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
        <Button id=id variant=variant described_by=described_by on_press=on_press
            state=Signal::derive(move || {
                let busy = busy.get();
                let reason = reasons.with_value(|(blocked, loading)| if busy { loading } else { blocked }.get(english.get()));
                action_state(blocked.get(), busy, reason)
            })>{children()}</Button>
    }.into_any()
}

/// Real form submission stays native (including Enter); the form owns the submit guard/effect.
#[component]
pub fn FormSubmit(
    #[prop(into)] id: String,
    english: RwSignal<bool>,
    blocked: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(BlockReason::ContextReady) else {
        return view! { <ErrorState title=text(Key::ControlUnavailable, english.get_untracked())><p>{text(Key::PermissionHelp, english.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reason = StoredValue::new(reason);
    let reason_id = format!("{id}-reason");
    let described_by = reason_id.clone();
    view! {
        <div class="cui-button-field">
            <button id=id class="cui-button" data-variant="tonal" type="submit" disabled=move || blocked.get()
                aria-describedby=move || blocked.get().then(|| described_by.clone())>{children()}</button>
            <span id=reason_id class="cui-button-reason" hidden=move || !blocked.get()>{move || reason.with_value(|r| r.get(english.get()).as_str().to_owned())}</span>
        </div>
    }.into_any()
}

fn confirm_discard_request(blocked: bool, open: RwSignal<bool>, confirm: Callback<(), bool>) {
    if open.get_untracked() && !blocked && confirm.run(()) {
        open.set(false);
    }
}

/// Only explicit confirmation dispatches discard. Escape/cancel preserve the app draft.
#[component]
pub fn DiscardDraftDialog(
    open: RwSignal<bool>,
    english: RwSignal<bool>,
    blocked: Signal<bool>,
    on_confirm: Callback<(), bool>,
    on_error: Callback<DialogError>,
) -> impl IntoView {
    // Dialog text is structural in v0.2.0. Locale changes remount chrome, never app draft state.
    move || {
        let language = english.get();
        view! { <Dialog id="revision-discard-dialog" title=text(Key::Discard, language) close_label=text(Key::Cancel, language)
        description=text(Key::DiscardConfirm, language) open=open
        on_close_request=Callback::new(move |_| open.set(false)) on_error=on_error
        actions=move || view! {
            <Button id="revision-discard-cancel" variant=ButtonVariant::Secondary on_press=Callback::new(move |_| open.set(false))>{move || text(Key::Cancel, english.get())}</Button>
            <ActionButton id="revision-discard-confirm" english=english variant=ButtonVariant::Danger blocked=blocked
                on_press=Callback::new(move |_| confirm_discard_request(blocked.get_untracked(), open, on_confirm))>{move || text(Key::DiscardNow, english.get())}</ActionButton>
        }><p>{move || text(Key::PermissionHelp, english.get())}</p></Dialog> }
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
    english: RwSignal<bool>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn pane_control_is_a_native_pressed_button_and_reselection_keeps_current_pane() {
        let owner = Owner::new();
        owner.with(|| {
            let panel = RwSignal::new(WorkspacePane::Proposal);
            let english = RwSignal::new(false);
            let html = view! { <WorkspaceNavigation panel=panel english=english/> }.to_html();
            assert_eq!(html.matches("aria-pressed=\"true\"").count(), 1);
            assert_eq!(html.matches("type=\"button\"").count(), 4);
            assert!(!html.contains("role=\"tab"));
            for pane in WorkspacePane::ALL {
                assert!(html.contains(pane.id()));
                pane_selection(panel, pane).run(true);
                assert_eq!(panel.get_untracked(), pane);
                pane_selection(panel, pane).run(false);
                assert_eq!(panel.get_untracked(), pane);
            }
        });
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
                on_change=Callback::new(move |raw| events.update(|events| events.push(raw))) english=RwSignal::new(false) readonly=Signal::derive(move || readonly) error=Signal::derive(|| Some("Giá trị 0 được giữ · chưa có trong profile".to_owned()))/> }.to_html();
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
            let html = view! { <ActionButton id="revision-test-save" english=RwSignal::new(false) blocked=true reason=BlockReason::SaveReview
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
                let html = view! { <DiscardDraftDialog open=RwSignal::new(true) english=RwSignal::new(english) blocked=Signal::derive(|| false)
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
                let html = view! { <FormSubmit id="session-submit" english=RwSignal::new(false) blocked=Signal::derive(move || blocked)>"Đăng nhập"</FormSubmit> }.to_html();
                assert!(html.contains("type=\"submit\""));
                assert_eq!(html.contains(" disabled"), blocked);
                assert_eq!(html.contains("aria-describedby=\"session-submit-reason\""), blocked);
                assert!(html.contains(text(Key::ControlBlocked, false)));
            });
        }
    }
}
