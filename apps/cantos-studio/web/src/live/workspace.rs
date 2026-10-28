//! App-local forms compose established Studio controls; no shared component is introduced.
use super::{
    browser,
    copy::{self, text, Key},
    feedback::{self, Outcome, Target},
    layout::{self, EditorStatus, Pane, Task, Viewing},
    projection::{self, Refusal},
    *,
};
use crate::contract::*;
use crate::ui::{ActionButton, BlockReason, EDITOR_FIELD_DENSITY};
use cantos_ui::{
    ButtonVariant, FieldState, IconName, Select, SelectOption, TextArea, TextField,
    TextFieldVariant,
};
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
    /// Event-time admission of a *new* intent against the current selection. Retry and receipt
    /// reconciliation of an existing intent never pass through it.
    pub(super) admit: Option<Callback<Command, bool>>,
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
        self.start(command, None);
    }
    fn send_page(self, command: Command) {
        let target = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
            .and_then(|element| super::pages::focus_target(&command, &element.id()));
        self.start(command, target);
    }
    fn start(self, command: Command, page_focus: Option<&'static str>) {
        if self.admit.is_some_and(|admit| !admit.run(command.clone())) {
            return;
        }
        let Some((sending, ticket)) = self
            .state
            .get_untracked()
            .begin_in_session(command.clone(), self.session_pending.get_untracked())
        else {
            return;
        };
        self.run(sending, ticket, command, page_focus);
    }
    pub(super) fn reconcile(self) {
        if self.session_pending.get_untracked() {
            return;
        }
        if let Some((sending, ticket, command)) = self.state.get_untracked().begin_reconcile() {
            self.run(sending, ticket, command, None);
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
        self.run(sending, ticket, command, None);
    }
    fn run(
        self,
        sending: RequestState,
        ticket: Ticket,
        command: Command,
        page_focus: Option<&'static str>,
    ) {
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
                    let succeeded = acknowledged.is_ok();
                    self.complete.run((acknowledged_command, acknowledged));
                    if let Some(target) = page_focus {
                        self.focus_page_if_lost(
                            ticket,
                            super::pages::outcome_focus_target(target, succeeded),
                        );
                    }
                }
            }
        });
    }
    fn focus_page_if_lost(self, ticket: Ticket, target: &'static str) {
        leptos::leptos_dom::helpers::request_animation_frame(move || {
            if !self
                .state
                .try_with_untracked(|state| state.settled(ticket))
                .unwrap_or(false)
                || self.session_pending.try_get_untracked().unwrap_or(true)
            {
                return;
            }
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let lost = document
                .active_element()
                .is_none_or(|element| element.tag_name().eq_ignore_ascii_case("body"));
            if !lost {
                return;
            }
            if let Some(target) = document
                .get_element_by_id(target)
                .filter(|element| {
                    element
                        .closest("[hidden], [inert]")
                        .ok()
                        .flatten()
                        .is_none()
                })
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = target.focus();
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
/// The shell captures exact visible values; the pure owner decides dirty/acknowledgement state.
#[derive(Clone, Copy)]
struct AncillaryFields {
    notes: RwSignal<String>,
    profile: RwSignal<String>,
    permitted_use: RwSignal<String>,
    attribution: RwSignal<String>,
    naming: RwSignal<String>,
    qa: RwSignal<String>,
    producer: RwSignal<String>,
    provenance: RwSignal<String>,
    method: RwSignal<String>,
    quality: RwSignal<String>,
    review_kind: RwSignal<String>,
    decision: RwSignal<String>,
    review_notes: RwSignal<String>,
    evidence: RwSignal<String>,
    confirmed: Signal<bool>,
    consent: RwSignal<Option<ReviewConsent>>,
}
impl AncillaryFields {
    fn snapshot(self) -> super::ancillary::Form {
        super::ancillary::Form {
            brief: super::ancillary::BriefForm {
                notes: self.notes.get_untracked(),
                profile: self.profile.get_untracked(),
                permitted_use: self.permitted_use.get_untracked(),
                attribution: self.attribution.get_untracked(),
                naming: self.naming.get_untracked(),
                qa: self.qa.get_untracked(),
            },
            producer: super::ancillary::ProducerForm {
                producer: self.producer.get_untracked(),
                provenance: self.provenance.get_untracked(),
                method: self.method.get_untracked(),
                quality: self.quality.get_untracked(),
            },
            review: super::ancillary::ReviewForm {
                kind: self.review_kind.get_untracked(),
                decision: self.decision.get_untracked(),
                notes: self.review_notes.get_untracked(),
                evidence: self.evidence.get_untracked(),
                confirmed: self.confirmed.get_untracked(),
            },
        }
    }
    fn restore(self, form: &super::ancillary::Form) {
        for (field, value) in [
            (self.notes, &form.brief.notes),
            (self.profile, &form.brief.profile),
            (self.permitted_use, &form.brief.permitted_use),
            (self.attribution, &form.brief.attribution),
            (self.naming, &form.brief.naming),
            (self.qa, &form.brief.qa),
            (self.producer, &form.producer.producer),
            (self.provenance, &form.producer.provenance),
            (self.method, &form.producer.method),
            (self.quality, &form.producer.quality),
            (self.review_kind, &form.review.kind),
            (self.decision, &form.review.decision),
            (self.review_notes, &form.review.notes),
            (self.evidence, &form.review.evidence),
        ] {
            if field.get_untracked() != *value {
                field.set(value.clone());
            }
        }
        // A remembered checkbox cannot authorize a newly loaded rendition context.
        if self.consent.with_untracked(Option::is_some) {
            self.consent.set(None);
        }
    }
}
#[component]
pub fn ContentWorkspace(
    actor: Signal<String>,
    locale: Signal<crate::localization::Locale>,
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
    // View-only: which task area and (on narrow layouts) which pane is shown. Neither carries a
    // draft or an authority; every area stays mounted, so hiding one never discards its inputs.
    let task = RwSignal::new(Task::default());
    let pane = RwSignal::new(Pane::default());
    let dashboard_query = RwSignal::new(super::dashboard::DashboardQuery::default());
    let file = RwSignal::new(None::<(Arc<Vec<u8>>, String)>);
    let file_generation = RwSignal::new(0u64);
    let upload_file_generation = StoredValue::new(None::<u64>);
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
    let ancillary = RwSignal::new(super::ancillary::AncillaryDrafts::default());
    let artwork_dirty = RwSignal::new(false);
    let delegation_dirty = RwSignal::new(false);
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
    Effect::new(move |_| {
        let value = confirmed.get();
        ancillary.update(|owner| owner.consent(value));
    });
    let ancillary_fields = AncillaryFields {
        notes,
        profile,
        permitted_use: use_note,
        attribution,
        naming,
        qa,
        producer,
        provenance: producer_provenance,
        method,
        quality,
        review_kind,
        decision,
        review_notes,
        evidence,
        confirmed,
        consent,
    };
    let audio_unsaved = Signal::derive(move || {
        file.with(|file| {
            facts.with(|facts| {
                super::ancillary::audio_dirty(
                    file.as_ref()
                        .map(|(bytes, sha256)| (sha256.as_str(), bytes.len() as u64)),
                    facts.submission.as_ref(),
                )
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
        admit: Some(Callback::new(move |command: Command| {
            facts
                .with_untracked(|f| projection::admit(&command, f.work.as_ref(), f.unit.as_ref()))
                .is_ok()
        })),
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
                        if matches!(&command, Command::CreateWork(_)) {
                            let _ = task.try_set(Task::Details);
                        }
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
                        let consume_file = file.with_untracked(|file| {
                            super::ancillary::consume_uploaded_file(
                                file.as_ref()
                                    .map(|(bytes, sha256)| (sha256.as_str(), bytes.len() as u64)),
                                file_generation.get_untracked(),
                                upload_file_generation.get_value(),
                                &command,
                                &reply,
                            )
                        });
                        if consume_file {
                            file.set(None);
                            file_error.set(false);
                            if let Some(input) = web_sys::window()
                                .and_then(|window| window.document())
                                .and_then(|document| document.get_element_by_id("live-audio-file"))
                                .and_then(|element| {
                                    element.dyn_into::<web_sys::HtmlInputElement>().ok()
                                })
                            {
                                input.set_value("");
                            }
                        }
                        if matches!(&command, Command::Upload { .. }) {
                            upload_file_generation.set_value(None);
                        }
                        let before_ancillary =
                            ancillary.with_untracked(super::ancillary::AncillaryDrafts::current);
                        let _ = ancillary.try_update(|owner| owner.acknowledge(&command, &reply));
                        let after_ancillary =
                            ancillary.with_untracked(super::ancillary::AncillaryDrafts::current);
                        if before_ancillary != after_ancillary {
                            ancillary_fields.restore(&after_ancillary);
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
                        if matches!(command, Command::Revoke { .. }) {
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
                            let _ = task.try_set(Task::Write);
                            let _ = pane.try_set(Pane::Work);
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
                        let next_unit = facts.with_untracked(Facts::unit_id);
                        if next_unit != selected {
                            if let Some(form) = ancillary.try_update(|owner| owner.open(&next_unit))
                            {
                                ancillary_fields.restore(&form);
                            }
                        }
                    }
                    Err(error) if feedback::is_missing_head(&command, &error) => {
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
                task.set(Task::default());
                pane.set(Pane::default());
                facts.set(Facts::default());
                drafts.set(super::drafts::AuthoringDrafts::default());
                ancillary.set(super::ancillary::AncillaryDrafts::default());
                artwork_dirty.set(false);
                delegation_dirty.set(false);
                publication_dirty.set(false);
                catalog_dirty.set(false);
                units_complete.set(false);
                replace_head.set_value(None);
                dashboard_query.set(super::dashboard::DashboardQuery::default());
                file_busy.set(false);
                file_error.set(false);
                upload_file_generation.set_value(None);
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
            ancillary.with(super::ancillary::AncillaryDrafts::has_unsaved)
                || audio_unsaved.get()
                || artwork_dirty.get()
                || delegation_dirty.get()
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
    let submit_ancillary = Callback::new(move |result: Result<Command, ClientError>| {
        if blocked.get_untracked() {
            return;
        }
        match result {
            Ok(command) => {
                let snapshot = ancillary_fields.snapshot();
                ancillary.update(|owner| owner.submit(&command, &snapshot));
                dispatch.send(command);
            }
            Err(_) => dispatch.input_error(),
        }
    });
    let confirm_context_change = move |work_context: bool| -> bool {
        let snapshot = ancillary_fields.snapshot();
        // Capture the current checkbox projection as well as typing before changing context.
        ancillary.update(|owner| owner.input("live-review-confirm", snapshot.clone()));
        let form_dirty = ancillary.with_untracked(super::ancillary::AncillaryDrafts::current_dirty);
        let requires_confirmation = form_dirty
            || audio_unsaved.get_untracked()
            || publication_dirty.get_untracked()
            || (work_context
                && (catalog_dirty.get_untracked()
                    || artwork_dirty.get_untracked()
                    || delegation_dirty.get_untracked()));
        if !requires_confirmation {
            return true;
        }
        if !crate::view::confirm_discard(locale.get_untracked()) {
            return false;
        }
        if form_dirty {
            if !ancillary
                .try_update(|owner| owner.discard(&snapshot))
                .unwrap_or(false)
            {
                return false;
            }
            ancillary_fields
                .restore(&ancillary.with_untracked(super::ancillary::AncillaryDrafts::current));
        }
        true
    };
    let read_works = Callback::new(move |cursor: Option<String>| {
        if blocked.get_untracked() {
            return;
        }
        dashboard_query.update(|query| {
            *query = query.start(&actor.get_untracked(), session_epoch.get_untracked());
        });
        dispatch.send_page(Command::Works { cursor });
    });
    Effect::new(move |_| {
        let owner = actor.get();
        let epoch = session_epoch.get();
        let target = format!("workspace:{epoch}");
        if dashboard.get()
            && active.get()
            && !session_pending.get()
            && !owner.is_empty()
            && !blocked.get()
            && state.with(|s| s.actor() == owner && s.target() == target && s.pending().is_none())
            && !file_busy.get()
            && !delegation_unresolved.get()
            && !dashboard_query.with(|query| query.attempted(&owner, epoch))
        {
            dashboard_query.update(|query| *query = query.start(&owner, epoch));
            dispatch.send(Command::Works { cursor: None });
        }
    });
    let pick_work = Callback::new(move |id: String| {
        if blocked.get_untracked() {
            return;
        }
        if facts.with_untracked(|f| f.work.as_ref().is_some_and(|work| work.id == id)) {
            // Choosing the open Work again only navigates to it; nothing is reset or refetched.
            task.set(Task::Details);
            pane.set(Pane::Work);
            return;
        }
        if !confirm_context_change(true) {
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
                pages: f.pages.for_work(),
                ..Facts::default()
            };
        });
        credential.set(None);
        consent.set(None);
        let form = ancillary
            .try_update(|owner| owner.open(""))
            .unwrap_or_default();
        ancillary_fields.restore(&form);
        // A different Work always starts at its details; the chapter list stays in view on narrow
        // layouts. A creation form is closed, not discarded: its inputs live in the signals above.
        task.set(Task::Details);
        pane.set(Pane::Browse);
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
            task.set(Task::Write);
            pane.set(Pane::Work);
            return;
        }
        if !confirm_context_change(false) {
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
            f.pages.clear_history();
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
        let form = ancillary
            .try_update(|owner| owner.open(&id))
            .unwrap_or_default();
        ancillary_fields.restore(&form);
        task.set(Task::Write);
        pane.set(Pane::Work);
        dispatch.send(Command::ContentHead { unit: id });
    });
    let dashboard_pick = Callback::new(move |id: String| {
        if session_pending.get_untracked() {
            return;
        }
        // Continue means exposing the same mounted selection; never reset its unit or refetch.
        if facts.with_untracked(|f| f.work.as_ref().is_some_and(|work| work.id == id)) {
            pane.set(if facts.with_untracked(|f| f.unit.is_some()) {
                Pane::Work
            } else {
                Pane::Browse
            });
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
        task.set(Task::CreateWork);
        pane.set(Pane::Work);
        focus_workspace_field("live-work-title");
    });
    let open_audio = Callback::new(move |()| {
        if session_pending.get_untracked() {
            return;
        }
        on_open_content.run(());
        task.set(Task::Audio);
        pane.set(Pane::Work);
        focus_workspace_field("live-audio-section");
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
            // The pure owner ignores lookup controls and independently owned child-panel fields.
            _ => {
                let form = ancillary_fields.snapshot();
                ancillary.update(|owner| owner.input(&id, form));
            }
        }
    });
    let read_head = Callback::new(move |()| {
        if blocked.get_untracked() {
            return;
        }
        let unit = facts.with_untracked(Facts::unit_id);
        let current = drafts.with_untracked(|drafts| drafts.content(&unit));
        if drafts.with_untracked(|drafts| drafts.content_dirty(&unit))
            && !crate::view::confirm_discard(locale.get_untracked())
        {
            return;
        }
        replace_head.set_value(Some((unit.clone(), current)));
        dispatch.send(Command::ContentHead { unit });
    });
    // The enhanced menu has non-native option rows: the fieldset alone cannot enforce its lock.
    let content_locked = Signal::derive(move || {
        facts.with(|facts| drafts.with(|drafts| drafts.content_base(&facts.unit_id()).is_none()))
    });
    let open_selected_editor =
        Callback::new(move |()| on_editor.run(facts.with_untracked(Facts::editor_script)));
    let has_work = Signal::derive(move || facts.with(|f| f.work.is_some()));
    let has_unit = Signal::derive(move || facts.with(|f| f.unit.is_some()));
    let shown = Signal::derive(move || layout::resolve(task.get(), has_work.get(), has_unit.get()));
    let hide_area = move |area: Task| shown.get() != Some(area);
    // One capability projection feeds fields, buttons and event-time admission; reads stay open.
    let work_refusal =
        Signal::derive(move || facts.with(|f| projection::edit_work(f.work.as_ref()).err()));
    let unit_refusal = Signal::derive(move || {
        facts.with(|f| projection::edit_unit(f.work.as_ref(), f.unit.as_ref()).err())
    });
    let lifecycle_refusal = Signal::derive(move || {
        facts.with(|f| projection::lifecycle_reason(f.work.as_ref(), f.unit.as_ref()))
    });
    let read_only = Signal::derive(move || lifecycle_refusal.get().is_some());
    let reason_text = move |refusal: Option<Refusal>| -> String {
        refusal
            .and_then(Refusal::reason_key)
            .map(|key| text(key, locale.get()).to_owned())
            .unwrap_or_default()
    };
    let work_ids = Memo::new(move |_| {
        facts.with(|f| projection::row_ids(f.works.iter().map(|work| work.id.as_str())))
    });
    let unit_ids = Memo::new(move |_| {
        facts.with(|f| projection::row_ids(f.units.iter().map(|unit| unit.id.as_str())))
    });
    let report = Memo::new(move |_| state.with(feedback::feedback));
    // Names the operation and target a settled outcome answers; a title is looked up by ID, so a
    // removed or unlisted entity simply has none.
    let feedback_context = move || -> String {
        let report = report.get();
        let locale = locale.get();
        let title = facts.with(|f| match &report.target {
            Target::Work(id) => f
                .work
                .iter()
                .chain(f.works.iter())
                .find(|work| &work.id == id)
                .map(|work| work.details.title.clone()),
            Target::Unit(id) => f
                .unit
                .iter()
                .chain(f.units.iter())
                .find(|unit| &unit.id == id)
                .map(|unit| unit.details.title.clone()),
            Target::None => None,
        });
        // An outcome about something no longer selected always names its task, so it cannot be
        // read as the result of what is open now.
        let selected = facts.with(|f| {
            report.target.is_selected(
                f.work.as_ref().map(|work| work.id.as_str()),
                f.unit.as_ref().map(|unit| unit.id.as_str()),
            )
        });
        let label = match report.outcome {
            Outcome::Failed | Outcome::Unknown | Outcome::Working => true,
            Outcome::Done | Outcome::MissingHead => !selected,
            Outcome::Idle => false,
        }
        .then(|| report.operation.label_key())
        .flatten()
        .map(|key| text(key, locale).to_owned());
        [label, title]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let outcome_text = move || -> &'static str {
        let report = report.get();
        let locale = locale.get();
        text(
            match report.outcome {
                Outcome::Idle => Key::Idle,
                Outcome::Working => Key::Pending,
                Outcome::Done => report.operation.done_key(),
                Outcome::MissingHead => Key::NoContent,
                Outcome::Failed => Key::Failed,
                Outcome::Unknown => Key::Ambiguous,
            },
            locale,
        )
    };
    let field_state = Signal::derive(move || {
        if read_only.get() {
            FieldState::ReadOnly
        } else {
            FieldState::Editable
        }
    });
    let editor_status = Signal::derive(move || {
        let unit = facts.with(Facts::unit_id);
        let saving = state.with(|s| layout::saving_selected(s.pending(), s.phase(), &unit));
        let (base, unsaved) = drafts.with(|d| (d.content_base(&unit), d.content_dirty(&unit)));
        layout::editor_status(base, unsaved, saving, read_only.get())
    });
    let editor_status_text = move || -> String {
        let locale = locale.get();
        match editor_status.get() {
            EditorStatus::NotRead => text(Key::StatusNotRead, locale).to_owned(),
            EditorStatus::NoSavedContent => text(Key::NoContent, locale).to_owned(),
            EditorStatus::Saved(revision) => {
                copy::labelled_revision(text(Key::StatusSaved, locale), revision)
            }
            EditorStatus::Unsaved(_) => text(Key::StatusUnsaved, locale).to_owned(),
            EditorStatus::Saving => text(Key::StatusSaving, locale).to_owned(),
            EditorStatus::ArchivedReadOnly => text(Key::StatusArchived, locale).to_owned(),
        }
    };
    let viewing = Signal::derive(move || {
        facts.with(|f| layout::viewing(f.content.as_ref(), f.content_head.as_ref(), &f.unit_id()))
    });
    view! {<section id="workspace-content" aria-labelledby=move || if dashboard.get() {"dashboard-title"} else {"live-title"}
                on:input=move |event| { if let Some(id) = event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).filter(|element|!element.tag_name().eq_ignore_ascii_case("select")).map(|element| element.id()) { track_input.run(id); } }
                on:change=move |event| { if let Some(id) = event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).filter(|element|!element.tag_name().eq_ignore_ascii_case("select")).map(|element| element.id()) { track_input.run(id); } }>
                <div hidden=move || !dashboard.get() inert=move || !dashboard.get()>
                    <super::dashboard::Dashboard facts=facts query=dashboard_query locale=locale blocked=blocked session_pending=session_pending
                        on_new_work=open_new_work on_import=on_import on_editor=open_selected_editor on_continue=on_open_content on_audio=open_audio
                        on_refresh=Callback::new(move |()| read_works.run(None))
                        on_previous=Callback::new(move |()| { if let Some(cursor) = facts.with_untracked(|f| f.pages.previous_works()) { read_works.run(cursor); } })
                        on_next=Callback::new(move |()| { if let Some(cursor) = facts.with_untracked(|f| f.works_next.clone()) { read_works.run(Some(cursor)); } })
                        on_pick=dashboard_pick/>
                </div>
    <div class="cw" hidden=move || dashboard.get() inert=move || dashboard.get() data-pane=move ||pane.get().as_str() data-area=move ||shown.get().map(Task::as_str).unwrap_or("none")>
                <h2 id="live-title" class="cui-sr-only" tabindex="-1">{move ||text(Key::Pane,locale.get())}</h2>
            <header id="live-context" class="cw-context" aria-label=move ||text(Key::ContextLabel,locale.get()) hidden=move ||!has_work.get()>
                    <button id="live-browse" type="button" class="cui-button cw-browse" data-variant="text" on:click=move |_| pane.set(Pane::Browse)>{move ||text(Key::BackToList,locale.get())}</button>
                    <div class="cw-heading">
                        <div class="cw-trail">
                            <Show when=move ||has_work.get()>
                                <span class="cw-trail-item" lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move ||facts.with(|f|f.work.as_ref().map(|w|copy::labelled_revision(&w.details.title,w.revision)).unwrap_or_default())}</span>
                            </Show>
                            <Show when=move ||has_unit.get()>
                                <span class="cw-trail-sep" aria-hidden="true"></span>
                                <span class="cw-trail-item" lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move ||facts.with(|f|f.unit.as_ref().map(|u|copy::labelled_revision(&u.details.title,u.revision)).unwrap_or_default())}</span>
                            </Show>
                        </div>
                    </div>
                    <p class="cw-state" role="status" data-state=move ||editor_status.get().as_str() hidden=move ||!has_unit.get()>{move ||editor_status_text()}</p>
                </header>
                <p class="cw-unsaved" role="status" hidden=move ||!drafts.with(super::drafts::AuthoringDrafts::has_unsaved)>{move ||text(Key::UnsavedDrafts,locale.get())}</p>
                <div id="live-status" class="live-status cw-feedback" tabindex="-1" role="status" aria-live="polite" data-outcome=move ||format!("{:?}",report.get().outcome) hidden=move ||state.with(|s|s.phase()==Phase::Idle)><p>{outcome_text}</p>
                    <p class="cw-feedback-target" hidden=move ||feedback_context().is_empty()>{feedback_context}</p>
                    {move ||if report.get().outcome==Outcome::MissingHead {String::new()} else {state.with(|s|s.error().map(|error|text(copy::error_key(error),locale.get())).unwrap_or_default()).to_owned()}}
                    <p>{move ||if state.with(RequestState::is_empty){text(Key::Empty,locale.get())}else{""}}</p>
                    <super::field_issues::FieldIssues locale=locale issues=Signal::derive(move ||state.with(|s|match s.error() {Some(ClientError::Http {error,..})=>error.issues.clone(),_=>vec![]}))/>
                    <div class="toolbar" hidden=move ||state.with(|s|s.phase()!=Phase::Pending&&!s.can_reconcile()&&!s.can_retry())><span hidden=move ||state.with(|s|s.phase()!=Phase::Pending)><ActionButton id="live-cancel" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=Signal::derive(move ||state.with(|s|s.phase()!=Phase::Pending)) on_press=Callback::new(move |_|{cancel_request.run(());focus_workspace_field("live-status");})>{move ||text(Key::Cancel,locale.get())}</ActionButton></span>
                    <span hidden=move ||state.with(|s|!s.can_reconcile())><ActionButton id="live-reconcile" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||session_pending.get()||state.with(|s|!s.can_reconcile())) on_press=Callback::new(move |_|{dispatch.reconcile();focus_workspace_field("live-status");})>{move ||text(Key::Reconcile,locale.get())}</ActionButton></span>
                    <span hidden=move ||state.with(|s|!s.can_retry())><ActionButton id="live-retry" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||session_pending.get()||state.with(|s|!s.can_retry())) on_press=Callback::new(move |_|{dispatch.retry();focus_workspace_field("live-status");})>{move ||text(Key::Retry,locale.get())}</ActionButton></span></div>
                </div>
                <div class="cw-body">
                    <aside id="live-nav" class="cw-nav" aria-label=move ||text(Key::NavLabel,locale.get())>
                        <section class="cw-nav-section" aria-labelledby="live-works-heading">
                            <div class="cw-nav-heading"><h3 id="live-works-heading">{move ||text(Key::Work,locale.get())}</h3>
                                <ActionButton id="live-new-work" locale=locale variant=ButtonVariant::Tonal icon=IconName::Add reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|open_new_work.run(()))>{move ||text(Key::NewWork,locale.get())}</ActionButton></div>
                            <div class="toolbar cw-pager"><ActionButton id="live-works" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|read_works.run(None))>{move ||text(Key::Refresh,locale.get())}</ActionButton>
                            <span hidden=move ||facts.with(|f|f.pages.previous_works().is_none())><ActionButton id="live-works-previous" locale=locale variant=ButtonVariant::Text reason=BlockReason::PageNavigation blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.pages.previous_works().is_none())) on_press=Callback::new(move |_| { if let Some(cursor) = facts.with_untracked(|f|f.pages.previous_works()) { read_works.run(cursor); } })>{move ||text(Key::Previous,locale.get())}</ActionButton></span>
                            <span hidden=move ||facts.with(|f|f.works_next.is_none())><ActionButton id="live-works-next" locale=locale variant=ButtonVariant::Text reason=BlockReason::PageNavigation blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.works_next.is_none())) on_press=Callback::new(move |_| { if let Some(cursor) = facts.with_untracked(|f| f.works_next.clone()) { read_works.run(Some(cursor)); } })>{move ||text(Key::Next,locale.get())}</ActionButton></span></div>
                            <ul id="live-works-list" class="live-list cw-list" tabindex="-1" aria-label=move ||text(Key::WorkResults,locale.get())>
                                <For each=move||work_ids.get() key=|id|id.clone() children=move |id| {
                                    let row=Memo::new({let id=id.clone();move |_|facts.with(|f|f.works.iter().find(|work|work.id==id).cloned())});
                                    let current=id.clone();
                                    view! {<li><button class="cui-button" data-variant="text" type="button" data-lifecycle=move||row.get().map(|w|w.lifecycle) aria-current=move ||facts.with(|f|f.work.as_ref().is_some_and(|work|work.id==current)).then_some("true") disabled=move ||blocked.get() on:click=move |_|pick_work.run(id.clone())><span lang=move||row.get().map(|w|w.details.language)>{move||row.get().map(|w|w.details.title)}</span><small>{move ||row.get().map(|w|copy::labelled_revision(copy::work_kind(&w.kind,locale.get()),w.revision))}</small></button></li>}
                                }/>
                            </ul>
                        </section>
                        <Show when=move ||has_work.get()>
                            <section class="cw-nav-section" aria-labelledby="live-units-heading">
                                <div class="cw-nav-heading"><h3 id="live-units-heading">{move ||text(Key::Unit,locale.get())}</h3>
                                    <ActionButton id="live-new-unit" locale=locale variant=ButtonVariant::Tonal icon=IconName::Add reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||work_refusal.get().is_some()) on_press=Callback::new(move |_| { task.set(Task::CreateUnit); pane.set(Pane::Work); focus_workspace_field("live-unit-title"); })>{move ||text(Key::NewUnit,locale.get())}</ActionButton></div>
                                <div class="toolbar cw-pager"><ActionButton id="live-refresh-units" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| { if let Some(work)=facts.with_untracked(|f|f.work.clone()) {dispatch.send_page(Command::Units {work:work.id,cursor:None});} })>{move ||text(Key::RefreshUnits,locale.get())}</ActionButton>
                                <span hidden=move ||facts.with(|f|f.pages.previous_units().is_none())><ActionButton id="live-units-previous" locale=locale variant=ButtonVariant::Text reason=BlockReason::PageNavigation blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.pages.previous_units().is_none())) on_press=Callback::new(move |_| {if let Some((work,cursor))=facts.with_untracked(|f|Some((f.work.as_ref()?.id.clone(),f.pages.previous_units()?))) {dispatch.send_page(Command::Units {work,cursor});}})>{move ||text(Key::Previous,locale.get())}</ActionButton></span>
                                <span hidden=move ||facts.with(|f|f.units_next.is_none())><ActionButton id="live-units-next" locale=locale variant=ButtonVariant::Text reason=BlockReason::PageNavigation blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.units_next.is_none())) on_press=Callback::new(move |_| {if let Some(work)=facts.with_untracked(|f|f.work.clone()) {dispatch.send_page(Command::Units {work:work.id,cursor:facts.with_untracked(|f|f.units_next.clone())});}})>{move ||text(Key::Next,locale.get())}</ActionButton></span></div>
                                <ul id="live-units-list" class="live-list cw-list" tabindex="-1" aria-label=move ||text(Key::UnitResults,locale.get())>
                                    <For each=move||unit_ids.get() key=|id|id.clone() children=move |id| {
                                        let row=Memo::new({let id=id.clone();move |_|facts.with(|f|f.units.iter().find(|unit|unit.id==id).cloned())});
                                        let current=id.clone();
                                        view! {<li><button type="button" class="cui-button" data-variant="text" data-lifecycle=move||row.get().map(|u|u.lifecycle) aria-current=move ||facts.with(|f|f.unit.as_ref().is_some_and(|unit|unit.id==current)).then_some("true") disabled=move ||blocked.get() on:click=move |_|pick_unit.run(id.clone())><span lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move||row.get().map(|u|u.details.title)}</span><small>{move ||row.get().map(|u|copy::labelled_revision(copy::unit_kind(&u.kind,locale.get()),u.revision))}</small></button></li>}
                                    }/>
                                </ul>
                            </section>
                        </Show>
                    </aside>
                    <div id="live-main" class="cw-main">
                        <div class="cw-tabs" role="group" aria-label=move ||text(Key::TasksLabel,locale.get()) hidden=move ||!has_work.get()>
                            {Task::AREAS.into_iter().map(|area| view! {<button id=area.chip_id() type="button" class="cui-button" data-variant=move ||if shown.get()==Some(area) {"tonal"} else {"text"} aria-pressed=move ||(shown.get()==Some(area)).to_string() hidden=move ||!area.available(has_work.get(),has_unit.get()) on:click=move |_|{task.set(area);pane.set(Pane::Work);}>{move ||text(task_label(area),locale.get())}</button>}).collect_view()}
                        </div>
                        <p class="help cw-empty" hidden=move ||shown.get().is_some()>{move ||text(Key::NoWorkSelected,locale.get())}</p>

                        <section id="live-create-work-form" class="cw-panel cw-form" aria-labelledby="live-create-work-title" hidden=move ||hide_area(Task::CreateWork) inert=move ||hide_area(Task::CreateWork)>
                            <h3 id="live-create-work-title">{move ||text(Key::CreateWork,locale.get())}</h3>
                            <div class="live-grid cw-form-grid">
                                <TextField id="live-work-title" label=Signal::derive(move ||text(Key::Title,locale.get()).to_owned()) value=title on_input=Callback::new(move |value|title.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextArea id="live-work-synopsis" label=Signal::derive(move ||text(Key::Synopsis,locale.get()).to_owned()) value=synopsis on_input=Callback::new(move |value|synopsis.set(value)) class="live-field"/>
                    <Select id="live-work-kind" label=Signal::derive(move ||text(Key::Kind,locale.get()).to_owned()) value=kind on_input=Callback::new(move |value|{kind.set(value);track_input.run("live-work-kind".to_owned());}) options=Signal::derive(move ||["novel","story","podcast"].map(|kind|SelectOption::new(kind,copy::work_kind(kind,locale.get()))).to_vec()) class="live-field"/>
                    <TextField id="live-rights" label=Signal::derive(move ||text(Key::Rights,locale.get()).to_owned()) value=rights on_input=Callback::new(move |value|rights.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                            </div>
                            <p class="help">{move ||text(Key::CreateKept,locale.get())}</p>
                            <div class="cw-actions"><ActionButton id="live-create-work" locale=locale variant=ButtonVariant::Primary reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                    if blocked.get_untracked() { return; }
                    if !confirm_context_change(true) { return; }
                    dispatch.build((||Ok(Command::CreateWork(CreateWorkRequest {operation_id:uuid()?,id:uuid()?,kind:kind.get_untracked(),details:WorkDetailsDto {title:title.get_untracked(),synopsis:synopsis.get_untracked(),language:"vi-VN".into(),visibility:"private".into(),rights_refs:refs(rights.get_untracked())}})))());
                })>{move ||text(Key::CreateWork,locale.get())}</ActionButton>
                            <ActionButton id="live-close-create-work" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady on_press=Callback::new(move |_| { task.set(Task::Write); pane.set(Pane::Browse); })>{move ||text(Key::CloseForm,locale.get())}</ActionButton></div>
                        </section>

                        <section id="live-create-unit-form" class="cw-panel cw-form" aria-labelledby="live-create-unit-title" hidden=move ||hide_area(Task::CreateUnit) inert=move ||hide_area(Task::CreateUnit)>
                            <h3 id="live-create-unit-title">{move ||text(Key::CreateUnit,locale.get())}</h3>
                            <div class="cw-form-stack">
                                <TextField id="live-unit-title" label=Signal::derive(move ||text(Key::Title,locale.get()).to_owned()) value=new_unit_title on_input=Callback::new(move |value|new_unit_title.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextArea id="live-unit-description" label=Signal::derive(move ||text(Key::Synopsis,locale.get()).to_owned()) value=new_unit_description on_input=Callback::new(move |value|new_unit_description.set(value)) class="live-field"/>
                            </div>
                            <p class="help">{move ||text(Key::CreateKept,locale.get())}</p>
                            <p class="help cw-reason" role="status" hidden=move||work_refusal.get().and_then(Refusal::reason_key).is_none()>{move||reason_text(work_refusal.get())}</p>
                            <div class="cw-actions">                    <ActionButton id="live-create-unit" locale=locale variant=ButtonVariant::Primary reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||work_refusal.get().is_some()) on_press=Callback::new(move |_| {
                            if blocked.get_untracked() || work_refusal.get_untracked().is_some() { return; }
                            if !confirm_context_change(false) { return; }
                            dispatch.build((|| {let w=facts.with_untracked(|f|f.work.clone()).ok_or(ClientError::InvalidInput)?;Ok(Command::CreateUnit {work:w.id,request:CreateContentUnitRequest {operation_id:uuid()?,id:uuid()?,kind:if w.kind=="podcast" {"episode"}else {"chapter"}.into(),expected_work_revision:w.revision,details:UnitDetailsDto {title:new_unit_title.get_untracked(),description:new_unit_description.get_untracked(),source:None,script:None,content_digest:None}}})})());
                        })>{move ||text(Key::CreateUnit,locale.get())}</ActionButton>
                            <ActionButton id="live-close-create-unit" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady on_press=Callback::new(move |_| { task.set(Task::Write); pane.set(Pane::Work); })>{move ||text(Key::CloseForm,locale.get())}</ActionButton></div>
                        </section>

                        <section id="live-area-write" class="cw-panel" aria-labelledby="live-write-title" hidden=move ||hide_area(Task::Write) inert=move ||hide_area(Task::Write)>
                            <Show when=move ||has_unit.get()>
                                <div class="cw-panel-head"><h3 id="live-write-title">{move ||text(Key::Content,locale.get())}</h3>
                                    <div class="toolbar"><ActionButton id="live-content-head" locale=locale variant=ButtonVariant::Text icon=IconName::Refresh reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|read_head.run(()))>{move ||text(Key::ReadHead,locale.get())}</ActionButton>
                                    <ActionButton id="live-open-editor" locale=locale variant=ButtonVariant::Secondary reason=BlockReason::ContextReady blocked=session_pending on_press=Callback::new(move |_|open_selected_editor.run(()))>{move ||text(Key::OpenEditor,locale.get())}</ActionButton></div></div>
                                <p class="help cw-reason" id="live-content-reason" role="status" hidden=move||lifecycle_refusal.get().is_none()>{move||reason_text(lifecycle_refusal.get())}</p>
                                <fieldset class="live-content-fields" disabled=move ||content_locked.get()><legend class="cui-sr-only">{move ||text(Key::Content,locale.get())}</legend>
                                                        <super::content_kind::ContentKindSelect locale=locale value=body.into() locked=Signal::derive(move ||content_locked.get()||read_only.get())
                            editor_script_available=Signal::derive(move ||editor_script.with(|script|script.is_some()))
                            on_change=Callback::new(move |value|{if !content_locked.get_untracked(){body.set(value);track_input.run("live-content-kind".to_owned());}})/>
                                    <div hidden=move ||body.get()!="editor-script"><p class="help">{move ||text(Key::ScriptBindingHelp,locale.get())}</p>
                                    <p><code>{move ||facts.with(|f|drafts.with(|drafts|drafts.content(&f.unit_id()).script.map(|script|copy::script_binding(&script)).unwrap_or_default()))}</code></p></div>
                                    <TextArea id="live-manuscript" label=Signal::derive(move ||text(Key::Text,locale.get()).to_owned()) value=manuscript on_input=Callback::new(move |value|manuscript.set(value)) state=field_state class="live-field cw-manuscript" rows=14/><TextArea id="live-provenance" label=Signal::derive(move ||text(Key::Provenance,locale.get()).to_owned()) value=provenance on_input=Callback::new(move |value|provenance.set(value)) state=field_state class="live-field"/><TextField id="live-content-rights" label=Signal::derive(move ||text(Key::Rights,locale.get()).to_owned()) value=content_rights on_input=Callback::new(move |value|content_rights.set(value)) state=field_state class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                                    <div class="cw-actions">                    <ActionButton id="live-append-content" locale=locale variant=ButtonVariant::Primary icon=IconName::Save reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||unit_refusal.get().is_some()||facts.with(|f|drafts.with(|drafts|drafts.content_base(&f.unit_id()).is_none()||drafts.content(&f.unit_id()).details().is_none()))) on_press=Callback::new(move |_|{
                                        // Admission precedes the draft's submitted-snapshot mark, so a refused press changes nothing.
                                        if blocked.get_untracked()||unit_refusal.get_untracked().is_some() {return;}
                                        dispatch.build((|| {let(w,u)=context(facts)?;let expected_revision=drafts.with_untracked(|drafts|drafts.content_base(&u.id)).ok_or(ClientError::InvalidInput)?;let details=drafts.with_untracked(|drafts|drafts.content(&u.id).details()).ok_or(ClientError::InvalidInput)?;let request=AppendContentRequest {operation_id:uuid()?,id:uuid()?,expected_revision,expected_work_revision:w.revision,details};drafts.update(|drafts|drafts.submitted_content(&u.id,&request));Ok(Command::AppendContent {unit:u.id,request})})())})>{move ||text(Key::SaveContent,locale.get())}</ActionButton></div>
                                </fieldset>
                                <p class="help">{move ||text(Key::Help,locale.get())}</p>
                            </Show>
                        </section>

                        <section id="live-area-history" class="cw-panel" aria-labelledby="live-history-title" hidden=move ||hide_area(Task::History) inert=move ||hide_area(Task::History)>
                            <Show when=move ||has_unit.get()>
                                <h3 id="live-history-title">{move ||text(Key::TaskHistory,locale.get())}</h3>
                                <div class="toolbar"><ActionButton id="live-content-history" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.send_page(Command::ContentHistory {unit:facts.with_untracked(Facts::unit_id),after:0}))>{move ||text(Key::History,locale.get())}</ActionButton>
                                <span hidden=move ||facts.with(|f|f.pages.previous_history().is_none())><ActionButton id="live-history-previous" locale=locale variant=ButtonVariant::Text reason=BlockReason::PageNavigation blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.pages.previous_history().is_none())) on_press=Callback::new(move |_| {if let Some((unit,after))=facts.with_untracked(|f|f.pages.previous_history().map(|after|(f.unit_id(),after))) {dispatch.send_page(Command::ContentHistory {unit,after});}})>{move ||text(Key::Previous,locale.get())}</ActionButton></span>
                                <span hidden=move ||facts.with(|f|f.history.as_ref().is_none_or(|h|h.next_after.is_none()))><ActionButton id="live-history-next" locale=locale variant=ButtonVariant::Text reason=BlockReason::PageNavigation blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.history.as_ref().is_none_or(|h|h.next_after.is_none()))) on_press=Callback::new(move |_| {if let Some((unit,after))=facts.with_untracked(|f|f.history.as_ref()?.next_after.map(|after|(f.unit_id(),after))) {dispatch.send_page(Command::ContentHistory {unit,after});}})>{move ||text(Key::Next,locale.get())}</ActionButton></span></div>
                                <p class="help">{move ||text(Key::HistoryPageHelp,locale.get())}</p>
                                                    <ul id="live-history-list" tabindex="-1" aria-label=move ||text(Key::HistoryResults,locale.get())><For each=move ||facts.with(|f|f.history.as_ref().map(|h|h.revisions.clone()).unwrap_or_default()) key=|revision|revision.id.clone() children=move |revision| {let id=revision.id.clone();view! {<li><button class="cui-button" data-variant="tonal" type="button" disabled=move ||blocked.get() on:click=move |_|dispatch.send(Command::ContentRevision {unit:facts.with_untracked(Facts::unit_id),revision:id.clone()})>{copy::revision_identity(revision.revision,&revision.id)}</button></li>}}/></ul>
                                <section class="cw-viewer" aria-labelledby="live-viewer-title">
                                    <h4 id="live-viewer-title">{move ||text(Key::RevisionViewer,locale.get())}</h4>
                                    <p class="cw-viewer-kind" data-viewing=move ||viewing.get().as_str()>{move ||text(match viewing.get() {Viewing::Nothing=>Key::RevisionNone,Viewing::Current=>Key::RevisionCurrent,Viewing::Historical=>Key::RevisionHistorical,Viewing::Unverified=>Key::RevisionUnverified},locale.get())}</p>
                                    <Show when=move ||viewing.get()!=Viewing::Nothing>
                                        <p><code>{move ||facts.with(|f|f.content.as_ref().map(|c|copy::revision_identity(c.revision,&c.id)).unwrap_or_default())}</code></p>
                                        <pre class="cw-reading" tabindex="0" lang=move ||facts.with(|f|f.content.as_ref().map(|c|c.details.language.clone()))>{move ||facts.with(|f|f.content.as_ref().and_then(|c|match &c.details.body {ContentBody::Text {text,..}=>Some(text.clone()),_=>None}).unwrap_or_default())}</pre>
                                    </Show>
                                    <details><summary>{move ||text(Key::Diagnostics,locale.get())}</summary><pre>{move ||facts.with(|f|f.content.as_ref().and_then(|c|serde_json::to_string_pretty(c).ok()).unwrap_or_default())}</pre></details>
                                </section>
                            </Show>
                        </section>

                        <section id="live-area-details" class="cw-panel" aria-labelledby="live-details-title" hidden=move ||hide_area(Task::Details) inert=move ||hide_area(Task::Details)>
                            <h3 id="live-details-title" class="cui-sr-only">{move ||text(Key::TaskDetails,locale.get())}</h3>
                            <Show when=move ||has_work.get()>
                                <section class="cw-block" aria-labelledby="live-work-details-title"><h4 id="live-work-details-title">{move ||text(Key::WorkDetails,locale.get())}</h4>
                                    <super::catalog_controls::WorkControls facts=facts dispatch=dispatch locale=locale blocked=blocked unsaved=Signal::derive(move ||dirty.get()) dirty=catalog_dirty/>
                                </section>
                            </Show>
                            <p class="help" hidden=move ||has_unit.get()||!has_work.get()>{move ||text(Key::NoUnitSelected,locale.get())}</p>
                            <Show when=move ||has_unit.get()>
                                <section class="cw-block" aria-labelledby="live-unit-details-title"><h4 id="live-unit-details-title">{move ||text(Key::UnitDetails,locale.get())}</h4>
                                    <p class="help cw-reason" id="live-unit-reason" role="status" hidden=move||lifecycle_refusal.get()!=Some(Refusal::UnitArchived)>{move||reason_text(lifecycle_refusal.get())}</p>
                                    <div class="cw-form-stack">                <TextField id="live-selected-unit-title" state=field_state label=Signal::derive(move ||text(Key::Title,locale.get()).to_owned()) value=unit_title on_input=Callback::new(move |value|unit_title.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextArea id="live-selected-unit-description" label=Signal::derive(move ||text(Key::Synopsis,locale.get()).to_owned()) value=description on_input=Callback::new(move |value|description.set(value)) state=field_state class="live-field"/></div>
                                    <div class="cw-actions">                <ActionButton id="live-patch-unit" locale=locale variant=ButtonVariant::Primary icon=IconName::Save reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||unit_refusal.get().is_some()) on_press=Callback::new(move |_|{
                                        if blocked.get_untracked()||unit_refusal.get_untracked().is_some() {return;}
                                        dispatch.build((|| {let (w,u)=context(facts)?;Ok(Command::PatchUnit {unit:u.id.clone(),request:PatchContentUnitRequest {operation_id:uuid()?,expected_revision:drafts.with_untracked(|drafts|drafts.metadata_base(&u.id)).ok_or(ClientError::InvalidInput)?,expected_work_revision:w.revision,details:UnitPatchDto {title:PatchField::Value(unit_title.get_untracked()),description:PatchField::Value(description.get_untracked()),..UnitPatchDto::default()}}})})())})>{move ||text(Key::SaveMetadata,locale.get())}</ActionButton>
                                                    <ActionButton id="live-reload-unit-metadata" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                        if blocked.get_untracked() { return; }
                        if let Some(unit) = facts.with_untracked(|facts| facts.unit.clone()) {
                            if drafts.with_untracked(|drafts|drafts.metadata_dirty(&unit.id)) && !crate::view::confirm_discard(locale.get_untracked()) { return; }
                            let metadata = drafts.try_update(|drafts|drafts.reload_metadata(&unit)).unwrap_or_default();
                            unit_title.set(metadata.title); description.set(metadata.description);
                        }
                    })>{move ||text(Key::ReloadMetadata,locale.get())}</ActionButton></div>
                                    <details><summary>{move ||text(Key::Diagnostics,locale.get())}</summary><p><code>{move ||facts.with(Facts::unit_id)}</code></p></details>
                                </section>
                                <section class="cw-block" aria-labelledby="live-order-title"><h4 id="live-order-title">{move ||text(Key::OrderZone,locale.get())}</h4>
                                    <super::catalog_controls::UnitControls facts=facts dispatch=dispatch locale=locale blocked=blocked unsaved=Signal::derive(move ||dirty.get()) units_complete=Signal::derive(move ||units_complete.get())/>
                                </section>
                            </Show>
                        </section>

                        <section id="live-area-resources" class="cw-panel" aria-label=move ||text(Key::TaskResources,locale.get()) hidden=move ||hide_area(Task::Resources) inert=move ||hide_area(Task::Resources)>
                                        <super::artwork::panel::ArtworkPanel actor=actor work=Signal::derive(move ||facts.with(|f|f.work.clone())) locale=locale
                    session_pending=session_pending session_epoch=session_epoch workspace_busy=Signal::derive(move ||file_busy.get()||delegation_unresolved.get()||state.with(|s|s.pending().is_some()))
                    unresolved=artwork_unresolved busy=artwork_busy dirty=artwork_dirty on_revision=Callback::new(move |revision:u64|facts.update(|f|{
                        if let Some(work)=&mut f.work {work.revision=work.revision.max(revision);
                            if let Some(cached)=f.works.iter_mut().find(|w|w.id==work.id){cached.revision=work.revision;}}
                    }))/>
                                        <super::delegation::panel::DelegationPanel actor=actor work=Signal::derive(move || facts.with(|f| f.work.clone())) locale=locale
                    session_pending=session_pending unresolved=delegation_unresolved busy=delegation_busy dirty=delegation_dirty/>
                        </section>

                        <section id="live-area-audio" class="cw-panel" aria-label=move ||text(Key::TaskAudio,locale.get()) hidden=move ||hide_area(Task::Audio) inert=move ||hide_area(Task::Audio)>
                            <Show when=move ||has_unit.get()>
                                                <section class="live-section"><h3>{move ||text(Key::Brief,locale.get())}</h3><div class="live-grid">
                        <TextField id="live-brief-version" label=Signal::derive(move ||text(Key::Version,locale.get()).to_owned()) value=brief_version on_input=Callback::new(move |value|brief_version.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextArea id="live-direction" label=Signal::derive(move ||text(Key::Notes,locale.get()).to_owned()) value=notes on_input=Callback::new(move |value|notes.set(value)) class="live-field"/>
                        <Select id="live-profile" label=Signal::derive(move ||text(Key::Profile,locale.get()).to_owned()) value=profile on_input=Callback::new(move |value|{profile.set(value);track_input.run("live-profile".to_owned());}) options=Signal::derive(move ||std::iter::once(SelectOption::new("",text(Key::SelectProfile,locale.get()))).chain([20,24,32,40].map(|rate|SelectOption::new(format!("opus-mono-{rate}-v1"),format!("Ogg Opus mono · {rate} kbps")))).collect::<Vec<_>>()) class="live-field"/>
                        <TextField id="live-use" label=Signal::derive(move ||text(Key::Use,locale.get()).to_owned()) value=use_note on_input=Callback::new(move |value|use_note.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextField id="live-attribution" label=Signal::derive(move ||text(Key::Attribution,locale.get()).to_owned()) value=attribution on_input=Callback::new(move |value|attribution.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextField id="live-naming" label=Signal::derive(move ||text(Key::Naming,locale.get()).to_owned()) value=naming on_input=Callback::new(move |value|naming.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextArea id="live-qa" label=Signal::derive(move ||text(Key::Qa,locale.get()).to_owned()) value=qa on_input=Callback::new(move |value|qa.set(value)) class="live-field"/>
                    </div><div class="toolbar"><ActionButton id="live-create-brief" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|super::brief_controls::brief_content(f,drafts.with(|d|d.content_dirty(&f.unit_id()))).is_none())||!valid_profile(&profile.get())) on_press=Callback::new(move |_|submit_ancillary.run((|| {let(_,u)=context(facts)?;let c=facts.with_untracked(|f|super::brief_controls::brief_content(f,drafts.with_untracked(|d|d.content_dirty(&f.unit_id()))).cloned()).ok_or(ClientError::InvalidInput)?;let(mode,capability)=super::brief_controls::production_mode(&c.details.body);Ok(Command::CreateBrief {unit:u.id,request:CreateProductionBriefRequest {operation_id:uuid()?,id:uuid()?,expected_version:brief_version.get_untracked().parse().map_err(|_|ClientError::InvalidInput)?,content_revision_id:c.id,content_revision_digest:c.revision_digest,guidance:BriefGuidance {editorial:EditorialIntent {mode,notes:notes.get_untracked()},voice:None,sound_design:None,required_capabilities:vec![capability,ProducerCapability::OggOpusMono],delivery:DeliveryConstraints {profile_id:profile.get_untracked(),permitted_use:use_note.get_untracked(),attribution:attribution.get_untracked(),naming:naming.get_untracked(),qa_instructions:qa.get_untracked()}}}})})()))>{move ||text(Key::CreateBrief,locale.get())}</ActionButton>
                        <ActionButton id="live-read-brief" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.build(brief_version.get_untracked().parse().map(|version|Command::Brief {unit:facts.with_untracked(Facts::unit_id),version}).map_err(|_|ClientError::InvalidInput)))>{move ||text(Key::ReadBrief,locale.get())}</ActionButton></div>
                        <p class="help">{move ||text(Key::BriefHeadHelp,locale.get())}</p>
                        <p><code>{move ||facts.with(|f|f.content_head.as_ref().map(copy::content_binding).unwrap_or_default())}</code></p>
                        <super::brief_controls::BriefDocument brief=Signal::derive(move ||facts.with(|f|f.brief.clone())) locale=locale blocked=blocked/>
                    </section>
                    <section id="live-audio-section" class="live-section" tabindex="-1" aria-labelledby="live-audio-title"><h3 id="live-audio-title">{move ||text(Key::Audio,locale.get())}</h3><label for="live-audio-file">{move ||text(Key::File,locale.get())}</label>
                        <input id="live-audio-file" type="file" accept=".ogg,audio/ogg" disabled=move ||blocked.get() on:change=move |event| {
                            file.set(None);file_error.set(false);let Some(generation)=file_generation.get_untracked().checked_add(1) else {dispatch.input_error();return;};file_generation.set(generation);
                            let actor_snapshot=actor.get_untracked();let unit_snapshot=facts.with_untracked(Facts::unit_id);
                            let selected=event.target().and_then(|t|t.dyn_into::<web_sys::HtmlInputElement>().ok()).and_then(|i|i.files()).and_then(|f|f.get(0));
                            if let Some(selected)=selected {file_busy.set(true);spawn_local(async move {let result=browser::read_audio(selected).await;if file_generation.try_get_untracked()==Some(generation)&&actor.try_get_untracked()==Some(actor_snapshot)&&facts.try_with_untracked(Facts::unit_id)==Some(unit_snapshot) {let _=file_busy.try_set(false);match result {Ok(value)=>{let _=file.try_set(Some(value));},Err(_)=>{let _=file_error.try_set(true);}}}});}
                        }/>
                        <p>{move ||if file.with(|f|f.is_some()) {text(Key::FileReady,locale.get())}else {""}}</p><p role="alert" hidden=move ||!file_error.get()>{move ||text(Key::FileReadFailed,locale.get())}</p><div class="live-grid"><TextField id="live-producer" label=Signal::derive(move ||text(Key::Producer,locale.get()).to_owned()) value=producer on_input=Callback::new(move |value|producer.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextField id="live-method" label=Signal::derive(move ||text(Key::Method,locale.get()).to_owned()) value=method on_input=Callback::new(move |value|method.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><TextArea id="live-producer-provenance" label=Signal::derive(move ||text(Key::ProducerProvenance,locale.get()).to_owned()) value=producer_provenance on_input=Callback::new(move |value|producer_provenance.set(value)) class="live-field"/><TextArea id="live-quality" label=Signal::derive(move ||text(Key::Quality,locale.get()).to_owned()) value=quality on_input=Callback::new(move |value|quality.set(value)) class="live-field"/></div>
                        <ActionButton id="live-prepare" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||file.with(|f|f.is_none())||facts.with(|f|f.brief.as_ref().is_none_or(|b|b.freshness!=BriefFreshness::Current))) on_press=Callback::new(move |_|submit_ancillary.run((|| {let(w,u)=context(facts)?;let b=facts.with_untracked(|f|f.brief.clone()).ok_or(ClientError::InvalidInput)?;let binding=super::brief_controls::submission_binding(&b,&w.id,&u.id)?;let(bytes,sha)=file.get_untracked().ok_or(ClientError::InvalidInput)?;let expires_at=(js_sys::Date::now() as i64).checked_add(3_600_000).ok_or(ClientError::InvalidInput)?;Ok(Command::Prepare {unit:u.id.clone(),request:CreateAudioSubmissionRequest {operation_id:uuid()?,id:uuid()?,binding,expected_sha256:sha,expected_byte_length:bytes.len() as u64,declarations:ProducerDeclarations {producer:producer.get_untracked(),method:method.get_untracked(),provenance:producer_provenance.get_untracked(),rights_refs:b.brief.work.rights_refs.clone(),quality_report:quality.get_untracked()},upload_token:browser::upload_token()?,expires_at}})})()))>{move ||text(Key::Prepare,locale.get())}</ActionButton>
                        <TextField id="live-submission-id" label=Signal::derive(move ||text(Key::Submission,locale.get()).to_owned()) value=submission_id on_input=Callback::new(move |value|submission_id.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                        <div class="toolbar"><ActionButton id="live-read-submission" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.send(Command::Submission {unit:facts.with_untracked(Facts::unit_id),submission:submission_id.get_untracked()}))>{move ||text(Key::ReadSubmission,locale.get())}</ActionButton>
                        <ActionButton id="live-upload" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.submission.as_ref().is_none_or(|s|credential.with(|c|c.as_ref().and_then(|c|c.for_submission(&s.submission.id)).is_none())))||file.with(|f|f.is_none())||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.state!=AudioSubmissionState::AwaitingBytes))) on_press=Callback::new(move |_|dispatch.build((|| {let s=facts.with_untracked(|f|f.submission.clone()).ok_or(ClientError::InvalidInput)?;let(bytes,sha)=file.get_untracked().ok_or(ClientError::InvalidInput)?;if sha!=s.submission.expected_sha256 || bytes.len() as u64!=s.submission.expected_byte_length {return Err(ClientError::InvalidInput)};let command=Command::Upload {binding:s.submission.binding,sha256:sha,submission:s.submission.id.clone(),credential:credential.with_untracked(|c|c.as_ref().and_then(|c|c.for_submission(&s.submission.id)).cloned()).ok_or(ClientError::InvalidInput)?,bytes};upload_file_generation.set_value(Some(file_generation.get_untracked()));Ok(command)})()))>{move ||text(Key::Upload,locale.get())}</ActionButton>
                        <ActionButton id="live-revoke" locale=locale variant=ButtonVariant::Secondary reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {if crate::view::confirm_discard(locale.get_untracked()) {dispatch.send(Command::Revoke {unit:facts.with_untracked(Facts::unit_id),submission:submission_id.get_untracked()});}})>{move ||text(Key::Revoke,locale.get())}</ActionButton></div>
                        <p role="status">{move ||facts.with(|f|f.submission.as_ref().map(|s|copy::submission_state(s.state,locale.get())).unwrap_or_default())}</p>
                        <p class="help" hidden=move ||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.state!=AudioSubmissionState::AwaitingBytes||credential.with(|c|c.as_ref().and_then(|c|c.for_submission(&s.submission.id)).is_some())))>{move ||text(Key::CredentialMissing,locale.get())}</p>
                        <p><code>{move ||facts.with(|f|f.submission.as_ref().map(|s|copy::submission_binding(&s.submission)).unwrap_or_default())}</code></p>
                        <p><code>{move ||facts.with(|f|f.submission.as_ref().and_then(|s|s.rendition.as_ref()).map(copy::rendition_summary).unwrap_or_default())}</code></p>
                        <details><summary>{move ||text(Key::InspectDocument,locale.get())}</summary><pre>{move ||facts.with(|f|f.submission.as_ref().and_then(|s|serde_json::to_string_pretty(s).ok()).unwrap_or_default())}</pre></details>
                    </section>
                    <super::audio_preview::AudioPreview facts=facts dispatch=dispatch actor=actor active=active session_pending=session_pending locale=locale blocked=blocked/>
                    <section class="live-section"><h3>{move ||text(Key::Review,locale.get())}</h3><div class="live-grid">
                        <Select id="live-review-kind" label=Signal::derive(move ||text(Key::ReviewKind,locale.get()).to_owned()) value=review_kind on_input=Callback::new(move |value|{review_kind.set(value);track_input.run("live-review-kind".to_owned());}) options=Signal::derive(move ||[("qc",AudioReviewKind::Qc),("rights",AudioReviewKind::Rights),("editorial",AudioReviewKind::Editorial)].map(|(value,kind)|SelectOption::new(value,copy::review_kind(kind,locale.get()))).to_vec()) class="live-field"/>
                        <Select id="live-decision" label=Signal::derive(move ||text(Key::Decision,locale.get()).to_owned()) value=decision on_input=Callback::new(move |value|{decision.set(value);track_input.run("live-decision".to_owned());}) options=Signal::derive(move ||[("rejected",AudioReviewDecision::Rejected),("approved",AudioReviewDecision::Approved),("withdrawn",AudioReviewDecision::Withdrawn)].map(|(value,decision)|SelectOption::new(value,copy::review_decision(decision,locale.get()))).to_vec()) class="live-field"/>
                        <TextArea id="live-review-notes" label=Signal::derive(move ||text(Key::Notes,locale.get()).to_owned()) value=review_notes on_input=Callback::new(move |value|review_notes.set(value)) class="live-field"/><TextField id="live-evidence" label=Signal::derive(move ||text(Key::Evidence,locale.get()).to_owned()) value=evidence on_input=Callback::new(move |value|evidence.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                    </div><crate::ui::ReviewCheckbox id="live-review-confirm" locale=locale
                        label=Signal::derive(move ||text(Key::Confirm,locale.get()).to_owned()) checked=confirmed
                        on_change=Callback::new(move |checked|consent.set(if checked{current_review.get_untracked()}else{None}))/>
        <p>{move ||text(Key::ConfirmHelp,locale.get())}</p>
                        <ActionButton id="live-review" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||!confirmed.get()||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.rendition.is_none()))) on_press=Callback::new(move |_|submit_ancillary.run((|| {let current=current_review.get_untracked().ok_or(ClientError::InvalidInput)?;if !consent.with_untracked(|c|c.as_ref().is_some_and(|c|c.authorizes(&current))) {return Err(ClientError::InvalidInput)};Ok(Command::Review {unit:current.unit.clone(),submission:current.submission.clone(),request:current.request(uuid()?)})})()))>{move ||text(Key::Review,locale.get())}</ActionButton>
                        <ActionButton id="live-candidate" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||facts.with(|f|f.submission.as_ref().is_none_or(|s|s.rendition.is_none()))) on_press=Callback::new(move |_|dispatch.build((|| {let(_,u)=context(facts)?;let s=facts.with_untracked(|f|f.submission.clone()).ok_or(ClientError::InvalidInput)?;let r=s.rendition.ok_or(ClientError::InvalidInput)?;Ok(Command::Candidate {unit:u.id,submission:s.submission.id,request:CreateReleaseCandidateRequest {operation_id:uuid()?,rendition_digest:r.digest}})})()))>{move ||text(Key::CreateCandidate,locale.get())}</ActionButton>
                        <TextField id="live-candidate-id" label=Signal::derive(move ||text(Key::Candidate,locale.get()).to_owned()) value=candidate_id on_input=Callback::new(move |value|candidate_id.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/><ActionButton id="live-read-candidate" locale=locale variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_|dispatch.send(Command::CandidateRead {unit:facts.with_untracked(Facts::unit_id),submission:submission_id.get_untracked(),candidate:candidate_id.get_untracked()}))>{move ||text(Key::ReadCandidate,locale.get())}</ActionButton>
                        <p>{move ||text(Key::Private,locale.get())}</p>
                        <ul>{move ||facts.with(|f|f.candidate.as_ref().map(|c|c.blockers.clone()).unwrap_or_default()).into_iter().map(|blocker|view!{<li>{move ||copy::audio_blocker(blocker,locale.get())}</li>}).collect_view()}</ul>
                        <p><code>{move ||facts.with(|f|f.candidate.as_ref().map(|c|copy::candidate_binding(&c.candidate)).unwrap_or_default())}</code></p>
                        <details><summary>{move ||text(Key::InspectDocument,locale.get())}</summary><pre>{move ||facts.with(|f|f.candidate.as_ref().and_then(|c|serde_json::to_string_pretty(c).ok()).unwrap_or_default())}</pre></details>
                    </section>
                    <super::publication::PublicationPanel facts=facts dispatch=dispatch locale=locale blocked=blocked dirty=publication_dirty/>
                            </Show>
                        </section>
                    </div>
                </div>
            </div>
            </section>}
}

fn task_label(task: Task) -> Key {
    match task {
        Task::Write => Key::TaskWrite,
        Task::History => Key::TaskHistory,
        Task::Details => Key::TaskDetails,
        Task::Resources => Key::TaskResources,
        Task::Audio => Key::TaskAudio,
        Task::CreateWork => Key::NewWork,
        Task::CreateUnit => Key::NewUnit,
    }
}

fn focus_workspace_field(id: &'static str) {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
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
