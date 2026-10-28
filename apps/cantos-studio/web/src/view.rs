use crate::account_menu::AccountEntry;
use crate::contract::{ApiError, ErrorCode, FieldIssue, ValidateScriptRequest};
use crate::ui::{
    ActionButton, BlockReason, DiscardDraftDialog, FormSubmit, WorkspaceNavigation, WorkspacePane,
};
use crate::{
    adaptation::ScriptAdaptation,
    api::{ObservedSession, StudioContext, StudioHttp},
    authoring_view::StructuredEditor,
    editor::{Editor, Intent, ValidationPreview},
    entry::{self, Entry},
    import::ManuscriptImport,
    inspector::RevisionInspector,
    messages::entry::{text as entry_text, Key as EntryKey},
    messages::{
        self,
        workspace::{text, Key},
        Locale,
    },
    production::ProductionInputs,
    session::{admits, AuthIntent, SessionState, SessionTicket},
    signin::{sign_in_failure, SignInForm},
    signin_form::SignInPanel,
};
use cantos_ui::{
    AccessibleName, Announcement, BackButton, Button, ButtonVariant, Container, ContainerWidth,
    Density, FieldLabel, IconName, LoadingState, Logo, LogoTone, LogoVariant, Status as UiStatus,
    Theme, ThemeCycleButton, UiRoot,
};
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;

pub(crate) fn operation_id() -> Result<String, ApiError> {
    let error = || ApiError {
        code: ErrorCode::Unavailable,
        current_revision: None,
        issues: vec![],
    };
    let window = web_sys::window().ok_or_else(error)?;
    let crypto = window.crypto().map_err(|_| error())?;
    let mut bytes = [0u8; 16];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| error())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

/// Mirrors the interface locale onto `<html lang>` so the whole document, including browser UI
/// and content outside `<main>`, is announced in the chosen language.
fn sync_document_language(locale: Locale) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("lang", locale.lang());
        let _ = root.set_attribute("dir", locale.dir());
    }
}

pub(crate) fn confirm_discard(locale: crate::localization::Locale) -> bool {
    web_sys::window().is_some_and(|window| {
        window
            .confirm_with_message(text(Key::DiscardConfirm, locale))
            .unwrap_or(false)
    })
}

pub(crate) fn check_script(
    preview: RwSignal<ValidationPreview>,
    issues: RwSignal<Vec<FieldIssue>>,
    api: StudioHttp,
    actor: String,
    snapshot: String,
    current: Signal<String>,
) {
    if preview.with_untracked(|preview| preview.busy) || actor.is_empty() {
        return;
    }
    let checking = preview
        .get_untracked()
        .start(actor.clone(), snapshot.clone());
    let ticket = checking.ticket;
    preview.set(checking);
    issues.set(vec![]);
    let api = api.snapshot();
    spawn_local(async move {
        let result = api
            .validate(&ValidateScriptRequest {
                script_json: snapshot.clone(),
            })
            .await
            .map(|response| response.issues);
        let _ = preview.try_update(|p| *p = p.finish(ticket, &actor, &snapshot, result));
        if preview
            .try_with_untracked(|p| {
                p.ticket == ticket
                    && p.actor == actor
                    && p.snapshot == snapshot
                    && current.get_untracked() == snapshot
            })
            .unwrap_or(false)
        {
            let _ = issues.try_set(preview.with_untracked(|p| p.issues.clone()));
        }
    });
}

#[component]
pub(crate) fn ValidationControls(
    #[prop(into)] id: String,
    preview: RwSignal<ValidationPreview>,
    issues: RwSignal<Vec<FieldIssue>>,
    draft: Signal<String>,
    actor: Signal<String>,
    locale: Signal<crate::localization::Locale>,
    disabled: Signal<bool>,
) -> impl IntoView {
    let api = expect_context::<StudioContext>().api;
    view! {
        <div class="validation-controls">
            <ActionButton id=id locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move || disabled.get() || actor.get().is_empty()) busy=Signal::derive(move || preview.with(|p| p.busy)) on_press=Callback::new(move |_| {
                if !disabled.get_untracked() { check_script(preview, issues, api, actor.get_untracked(), draft.get_untracked(), draft); }
            })>{move || text(Key::Validate, locale.get())}</ActionButton>
            <UiStatus announcement=Announcement::Polite>{move || preview.with(|p| {
                if p.busy { text(Key::Validating, locale.get()) }
                else if p.is_current(&actor.get(), &draft.get()) { text(Key::Valid, locale.get()) }
                else if !p.snapshot.is_empty() && (p.snapshot != draft.get() || p.actor != actor.get()) { text(Key::Invalidated, locale.get()) }
                else if let Some(error) = &p.error { messages::status(crate::editor::error_status(error), locale.get()) }
                else if !p.snapshot.is_empty() { text(Key::Invalidated, locale.get()) }
                else { text(Key::ValidateFirst, locale.get()) }
            })}</UiStatus>
            <p class="help">{move || text(Key::ValidateHelp, locale.get())}</p>
        </div>
    }
}

fn dispatch_save(
    state: RwSignal<Editor>,
    issues: RwSignal<Vec<FieldIssue>>,
    api: StudioHttp,
    intent: Intent,
) {
    let api = api.snapshot();
    spawn_local(async move {
        match api.save(&intent.script, &intent.request).await {
            Ok(response) => {
                let _ = state.try_update(|e| *e = e.saved(&intent, response));
            }
            Err(error) => {
                if state
                    .try_with_untracked(|e| e.accepts_save_findings(&intent))
                    .unwrap_or(false)
                {
                    let _ = issues.try_set(error.issues.clone());
                }
                let _ = state.try_update(|e| *e = e.save_failed(&intent, &error));
            }
        }
    });
}

fn begin_session(
    state: RwSignal<Editor>,
    session: RwSignal<SessionState>,
) -> Option<SessionTicket> {
    let (next, ticket) = session.get_untracked().begin()?;
    session.set(next);
    // Keep the owner of every mounted workspace. SessionState independently hides and
    // disables its private projection while authentication is unconfirmed.
    state.update(|editor| *editor = editor.checking_session());
    Some(ticket)
}

fn settle_session(
    state: RwSignal<Editor>,
    session: RwSignal<SessionState>,
    ticket: SessionTicket,
    result: Result<Option<ObservedSession>, ApiError>,
    unsaved: Signal<bool>,
) {
    let Some(current) = session.try_get_untracked() else {
        return;
    };
    if !current.accepts(ticket) {
        return;
    }
    match result {
        Ok(Some(response)) => {
            StudioHttp::accept_fence(Some(response.fence), current.epoch());
            let next = current.settled(ticket, Some(response.response.actor_id));
            let has_unsaved = unsaved.get_untracked();
            let _ = state.try_update(|editor| {
                *editor = editor.signed_in(next.retained_owner(editor.actor(), has_unsaved))
            });
            let _ = session.try_set(next);
        }
        Ok(None) => {
            StudioHttp::accept_fence(None, current.epoch());
            let error = ApiError {
                code: ErrorCode::Unauthenticated,
                current_revision: None,
                issues: vec![],
            };
            let _ = state.try_update(|editor| *editor = editor.session_failed(&error));
            let _ = session.try_set(current.settled(ticket, None));
        }
        Err(error) => {
            StudioHttp::accept_fence(None, current.epoch());
            let _ = state.try_update(|editor| *editor = editor.session_failed(&error));
            let _ = session.try_set(current.settled(ticket, None));
        }
    }
}

fn observe_session(
    state: RwSignal<Editor>,
    session: RwSignal<SessionState>,
    api: StudioHttp,
    unsaved: Signal<bool>,
) {
    let Some(ticket) = begin_session(state, session) else {
        return;
    };
    let api = api.snapshot();
    spawn_local(async move {
        let result = api.current_session().await;
        settle_session(state, session, ticket, result, unsaved);
    });
}

#[component]
pub fn Studio() -> impl IntoView {
    #[cfg(debug_assertions)]
    if crate::forge::view::requested() {
        let theme = RwSignal::new(Theme::Light);
        return view! {<UiRoot class="studio-shell" theme=theme density=Density::Comfortable><crate::forge::view::Preview theme=theme/></UiRoot>}.into_any();
    }
    // The existing Studio composition root also hosts its offline debug form preview.
    #[cfg(debug_assertions)]
    if crate::live::artwork::preview::requested() {
        let theme = RwSignal::new(Theme::Light);
        return view! {<UiRoot class="studio-shell" theme=theme density=Density::Comfortable><crate::live::artwork::preview::Preview theme=theme/></UiRoot>}.into_any();
    }
    #[cfg(debug_assertions)]
    if crate::authoring_preview::requested() {
        let theme = RwSignal::new(Theme::Light);
        return view! {<UiRoot class="studio-shell" theme=theme density=Density::Comfortable><crate::authoring_preview::Preview theme=theme/></UiRoot>}.into_any();
    }

    let bootstrap_api = StudioHttp::new();
    let state = RwSignal::new(Editor::default());
    let issues = RwSignal::new(Vec::<FieldIssue>::new());
    let preview = RwSignal::new(ValidationPreview::default());
    let session = RwSignal::new(SessionState::default());
    let cancelling = RwSignal::new(false);
    // Secrets live only in these forms, only until the typed intent is built.
    let entry_form = RwSignal::new(SignInForm::default());
    let account_form = RwSignal::new(SignInForm::default());
    let reauth_open = RwSignal::new(false);
    let account_menu_open = RwSignal::new(false);
    // Typed reauthentication input never outlives the popover that collected it.
    Effect::new(move |_| {
        if !account_menu_open.get() {
            reauth_open.set(false);
            account_form.update(SignInForm::clear);
        }
    });
    let session_epoch = Signal::derive(move || session.with(SessionState::epoch));
    // Retain draft ownership in Editor; only the API-confirmed actor is shown as signed in.
    let authenticated_actor =
        Signal::derive(move || session.with(|session| session.actor().map(str::to_owned)));
    let content_activity = RwSignal::new(false);
    let content_busy = RwSignal::new(false);
    let content_dirty = RwSignal::new(false);
    let import_dirty = RwSignal::new(false);
    let import_activity = RwSignal::new(false);
    let selection = RwSignal::new(String::new());
    let preference = crate::localization::load_preference();
    let locale_choice = RwSignal::new(preference.locale);
    let locale_warning = RwSignal::new(preference.warning);
    let locale = Signal::derive(move || locale_choice.get());
    let theme = RwSignal::new(Theme::System);
    let panel = RwSignal::new(WorkspacePane::from_hash(
        &web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default(),
    ));
    Effect::new(move |_| {
        let hash = format!("#{}", panel.get().id());
        if let Some(window) = web_sys::window() {
            if window.location().hash().ok().as_deref() != Some(&hash) {
                if let Ok(history) = window.history() {
                    let _ =
                        history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&hash));
                }
            }
        }
    });
    Effect::new(move |_| {
        let id = format!("{}-control", panel.get().id());
        leptos::leptos_dom::helpers::request_animation_frame(move || {
            crate::navigation::reveal_in_strip(&id)
        });
    });
    let pane_history =
        leptos::leptos_dom::helpers::window_event_listener_untyped("popstate", move |_| {
            if let Some(hash) = web_sys::window().and_then(|w| w.location().hash().ok()) {
                panel.set(WorkspacePane::from_hash(&hash));
            }
        });
    on_cleanup(move || pane_history.remove());
    let discard_open = RwSignal::new(false);
    let dialog_failure = RwSignal::new(false);
    let activity = RwSignal::new(false);
    let proposal_dirty = RwSignal::new(false);
    let proposal_activity = RwSignal::new(false);
    let inspector_pending = RwSignal::new(false);
    let production_dirty = RwSignal::new(false);
    let production_activity = RwSignal::new(false);
    let reviewed = RwSignal::new(false);
    let unsaved = Signal::derive(move || {
        state.with(|editor| editor.is_dirty() || editor.pending().is_some())
            || activity.get()
            || proposal_activity.get()
            || production_activity.get()
            || content_activity.get()
            || import_activity.get()
            || content_dirty.get()
            || import_dirty.get()
            || proposal_dirty.get()
            || production_dirty.get()
    });
    let session_pending = Signal::derive(move || {
        session.with(|session| {
            !session.projects_owner(&state.with(|editor| editor.actor().to_owned()))
        })
    });
    let entry_state = Memo::new(move |_| {
        session.with(|session| {
            state.with(|editor| entry::presentation(session, editor.actor(), editor.status()))
        })
    });
    let on_denial = Callback::new(move |error: ApiError| {
        // StudioHttp already checked the immutable request UUID and observation epoch. Preserve
        // the draft owner while a new API observation hides every private workspace.
        if !session
            .try_with_untracked(SessionState::pending)
            .unwrap_or(true)
        {
            let _ = session.try_update(|s| *s = s.invalidated());
            let _ = state.try_update(|e| *e = e.session_failed(&error));
            observe_session(state, session, bootstrap_api, unsaved);
        }
    });
    let api = bootstrap_api.with_denial(on_denial);
    let credential_loss = leptos::leptos_dom::helpers::window_event_listener_untyped(
        cantos_web_auth::browser::invalidation_event(cantos_web_auth::Surface::Studio),
        move |_| {
            if crate::api_endpoint::auth().is_ok_and(|client| !client.has_session()) {
                on_denial.run(ApiError {
                    code: ErrorCode::Unauthenticated,
                    current_revision: None,
                    issues: vec![],
                });
            }
        },
    );
    on_cleanup(move || credential_loss.remove());
    provide_context(StudioContext { api });
    on_cleanup(move || StudioHttp::accept_fence(None, 0));
    let retained_owner = StoredValue::new(String::new());
    Effect::new(move |_| {
        let actor = state.with(|editor| editor.actor().to_owned());
        if actor != retained_owner.get_value() {
            retained_owner.set_value(actor);
            // A clean account adoption must also discard the shell's private identifiers,
            // validation and review metadata. Dirty owners never cross this boundary.
            selection.set(String::new());
            preview.set(ValidationPreview::default());
            issues.set(vec![]);
            reviewed.set(false);
        }
    });
    let actor = Signal::derive(move || state.with(|e| e.actor().to_owned()));
    let draft = Signal::derive(move || {
        state.with(|e| {
            if e.can_edit() {
                e.draft().to_owned()
            } else {
                String::new()
            }
        })
    });
    let field_returns =
        StoredValue::new(Vec::<(WorkspacePane, crate::navigation::FieldReturn)>::new());
    let focusout =
        leptos::leptos_dom::helpers::window_event_listener_untyped("focusout", move |event| {
            let Some(target) = event
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            else {
                return;
            };
            // The originating pane owns the caret, even when authentication has just hidden it
            // or the navigation intent already changed the selected pane.
            let Some(pane) = WorkspacePane::ALL
                .into_iter()
                .filter(|pane| *pane != WorkspacePane::Dashboard)
                .find(|pane| {
                    target
                        .closest(&format!("#{}", pane.id()))
                        .ok()
                        .flatten()
                        .is_some()
                })
            else {
                return;
            };
            if let Some(snapshot) = crate::navigation::FieldReturn::capture(target) {
                field_returns.update_value(|returns| {
                    returns.retain(|(saved, _)| *saved != pane);
                    returns.push((pane, snapshot));
                });
            }
        });
    on_cleanup(move || focusout.remove());
    let focus_owner = StoredValue::new(String::new());
    let focus_context = Memo::new(move |_| (entry_state.get(), panel.get()));
    Effect::new(move |_| {
        let (entry, pane) = focus_context.get();
        let owner = actor.get_untracked();
        if focus_owner.get_value() != owner {
            field_returns.set_value(vec![]);
            focus_owner.set_value(owner);
        }
        let epoch = session_epoch.get_untracked();
        leptos::leptos_dom::helpers::request_animation_frame(move || {
            if session_epoch.try_get_untracked() != Some(epoch)
                || panel.try_get_untracked() != Some(pane)
                || entry_state.try_get_untracked() != Some(entry)
            {
                return;
            }
            if entry == Entry::Workspace && crate::navigation::pane_has_focus(pane.id()) {
                return;
            }
            let heading = if entry != Entry::Workspace {
                "studio-entry-title"
            } else if pane == WorkspacePane::Dashboard {
                "dashboard-title"
            } else {
                "studio-pane-title"
            };
            let restored = entry == Entry::Workspace
                && field_returns
                    .try_with_value(|returns| {
                        returns
                            .iter()
                            .find(|(saved, _)| *saved == pane)
                            .is_some_and(|(_, field)| field.restore())
                    })
                    .unwrap_or(false);
            if !restored {
                crate::navigation::focus_heading(heading);
            }
        });
    });
    let copy = move || messages::copy(locale.get());
    let blocked = move || {
        session_pending.get()
            || state.with(|e| e.busy() || e.pending().is_some())
            || inspector_pending.get()
    };
    let save_blocked = move || {
        blocked()
            || activity.get()
            || !reviewed.get()
            || !state.with(Editor::can_save)
            || !preview.with(|p| p.is_current(&actor.get(), &draft.get()))
    };
    Effect::new(move |_| {
        if activity.get() {
            reviewed.set(false);
        }
    });
    Effect::new(move |_| sync_document_language(locale.get()));
    // Reconciliation adopts a different base only after a fresh review of the displayed head.
    let review_context = Memo::new(move |_| {
        state.with(|editor| {
            (
                editor.actor().to_owned(),
                editor.script().to_owned(),
                editor.base(),
                editor
                    .remote()
                    .map(|revision| (revision.revision, revision.export_digest.clone())),
            )
        })
    });
    Effect::new(move |_| {
        review_context.get();
        reviewed.set(false);
    });
    // A browser-owned confirmation covers back, reload and close. Drafts remain session memory only.
    let unload =
        leptos::leptos_dom::helpers::window_event_listener_untyped("beforeunload", move |event| {
            if state.with_untracked(Editor::has_unsaved)
                || activity.get_untracked()
                || proposal_dirty.get_untracked()
                || inspector_pending.get_untracked()
                || production_dirty.get_untracked()
                || content_dirty.get_untracked()
                || content_activity.get_untracked()
                || import_dirty.get_untracked()
                || import_activity.get_untracked()
                || production_activity.get_untracked()
            {
                event.prevent_default();
                if let Some(event) = event.dyn_ref::<web_sys::BeforeUnloadEvent>() {
                    event.set_return_value("");
                }
            }
        });
    on_cleanup(move || unload.remove());
    let restored =
        leptos::leptos_dom::helpers::window_event_listener_untyped("pageshow", move |_| {
            let _ = state.try_update(|_| ());
            let _ = selection.try_update(|_| ());
            // Browser-history restoration can carry another tab's old presentation. Preserve
            // in-flight authoring intent; the explicit session check remains available.
            if !session.with_untracked(SessionState::pending)
                && !state.with_untracked(Editor::busy)
                && !activity.get_untracked()
                && !proposal_activity.get_untracked()
                && !content_activity.get_untracked()
                && !production_activity.get_untracked()
                && !inspector_pending.get_untracked()
                && !import_activity.get_untracked()
                && !unsaved.get_untracked()
            {
                observe_session(state, session, api, unsaved);
            }
        });
    on_cleanup(move || restored.remove());

    // Reload keeps no RAM credential and Studio never reads the HttpOnly cookie: always ask the API
    // who the browser is. Only an API-admitted session opens a workspace; sign-in is explicit.
    observe_session(state, session, api, unsaved);

    let auth_blocked = Signal::derive(move || {
        session.with(SessionState::pending)
            || state.with(Editor::busy)
            || activity.get()
            || proposal_activity.get()
            || content_busy.get()
            || production_activity.get()
            || inspector_pending.get()
            || import_activity.get()
    });
    let refresh = Callback::new(move |_| {
        if !auth_blocked.get_untracked() {
            observe_session(state, session, api, unsaved);
        }
    });
    // Logout never waits for a save, upload, import or request: it closes local authority now.
    let logout_blocked =
        Signal::derive(move || !session.with(|session| admits(AuthIntent::Logout, session, false)));
    let logout = Callback::new(move |_| {
        if !session.with_untracked(|session| admits(AuthIntent::Logout, session, false)) {
            return;
        }
        let Some(ticket) = begin_session(state, session) else {
            return;
        };
        entry_form.update(SignInForm::clear);
        account_form.update(SignInForm::clear);
        reauth_open.set(false);
        let api = api.snapshot();
        // Synchronously, before the first await: the owner fences every lease (so a late reply of
        // this account is an unknown outcome, never an acknowledgement), and the page-wide fence
        // is withdrawn so nothing new is dispatched and a late denial of A cannot touch B.
        // Unsaved drafts and unresolved operations stay with the editor and each panel's owner.
        let invalidated = api.begin_logout();
        StudioHttp::accept_fence(None, session.get_untracked().epoch());
        spawn_local(async move {
            let result = match invalidated {
                Ok(()) => api.logout().await.map(|()| None),
                Err(error) => Err(error),
            };
            settle_session(state, session, ticket, result, unsaved);
        });
    });
    // One typed sign-in intent. A retained draft (or an explicit reauthentication) pins its owner:
    // another account is refused and revoked by the identity client, never adopted, so the owner's
    // private drafts are never projected to it. Nothing is replayed after a failure.
    let sign_in = move |form: RwSignal<SignInForm>, reauthenticate: bool| {
        if auth_blocked.get_untracked() {
            return;
        }
        let expected = (unsaved.get_untracked() || reauthenticate)
            .then(|| state.with_untracked(|e| e.actor().to_owned()))
            .filter(|a| !a.is_empty());
        let Some((issued, intent)) = form.try_update(|f| f.begin(expected)).flatten() else {
            return;
        };
        let Some(ticket) = begin_session(state, session) else {
            form.update(|f| f.abandon(issued));
            return;
        };
        let api = api.snapshot();
        let typed_email = intent.email.clone();
        spawn_local(async move {
            let result = api.sign_in(intent).await;
            let outcome = result.as_ref().map(|_| ()).map_err(|failure| *failure);
            let _ = form.try_update(|f| {
                f.finish(issued, outcome);
            });
            match outcome {
                Ok(()) => {
                    // Nothing typed survives a completed sign-in, in either form.
                    let _ = entry_form.try_update(SignInForm::clear);
                    let _ = account_form.try_update(SignInForm::clear);
                }
                Err(failure) if reauthenticate => {
                    // A failed reauthentication returns to the entry; show the reason there.
                    let _ =
                        entry_form.try_update(|f| f.show(&typed_email, sign_in_failure(failure)));
                    let _ = account_form.try_update(SignInForm::clear);
                }
                Err(_) => {}
            }
            let _ = reauth_open.try_set(false);
            settle_session(
                state,
                session,
                ticket,
                result.map(Some).map_err(crate::api::auth_error),
                unsaved,
            );
        });
    };
    let submit_entry = Callback::new(move |()| sign_in(entry_form, false));
    let submit_reauthentication = Callback::new(move |()| sign_in(account_form, true));
    let cancel_check = Callback::new(move |_| {
        if !session.with_untracked(SessionState::pending) || cancelling.get_untracked() {
            return;
        }
        let Ok(auth) = crate::api_endpoint::auth() else {
            return;
        };
        let _ = auth.cancel_authorization();
        let _ = auth.invalidate();
        let invalidated = session.get_untracked().invalidated();
        let Some((waiting, ticket)) = invalidated.begin() else {
            return;
        };
        StudioHttp::accept_fence(None, waiting.epoch());
        session.set(waiting);
        cancelling.set(true);
        state.update(|e| *e = e.release_session_check());
        spawn_local(async move {
            let result = auth
                .settled_cleanup()
                .await
                .map(|()| None)
                .map_err(crate::api::auth_error);
            let _ = cancelling.try_set(false);
            settle_session(state, session, ticket, result, unsaved);
        });
    });
    let editor_script = Signal::derive(move || {
        if session_pending.get() || activity.get() {
            return None;
        }
        state.with(Editor::accepted_catalog_script)
    });
    let open_import = Callback::new(move |()| panel.set(WorkspacePane::Import));
    let open_editor = Callback::new(move |target: Option<crate::contract::CatalogScriptRef>| {
        let Some(target) = target else {
            panel.set(WorkspacePane::Revisions);
            return;
        };
        let current = state.get_untracked();
        if current.script() == target.id
            && (current.has_unsaved()
                || current.accepted_catalog_script().as_ref() == Some(&target))
        {
            // Returning from Catalog never reloads over a retained editor or pending save.
            selection.set(target.id);
            panel.set(WorkspacePane::Revisions);
            return;
        }
        if blocked() || activity.get_untracked() || production_activity.get_untracked() {
            return;
        }
        let discard = !current.is_dirty() || confirm_discard(locale.get_untracked());
        let Some(selected) = current.select_catalog_script(target.id.clone(), discard) else {
            return;
        };
        selection.set(target.id.clone());
        issues.set(vec![]);
        reviewed.set(false);
        preview.set(ValidationPreview::default());
        panel.set(WorkspacePane::Revisions);
        let Some((reading, ticket)) = selected.start_read() else {
            state.set(selected);
            return;
        };
        state.set(reading);
        let api = api.snapshot();
        spawn_local(async move {
            let result = api.read_revision(ticket.script(), target.revision).await;
            let _ = state.try_update(|editor| {
                *editor = match result {
                    Ok(response) => editor.loaded_catalog_revision(
                        &ticket,
                        response,
                        &target,
                        activity.get_untracked(),
                    ),
                    Err(error) => editor.read_failed(&ticket, &error),
                };
            });
        });
    });
    let open_content = Callback::new(move |()| panel.set(WorkspacePane::Content));

    view! {
        <UiRoot class="studio-shell" theme=theme density=Density::Comfortable>
        <Container width=ContainerWidth::Fluid>
        <main class="studio-main" lang=move || locale.get().lang() dir=move ||locale.get().dir()>
            <p role="status" hidden=move ||locale_warning.get().is_none()>{move ||crate::localization::preference_warning(locale_warning.get(),locale.get())}</p>
            <header class="studio-header">
                <div class="studio-brand" aria-label="Cantos Studio">
                    <Logo variant=LogoVariant::CantosStudio tone=LogoTone::Light class="studio-logo-light"/>
                    <Logo variant=LogoVariant::CantosStudio tone=LogoTone::Dark class="studio-logo-dark"/>
                </div>
                <div class="toolbar studio-topbar">
                    <span class="studio-language-tip" title=move ||crate::localization::generated::locale::text(crate::localization::generated::locale::Key::Language,locale.get())>
                    <cantos_ui::Select id="studio-language" class="studio-language-field"
                        density=cantos_ui::SelectDensity::Compact label_placement=cantos_ui::SelectLabelPlacement::Inline
                        label=Signal::derive(move ||crate::localization::generated::locale::text(crate::localization::generated::locale::Key::Language,locale.get()).to_owned())
                        value=Signal::derive(move ||locale.get().lang().to_owned())
                        short_label=Callback::new(|tag:String| tag.split('-').next().map(str::to_owned))
                        options={Locale::supported().map(|option|cantos_ui::SelectOption { lang: Some(option.lang().to_owned()), ..cantos_ui::SelectOption::new(option.lang(),option.autonym()) }).collect::<Vec<_>>()}
                        on_input=Callback::new(move |tag:String| {
                            if let Some(selected)=Locale::from_lang(&tag) {
                                locale_choice.set(selected);
                                locale_warning.set(crate::localization::save_preference(selected).err());
                            } else { locale_warning.set(Some(cantos_localization::PreferenceWarning::Unsupported)); }
                        })/>
                    </span>
                    <ThemeCycleButton id="theme-select" theme=theme show_label=true labels=Signal::derive(move ||crate::localization::theme_labels(locale.get()))/>
                    <AccountEntry actor=authenticated_actor
                        available=Signal::derive(move || entry_state.get() == Entry::Workspace)
                        open=account_menu_open
                        label=Signal::derive(move || entry_text(EntryKey::Session, locale.get()).to_owned())
                        close_label=Signal::derive(move || entry_text(EntryKey::CloseAccount, locale.get()).to_owned())>
                        <ActionButton id="account-refresh" icon=IconName::Refresh locale=locale variant=ButtonVariant::Tonal blocked=auth_blocked on_press=refresh>{move || text(Key::SessionRefresh,locale.get())}</ActionButton>
                        <ActionButton id="account-reauthenticate" icon=IconName::Login locale=locale variant=ButtonVariant::Text blocked=auth_blocked on_press=Callback::new(move |_| reauth_open.update(|open| *open = !*open))>{move ||entry_text(EntryKey::Reauthenticate,locale.get())}</ActionButton>
                        <Show when=move || reauth_open.get()>
                            <SignInPanel id="account" form=account_form locale=locale blocked=auth_blocked on_submit=submit_reauthentication reauthenticate=true/>
                        </Show>
                        <ActionButton id="account-logout" icon=IconName::Logout locale=locale variant=ButtonVariant::Danger blocked=logout_blocked on_press=logout>{move || text(Key::SessionLogout,locale.get())}</ActionButton>
                    </AccountEntry>
                </div>
            </header>
            <section class="studio-entry" hidden=move ||entry_state.get()==Entry::Workspace aria-labelledby="studio-entry-title" aria-busy=move ||session.with(SessionState::pending)>
                <aside class="entry-story" aria-labelledby="entry-story-title">
                    <div class="entry-artwork" aria-hidden="true">
                        <Logo variant=LogoVariant::Symbol tone=LogoTone::Light class="studio-logo-light"/>
                        <Logo variant=LogoVariant::Symbol tone=LogoTone::Dark class="studio-logo-dark"/>
                    </div>
                    <div class="entry-story-copy">
                        <p class="entry-eyebrow">"Cantos Studio"</p>
                        <h2 id="entry-story-title">{move ||entry_text(EntryKey::Welcome,locale.get())}</h2>
                        <p>{move ||entry_text(EntryKey::WelcomeHelp,locale.get())}</p>
                    </div>
                </aside>
                <div class="entry-form">
                <h1 id="studio-entry-title" tabindex="-1">{move ||entry_text(crate::messages::entry::title(entry_state.get()),locale.get())}</h1>
                {move ||crate::messages::entry::help(entry_state.get()).map(|key|view!{<p class="help">{entry_text(key,locale.get())}</p>})}
                <div hidden=move ||!session.with(SessionState::pending) inert=move ||!session.with(SessionState::pending)>
                    <LoadingState announcement=Announcement::Polite>{move ||if cancelling.get(){entry_text(EntryKey::CleanupPending,locale.get())}else{text(Key::SessionChecking,locale.get())}}</LoadingState>
                    <Button id="session-cancel" variant=ButtonVariant::Text state=Signal::derive(move ||if cancelling.get(){cantos_ui::DisabledReason::new(entry_text(EntryKey::CleanupPending,locale.get())).map(cantos_ui::ButtonState::Disabled).unwrap_or(cantos_ui::ButtonState::Enabled)}else{cantos_ui::ButtonState::Enabled}) on_press=cancel_check>{move ||entry_text(EntryKey::Cancel,locale.get())}</Button>
                </div>
                <div hidden=move ||session.with(SessionState::pending) inert=move ||session.with(SessionState::pending)>
                <Show when=move ||entry_state.get()!=Entry::SignInUnavailable>
                    <SignInPanel id="session" form=entry_form locale=locale blocked=auth_blocked on_submit=submit_entry/>
                </Show>
                <p class="help" hidden=move || !unsaved.get()>{move || entry_text(EntryKey::MemoryHelp,locale.get())}</p>
                <div class="entry-recovery">
                    <div hidden=move ||entry_state.get()==Entry::Login && !unsaved.get()>
                    <ActionButton id="session-refresh" locale=locale variant=ButtonVariant::Text blocked=auth_blocked on_press=refresh>{move ||entry_text(EntryKey::Retry,locale.get())}</ActionButton>
                    </div>
                    <div hidden=move ||authenticated_actor.get().is_none()>
                        <ActionButton id="session-logout" locale=locale variant=ButtonVariant::Text blocked=auth_blocked on_press=logout>{move ||text(Key::SessionLogout,locale.get())}</ActionButton>
                    </div>
                </div>
                </div>
                <p class="help" hidden=move || !unsaved.get()>{move ||text(Key::SessionRetained,locale.get())}</p>
                </div>
            </section>
            <div id="studio-private-workspaces" hidden=move ||session_pending.get() inert=move ||session_pending.get()>
            <div class="studio-workbench">
            <WorkspaceNavigation panel=panel locale=locale/>
            <div class="studio-workbench-content">
            <div class="studio-pane-heading" hidden=move ||panel.get()==WorkspacePane::Dashboard>
                <h2 id="studio-pane-title" tabindex="-1">{move ||panel.get().label(locale.get())}</h2>
                {move || AccessibleName::new(entry_text(EntryKey::Back,locale.get())).ok().map(|name| view! {
                    <BackButton id="studio-back-desk" name=name on_back=Callback::new(move |_|panel.set(WorkspacePane::Dashboard))/>
                })}
            </div>
            <div hidden=move || !matches!(panel.get(),WorkspacePane::Content|WorkspacePane::Dashboard)><crate::live::workspace::ContentWorkspace actor=actor locale=locale active=Signal::derive(move ||!session_pending.get()) dashboard=Signal::derive(move ||panel.get()==WorkspacePane::Dashboard) on_import=open_import on_editor=open_editor editor_script=editor_script on_open_content=open_content session_epoch=session_epoch session_pending=session_pending activity=content_activity busy=content_busy dirty=content_dirty/></div>
            <div id="workspace-import" hidden=move || panel.get() != WorkspacePane::Import><ManuscriptImport actor=actor locale=locale session_pending=session_pending dirty=import_dirty activity=import_activity /></div>
            <div id="workspace-proposal" hidden=move || panel.get() != WorkspacePane::Proposal><ScriptAdaptation actor=actor locale=locale session_pending=session_pending dirty=proposal_dirty activity=proposal_activity /></div>
            <div id="workspace-production" hidden=move || panel.get() != WorkspacePane::Production><ProductionInputs editor=state locale=locale session_pending=session_pending script_activity=activity dirty=production_dirty activity=production_activity /></div>
            <section id="workspace-revisions" hidden=move || panel.get() != WorkspacePane::Revisions aria-labelledby="revision-title">
                <h2 id="revision-title">{move || text(Key::Revisions, locale.get())}</h2>
                <form on:submit=move |event| {
                    event.prevent_default(); if blocked() || activity.get_untracked() || production_activity.get_untracked() { return; }
                    let target = selection.get_untracked();
                    if state.with_untracked(|e| e.script() != target) {
                        if (state.with_untracked(Editor::is_dirty) || production_dirty.get_untracked() || content_dirty.get_untracked() || content_activity.get_untracked()) && !confirm_discard(locale.get_untracked()) { return; }
                        state.update(|e| *e = e.discard_and_select(target)); issues.set(vec![]); reviewed.set(false); preview.set(ValidationPreview::default());
                    }
                    let Some((reading, ticket)) = state.get_untracked().start_read() else { return; };
                    state.set(reading);
                    let api = api.snapshot();
    spawn_local(async move {
                        let result = api.read(ticket.script()).await;
                        let _ = state.try_update(|e| *e = match result { Ok(response) => e.loaded_with_buffer(&ticket, response, activity.get_untracked()), Err(error) => e.read_failed(&ticket, &error) });
                    });
                }>
                    <FieldLabel for_id="script-id">{move || copy().script}</FieldLabel>
                    <div class="toolbar"><input class="cui-field-control" id="script-id" autocomplete="off" spellcheck="false" disabled=blocked prop:value=move || selection.get() on:input=move |e| selection.set(event_target_value(&e)) />
                        <FormSubmit id="revision-read" locale=locale blocked=Signal::derive(move || blocked() || activity.get() || production_activity.get())>{move || copy().read}</FormSubmit></div>
                </form>
                <div class="revision-strip">
                    <p class="identity"><code>{move || state.with(|e| e.script().to_owned())}</code>" · "{move || copy().base}<output id="base-revision">{move || state.with(Editor::base)}</output></p>
                    <p>{move || if state.with(Editor::is_dirty) || activity.get() { text(Key::Unsaved, locale.get()) } else { text(Key::Saved, locale.get()) }}</p>
                </div>
                <StructuredEditor draft=draft locale=locale activity=activity namespace="revision"
                    readonly=Signal::derive(move || session_pending.get() || state.with(|e| e.busy() || !e.can_edit()))
                    baseline=Signal::derive(move || state.with(|e| e.remote().map(|r| r.script_json.clone()).unwrap_or_default()))
                    issues=Signal::derive(move || issues.get())
                    on_change=Callback::new(move |text| { state.update(|e| *e = e.edit(text)); issues.set(vec![]); reviewed.set(false); })>
                    <RevisionInspector state=state locale=locale activity=inspector_pending draft_activity=activity />
                </StructuredEditor>
                <ValidationControls id="revision-validate" preview=preview issues=issues draft=draft actor=actor locale=locale disabled=Signal::derive(move || blocked() || activity.get()) />
                <crate::ui::ReviewCheckbox id="revision-reviewed" locale=locale
                    label=Signal::derive(move ||text(Key::ReviewBeforeSave,locale.get()).to_owned())
                    checked=reviewed blocked=Signal::derive(move ||blocked() || activity.get() || !preview.with(|p|p.is_current(&actor.get(),&draft.get())))
                    on_change=Callback::new(move |checked|reviewed.set(checked))/>

                <div class="toolbar">
                    <ActionButton id="revision-save" icon=IconName::Save locale=locale blocked=Signal::derive(save_blocked) busy=Signal::derive(move || state.with(Editor::busy)) reason=BlockReason::SaveReview described_by="request-reason" on_press=Callback::new(move |_| {
                        if save_blocked() { return; }
                        match operation_id() {
                            Ok(op) => if let Some((saving,intent)) = state.get_untracked().start_save(op) { state.set(saving); issues.set(vec![]); dispatch_save(state,issues,api,intent); },
                            Err(error) => state.update(|e| *e = e.failed(&error)),
                        }
                    })>{move || copy().save}</ActionButton>
                    <Show when=move || state.with(Editor::awaits_reconciliation)><ActionButton id="revision-retry" icon=IconName::Refresh locale=locale variant=ButtonVariant::Tonal reason=BlockReason::Pending blocked=Signal::derive(move || session_pending.get() || activity.get() || state.with(|e| e.pending().is_some_and(|i| i.actor != e.actor()))) on_press=Callback::new(move |_| {
                        if session_pending.get_untracked() || activity.get_untracked() { return; }
                        if let Some((saving,intent)) = state.get_untracked().retry() { state.set(saving); dispatch_save(state,issues,api,intent); }
                    })>{move || copy().retry}</ActionButton></Show>
                    <ActionButton id="revision-discard" icon=IconName::DeleteOutline locale=locale variant=ButtonVariant::Secondary
                        focus_on_press=true
                        blocked=Signal::derive(move || blocked() || activity.get() || production_activity.get())
                        on_press=Callback::new(move |_| {
                            if blocked() || activity.get_untracked() || production_activity.get_untracked() { return; }
                            dialog_failure.set(false); discard_open.set(true);
                        })>{move || text(Key::Discard, locale.get())}</ActionButton>
                </div>
                <p id="request-reason" class="help">{move || state.with(|e| if e.busy() { copy().busy } else if e.pending().is_some() { copy().pending } else if e.actor().is_empty() { copy().sign_in_first } else if save_blocked() { text(Key::ValidateFirst, locale.get()) } else { "" })}</p>
            </section>
            <div hidden=move ||panel.get()!=WorkspacePane::Revisions><UiStatus announcement=Announcement::Polite><span id="status">{move || messages::status(state.with(Editor::status),locale.get())}</span></UiStatus></div>
            <Show when=move || dialog_failure.get()><UiStatus announcement=Announcement::Polite><p>{move || text(Key::DialogFailure, locale.get())}</p></UiStatus></Show>
            <DiscardDraftDialog open=discard_open locale=locale
                blocked=Signal::derive(move || blocked() || activity.get() || production_activity.get())
                on_error=Callback::new(move |_| { dialog_failure.set(true); discard_open.set(false); })
                on_confirm=Callback::new(move |()| {
                    if blocked() || activity.get_untracked() || production_activity.get_untracked() { return false; }
                    state.update(|e| *e = e.discard_and_select(String::new())); selection.set(String::new());
                    reviewed.set(false); issues.set(vec![]); preview.set(ValidationPreview::default()); true
                })/>
            </div>
            </div>
            </div>
        </main>
        </Container>
        </UiRoot>
    }.into_any()
}
