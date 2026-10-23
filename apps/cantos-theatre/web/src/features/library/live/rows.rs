//! Section-scoped identities and reactive contents. Linear lookups deliberately retain the
//! existing read model: keyed DOM identity does not imply constant-time selector work.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AudioRowUi {
    audio: AudioIdentity,
    position: Option<u64>,
    completion: Option<Completion>,
    resume: ResumeStatus,
    presentation: Option<AudioPresentation>,
    feedback: ResumeAttemptState,
    command: ProgressCommandPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SavedRowUi {
    work_id: String,
    presentation: Option<WorkPresentation>,
    command: SavedCommandPhase,
}

pub(super) fn membership(state: &AccountViewState, section: Section) -> Vec<String> {
    let Some(projection) = state.library() else {
        return Vec::new();
    };
    match section {
        Section::Continue => projection
            .continuing
            .iter()
            .map(|i| i.progress.audio.work_id.clone())
            .collect(),
        Section::Saved => projection
            .saved
            .iter()
            .map(|i| i.saved.work_id.clone())
            .collect(),
        Section::History => projection
            .history
            .iter()
            .map(|i| i.audio.work_id.clone())
            .collect(),
    }
}

pub(super) fn audio_ui(
    state: &AccountViewState,
    section: Section,
    work_id: &str,
) -> Option<AudioRowUi> {
    let projection = state.library()?;
    let (audio, position, completion, resume, presentation) = match section {
        Section::Continue => {
            let item = projection
                .continuing
                .iter()
                .find(|i| i.progress.audio.work_id == work_id)?;
            (
                &item.progress.audio,
                Some(item.progress.position_ms),
                Some(item.progress.completion),
                item.resume,
                &item.presentation,
            )
        }
        Section::History => {
            let item = projection
                .history
                .iter()
                .find(|i| i.audio.work_id == work_id)?;
            (
                &item.audio,
                item.progress.as_ref().map(|p| p.position_ms),
                item.progress.as_ref().map(|p| p.completion),
                item.resume,
                &item.presentation,
            )
        }
        Section::Saved => return None,
    };
    let sync = state.sync();
    let feedback = sync
        .map(|s| s.resume_attempt())
        .filter(|attempt| match attempt {
            ResumeAttemptState::Restored { publication_id, .. }
            | ResumeAttemptState::Refused { publication_id, .. } => {
                publication_id == &audio.publication_id
            }
            _ => false,
        })
        .cloned()
        .unwrap_or_default();
    Some(AudioRowUi {
        audio: audio.clone(),
        position,
        completion,
        resume,
        presentation: presentation.clone(),
        feedback,
        command: sync
            .and_then(|s| s.progress_for(&audio.publication_id))
            .map(|p| p.command.clone())
            .unwrap_or_default(),
    })
}

pub(super) fn saved_ui(state: &AccountViewState, work_id: &str) -> Option<SavedRowUi> {
    let item = state
        .library()?
        .saved
        .iter()
        .find(|i| i.saved.work_id == work_id)?;
    Some(SavedRowUi {
        work_id: item.saved.work_id.clone(),
        presentation: item.presentation.clone(),
        command: state
            .sync()
            .and_then(|s| s.saved_work(work_id))
            .map(|s| s.command.clone())
            .unwrap_or_default(),
    })
}

#[component]
fn RowTitle(
    presentation: Signal<Option<WorkPresentation>>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let work = Memo::new(move |_| {
        presentation
            .get()
            .and_then(|w| PublicWorkId::parse(&w.work_id).map(|id| (w, id)))
    });
    view! {
        <h3 class="theatre-row-label" lang=move || work.with(|w| w.as_ref().map(|(w, _)| w.language.clone()))>
            <Show when=move || work.with(Option::is_some) fallback=move || messages::text(Key::LiveLibraryMetadataUnavailable, locale.get())>
                <a href=move || work.with(|w| w.as_ref().map(|(_, id)| Route::PublicWork(*id).hash()))>
                    {move || work.with(|w| w.as_ref().map(|(w, _)| w.title.clone()).unwrap_or_default())}
                </a>
            </Show>
        </h3>
    }
}

fn position_text(row: &AudioRowUi, locale: Locale) -> String {
    row.position.map_or_else(
        || messages::text(Key::LiveLibraryNoPosition, locale).to_owned(),
        |position| {
            let base = messages::format(
                Key::LivePosition,
                locale,
                &[
                    ("position", clock(position)),
                    ("total", clock(row.audio.duration_ms)),
                ],
            );
            if row.completion == Some(Completion::Completed) {
                format!(
                    "{base} · {}",
                    messages::text(Key::LiveResumeCompleted, locale)
                )
            } else {
                base
            }
        },
    )
}

fn resume_feedback(row: &AudioRowUi, locale: Locale) -> Option<String> {
    match row.feedback {
        ResumeAttemptState::Restored { position_ms, .. } => Some(messages::format(
            Key::LiveResumeRestored,
            locale,
            &[("time", clock(u64::from(position_ms)))],
        )),
        ResumeAttemptState::Refused { reason, .. } => Some(
            messages::text(
                match reason {
                    ResumeRefusal::ReleaseReplaced => Key::LiveResumeReplaced,
                    ResumeRefusal::AccessUnavailable => Key::LiveResumeUnavailable,
                    ResumeRefusal::LoadFailed | ResumeRefusal::Protocol => {
                        Key::LiveResumeLoadFailed
                    }
                },
                locale,
            )
            .to_owned(),
        ),
        _ => None,
    }
}

/// Read at activation time, including after replacement/absence but before DOM reconciliation.
fn resume_current(row: Memo<Option<AudioRowUi>>, act: LibraryActions) {
    if let Some(row) = row
        .get_untracked()
        .filter(|r| r.resume == ResumeStatus::ExactAvailable)
    {
        act.on_resume.run((
            row.audio,
            row.position.unwrap_or(0),
            row.resume,
            row.presentation,
        ));
    }
}

#[component]
fn AudioRow(
    state: ReadSignal<AccountViewState>,
    work_id: String,
    section: Section,
    actions: Option<LibraryActions>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let row = Memo::new(move |_| state.with(|s| audio_ui(s, section, &work_id)));
    let presentation = Signal::derive(move || {
        row.with(|r| {
            r.as_ref()
                .and_then(|r| r.presentation.as_ref().map(|p| p.work.clone()))
        })
    });
    let has_unit =
        Memo::new(move |_| row.with(|r| r.as_ref().is_some_and(|r| r.presentation.is_some())));
    // Button IDs contain the publication. Replace only that control when the release changes;
    // the Work-keyed row and its other nodes remain mounted. The shared Button ID is static.
    let releases = Memo::new(move |_| {
        row.with(|r| {
            r.as_ref()
                .filter(|r| r.resume == ResumeStatus::ExactAvailable)
                .map(|r| vec![r.audio.publication_id.clone()])
                .unwrap_or_default()
        })
    });
    let command = Memo::new(move |_| {
        row.with(|r| {
            r.as_ref()
                .map(|r| (r.audio.publication_id.clone(), r.command.clone()))
        })
    });
    let feedback =
        Memo::new(move |_| row.with(|r| r.as_ref().and_then(|r| resume_feedback(r, locale.get()))));
    // The command panel may change shape. Its actions still resolve this row's current release.
    let guarded = actions.map(|act| LibraryActions {
        on_resolve_progress: Callback::new(move |(_, here)| {
            if let Some(r) = row.get_untracked().filter(|r| {
                matches!(
                    r.command,
                    ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses { .. })
                )
            }) {
                act.on_resolve_progress.run((r.audio.publication_id, here));
            }
        }),
        on_retry_progress: Callback::new(move |_| {
            if let Some(r) = row.get_untracked().filter(|r| {
                matches!(
                    r.command,
                    ProgressCommandPhase::UnknownOutcome { .. }
                        | ProgressCommandPhase::Failed { .. }
                )
            }) {
                act.on_retry_progress.run(r.audio.publication_id);
            }
        }),
        ..act
    });
    view! {
        <Show when=move || row.with(Option::is_some)>
            <li class="theatre-row" data-publication-id=move || row.with(|r| r.as_ref().map(|r| r.audio.publication_id.clone()))>
                <div class="theatre-row-copy">
                    <RowTitle presentation locale/>
                    <Show when=move || has_unit.get()>
                        <p lang=move || row.with(|r| r.as_ref().and_then(|r| r.presentation.as_ref().map(|p| p.work.language.clone())))>
                            {move || row.with(|r| r.as_ref().and_then(|r| r.presentation.as_ref().map(|p| p.unit_title.clone())).unwrap_or_default())}
                        </p>
                    </Show>
                    <p>{move || row.with(|r| r.as_ref().map(|r| position_text(r, locale.get())).unwrap_or_default())}</p>
                    <p class="theatre-muted">{move || row.with(|r| r.as_ref().map(|r| messages::text(availability(r.resume), locale.get())))}</p>
                    <Show when=move || feedback.with(Option::is_some)>
                        <p class="theatre-muted" role="status">{move || feedback.get().unwrap_or_default()}</p>
                    </Show>
                    {move || command.get().map(|(pub_id, progress_command)| progress_feedback(section, pub_id, progress_command, guarded, locale.get()))}
                </div>
                {actions.map(|act| view! {
                    <For each=move || releases.get() key=|id| id.clone() children=move |publication_id| view! {
                        <div class="theatre-row-actions">
                            <Button id=format!("theatre-library-{}-resume-{}", section.name(), publication_id) variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |_| resume_current(row, act))>
                                {move || messages::text(Key::LiveResumeOpen, locale.get())}
                                <Show when=move || has_unit.get()>
                                    <span class="cui-sr-only">" "{move || row.with(|r| r.as_ref().and_then(|r| r.presentation.as_ref().map(|p| p.unit_title.clone())).unwrap_or_default())}</span>
                                </Show>
                            </Button>
                        </div>
                    }/>
                })}
            </li>
        </Show>
    }
}

fn saved_status(command: &SavedCommandPhase) -> Option<Key> {
    match command {
        SavedCommandPhase::UnknownOutcome { .. } => Some(Key::LiveSaveUnknown),
        SavedCommandPhase::Conflict { .. } => Some(Key::LiveSaveConflict),
        SavedCommandPhase::Failed { .. } => Some(Key::LiveSaveError),
        SavedCommandPhase::Idle | SavedCommandPhase::Pending { .. } => None,
    }
}

#[component]
fn SavedRow(
    state: ReadSignal<AccountViewState>,
    work_id: String,
    actions: Option<LibraryActions>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let id = StoredValue::new(work_id.clone());
    let row = Memo::new(move |_| state.with(|s| saved_ui(s, &work_id)));
    let presentation =
        Signal::derive(move || row.with(|r| r.as_ref().and_then(|r| r.presentation.clone())));
    let status =
        Memo::new(move |_| row.with(|r| r.as_ref().and_then(|r| saved_status(&r.command))));
    let btn_state = Signal::derive(move || {
        if row.with(|r| {
            r.as_ref()
                .is_some_and(|r| matches!(r.command, SavedCommandPhase::Pending { .. }))
        }) {
            DisabledReason::new(messages::text(Key::LiveSaveRemoving, locale.get()))
                .map(ButtonState::Loading)
                .unwrap_or_default()
        } else {
            ButtonState::Enabled
        }
    });
    view! {
        <Show when=move || row.with(Option::is_some)>
            <li class="theatre-row" data-work-id=id.get_value()>
                <div class="theatre-row-copy">
                    <RowTitle presentation locale/>
                    <Show when=move || status.with(Option::is_some)>
                        <p class="theatre-muted" role="status">{move || status.get().map(|key| messages::text(key, locale.get()))}</p>
                        {actions.map(|act| view! {
                            <Button id=format!("theatre-library-saved-retry-{}", id.get_value()) variant=ButtonVariant::Text
                                on_press=Callback::new(move |_| {
                                    if let Some(r) = row.get_untracked().filter(|r| saved_status(&r.command).is_some()) { act.on_retry_saved.run(r.work_id); }
                                })>
                                {move || messages::text(Key::PublicRetry, locale.get())}
                            </Button>
                        })}
                    </Show>
                </div>
                {actions.map(|act| view! {
                    <div class="theatre-row-actions">
                        <Button id=format!("theatre-library-saved-unsave-{}", id.get_value()) variant=ButtonVariant::Text state=btn_state
                            on_press=Callback::new(move |_| {
                                if let Some(r) = row.get_untracked().filter(|r| !matches!(r.command, SavedCommandPhase::Pending { .. })) { act.on_unsave.run((r.work_id, r.presentation)); }
                            })>
                            {move || messages::text(Key::RemoveSave, locale.get())}
                            {move || presentation.get().map(|w| view! { <span class="cui-sr-only">" "{w.title}</span> })}
                        </Button>
                    </div>
                })}
            </li>
        </Show>
    }
}

#[component]
pub(super) fn LiveRow(
    state: ReadSignal<AccountViewState>,
    work_id: String,
    section: Section,
    actions: Option<LibraryActions>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    match section {
        Section::Saved => view! { <SavedRow state work_id actions locale/> }.into_any(),
        Section::Continue | Section::History => {
            view! { <AudioRow state work_id section actions locale/> }.into_any()
        }
    }
}

fn progress_feedback(
    section: Section,
    pub_id: String,
    progress_command: ProgressCommandPhase,
    actions: Option<LibraryActions>,
    locale: Locale,
) -> Option<AnyView> {
    match progress_command {
        ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses {
            local_position_ms,
            server,
            ..
        }) => {
            let prompt = messages::format(
                Key::LiveProgressConflictPrompt,
                locale,
                &[
                    ("local", clock(local_position_ms)),
                    ("server", clock(server.position_ms)),
                ],
            );
            let keep_here = messages::format(
                Key::LiveProgressResolveHere,
                locale,
                &[("time", clock(local_position_ms))],
            );
            let keep_other = messages::format(
                Key::LiveProgressResolveOther,
                locale,
                &[("time", clock(server.position_ms))],
            );
            let pub_here = pub_id.clone();
            let pub_other = pub_id.clone();
            Some(view! {
                            <div class="theatre-conflict-panel" role="status">
                                <p>{prompt}</p>
                                {actions.map(|act| view! {
                                    <div class="theatre-actions">
                                        <Button id=format!("theatre-library-{}-resolve-here-{}", section.name(), pub_here)
                                            variant=ButtonVariant::Tonal
                                            on_press=Callback::new(move |_| act.on_resolve_progress.run((pub_here.clone(), true)))>
                                            {keep_here}
                                        </Button>
                                        <Button id=format!("theatre-library-{}-resolve-other-{}", section.name(), pub_other)
                                            variant=ButtonVariant::Text
                                            on_press=Callback::new(move |_| act.on_resolve_progress.run((pub_other.clone(), false)))>
                                            {keep_other}
                                        </Button>
                                    </div>
                                })}
                            </div>
                        }.into_any())
        }
        ProgressCommandPhase::Conflict(ProgressConflict::KeepServer { server, .. }) => {
            let msg = messages::format(
                Key::LiveProgressKeepServer,
                locale,
                &[("time", clock(server.position_ms))],
            );
            Some(view! { <p class="theatre-muted" role="status">{msg}</p> }.into_any())
        }
        ProgressCommandPhase::UnknownOutcome { .. } => {
            let pub_retry = pub_id.clone();
            Some(view! {
                            <p class="theatre-muted" role="status">{messages::text(Key::LiveProgressUnknown, locale)}</p>
                            {actions.map(|act| view! {
                                <Button id=format!("theatre-library-{}-progress-retry-{}", section.name(), pub_retry)
                                    variant=ButtonVariant::Text
                                    on_press=Callback::new(move |_| act.on_retry_progress.run(pub_retry.clone()))>
                                    {messages::text(Key::PublicRetry, locale)}
                                </Button>
                            })}
                        }.into_any())
        }
        ProgressCommandPhase::Failed { .. } => {
            let pub_retry = pub_id.clone();
            Some(view! {
                            <p class="theatre-muted" role="status">{messages::text(Key::LiveProgressError, locale)}</p>
                            {actions.map(|act| view! {
                                <Button id=format!("theatre-library-{}-progress-retry-{}", section.name(), pub_retry)
                                    variant=ButtonVariant::Text
                                    on_press=Callback::new(move |_| act.on_retry_progress.run(pub_retry.clone()))>
                                    {messages::text(Key::PublicRetry, locale)}
                                </Button>
                            })}
                        }.into_any())
        }
        ProgressCommandPhase::Idle | ProgressCommandPhase::Pending { .. } => None,
    }
}
#[cfg(test)]
mod tests;
