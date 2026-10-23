//! One owner-bound request authority; the form dispatches intents and never performs I/O.
use super::*;
use crate::contract::*;
use crate::live::{
    workspace::{uuid, Dispatch},
    ClientError, Command, Phase, Reply, RequestState,
};
use leptos::{prelude::*, task::spawn_local};
use web_sys::AbortController;

fn forget_urls(model: &Projection) {
    if let Some(file) = &model.local {
        platform::revoke(&file.url);
    }
    if let Some(url) = &model.current_url {
        platform::revoke(url);
    }
}
#[component]
pub fn ArtworkPanel(
    actor: Signal<String>,
    work: Signal<Option<CatalogWork>>,
    english: Signal<bool>,
    session_pending: Signal<bool>,
    session_epoch: Signal<u64>,
    workspace_busy: Signal<bool>,
    unresolved: RwSignal<bool>,
    busy: RwSignal<bool>,
    dirty: RwSignal<bool>,
    on_revision: Callback<u64>,
) -> impl IntoView {
    let state = RwSignal::new(RequestState::default());
    let model = RwSignal::new(Projection::default());
    let draft = RwSignal::new(Draft::default());
    let validating = RwSignal::new(false);
    let file_generation = RwSignal::new(0u64);
    let submitted_selection = StoredValue::new(None::<(String, Draft, u64)>);
    let preview_generation = RwSignal::new(0u64);
    let preview_abort = StoredValue::new_local(None::<AbortController>);
    let clear_open = RwSignal::new(false);
    let preview_nonce = RwSignal::new(0u64);
    let api = expect_context::<crate::api::StudioContext>().api;
    let work_id =
        Memo::new(move |_| work.with(|w| w.as_ref().map(|w| w.id.clone()).unwrap_or_default()));
    let owner = Signal::derive(move || {
        !actor.get().is_empty()
            && work.with(|w| {
                w.as_ref()
                    .is_some_and(|w| w.owner_id == actor.get() && w.lifecycle != "archived")
            })
    });
    let dispatch = Dispatch {
        state,
        session_pending,
        abort: StoredValue::new_local(None::<AbortController>),
        api,
        progress: Some(Callback::new(move |progress: platform::Transfer| {
            let _ = model.try_update(|m| {
                m.transferred = Some(progress.bytes);
                m.stage = if progress.complete {
                    Stage::AwaitingAcknowledgement
                } else {
                    Stage::Uploading
                };
            });
        })),
        complete: Callback::new(
            move |(command, result): (Command, Result<Reply, ClientError>)| {
                match result {
                    Ok(reply) => {
                        let _ = model.try_update(|m| *m = m.apply(&reply));
                        if let Reply::Artwork(art) = &reply {
                            let _ = preview_nonce.try_update(|n| *n = n.saturating_add(1));
                            on_revision.run(art.work_revision);
                            if matches!(command, Command::SetArtwork { .. }) {
                                let _ = model.try_update(|m| {
                                    m.compared_revision = None;
                                    m.reported_revision = None;
                                });
                            }
                            if let Command::SetArtwork { request, .. } = &command {
                                if request.artwork.is_some() {
                                    let accepted = submitted_selection
                                        .try_update_value(|snapshot| {
                                            if snapshot.as_ref().is_some_and(|(operation, _, _)| {
                                                operation == &request.operation_id
                                            }) {
                                                snapshot.take()
                                            } else {
                                                None
                                            }
                                        })
                                        .flatten();
                                    if let Some((_, submitted, generation)) = accepted {
                                        let cleared = draft
                                            .try_update(|current| {
                                                current.acknowledge_selection(
                                                    &submitted,
                                                    file_generation.get_untracked() == generation,
                                                )
                                            })
                                            .unwrap_or(false);
                                        if cleared {
                                            let _ = model.try_update(|m| {
                                                if let Some(local) = m.discard_local() {
                                                    platform::revoke(&local.url);
                                                }
                                            });
                                            platform::reset_file();
                                        }
                                    }
                                }
                                let _ = model.try_update(|m| m.stage = Stage::Saved);
                            }
                        }
                    }
                    Err(error) => {
                        let unknown = state.with_untracked(|s| s.phase() == Phase::Ambiguous);
                        let _ = model.try_update(|m| {
                            m.transferred = None;
                            m.stage = if unknown {
                                Stage::Unknown
                            } else {
                                match &error {
                                    ClientError::Http {
                                        status: 401 | 403 | 404,
                                        ..
                                    } => Stage::Forbidden,
                                    ClientError::Http { error, .. }
                                        if error.code == ErrorCode::StaleRevision =>
                                    {
                                        if let Command::SetArtwork { request, .. } = &command {
                                            m.compared_revision =
                                                Some(request.expected_work_revision);
                                            m.reported_revision = error.current_revision;
                                        }
                                        Stage::Conflict
                                    }
                                    ClientError::Http { status: 409, error }
                                        if error
                                            .issues
                                            .iter()
                                            .any(|issue| issue.rule == "intent_expired") =>
                                    {
                                        m.intent_state =
                                            m.intent.as_ref().map(|_| ImageIntentState::Expired);
                                        Stage::Expired
                                    }
                                    _ => Stage::Failed,
                                }
                            };
                        });
                    }
                }
                platform::focus_status();
            },
        ),
    };
    let blocked = Signal::derive(move || {
        session_pending.get()
            || !owner.get()
            || workspace_busy.get()
            || validating.get()
            || state.with(|s| s.pending().is_some())
    });
    Effect::new(move |_| {
        if session_pending.get() {
            clear_open.set(false);
        }
        let actor = actor.get();
        let work = work_id.get();
        let epoch = session_epoch.get();
        let target = format!("{work}:{epoch}");
        if state.with_untracked(|s| s.actor() != actor || s.target() != target) {
            let changed = state.with_untracked(|s| {
                s.actor() != actor || !s.target().starts_with(&format!("{work}:"))
            });
            dispatch.abort();
            state.update(|s| {
                if let Some(next) = s.context(&actor, &target) {
                    *s = next;
                }
            });
            if changed {
                forget_urls(&model.get_untracked());
                model.set(Projection::default());
                draft.set(Draft::default());
                submitted_selection.set_value(None);
                file_generation.update(|g| *g = g.saturating_add(1));
                validating.set(false);
                clear_open.set(false);
                platform::reset_file();
            }
        }
        if owner.get()
            && !session_pending.get()
            && !work.is_empty()
            && state.with(|s| s.phase() == Phase::Idle)
            && model.with_untracked(|m| m.current.is_none())
        {
            dispatch.send(Command::Artwork { work });
        }
    });
    Effect::new(move |_| {
        let phase = state.with(RequestState::phase);
        if phase == Phase::Pending {
            let stage = state.with(|s| match s.pending() {
                Some(Command::PrepareImage { .. }) => Stage::Preparing,
                Some(Command::UploadImage { .. }) => Stage::Uploading,
                Some(Command::SetArtwork { .. }) => Stage::AwaitingAcknowledgement,
                _ => Stage::Loading,
            });
            let operation = state.with(|s| match s.pending() {
                Some(Command::PrepareImage { request, .. }) => Some(request.operation_id.clone()),
                Some(Command::UploadImage { intent, .. }) => Some(intent.operation_id.clone()),
                Some(Command::SetArtwork { request, .. }) => Some(request.operation_id.clone()),
                _ => None,
            });
            model.update(|m| m.start_request(stage, operation));
        } else if phase == Phase::Ambiguous {
            model.update(|m| m.stage = Stage::Unknown);
        }
    });
    let has_local = Memo::new(move |_| model.with(|m| m.local.is_some()));
    let edited = Memo::new(move |_| draft.with(|d| *d != Draft::default()));
    Effect::new(move |_| {
        let phase = state.with(RequestState::phase);
        let local = has_local.get();
        let edited = edited.get();
        busy.set(validating.get() || phase == Phase::Pending);
        unresolved
            .set(validating.get() || state.with(|s| s.pending().is_some()) || local || edited);
        dirty.set(draft.with(|draft| draft.is_dirty(local)));
    });
    // Private previews use a different read authority so they cannot overwrite a mutation result.
    let current_cover = Memo::new(move |_| model.with(|m| m.current.clone()));
    Effect::new(move |_| {
        let _nonce = preview_nonce.get();
        let current = current_cover.get();
        let pending = session_pending.get();
        let epoch = session_epoch.get();
        let owner = owner.get();
        let generation = preview_generation.get_untracked().saturating_add(1);
        preview_generation.set(generation);
        preview_abort.update_value(|c| {
            if let Some(c) = c.take() {
                c.abort();
            }
        });
        if let Some(url) = model.write().take_current_preview() {
            platform::revoke(&url);
        }
        let Some((current, selection)) = current.and_then(|c| c.artwork.clone().map(|s| (c, s)))
        else {
            return;
        };
        if pending || !owner {
            return;
        }
        let Ok(controller) = AbortController::new() else {
            return;
        };
        preview_abort.set_value(Some(controller.clone()));
        let snapshot = api.snapshot();
        let Some(fence) = snapshot.fence() else {
            return;
        };
        spawn_local(async move {
            let result = platform::preview(&current.work_id, &selection, controller, fence).await;
            if preview_generation.try_get_untracked() != Some(generation)
                || session_epoch.try_get_untracked() != Some(epoch)
            {
                if let Ok((url, _)) = result {
                    platform::revoke(&url);
                }
                return;
            }
            match result {
                Ok((url, metadata)) => {
                    let _ = model.try_update(|m| {
                        m.current_url = Some(url);
                        m.current_media = Some(metadata);
                    });
                }
                Err(ClientError::SessionChanged) => {
                    snapshot.invalidate_projection(&crate::session::unconfirmed_response())
                }
                Err(ClientError::Http { status: 401, error }) => {
                    let _ = snapshot.notify_denial(Err::<(), _>(error));
                }
                Err(_) => {}
            }
        });
    });
    let on_file = Callback::new(move |event| {
        if blocked.get_untracked() {
            return;
        }
        let Some(file) = platform::selected(event) else {
            return;
        };
        let generation = file_generation.get_untracked().saturating_add(1);
        file_generation.set(generation);
        let actor_snapshot = actor.get_untracked();
        let work_snapshot = work_id.get_untracked();
        validating.set(true);
        model.update(|m| {
            m.stage = Stage::Validating;
        });
        spawn_local(async move {
            let result = platform::read(file).await;
            if file_generation.try_get_untracked() != Some(generation)
                || actor.try_get_untracked() != Some(actor_snapshot)
                || work_id.try_get_untracked() != Some(work_snapshot)
            {
                if let Ok(file) = result {
                    platform::revoke(&file.url);
                }
                return;
            }
            let _ = validating.try_set(false);
            match result {
                Ok(file) => {
                    let _ = model.try_update(|m| {
                        if let Some(old) = m.replace_local(file) {
                            platform::revoke(&old.url);
                        }
                    });
                }
                Err(problem) => {
                    let _ = model.try_update(|m| {
                        m.problem = Some(problem);
                        m.stage = Stage::Failed;
                    });
                }
            }
        });
    });
    let on_intent = Callback::new(move |intent| {
        if intent == Intent::CancelClear {
            clear_open.set(false);
            return;
        }
        if session_pending.get_untracked() || !owner.get_untracked() {
            return;
        }
        if matches!(intent, Intent::Reconcile | Intent::Replay) {
            if intent == Intent::Reconcile && state.with_untracked(RequestState::can_reconcile) {
                dispatch.reconcile();
            } else if state.with_untracked(RequestState::can_retry) {
                dispatch.retry();
            }
            return;
        }
        if blocked.get_untracked() {
            return;
        }
        let work = work_id.get_untracked();
        let result = (|| -> Result<Option<Command>, Problem> {
            let m = model.get_untracked();
            let d = draft.get_untracked();
            if matches!(intent, Intent::Prepare | Intent::Upload) && !m.file_ready() {
                return Err(m.problem.unwrap_or(Problem::File));
            }
            Ok(match intent {
                Intent::Refresh => Some(Command::Artwork { work }),
                Intent::Prepare => {
                    if !m.can_prepare() {
                        return Ok(None);
                    }
                    let Some(file) = m.local else {
                        return Ok(None);
                    };
                    let request = file.prepare(
                        &d,
                        uuid().map_err(|_| Problem::File)?,
                        uuid().map_err(|_| Problem::File)?,
                    )?;
                    Some(Command::PrepareImage { work, request })
                }
                Intent::Upload => {
                    if !m.can_upload() {
                        return Ok(None);
                    }
                    let (Some(file), Some(receipt)) = (m.local, m.intent) else {
                        return Ok(None);
                    };
                    if file.sha256 != receipt.request.expected_sha256
                        || file.mime != receipt.request.content_type
                        || file.bytes.len() as u64 != receipt.request.expected_byte_length
                    {
                        return Err(Problem::File);
                    }
                    Some(Command::UploadImage {
                        work,
                        intent: receipt,
                        bytes: file.bytes,
                    })
                }
                Intent::Select => {
                    let (Some(asset), Some(current)) = (m.asset, m.current) else {
                        return Ok(None);
                    };
                    let request = SetWorkArtworkRequest {
                        operation_id: uuid().map_err(|_| Problem::File)?,
                        expected_work_revision: current.work_revision,
                        artwork: Some(WorkArtworkSelection {
                            image_asset_id: asset.id,
                            image_asset_digest: asset.digest,
                            alt_text: d.alt()?,
                        }),
                    };
                    submitted_selection.set_value(Some((
                        request.operation_id.clone(),
                        d,
                        file_generation.get_untracked(),
                    )));
                    Some(Command::SetArtwork { work, request })
                }
                Intent::Clear => {
                    clear_open.set(true);
                    None
                }
                Intent::ConfirmClear => {
                    if !clear_open.get_untracked() {
                        return Ok(None);
                    }
                    clear_open.set(false);
                    let Some(current) = m.current else {
                        return Ok(None);
                    };
                    Some(Command::SetArtwork {
                        work,
                        request: SetWorkArtworkRequest {
                            operation_id: uuid().map_err(|_| Problem::File)?,
                            expected_work_revision: current.work_revision,
                            artwork: None,
                        },
                    })
                }
                Intent::DiscardLocal => {
                    model.update(|m| {
                        if let Some(file) = m.discard_local() {
                            platform::revoke(&file.url);
                        }
                    });
                    draft.set(Draft::default());
                    platform::reset_file();
                    None
                }
                Intent::CancelClear | Intent::Reconcile | Intent::Replay => None,
            })
        })();
        match result {
            Ok(Some(command)) => dispatch.send(command),
            Ok(None) => {}
            Err(problem) => {
                model.update(|m| m.report_form_problem(problem));
            }
        }
    });
    on_cleanup(move || {
        dispatch.abort();
        preview_abort.update_value(|c| {
            if let Some(c) = c.take() {
                c.abort();
            }
        });
        if let Some(m) = model.try_get_untracked() {
            forget_urls(&m);
        }
    });
    view! {<Show when=move ||work.with(|w|w.is_some())><super::form::ArtworkForm model=Signal::derive(move ||model.get()) draft=draft english=english owner=owner blocked=blocked on_intent=on_intent on_file=on_file clear_open=clear_open
    can_reconcile=Signal::derive(move ||!session_pending.get()&&!workspace_busy.get()&&state.with(RequestState::can_reconcile))
    can_replay=Signal::derive(move ||!session_pending.get()&&!workspace_busy.get()&&state.with(RequestState::can_retry))/></Show>}
}
