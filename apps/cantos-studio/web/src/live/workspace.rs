//! App-local forms compose established Studio controls; no shared component is introduced.
use super::{
    browser,
    copy::{self, text, Key},
    *,
};
use crate::contract::*;
use crate::ui::{ActionButton, BlockReason};
use leptos::{prelude::*, task::spawn_local};
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::AbortController;

/// One request authority per panel: session fence, deadline, abort and exact retry.
#[derive(Clone, Copy)]
pub(super) struct Dispatch {
    pub(super) state: RwSignal<RequestState>,
    pub(super) session_pending: Signal<bool>,
    pub(super) abort: StoredValue<Option<AbortController>, LocalStorage>,
    pub(super) complete: Callback<(Command, Result<Reply, ClientError>)>,
    pub(super) api: crate::api::StudioHttp,
    pub(super) progress: Option<Callback<artwork::platform::Transfer>>,
}
impl Dispatch {
    pub(super) fn abort(self) {
        self.abort.update_value(|controller| {
            if let Some(c) = controller.take() {
                c.abort();
            }
        });
    }
    pub(super) fn cancel(self) {
        self.abort();
        self.state.update(|s| {
            if let Some(next) = s.cancel() {
                *s = next;
            }
        });
    }
    pub(super) fn send(self, command: Command) {
        let Some((sending, ticket)) = self
            .state
            .get_untracked()
            .begin_in_session(command.clone(), self.session_pending.get_untracked())
        else {
            return;
        };
        self.run(sending, ticket, command);
    }
    pub(super) fn reconcile(self) {
        if self.session_pending.get_untracked() {
            return;
        }
        if let Some((sending, ticket, command)) = self.state.get_untracked().begin_reconcile() {
            self.run(sending, ticket, command);
        }
    }
    pub(super) fn retry(self) {
        if self.session_pending.get_untracked() {
            return;
        }
        let Some((sending, ticket)) = self.state.get_untracked().retry() else {
            return;
        };
        let Some(command) = sending.pending().cloned() else {
            return;
        };
        self.run(sending, ticket, command);
    }
    fn run(self, sending: RequestState, ticket: Ticket, command: Command) {
        self.abort();
        let Ok(controller) = AbortController::new() else {
            self.input_error();
            return;
        };
        self.abort.set_value(Some(controller.clone()));
        // A receipt read executes a different command while completing the original intent.
        let Some(acknowledged_command) = sending.pending().cloned() else {
            return;
        };
        self.state.set(sending);
        let api = self.api.snapshot();
        let session_fence = api.fence();
        spawn_local(async move {
            let progress = self.progress.map(|callback| {
                Callback::new(move |value| {
                    if self
                        .state
                        .try_with_untracked(|s| s.accepts(ticket))
                        .unwrap_or(false)
                    {
                        callback.run(value);
                    }
                })
            });
            let reply = browser::execute(&command, controller, session_fence, progress).await;
            if self
                .state
                .try_with_untracked(|s| s.accepts(ticket))
                .unwrap_or(false)
            {
                let _ = self
                    .state
                    .try_update(|s| *s = s.finish(ticket, reply.clone()));
                // Receipt reconciliation executes an owner GET even when the retained intent
                // is a producer Upload. Classify the executed request, not that original intent.
                if command.credentials() == Credentials::OwnerSession
                    && reply == Err(ClientError::SessionChanged)
                {
                    api.invalidate_projection(&crate::session::unconfirmed_response());
                }
                let denial = reply
                    .as_ref()
                    .err()
                    .and_then(|e| e.owner_session_denial(&command))
                    .cloned();
                if let Some(error) = denial {
                    let _ = api.notify_denial(Err::<(), _>(error));
                }
                let acknowledged = self
                    .state
                    .try_with_untracked(|s| {
                        s.completion()
                            .map(|result| result.cloned().map_err(Clone::clone))
                    })
                    .flatten();
                if let Some(acknowledged) = acknowledged {
                    self.complete.run((acknowledged_command, acknowledged));
                }
            }
        });
    }
    fn input_error(self) {
        self.state.update(|s| *s = s.input_failed());
    }
    pub(super) fn build(self, result: Result<Command, ClientError>) {
        match result {
            Ok(command) => self.send(command),
            Err(_) => self.input_error(),
        }
    }
}
pub(super) fn uuid() -> Result<String, ClientError> {
    crate::view::operation_id().map_err(|_| ClientError::Transport)
}
fn refs(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn context(facts: RwSignal<Facts>) -> Result<(CatalogWork, CatalogContentUnit), ClientError> {
    facts.with_untracked(|f| {
        Ok((
            f.work.clone().ok_or(ClientError::InvalidInput)?,
            f.unit.clone().ok_or(ClientError::InvalidInput)?,
        ))
    })
}
#[component]
pub(super) fn Field(
    id: &'static str,
    label: Key,
    value: RwSignal<String>,
    english: Signal<bool>,
    #[prop(default = false)] multiline: bool,
) -> impl IntoView {
    view! {<div class="live-field"><label for=id>{move || text(label,english.get())}</label>
        {if multiline {view! {<textarea id=id class="cui-field-control" prop:value=move || value.get() on:input=move |e|value.set(event_target_value(&e))/>}.into_any()}
        else {view! {<input id=id class="cui-field-control" autocomplete="off" prop:value=move ||value.get() on:input=move |e|value.set(event_target_value(&e))/>}.into_any()}}
    </div>}
}
#[component]
pub(super) fn SelectField(
    id: &'static str,
    label: Key,
    value: RwSignal<String>,
    english: Signal<bool>,
    children: Children,
) -> impl IntoView {
    view! {<div class="live-field"><label for=id>{move ||text(label,english.get())}</label><select id=id class="cui-field-control" prop:value=move || value.get() on:change=move |e|value.set(event_target_value(&e))>{children()}</select></div>}
}
#[derive(Clone, Copy)]
struct ContentFields {
    body: RwSignal<String>,
    manuscript: RwSignal<String>,
    provenance: RwSignal<String>,
    rights: RwSignal<String>,
}
impl ContentFields {
    fn restore(self, value: &super::drafts::ContentForm) {
        self.body.set(value.kind.clone());
        self.manuscript.set(value.text.clone());
        self.provenance.set(value.provenance.clone());
        self.rights.set(value.rights.join(", "));
    }
    fn snapshot(self, mut value: super::drafts::ContentForm) -> super::drafts::ContentForm {
        value.kind = self.body.get_untracked();
        value.text = self.manuscript.get_untracked();
        value.provenance = self.provenance.get_untracked();
        value.rights = refs(self.rights.get_untracked());
        value
    }
}
#[component]
pub fn ContentWorkspace(
    actor: Signal<String>,
    english: Signal<bool>,
    active: Signal<bool>,
    session_epoch: Signal<u64>,
    session_pending: Signal<bool>,
    activity: RwSignal<bool>,
    busy: RwSignal<bool>,
    dirty: RwSignal<bool>,
    dashboard: Signal<bool>,
    on_import: Callback<()>,
    on_editor: Callback<Option<CatalogScriptRef>>,
    editor_script: Signal<Option<CatalogScriptRef>>,
    on_open_content: Callback<()>,
) -> impl IntoView {
    let facts = RwSignal::new(Facts::default());
    let state = RwSignal::new(RequestState::default());
    let dashboard_query = RwSignal::new(super::dashboard::DashboardQuery::default());
    let file = RwSignal::new(None::<(Arc<Vec<u8>>, String)>);
    let file_generation = RwSignal::new(0u64);
    let file_busy = RwSignal::new(false);
    let file_error = RwSignal::new(false);
    let credential = RwSignal::new(None::<super::brief_controls::LocalUploadPermission>);
    let title = RwSignal::new(String::new());
    let synopsis = RwSignal::new(String::new());
    let rights = RwSignal::new(String::new());
    let kind = RwSignal::new(DEFAULT_WORK_KIND.to_string());
    let new_unit_title = RwSignal::new(String::new());
    let new_unit_description = RwSignal::new(String::new());
    let unit_title = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let body = RwSignal::new("text".to_string());
    let manuscript = RwSignal::new(String::new());
    let provenance = RwSignal::new(String::new());
    let content_rights = RwSignal::new(String::new());
    let drafts = RwSignal::new(super::drafts::AuthoringDrafts::default());
    let ancillary_dirty = RwSignal::new(false);
    let publication_dirty = RwSignal::new(false);
    let catalog_dirty = RwSignal::new(false);
    let units_complete = RwSignal::new(false);
    let replace_head = StoredValue::new(None::<(String, super::drafts::ContentForm)>);
    let content_fields = ContentFields {
        body,
        manuscript,
        provenance,
        rights: content_rights,
    };
    let brief_version = RwSignal::new("0".to_string());
    let notes = RwSignal::new(String::new());
    let profile = RwSignal::new(String::new());
    let use_note = RwSignal::new(String::new());
    let attribution = RwSignal::new(String::new());
    let naming = RwSignal::new(String::new());
    let qa = RwSignal::new(String::new());
    let producer = RwSignal::new(String::new());
    let producer_provenance = RwSignal::new(String::new());
    let method = RwSignal::new(String::new());
    let quality = RwSignal::new(String::new());
    let submission_id = RwSignal::new(String::new());
    let candidate_id = RwSignal::new(String::new());
    let review_kind = RwSignal::new("qc".to_string());
    let decision = RwSignal::new("rejected".to_string());
    let review_notes = RwSignal::new(String::new());
    let evidence = RwSignal::new(String::new());
    let consent = RwSignal::new(None::<ReviewConsent>);
    let current_review = Signal::derive(move || {
        facts.with(|f| {
            let s = f.submission.as_ref()?;
            let r = s.rendition.as_ref()?;
            Some(ReviewConsent {
                unit: f.unit_id(),
                submission: s.submission.id.clone(),
                rendition_digest: r.digest.clone(),
                sha256: r.verified_media.sha256.clone(),
                kind: match review_kind.get().as_str() {
                    "qc" => AudioReviewKind::Qc,
                    "rights" => AudioReviewKind::Rights,
                    "editorial" => AudioReviewKind::Editorial,
                    _ => return None,
                },
                decision: match decision.get().as_str() {
                    "approved" => AudioReviewDecision::Approved,
                    "rejected" => AudioReviewDecision::Rejected,
                    "withdrawn" => AudioReviewDecision::Withdrawn,
                    _ => return None,
                },
                notes: review_notes.get(),
                evidence_refs: refs(evidence.get()),
            })
        })
    });
    let confirmed = Signal::derive(move || {
        consent.with(|c| {
            c.as_ref().is_some_and(|c| {
                current_review.with(|current| {
                    current
                        .as_ref()
                        .is_some_and(|current| c.authorizes(current))
                })
            })
        })
    });
    let abort = StoredValue::new_local(None::<AbortController>);
    let artwork_unresolved = RwSignal::new(false);
    let artwork_busy = RwSignal::new(false);
    let delegation_unresolved = RwSignal::new(false);
    let delegation_busy = RwSignal::new(false);
    let dispatch = Dispatch {
        progress: None,
        state,
        session_pending,
        abort,
        api: expect_context::<crate::api::StudioContext>().api,
        complete: Callback::new(
            move |(command, reply): (Command, Result<Reply, ClientError>)| {
                if matches!(&command, Command::Works { .. }) {
                    let _ = dashboard_query.try_update(|query| *query = query.finish(&reply));
                }
                match reply {
                    Ok(reply) => {
                        if let (Command::Units { cursor, .. }, Reply::Units(page)) =
                            (&command, &reply)
                        {
                            let _ = units_complete
                                .try_set(cursor.is_none() && page.next_cursor.is_none());
                        }
                        if matches!(
                            &command,
                            Command::Reorder { .. }
                                | Command::ArchiveUnit { .. }
                                | Command::ArchiveWork { .. }
                        ) {
                            let _ = units_complete.try_set(false);
                        }
                        let selected = facts.with_untracked(Facts::unit_id);
                        let before = drafts.with_untracked(|drafts| drafts.content(&selected));
                        let confirmed = if matches!(&command, Command::ContentHead { .. }) {
                            replace_head.try_update_value(Option::take).flatten()
                        } else {
                            None
                        };
                        let _ = drafts.try_update(|drafts| {
                            drafts.acknowledge(&command, &reply);
                            if let (Command::ContentHead { unit }, Reply::Content(content)) =
                                (&command, &reply)
                            {
                                let replace = confirmed
                                    .as_ref()
                                    .filter(|(target, _)| target == unit)
                                    .map(|(_, snapshot)| snapshot);
                                drafts.head(content, replace);
                            }
                        });
                        let after = drafts.with_untracked(|drafts| drafts.content(&selected));
                        if before != after {
                            content_fields.restore(&after);
                        }
                        if let Reply::Brief(brief) = &reply {
                            let _ = brief_version.try_set(brief.brief.version.to_string());
                        }
                        if let Reply::Submission(submission) = &reply {
                            let _ = submission_id.try_set(submission.submission.id.clone());
                            if let Command::Prepare { request, .. } = &command {
                                let _ = credential.try_set(
                                    UploadCredential::new(request.upload_token.clone())
                                        .ok()
                                        .map(|token| {
                                            super::brief_controls::LocalUploadPermission::new(
                                                request.id.clone(),
                                                token,
                                            )
                                        }),
                                );
                            }
                        }
                        if let Reply::Candidate(candidate) = &reply {
                            let _ = candidate_id.try_set(candidate.candidate.id.clone());
                        }
                        if matches!(command, Command::Review { .. } | Command::Revoke { .. }) {
                            let _ = consent.try_set(None);
                        }
                        let created = match &command {
                            Command::CreateUnit { request, .. } => Some(request.id.clone()),
                            _ => None,
                        };
                        let opened_created_unit = facts
                            .try_update(|f| {
                                *f = f.apply_command(&command, reply);
                                created.as_ref().is_some_and(|id| f.select_created_unit(id))
                            })
                            .unwrap_or(false);
                        if let Some(unit) = opened_created_unit
                            .then(|| facts.with_untracked(|f| f.unit.clone()))
                            .flatten()
                        {
                            brief_version.set("0".into());
                            submission_id.set(String::new());
                            candidate_id.set(String::new());
                            credential.set(None);
                            consent.set(None);
                            file.set(None);
                            file_error.set(false);
                            let (metadata, content) = drafts
                                .try_update(|drafts| drafts.open_unit(&unit))
                                .unwrap_or_default();
                            unit_title.set(metadata.title);
                            description.set(metadata.description);
                            content_fields.restore(&content);
                        }
                    }
                    Err(ClientError::Http { status: 404, error })
                        if error.code == ErrorCode::NotFound
                            && matches!(command, Command::ContentHead { .. }) =>
                    {
                        let Command::ContentHead { unit } = &command else {
                            return;
                        };
                        let confirmed = replace_head.try_update_value(Option::take).flatten();
                        let replace = confirmed
                            .as_ref()
                            .filter(|(target, _)| target == unit)
                            .map(|(_, snapshot)| snapshot);
                        let before = drafts.with_untracked(|drafts| drafts.content(unit));
                        let _ = drafts.try_update(|drafts| drafts.empty_head(unit, replace));
                        let after = drafts.with_untracked(|drafts| drafts.content(unit));
                        if before != after {
                            content_fields.restore(&after);
                        }
                        let _ = facts.try_update(|f| {
                            f.content_base = Some(0);
                            f.content = None;
                            f.content_head = None;
                        });
                    }
                    Err(_) => {}
                }
            },
        ),
    };
    let cancel_request = Callback::new(move |()| {
        let reading_works = state.with_untracked(|s| {
            s.phase() == Phase::Pending && matches!(s.pending(), Some(Command::Works { .. }))
        });
        dispatch.cancel();
        if reading_works {
            dashboard_query.update(|query| *query = query.finish(&Err(ClientError::Cancelled)));
        }
    });
    Effect::new(move |_| {
        let actor = actor.get();
        let actor_changed = state.with_untracked(|s| s.actor() != actor);
        let session_target = format!("workspace:{}", session_epoch.get());
        if state.with_untracked(|s| s.actor() != actor || s.target() != session_target) {
            dispatch.abort();
            state.update(|s| {
                if let Some(next) = s.context(&actor, &session_target) {
                    *s = next;
                }
            });
            if actor_changed {
                // Reset child activity while the Work controls are still mounted, before the
                // selected-Work Show disposes them and their local reactive signals.
                artwork_unresolved.set(false);
                artwork_busy.set(false);
                delegation_unresolved.set(false);
                delegation_busy.set(false);
                facts.set(Facts::default());
                drafts.set(super::drafts::AuthoringDrafts::default());
                ancillary_dirty.set(false);
                publication_dirty.set(false);
                catalog_dirty.set(false);
                units_complete.set(false);
                replace_head.set_value(None);
                dashboard_query.set(super::dashboard::DashboardQuery::default());
                file_busy.set(false);
                file_error.set(false);
                for input in [
                    title,
                    synopsis,
                    new_unit_title,
                    new_unit_description,
                    content_rights,
                    rights,
                    unit_title,
                    description,
                    manuscript,
                    provenance,
                    notes,
                    profile,
                    use_note,
                    attribution,
                    naming,
                    qa,
                    producer,
                    producer_provenance,
                    method,
                    quality,
                    submission_id,
                    candidate_id,
                    review_notes,
                    evidence,
                ] {
                    input.set(String::new());
                }
                brief_version.set("0".into());
                kind.set(DEFAULT_WORK_KIND.into());
                body.set("text".into());
                review_kind.set("qc".into());
                decision.set("rejected".into());
                consent.set(None);
                dirty.set(false);
                credential.set(None);
                file.set(None);
                file_generation.update(|g| {
                    if let Some(next) = g.checked_add(1) {
                        *g = next;
                    }
                });
            }
        }
        if !active.get() && state.with_untracked(|s| s.phase() == Phase::Pending) {
            cancel_request.run(());
        }
    });
    Effect::new(move |_| {
        busy.set(
            file_busy.get()
                || artwork_busy.get()
                || delegation_busy.get()
                || state.with(|s| s.phase() == Phase::Pending),
        );
        activity.set(
            file_busy.get()
                || artwork_unresolved.get()
                || delegation_unresolved.get()
                || state.with(|s| s.pending().is_some()),
        )
    });
    Effect::new(move |_| {
        dirty.set(
            ancillary_dirty.get()
                || publication_dirty.get()
                || catalog_dirty.get()
                || drafts.with(super::drafts::AuthoringDrafts::has_unsaved),
        )
    });
    on_cleanup(move || {
        dispatch.abort();
        let _ = activity.try_set(false);
        let _ = busy.try_set(false);
    });
    let blocked = Signal::derive(move || {
        session_pending.get()
            || actor.get().is_empty()
            || file_busy.get()
            || artwork_unresolved.get()
            || delegation_unresolved.get()
            || state.with(|s| s.phase() == Phase::Pending || s.pending().is_some())
    });
    let read_works = Callback::new(move |cursor: Option<String>| {
        if blocked.get_untracked() {
            return;
        }
        dashboard_query.update(|query| {
            *query = query.start(&actor.get_untracked(), session_epoch.get_untracked());
        });
        dispatch.send(Command::Works { cursor });
    });
    Effect::new(move |_| {
        let owner = actor.get();
        let epoch = session_epoch.get();
        let target = format!("workspace:{epoch}");
        if dashboard.get()
            && active.get()
            && !session_pending.get()
            && !owner.is_empty()
            && state.with(|s| s.actor() == owner && s.target() == target && s.pending().is_none())
            && !file_busy.get()
            && !delegation_unresolved.get()
            && !dashboard_query.with(|query| query.attempted(&owner, epoch))
        {
            read_works.run(None);
        }
    });
    let pick_work = Callback::new(move |id: String| {
        if blocked.get_untracked() {
            return;
        }
        if facts.with_untracked(|f| f.work.as_ref().is_some_and(|work| work.id == id)) {
            return;
        }
        if (ancillary_dirty.get_untracked()
            || catalog_dirty.get_untracked()
            || publication_dirty.get_untracked())
            && !crate::view::confirm_discard(english.get_untracked())
        {
            return;
        }
        units_complete.set(false);
        replace_head.set_value(None);
        let creation = drafts.with_untracked(|drafts| drafts.new_unit(&id));
        new_unit_title.set(creation.title);
        new_unit_description.set(creation.description);
        cancel_request.run(());
        file.set(None);
        file_error.set(false);
        file_generation.update(|g| {
            if let Some(next) = g.checked_add(1) {
                *g = next;
            }
        });
        if let Some(input) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("live-audio-file"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
        {
            input.set_value("");
        }
        facts.update(|f| {
            let work = f.works.iter().find(|w| w.id == id).cloned();
            *f = Facts {
                work,
                works: f.works.clone(),
                works_next: f.works_next.clone(),
                ..Facts::default()
            };
        });
        credential.set(None);
        consent.set(None);
        dispatch.send(Command::Units {
            work: id,
            cursor: None,
        });
    });
    let pick_unit = Callback::new(move |id: String| {
        if blocked.get_untracked() {
            return;
        }
        if facts.with_untracked(|f| f.unit.as_ref().is_some_and(|unit| unit.id == id)) {
            return;
        }
        if (ancillary_dirty.get_untracked() || publication_dirty.get_untracked())
            && !crate::view::confirm_discard(english.get_untracked())
        {
            return;
        }
        replace_head.set_value(None);
        cancel_request.run(());
        file.set(None);
        file_error.set(false);
        file_generation.update(|g| {
            if let Some(next) = g.checked_add(1) {
                *g = next;
            }
        });
        if let Some(input) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("live-audio-file"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
        {
            input.set_value("");
        }
        facts.update(|f| {
            f.unit = f.units.iter().find(|u| u.id == id).cloned();
            if let Some(u) = &f.unit {
                let (metadata, content) = drafts
                    .try_update(|drafts| drafts.open_unit(u))
                    .unwrap_or_default();
                unit_title.set(metadata.title);
                description.set(metadata.description);
                content_fields.restore(&content);
            }
            f.content = None;
            f.content_head = None;
            f.content_base = None;
            f.history = None;
            f.brief = None;
            f.submission = None;
            f.candidate = None;
            f.clear_publication();
        });
        credential.set(None);
        consent.set(None);
        brief_version.set("0".into());
        submission_id.set(String::new());
        candidate_id.set(String::new());
        dispatch.send(Command::ContentHead { unit: id });
    });
    let dashboard_pick = Callback::new(move |id: String| {
        if session_pending.get_untracked() {
            return;
        }
        // Continue means exposing the same mounted selection; never reset its unit or refetch.
        if facts.with_untracked(|f| f.work.as_ref().is_some_and(|work| work.id == id)) {
            on_open_content.run(());
            return;
        }
        if blocked.get_untracked() {
            return;
        }
        pick_work.run(id.clone());
        if facts.with_untracked(|f| f.work.as_ref().is_some_and(|work| work.id == id)) {
            on_open_content.run(());
        }
    });
    let open_new_work = Callback::new(move |()| {
        if blocked.get_untracked() {
            return;
        }
        on_open_content.run(());
        focus_workspace_field("live-work-title", Some("live-create-work-form"));
    });
    let open_audio = Callback::new(move |()| {
        if session_pending.get_untracked() {
            return;
        }
        on_open_content.run(());
        focus_workspace_field("live-audio-section", None);
    });
    let track_input = Callback::new(move |id: String| {
        match id.as_str() {
            "live-work-title" | "live-work-synopsis" | "live-work-kind" | "live-rights" => {
                drafts.update(|drafts| {
                    drafts.edit_work(super::drafts::WorkForm {
                        title: title.get_untracked(),
                        synopsis: synopsis.get_untracked(),
                        kind: kind.get_untracked(),
                        rights: refs(rights.get_untracked()),
                    })
                });
            }
            "live-unit-title" | "live-unit-description" => {
                let work = facts.with_untracked(|f| {
                    f.work
                        .as_ref()
                        .map(|work| work.id.clone())
                        .unwrap_or_default()
                });
                drafts.update(|drafts| {
                    drafts.edit_new_unit(
                        &work,
                        super::drafts::UnitForm {
                            title: new_unit_title.get_untracked(),
                            description: new_unit_description.get_untracked(),
                        },
                    )
                });
            }
            "live-selected-unit-title" | "live-selected-unit-description" => {
                let unit = facts.with_untracked(Facts::unit_id);
                drafts.update(|drafts| {
                    drafts.edit_metadata(
                        &unit,
                        super::drafts::UnitForm {
                            title: unit_title.get_untracked(),
                            description: description.get_untracked(),
                        },
                    )
                });
            }
            "live-content-kind" | "live-manuscript" | "live-provenance" | "live-content-rights" => {
                let unit = facts.with_untracked(Facts::unit_id);
                drafts.update(|drafts| {
                    let mut current = content_fields.snapshot(drafts.content(&unit));
                    if id == "live-content-kind" && current.kind == "editor-script" {
                        current.script = editor_script.get_untracked();
                    }
                    drafts.edit_content(&unit, current);
                });
            }
            // These forms have their own draft ownership and contribute their own dirty flag.
            "live-edit-work-title" | "live-edit-work-synopsis" => {}
            id if id.starts_with("live-publication-") => {}
            _ => ancillary_dirty.set(true),
        }
    });
    let read_head = Callback::new(move |()| {
        if blocked.get_untracked() {
            return;
        }
        let unit = facts.with_untracked(Facts::unit_id);
        let current = drafts.with_untracked(|drafts| drafts.content(&unit));
        if drafts.with_untracked(|drafts| drafts.content_dirty(&unit))
            && !crate::view::confirm_discard(english.get_untracked())
        {
            return;
        }
        replace_head.set_value(Some((unit.clone(), current)));
        dispatch.send(Command::ContentHead { unit });
    });
    let open_selected_editor =
        Callback::new(move |()| on_editor.run(facts.with_untracked(Facts::editor_script)));
    view! {<section id="workspace-content" aria-labelledby=move || if dashboard.get() {"dashboard-title"} else {"live-title"}
        on:input=move |event| { if let Some(id) = event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).map(|element| element.id()) { track_input.run(id); } }
        on:change=move |event| { if let Some(id) = event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).map(|element| element.id()) { track_input.run(id); } }>
        <div hidden=move || !dashboard.get() inert=move || !dashboard.get()>
            <super::dashboard::Dashboard facts=facts query=dashboard_query english=english blocked=blocked session_pending=session_pending
                on_new_work=open_new_work on_import=on_import on_editor=open_selected_editor on_continue=on_open_content on_audio=open_audio
                on_refresh=Callback::new(move |()| read_works.run(None))
                on_next=Callback::new(move |()| { if let Some(cursor) = facts.with_untracked(|f| f.works_next.clone()) { read_works.run(Some(cursor)); } })
                on_pick=dashboard_pick/>
        </div>
        <div hidden=move || dashboard.get() inert=move || dashboard.get()>
        <h2 id="live-title" tabindex="-1">{move ||text(Key::Pane,english.get())}</h2><p>{move ||text(Key::Help,english.get())}</p>
        <p role="status" hidden=move ||!drafts.with(super::drafts::AuthoringDrafts::has_unsaved)>{move ||text(Key::UnsavedDrafts,english.get())}</p>
        <p role="status">{move ||if file_error.get(){text(Key::FileReadFailed,english.get())}else{""}}</p>
        <div class="live-status" role="status" aria-live="polite"><p>{move ||state.with(|s|text(match s.phase() {Phase::Idle=>Key::Idle,Phase::Pending=>Key::Pending,Phase::Ready=>Key::Ready,Phase::Failed=>Key::Failed,Phase::Ambiguous=>Key::Ambiguous},english.get()))}</p>
            {move ||state.with(|s|s.error().map(|error|text(copy::error_key(error),english.get())).unwrap_or_default())}
            <p>{move ||if state.with(RequestState::is_empty){text(Key::Empty,english.get())}else{""}}</p>
            <ul>{move ||state.with(|s|match s.error() {Some(ClientError::Http {error,..})=>error.issues.clone(),_=>vec![]}).into_iter().map(|issue|view! {<li><code>{issue.path}</code>" · "{issue.rule}</li>}).collect_view()}</ul>
            <div class="toolbar"><ActionButton id="live-cancel" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||state.with(|s|s.phase()!=Phase::Pending)) on_press=Callback::new(move |_|cancel_request.run(()))>{move ||text(Key::Cancel,english.get())}</ActionButton>
            <ActionButton id="live-reconcile" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||session_pending.get()||state.with(|s|!s.can_reconcile())) on_press=Callback::new(move |_|dispatch.reconcile())>{move ||text(Key::Reconcile,english.get())}</ActionButton>
            <ActionButton id="live-retry" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||session_pending.get()||state.with(|s|!s.can_retry())) on_press=Callback::new(move |_|dispatch.retry())>{move ||text(Key::Retry,english.get())}</ActionButton></div>
        </div>
        <div class="toolbar"><ActionButton id="live-works" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|read_works.run(None))>{move ||text(Key::Refresh,english.get())}</ActionButton>
        <ActionButton id="live-works-next" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.works_next.is_none())) on_press=Callback::new(move |_| { if let Some(cursor) = facts.with_untracked(|f| f.works_next.clone()) { read_works.run(Some(cursor)); } })>{move ||text(Key::Next,english.get())}</ActionButton></div>
        <ul class="live-list">{move ||facts.with(|f|f.works.clone()).into_iter().map(|w| {let id=w.id.clone();view! {<li><button class="cui-button" type="button" disabled=move ||blocked.get() on:click=move |_|pick_work.run(id.clone())><span lang=w.details.language.clone()>{w.details.title}</span><small>{move ||format!("{} · r{}",copy::work_kind(&w.kind,english.get()),w.revision)}</small></button></li>}}).collect_view()}</ul>
        <details id="live-create-work-form"><summary>{move ||text(Key::CreateWork,english.get())}</summary><div class="live-grid">
            <Field id="live-work-title" label=Key::Title value=title english=english/><Field id="live-work-synopsis" label=Key::Synopsis value=synopsis english=english multiline=true/>
            <SelectField id="live-work-kind" label=Key::Kind value=kind english=english><crate::ui::WorkKindOptions value=kind english=english/></SelectField>
            <Field id="live-rights" label=Key::Rights value=rights english=english/>
        </div><ActionButton id="live-create-work" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
            if blocked.get_untracked() { return; }
            if (ancillary_dirty.get_untracked() || catalog_dirty.get_untracked() || publication_dirty.get_untracked()) && !crate::view::confirm_discard(english.get_untracked()) { return; }
            dispatch.build((||Ok(Command::CreateWork(CreateWorkRequest {operation_id:uuid()?,id:uuid()?,kind:kind.get_untracked(),details:WorkDetailsDto {title:title.get_untracked(),synopsis:synopsis.get_untracked(),language:"vi-VN".into(),visibility:"private".into(),rights_refs:refs(rights.get_untracked())}})))());
        })>{move ||text(Key::CreateWork,english.get())}</ActionButton></details>
        <Show when=move ||facts.with(|f|f.work.is_some())>
            <h3 lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move ||facts.with(|f|f.work.as_ref().map(|w|format!("{} · r{}",w.details.title,w.revision)).unwrap_or_default())}</h3>
            <ul class="live-list">{move ||facts.with(|f|f.units.clone()).into_iter().map(|u| {let id=u.id.clone();view! {<li><button type="button" class="cui-button" disabled=move ||blocked.get() on:click=move |_|pick_unit.run(id.clone())><span lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{u.details.title}</span><small>{move ||format!("{} · r{}",copy::unit_kind(&u.kind,english.get()),u.revision)}</small></button></li>}}).collect_view()}</ul>
            <ActionButton id="live-units-next" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.units_next.is_none())) on_press=Callback::new(move |_| {if let Some(work)=facts.with_untracked(|f|f.work.clone()) {dispatch.send(Command::Units {work:work.id,cursor:facts.with_untracked(|f|f.units_next.clone())});}})>{move ||text(Key::Next,english.get())}</ActionButton>
            <details><summary>{move ||text(Key::CreateUnit,english.get())}</summary><Field id="live-unit-title" label=Key::Title value=new_unit_title english=english/><Field id="live-unit-description" label=Key::Synopsis value=new_unit_description english=english multiline=true/>
                <ActionButton id="live-create-unit" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                    if blocked.get_untracked() { return; }
                    if (ancillary_dirty.get_untracked() || catalog_dirty.get_untracked() || publication_dirty.get_untracked()) && !crate::view::confirm_discard(english.get_untracked()) { return; }
                    dispatch.build((|| {let w=facts.with_untracked(|f|f.work.clone()).ok_or(ClientError::InvalidInput)?;Ok(Command::CreateUnit {work:w.id,request:CreateContentUnitRequest {operation_id:uuid()?,id:uuid()?,kind:if w.kind=="podcast" {"episode"}else {"chapter"}.into(),expected_work_revision:w.revision,details:UnitDetailsDto {title:new_unit_title.get_untracked(),description:new_unit_description.get_untracked(),source:None,script:None,content_digest:None}}})})());
                })>{move ||text(Key::CreateUnit,english.get())}</ActionButton>
            </details>
            <super::catalog_controls::WorkControls facts=facts dispatch=dispatch english=english blocked=blocked unsaved=Signal::derive(move ||dirty.get()) dirty=catalog_dirty/>
        </Show>
        <super::artwork::panel::ArtworkPanel actor=actor work=Signal::derive(move ||facts.with(|f|f.work.clone())) english=english
            session_pending=session_pending session_epoch=session_epoch workspace_busy=Signal::derive(move ||file_busy.get()||delegation_unresolved.get()||state.with(|s|s.pending().is_some()))
            unresolved=artwork_unresolved busy=artwork_busy dirty=ancillary_dirty on_revision=Callback::new(move |revision:u64|facts.update(|f|{
                if let Some(work)=&mut f.work {work.revision=work.revision.max(revision);
                    if let Some(cached)=f.works.iter_mut().find(|w|w.id==work.id){cached.revision=work.revision;}}
            }))/>
        <super::delegation::panel::DelegationPanel actor=actor work=Signal::derive(move || facts.with(|f| f.work.clone())) english=english
            session_pending=session_pending unresolved=delegation_unresolved busy=delegation_busy dirty=ancillary_dirty/>
        <Show when=move ||facts.with(|f|f.unit.is_some())>
            <h3 lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move ||facts.with(|f|f.unit.as_ref().map(|u|format!("{} · r{}",u.details.title,u.revision)).unwrap_or_default())}</h3><p><code>{move ||facts.with(Facts::unit_id)}</code></p>
            <h4>{move ||text(Key::SelectedUnitMetadata,english.get())}</h4>
            <Field id="live-selected-unit-title" label=Key::Title value=unit_title english=english/><Field id="live-selected-unit-description" label=Key::Synopsis value=description english=english multiline=true/>
            <super::catalog_controls::UnitControls facts=facts dispatch=dispatch english=english blocked=blocked unsaved=Signal::derive(move ||dirty.get()) units_complete=Signal::derive(move ||units_complete.get())/>
            <ActionButton id="live-open-editor" english=english reason=BlockReason::ContextReady blocked=session_pending on_press=Callback::new(move |_|open_selected_editor.run(()))>{move ||text(Key::OpenEditor,english.get())}</ActionButton>
            <ActionButton id="live-reload-unit-metadata" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                if blocked.get_untracked() { return; }
                if let Some(unit) = facts.with_untracked(|facts| facts.unit.clone()) {
                    if drafts.with_untracked(|drafts|drafts.metadata_dirty(&unit.id)) && !crate::view::confirm_discard(english.get_untracked()) { return; }
                    let metadata = drafts.try_update(|drafts|drafts.reload_metadata(&unit)).unwrap_or_default();
                    unit_title.set(metadata.title); description.set(metadata.description);
                }
            })>{move ||text(Key::ReloadMetadata,english.get())}</ActionButton>
            <ActionButton id="live-patch-unit" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.build((|| {let (w,u)=context(facts)?;Ok(Command::PatchUnit {unit:u.id.clone(),request:PatchContentUnitRequest {operation_id:uuid()?,expected_revision:drafts.with_untracked(|drafts|drafts.metadata_base(&u.id)).ok_or(ClientError::InvalidInput)?,expected_work_revision:w.revision,details:UnitPatchDto {title:PatchField::Value(unit_title.get_untracked()),description:PatchField::Value(description.get_untracked()),..UnitPatchDto::default()}}})})()))>{move ||text(Key::SaveMetadata,english.get())}</ActionButton>
            <section class="live-section"><h3>{move ||text(Key::Content,english.get())}</h3><p>{move ||text(Key::NoContent,english.get())}</p>
                <div class="toolbar"><ActionButton id="live-content-head" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|read_head.run(()))>{move ||text(Key::ReadHead,english.get())}</ActionButton><ActionButton id="live-content-history" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.send(Command::ContentHistory {unit:facts.with_untracked(Facts::unit_id),after:0}))>{move ||text(Key::History,english.get())}</ActionButton></div>
                <fieldset class="live-content-fields" disabled=move ||facts.with(|facts|drafts.with(|drafts|drafts.content_base(&facts.unit_id()).is_none()))><legend>{move ||text(Key::Content,english.get())}</legend>
                <SelectField id="live-content-kind" label=Key::Body value=body english=english><option value="text">{move ||text(Key::Text,english.get())}</option><option value="recording">{move ||text(Key::Recording,english.get())}</option><option value="source" disabled>{move ||text(Key::Source,english.get())}</option><option value="script" disabled>{move ||text(Key::Script,english.get())}</option><option value="editor-script" disabled=move ||editor_script.with(|script|script.is_none())>{move ||text(Key::EditorScript,english.get())}</option></SelectField>
                <p class="help">{move ||text(Key::ScriptBindingHelp,english.get())}</p>
                <p><code>{move ||facts.with(|f|drafts.with(|drafts|drafts.content(&f.unit_id()).script.map(|script|copy::script_binding(&script)).unwrap_or_default()))}</code></p>
                <Field id="live-manuscript" label=Key::Text value=manuscript english=english multiline=true/><Field id="live-provenance" label=Key::Provenance value=provenance english=english multiline=true/><Field id="live-content-rights" label=Key::Rights value=content_rights english=english/>
                <ActionButton id="live-append-content" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|drafts.with(|drafts|drafts.content_base(&f.unit_id()).is_none()||drafts.content(&f.unit_id()).details().is_none()))) on_press=Callback::new(move |_|dispatch.build((|| {let(w,u)=context(facts)?;let expected_revision=drafts.with_untracked(|drafts|drafts.content_base(&u.id)).ok_or(ClientError::InvalidInput)?;let details=drafts.with_untracked(|drafts|drafts.content(&u.id).details()).ok_or(ClientError::InvalidInput)?;let request=AppendContentRequest {operation_id:uuid()?,id:uuid()?,expected_revision,expected_work_revision:w.revision,details};drafts.update(|drafts|drafts.submitted_content(&u.id,&request));Ok(Command::AppendContent {unit:u.id,request})})()))>{move ||text(Key::SaveContent,english.get())}</ActionButton>
                </fieldset>
                <pre>{move ||facts.with(|f|f.content.as_ref().and_then(|c|serde_json::to_string_pretty(c).ok()).unwrap_or_default())}</pre>
                <ul>{move ||facts.with(|f|f.history.clone()).map(|h|h.revisions.into_iter().map(|r| {let id=r.id.clone();view! {<li><button class="cui-button" type="button" disabled=move ||blocked.get() on:click=move |_|dispatch.send(Command::ContentRevision {unit:facts.with_untracked(Facts::unit_id),revision:id.clone()})>{format!("r{} · {}",r.revision,r.id)}</button></li>}}).collect_view())}</ul>
            </section>
            <section class="live-section"><h3>{move ||text(Key::Brief,english.get())}</h3><div class="live-grid">
                <Field id="live-brief-version" label=Key::Version value=brief_version english=english/><Field id="live-direction" label=Key::Notes value=notes english=english multiline=true/>
                <SelectField id="live-profile" label=Key::Profile value=profile english=english><option value="">{move ||text(Key::SelectProfile,english.get())}</option>{[20,24,32,40].map(|rate|view! {<option value=format!("opus-mono-{rate}-v1")>{format!("Ogg Opus mono · {rate} kbps")}</option>}).collect_view()}</SelectField>
                <Field id="live-use" label=Key::Use value=use_note english=english/><Field id="live-attribution" label=Key::Attribution value=attribution english=english/><Field id="live-naming" label=Key::Naming value=naming english=english/><Field id="live-qa" label=Key::Qa value=qa english=english multiline=true/>
            </div><div class="toolbar"><ActionButton id="live-create-brief" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|super::brief_controls::brief_content(f,drafts.with(|d|d.content_dirty(&f.unit_id()))).is_none())||!valid_profile(&profile.get())) on_press=Callback::new(move |_|dispatch.build((|| {let(_,u)=context(facts)?;let c=facts.with_untracked(|f|super::brief_controls::brief_content(f,drafts.with_untracked(|d|d.content_dirty(&f.unit_id()))).cloned()).ok_or(ClientError::InvalidInput)?;let(mode,capability)=super::brief_controls::production_mode(&c.details.body);Ok(Command::CreateBrief {unit:u.id,request:CreateProductionBriefRequest {operation_id:uuid()?,id:uuid()?,expected_version:brief_version.get_untracked().parse().map_err(|_|ClientError::InvalidInput)?,content_revision_id:c.id,content_revision_digest:c.revision_digest,guidance:BriefGuidance {editorial:EditorialIntent {mode,notes:notes.get_untracked()},voice:None,sound_design:None,required_capabilities:vec![capability,ProducerCapability::OggOpusMono],delivery:DeliveryConstraints {profile_id:profile.get_untracked(),permitted_use:use_note.get_untracked(),attribution:attribution.get_untracked(),naming:naming.get_untracked(),qa_instructions:qa.get_untracked()}}}})})()))>{move ||text(Key::CreateBrief,english.get())}</ActionButton>
                <ActionButton id="live-read-brief" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.build(brief_version.get_untracked().parse().map(|version|Command::Brief {unit:facts.with_untracked(Facts::unit_id),version}).map_err(|_|ClientError::InvalidInput)))>{move ||text(Key::ReadBrief,english.get())}</ActionButton></div>
                <p class="help">{move ||text(Key::BriefHeadHelp,english.get())}</p>
                <p><code>{move ||facts.with(|f|f.content_head.as_ref().map(|c|copy::content_binding(c)).unwrap_or_default())}</code></p>
                <super::brief_controls::BriefDocument brief=Signal::derive(move ||facts.with(|f|f.brief.clone())) english=english blocked=blocked/>
            </section>
            <section id="live-audio-section" class="live-section" tabindex="-1" aria-labelledby="live-audio-title"><h3 id="live-audio-title">{move ||text(Key::Audio,english.get())}</h3><label for="live-audio-file">{move ||text(Key::File,english.get())}</label>
                <input id="live-audio-file" type="file" accept=".ogg,audio/ogg" disabled=move ||blocked.get() on:change=move |event| {
                    file.set(None);file_error.set(false);let Some(generation)=file_generation.get_untracked().checked_add(1) else {dispatch.input_error();return;};file_generation.set(generation);
                    let actor_snapshot=actor.get_untracked();let unit_snapshot=facts.with_untracked(Facts::unit_id);
                    let selected=event.target().and_then(|t|t.dyn_into::<web_sys::HtmlInputElement>().ok()).and_then(|i|i.files()).and_then(|f|f.get(0));
                    if let Some(selected)=selected {file_busy.set(true);spawn_local(async move {let result=browser::read_audio(selected).await;if file_generation.try_get_untracked()==Some(generation)&&actor.try_get_untracked()==Some(actor_snapshot)&&facts.try_with_untracked(Facts::unit_id)==Some(unit_snapshot) {let _=file_busy.try_set(false);match result {Ok(value)=>{let _=file.try_set(Some(value));},Err(_)=>{let _=file_error.try_set(true);}}}});}
                }/>
                <p>{move ||if file.with(|f|f.is_some()) {text(Key::FileReady,english.get())}else {""}}</p><div class="live-grid"><Field id="live-producer" label=Key::Producer value=producer english=english/><Field id="live-method" label=Key::Method value=method english=english/><Field id="live-producer-provenance" label=Key::ProducerProvenance value=producer_provenance english=english multiline=true/><Field id="live-quality" label=Key::Quality value=quality english=english multiline=true/></div>
                <ActionButton id="live-prepare" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||file.with(|f|f.is_none())||facts.with(|f|f.brief.as_ref().is_none_or(|b|b.freshness!=BriefFreshness::Current))) on_press=Callback::new(move |_|dispatch.build((|| {let(w,u)=context(facts)?;let b=facts.with_untracked(|f|f.brief.clone()).ok_or(ClientError::InvalidInput)?;let binding=super::brief_controls::submission_binding(&b,&w.id,&u.id)?;let(bytes,sha)=file.get_untracked().ok_or(ClientError::InvalidInput)?;let expires_at=(js_sys::Date::now() as i64).checked_add(3_600_000).ok_or(ClientError::InvalidInput)?;Ok(Command::Prepare {unit:u.id.clone(),request:CreateAudioSubmissionRequest {operation_id:uuid()?,id:uuid()?,binding,expected_sha256:sha,expected_byte_length:bytes.len() as u64,declarations:ProducerDeclarations {producer:producer.get_untracked(),method:method.get_untracked(),provenance:producer_provenance.get_untracked(),rights_refs:b.brief.work.rights_refs.clone(),quality_report:quality.get_untracked()},upload_token:browser::upload_token()?,expires_at}})})()))>{move ||text(Key::Prepare,english.get())}</ActionButton>
                <Field id="live-submission-id" label=Key::Submission value=submission_id english=english/>
                <div class="toolbar"><ActionButton id="live-read-submission" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.send(Command::Submission {unit:facts.with_untracked(Facts::unit_id),submission:submission_id.get_untracked()}))>{move ||text(Key::ReadSubmission,english.get())}</ActionButton>
                <ActionButton id="live-upload" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.submission.as_ref().is_none_or(|s|credential.with(|c|c.as_ref().and_then(|c|c.for_submission(&s.submission.id)).is_none())))||file.with(|f|f.is_none())||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.state!=AudioSubmissionState::AwaitingBytes))) on_press=Callback::new(move |_|dispatch.build((|| {let s=facts.with_untracked(|f|f.submission.clone()).ok_or(ClientError::InvalidInput)?;let(bytes,sha)=file.get_untracked().ok_or(ClientError::InvalidInput)?;if sha!=s.submission.expected_sha256 || bytes.len() as u64!=s.submission.expected_byte_length {return Err(ClientError::InvalidInput)};Ok(Command::Upload {binding:s.submission.binding,sha256:sha,submission:s.submission.id.clone(),credential:credential.with_untracked(|c|c.as_ref().and_then(|c|c.for_submission(&s.submission.id)).cloned()).ok_or(ClientError::InvalidInput)?,bytes})})()))>{move ||text(Key::Upload,english.get())}</ActionButton>
                <ActionButton id="live-revoke" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {if crate::view::confirm_discard(english.get_untracked()) {dispatch.send(Command::Revoke {unit:facts.with_untracked(Facts::unit_id),submission:submission_id.get_untracked()});}})>{move ||text(Key::Revoke,english.get())}</ActionButton></div>
                <p role="status">{move ||facts.with(|f|f.submission.as_ref().map(|s|copy::submission_state(s.state,english.get())).unwrap_or_default())}</p>
                <p class="help" hidden=move ||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.state!=AudioSubmissionState::AwaitingBytes||credential.with(|c|c.as_ref().and_then(|c|c.for_submission(&s.submission.id)).is_some())))>{move ||text(Key::CredentialMissing,english.get())}</p>
                <p><code>{move ||facts.with(|f|f.submission.as_ref().map(|s|copy::submission_binding(&s.submission)).unwrap_or_default())}</code></p>
                <p><code>{move ||facts.with(|f|f.submission.as_ref().and_then(|s|s.rendition.as_ref()).map(|r|copy::rendition_summary(r)).unwrap_or_default())}</code></p>
                <details><summary>{move ||text(Key::InspectDocument,english.get())}</summary><pre>{move ||facts.with(|f|f.submission.as_ref().and_then(|s|serde_json::to_string_pretty(s).ok()).unwrap_or_default())}</pre></details>
            </section>
            <super::audio_preview::AudioPreview facts=facts dispatch=dispatch actor=actor active=active session_pending=session_pending english=english blocked=blocked/>
            <section class="live-section"><h3>{move ||text(Key::Review,english.get())}</h3><div class="live-grid">
                <SelectField id="live-review-kind" label=Key::ReviewKind value=review_kind english=english><option value="qc">{move ||copy::review_kind(AudioReviewKind::Qc,english.get())}</option><option value="rights">{move ||copy::review_kind(AudioReviewKind::Rights,english.get())}</option><option value="editorial">{move ||copy::review_kind(AudioReviewKind::Editorial,english.get())}</option></SelectField>
                <SelectField id="live-decision" label=Key::Decision value=decision english=english><option value="rejected">{move ||copy::review_decision(AudioReviewDecision::Rejected,english.get())}</option><option value="approved">{move ||copy::review_decision(AudioReviewDecision::Approved,english.get())}</option><option value="withdrawn">{move ||copy::review_decision(AudioReviewDecision::Withdrawn,english.get())}</option></SelectField>
                <Field id="live-review-notes" label=Key::Notes value=review_notes english=english multiline=true/><Field id="live-evidence" label=Key::Evidence value=evidence english=english/>
            </div><label class="adaptation-confirm" for="live-review-confirm"><input id="live-review-confirm" type="checkbox" prop:checked=move ||confirmed.get() on:change=move |e|consent.set(if event_target_checked(&e){current_review.get_untracked()}else{None})/><span>{move ||text(Key::Confirm,english.get())}</span></label><p>{move ||text(Key::ConfirmHelp,english.get())}</p>
                <ActionButton id="live-review" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||!confirmed.get()||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.rendition.is_none()))) on_press=Callback::new(move |_|dispatch.build((|| {let current=current_review.get_untracked().ok_or(ClientError::InvalidInput)?;if !consent.with_untracked(|c|c.as_ref().is_some_and(|c|c.authorizes(&current))) {return Err(ClientError::InvalidInput)};Ok(Command::Review {unit:current.unit.clone(),submission:current.submission.clone(),request:current.request(uuid()?)})})()))>{move ||text(Key::Review,english.get())}</ActionButton>
                <ActionButton id="live-candidate" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.rendition.is_none()))) on_press=Callback::new(move |_|dispatch.build((|| {let(_,u)=context(facts)?;let s=facts.with_untracked(|f|f.submission.clone()).ok_or(ClientError::InvalidInput)?;let r=s.rendition.ok_or(ClientError::InvalidInput)?;Ok(Command::Candidate {unit:u.id,submission:s.submission.id,request:CreateReleaseCandidateRequest {operation_id:uuid()?,rendition_digest:r.digest}})})()))>{move ||text(Key::CreateCandidate,english.get())}</ActionButton>
                <Field id="live-candidate-id" label=Key::Candidate value=candidate_id english=english/><ActionButton id="live-read-candidate" english=english reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.send(Command::CandidateRead {unit:facts.with_untracked(Facts::unit_id),submission:submission_id.get_untracked(),candidate:candidate_id.get_untracked()}))>{move ||text(Key::ReadCandidate,english.get())}</ActionButton>
                <p>{move ||text(Key::Private,english.get())}</p>
                <ul>{move ||facts.with(|f|f.candidate.as_ref().map(|c|c.blockers.clone()).unwrap_or_default()).into_iter().map(|blocker|view!{<li>{move ||copy::audio_blocker(blocker,english.get())}</li>}).collect_view()}</ul>
                <p><code>{move ||facts.with(|f|f.candidate.as_ref().map(|c|copy::candidate_binding(&c.candidate)).unwrap_or_default())}</code></p>
                <details><summary>{move ||text(Key::InspectDocument,english.get())}</summary><pre>{move ||facts.with(|f|f.candidate.as_ref().and_then(|c|serde_json::to_string_pretty(c).ok()).unwrap_or_default())}</pre></details>
            </section>
            <super::publication::PublicationPanel facts=facts dispatch=dispatch english=english blocked=blocked dirty=publication_dirty/>
        </Show>
        </div>
    </section>}
}

fn focus_workspace_field(id: &'static str, details: Option<&'static str>) {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        if let Some(details) = details.and_then(|id| document.get_element_by_id(id)) {
            let _ = details.set_attribute("open", "");
        }
        let target = document
            .get_element_by_id(id)
            .or_else(|| document.get_element_by_id("live-title"));
        if let Some(target) =
            target.and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = target.focus();
        }
    });
}
