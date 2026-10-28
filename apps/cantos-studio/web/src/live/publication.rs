//! Studio publication composition: server readiness, immutable decisions and explicit pointer reads.
//! This consumer never grants publication authority or infers current state from a replayed receipt.
use crate::contract::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicationFacts {
    pub readiness: Option<PublicationReadinessResponse>,
    pub clearance_receipt: Option<DistributionClearanceDocument>,
    pub approval_receipt: Option<PublicationApprovalDocument>,
    pub publish_receipt: Option<ActiveAudioReleaseResponse>,
    pub withdrawal_receipt: Option<AudioReleaseWithdrawalResponse>,
    pub active: Option<ActiveAudioReleaseResponse>,
    /// Original command identity supplied by the shared request owner, including receipts without an operation echo.
    pub form_ack: Option<String>,
}

/// The exact subject shown in confirmations, independent of mutable form fields or history selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationSubject {
    pub unit: String,
    pub title: String,
    pub candidate: String,
    pub digest: String,
    pub binding: AudioBinding,
    pub rendition_digest: String,
    pub sha256: String,
}
impl PublicationSubject {
    pub fn from_facts(facts: &super::Facts) -> Option<Self> {
        let unit = facts.unit.as_ref()?;
        let candidate = &facts.candidate.as_ref()?.candidate;
        if candidate.binding.unit_id != unit.id
            || candidate.binding.work_id != unit.work_id
            || facts
                .work
                .as_ref()
                .is_some_and(|work| work.id != unit.work_id)
        {
            return None;
        }
        Some(Self {
            unit: unit.id.clone(),
            title: unit.details.title.clone(),
            candidate: candidate.id.clone(),
            digest: candidate.digest.clone(),
            binding: candidate.binding.clone(),
            rendition_digest: candidate.rendition_digest.clone(),
            sha256: candidate.sha256.clone(),
        })
    }
    pub fn readiness<'a>(
        &self,
        facts: &'a PublicationFacts,
    ) -> Option<&'a PublicationReadinessResponse> {
        facts
            .readiness
            .as_ref()
            .filter(|r| r.candidate_id == self.candidate && r.candidate_digest == self.digest)
    }
    pub fn publish_request(
        &self,
        readiness: &PublicationReadinessResponse,
        operation_id: String,
    ) -> Option<PublishAudioReleaseRequest> {
        if readiness.candidate_id != self.candidate
            || readiness.candidate_digest != self.digest
            || !readiness.blockers.is_empty()
        {
            return None;
        }
        Some(PublishAudioReleaseRequest {
            operation_id,
            candidate_digest: self.digest.clone(),
            expected_pointer_version: readiness.pointer_version,
            expected_active_release_id: readiness.active_release_id.clone(),
        })
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RightsForm {
    decision: String,
    rights: String,
    evidence: String,
    until: String,
    notes: String,
}
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ApprovalForm {
    decision: String,
    notes: String,
}
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PublicationForm {
    rights: RightsForm,
    approval: ApprovalForm,
    withdrawal: String,
    visibility: String,
}
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum SubmittedForm {
    Rights(String, RightsForm),
    Approval(String, ApprovalForm),
    Withdrawal(String, String),
    Visibility {
        operation: String,
        work: String,
        value: String,
    },
}
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FormLedger {
    saved: PublicationForm,
    submitted: Option<SubmittedForm>,
}
#[cfg(any(target_arch = "wasm32", test))]
impl FormLedger {
    fn dirty(&self, form: &PublicationForm) -> bool {
        self.saved != *form
    }
    /// Only the actual submitted section is acknowledged. Later typing and other sections survive.
    fn acknowledge(&self, operation: &str, reply: &super::Reply) -> Option<Self> {
        let mut next = self.clone();
        match (self.submitted.as_ref()?, reply) {
            (SubmittedForm::Rights(id, form), super::Reply::DistributionRights(receipt))
                if operation == id && receipt.request.operation_id == *id =>
            {
                next.saved.rights = form.clone()
            }
            (SubmittedForm::Approval(id, form), super::Reply::PublicationApproval(receipt))
                if operation == id && receipt.request.operation_id == *id =>
            {
                next.saved.approval = form.clone()
            }
            // Withdrawal transport intentionally does not echo the operation. The shared one-flight
            // request owner has already correlated the release/pointer to its immutable command.
            (SubmittedForm::Withdrawal(id, form), super::Reply::Withdrawal(_))
                if operation == id =>
            {
                next.saved.withdrawal = form.clone()
            }
            (
                SubmittedForm::Visibility {
                    operation: id,
                    work,
                    value,
                },
                super::Reply::Mutation(receipt),
            ) if operation == id
                && receipt.work.id == *work
                && receipt.work.details.visibility == *value =>
            {
                next.saved.visibility = value.clone();
            }
            _ => return None,
        }
        next.submitted = None;
        Some(next)
    }
}
#[cfg(any(target_arch = "wasm32", test))]
fn refs(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
#[cfg(any(target_arch = "wasm32", test))]
impl RightsForm {
    fn request(
        &self,
        subject: &PublicationSubject,
        operation_id: String,
    ) -> Option<RecordDistributionRightsRequest> {
        Some(RecordDistributionRightsRequest {
            operation_id,
            candidate_digest: subject.digest.clone(),
            decision: match self.decision.as_str() {
                "cleared" => DistributionDecision::Cleared,
                "revoked" => DistributionDecision::Revoked,
                _ => return None,
            },
            rights_refs: refs(&self.rights),
            valid_until: self.until.trim().parse().ok()?,
            evidence_refs: refs(&self.evidence),
            notes: self.notes.clone(),
        })
    }
}
#[cfg(any(target_arch = "wasm32", test))]
impl ApprovalForm {
    fn request(
        &self,
        subject: &PublicationSubject,
        clearance: &DistributionClearanceDocument,
        operation_id: String,
    ) -> Option<RecordPublicationApprovalRequest> {
        if clearance.candidate_id != subject.candidate
            || clearance.request.candidate_digest != subject.digest
        {
            return None;
        }
        Some(RecordPublicationApprovalRequest {
            operation_id,
            candidate_digest: subject.digest.clone(),
            clearance_digest: clearance.digest.clone(),
            decision: match self.decision.as_str() {
                "approved" => PublicationApprovalDecision::Approved,
                "withdrawn" => PublicationApprovalDecision::Withdrawn,
                _ => return None,
            },
            notes: self.notes.clone(),
        })
    }
}

/// A visibility edit is a distinct, revision-guarded Catalog operation, never part of publish.
pub fn visibility_command(
    work: &CatalogWork,
    value: &str,
    operation_id: String,
) -> Option<super::Command> {
    if !matches!(value, "private" | "public") || value == work.details.visibility {
        return None;
    }
    Some(super::Command::PatchWork {
        work: work.id.clone(),
        request: PatchWorkRequest {
            operation_id,
            expected_revision: work.revision,
            details: WorkPatchDto {
                visibility: PatchField::Value(value.to_owned()),
                ..WorkPatchDto::default()
            },
        },
    })
}

#[cfg(target_arch = "wasm32")]
mod panel {
    use super::*;
    use crate::live::{
        copy::{self, text, Key},
        workspace::{uuid, Dispatch},
        ClientError, Command, Facts,
    };
    use crate::ui::{ActionButton, BlockReason, EDITOR_FIELD_DENSITY};
    use cantos_ui::{Select, SelectOption, TextArea, TextField, TextFieldVariant};
    use leptos::prelude::*;

    #[derive(Clone, Copy)]
    struct FormSignals {
        rights_decision: RwSignal<String>,
        rights: RwSignal<String>,
        evidence: RwSignal<String>,
        until: RwSignal<String>,
        rights_notes: RwSignal<String>,
        approval_decision: RwSignal<String>,
        approval_notes: RwSignal<String>,
        reason: RwSignal<String>,
        visibility: RwSignal<String>,
    }
    impl FormSignals {
        fn snapshot(self) -> PublicationForm {
            PublicationForm {
                rights: RightsForm {
                    decision: self.rights_decision.get(),
                    rights: self.rights.get(),
                    evidence: self.evidence.get(),
                    until: self.until.get(),
                    notes: self.rights_notes.get(),
                },
                approval: ApprovalForm {
                    decision: self.approval_decision.get(),
                    notes: self.approval_notes.get(),
                },
                withdrawal: self.reason.get(),
                visibility: self.visibility.get(),
            }
        }
        fn reset(self) {
            for field in [
                self.rights_decision,
                self.rights,
                self.evidence,
                self.until,
                self.rights_notes,
                self.approval_decision,
                self.approval_notes,
                self.reason,
                self.visibility,
            ] {
                field.set(String::new());
            }
        }
    }
    fn identity(
        subject: &PublicationSubject,
        readiness: Option<&PublicationReadinessResponse>,
    ) -> String {
        let mut value = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{} · r{}\n{}\n{}\n{}\n{}",
            subject.binding.work_id,
            subject.unit,
            subject.candidate,
            subject.digest,
            subject.binding.content_revision_id,
            subject.binding.content_revision_digest,
            subject.binding.brief_id,
            subject.binding.brief_version,
            subject.binding.brief_digest,
            subject.binding.profile_id,
            subject.rendition_digest,
            subject.sha256
        );
        if let Some(readiness) = readiness {
            value.push_str(&format!(
                "\n{} · {}",
                readiness.pointer_version,
                readiness.active_release_id.as_deref().unwrap_or("∅")
            ));
        }
        value
    }
    fn confirm(key: Key, title: &str, identity: &str, locale: crate::localization::Locale) -> bool {
        let message = copy::publication_confirmation(key, title, identity, locale);
        web_sys::window()
            .and_then(|window| window.confirm_with_message(&message).ok())
            .unwrap_or(false)
    }
    fn build_confirmed(dispatch: Dispatch, command: Result<Command, ClientError>) {
        // Declining a local confirmation sends nothing and leaves the prior observation intact.
        if !matches!(command, Err(ClientError::Cancelled)) {
            dispatch.build(command);
        }
    }
    fn document<T: serde::Serialize>(value: Option<&T>) -> String {
        value
            .and_then(|value| serde_json::to_string_pretty(value).ok())
            .unwrap_or_default()
    }
    #[component]
    pub(in crate::live) fn PublicationPanel(
        facts: RwSignal<Facts>,
        dispatch: Dispatch,
        locale: Signal<crate::localization::Locale>,
        blocked: Signal<bool>,
        dirty: RwSignal<bool>,
    ) -> impl IntoView {
        let fields = FormSignals {
            rights_decision: RwSignal::new(String::new()),
            rights: RwSignal::new(String::new()),
            evidence: RwSignal::new(String::new()),
            until: RwSignal::new(String::new()),
            rights_notes: RwSignal::new(String::new()),
            approval_decision: RwSignal::new(String::new()),
            approval_notes: RwSignal::new(String::new()),
            reason: RwSignal::new(String::new()),
            visibility: RwSignal::new(String::new()),
        };
        let ledger = RwSignal::new(FormLedger::default());
        let selected = StoredValue::new(String::new());
        Effect::new(move |_| {
            let unit = facts.with(Facts::unit_id);
            if selected.with_value(|selected| selected != &unit) {
                selected.set_value(unit);
                fields.reset();
                ledger.set(FormLedger::default());
            }
        });
        Effect::new(move |_| {
            dirty.set(ledger.with(|ledger| ledger.dirty(&fields.snapshot())));
        });
        Effect::new(move |_| {
            let operation = facts.with(|facts| facts.publication.form_ack.clone());
            let acknowledged = operation.and_then(|operation| {
                dispatch.state.with(|state| {
                    state.result().and_then(|reply| {
                        ledger.with_untracked(|ledger| ledger.acknowledge(&operation, reply))
                    })
                })
            });
            if let Some(next) = acknowledged {
                ledger.set(next);
            }
        });
        on_cleanup(move || {
            let _ = dirty.try_set(false);
        });
        let subject = Signal::derive(move || facts.with(PublicationSubject::from_facts));
        let readiness = Signal::derive(move || {
            subject.with(|subject| {
                subject.as_ref().and_then(|subject| {
                    facts.with(|facts| subject.readiness(&facts.publication).cloned())
                })
            })
        });
        let request_readiness = Callback::new(move |()| {
            if blocked.get_untracked() {
                return;
            }
            if let Some(subject) = subject.get_untracked() {
                facts.update(|facts| facts.publication.readiness = None);
                dispatch.send(Command::Readiness {
                    unit: subject.unit,
                    candidate: subject.candidate,
                    candidate_digest: subject.digest,
                });
            }
        });
        let record_rights = Callback::new(move |()| {
            if blocked.get_untracked() {
                return;
            }
            build_confirmed(
                dispatch,
                (|| {
                    let subject = subject.get_untracked().ok_or(ClientError::InvalidInput)?;
                    let form = untrack(|| fields.snapshot()).rights;
                    let id = uuid()?;
                    let request = form
                        .request(&subject, id.clone())
                        .ok_or(ClientError::InvalidInput)?;
                    if !confirm(
                        Key::RecordRights,
                        &subject.title,
                        &format!("{}\n{}", identity(&subject, None), document(Some(&request))),
                        locale.get_untracked(),
                    ) {
                        return Err(ClientError::Cancelled);
                    }
                    ledger
                        .update(|ledger| ledger.submitted = Some(SubmittedForm::Rights(id, form)));
                    facts.update(|facts| {
                        facts.publication.readiness = None;
                        facts.publication.active = None;
                    });
                    Ok(Command::RecordDistributionRights {
                        unit: subject.unit,
                        candidate: subject.candidate,
                        request,
                    })
                })(),
            );
        });
        let record_approval = Callback::new(move |()| {
            if blocked.get_untracked() {
                return;
            }
            build_confirmed(
                dispatch,
                (|| {
                    let subject = subject.get_untracked().ok_or(ClientError::InvalidInput)?;
                    let readiness = readiness.get_untracked().ok_or(ClientError::InvalidInput)?;
                    let clearance = readiness
                        .clearance
                        .as_ref()
                        .ok_or(ClientError::InvalidInput)?;
                    let form = untrack(|| fields.snapshot()).approval;
                    let id = uuid()?;
                    let request = form
                        .request(&subject, clearance, id.clone())
                        .ok_or(ClientError::InvalidInput)?;
                    if !confirm(
                        Key::RecordApproval,
                        &subject.title,
                        &format!(
                            "{}\n{}",
                            identity(&subject, Some(&readiness)),
                            document(Some(&request))
                        ),
                        locale.get_untracked(),
                    ) {
                        return Err(ClientError::Cancelled);
                    }
                    ledger.update(|ledger| {
                        ledger.submitted = Some(SubmittedForm::Approval(id, form))
                    });
                    facts.update(|facts| {
                        facts.publication.readiness = None;
                        facts.publication.active = None;
                    });
                    Ok(Command::RecordPublicationApproval {
                        unit: subject.unit,
                        candidate: subject.candidate,
                        request,
                    })
                })(),
            );
        });
        let publish = Callback::new(move |()| {
            if blocked.get_untracked() {
                return;
            }
            build_confirmed(
                dispatch,
                (|| {
                    let subject = subject.get_untracked().ok_or(ClientError::InvalidInput)?;
                    let readiness = readiness.get_untracked().ok_or(ClientError::InvalidInput)?;
                    let request = subject
                        .publish_request(&readiness, uuid()?)
                        .ok_or(ClientError::InvalidInput)?;
                    if !confirm(
                        Key::PublishConfirm,
                        &subject.title,
                        &identity(&subject, Some(&readiness)),
                        locale.get_untracked(),
                    ) {
                        return Err(ClientError::Cancelled);
                    }
                    facts.update(|facts| {
                        facts.publication.readiness = None;
                        facts.publication.active = None;
                    });
                    Ok(Command::Publish {
                        unit: subject.unit,
                        candidate: subject.candidate,
                        request,
                    })
                })(),
            );
        });
        let withdraw = Callback::new(move |()| {
            if blocked.get_untracked() {
                return;
            }
            build_confirmed(
                dispatch,
                (|| {
                    let (unit, title, active) = facts
                        .with_untracked(|facts| {
                            Some((
                                facts.unit_id(),
                                facts.unit.as_ref()?.details.title.clone(),
                                facts.publication.active.clone()?,
                            ))
                        })
                        .ok_or(ClientError::InvalidInput)?;
                    let release = active.release.as_ref().ok_or(ClientError::InvalidInput)?;
                    let reason = fields.reason.get_untracked();
                    if reason.trim().is_empty() {
                        return Err(ClientError::InvalidInput);
                    }
                    let id = uuid()?;
                    if !confirm(
                        Key::WithdrawConfirm,
                        &title,
                        &format!(
                            "{}\n{}\n{}",
                            release.id, release.manifest.digest, active.pointer_version
                        ),
                        locale.get_untracked(),
                    ) {
                        return Err(ClientError::Cancelled);
                    }
                    ledger.update(|ledger| {
                        ledger.submitted =
                            Some(SubmittedForm::Withdrawal(id.clone(), reason.clone()))
                    });
                    facts.update(|facts| {
                        facts.publication.readiness = None;
                        facts.publication.active = None;
                    });
                    Ok(Command::Withdraw {
                        unit,
                        request: WithdrawAudioReleaseRequest {
                            operation_id: id,
                            release_id: release.id.clone(),
                            expected_pointer_version: active.pointer_version,
                            reason,
                        },
                    })
                })(),
            );
        });
        let save_visibility = Callback::new(move |()| {
            if blocked.get_untracked() {
                return;
            }
            build_confirmed(
                dispatch,
                (|| {
                    let work = facts
                        .with_untracked(|facts| facts.work.clone())
                        .ok_or(ClientError::InvalidInput)?;
                    let value = fields.visibility.get_untracked();
                    let operation = uuid()?;
                    let command = visibility_command(&work, &value, operation.clone())
                        .ok_or(ClientError::InvalidInput)?;
                    let displayed = if value == "public" {
                        Key::PublicVisibility
                    } else {
                        Key::PrivateVisibility
                    };
                    if !confirm(
                        Key::VisibilityConfirm,
                        &work.details.title,
                        &format!(
                            "{} · r{} · {}",
                            work.id,
                            work.revision,
                            text(displayed, locale.get_untracked())
                        ),
                        locale.get_untracked(),
                    ) {
                        return Err(ClientError::Cancelled);
                    }
                    ledger.update(|ledger| {
                        ledger.submitted = Some(SubmittedForm::Visibility {
                            operation,
                            work: work.id,
                            value,
                        })
                    });
                    facts.update(|facts| {
                        facts.publication.readiness = None;
                        facts.publication.active = None;
                    });
                    Ok(command)
                })(),
            );
        });
        view! {<section id="live-publication-section" class="live-section" aria-labelledby="live-publication-title">
            <h3 id="live-publication-title">{move ||text(Key::Publication,locale.get())}</h3>
            <p>{move ||text(Key::PublicationHelp,locale.get())}</p>
            <h4>{move ||text(Key::WorkVisibility,locale.get())}</h4>
            <p>{move ||facts.with(|facts|facts.work.as_ref().map(|work|copy::work_visibility(&work.details.visibility,locale.get())).unwrap_or_default())}</p>
            <Select id="live-publication-work-visibility" label=Signal::derive(move ||text(Key::WorkVisibility,locale.get()).to_owned()) value=fields.visibility on_input=Callback::new(move |value|fields.visibility.set(value)) options=Signal::derive(move ||vec![SelectOption::new("",text(Key::Decision,locale.get())),SelectOption::new("private",text(Key::PrivateVisibility,locale.get())),SelectOption::new("public",text(Key::PublicVisibility,locale.get()))]) class="live-field"/>
            <p>{move ||text(Key::VisibilityHelp,locale.get())}</p>
            <ActionButton id="live-publication-save-visibility" locale=locale reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||fields.visibility.with(|value|!matches!(value.as_str(),"private"|"public"))||facts.with(|facts|facts.work.as_ref().is_none_or(|work|work.details.visibility==fields.visibility.get()))) on_press=Callback::new(move |_|save_visibility.run(()))>{move ||text(Key::SaveVisibility,locale.get())}</ActionButton>

            <p><code>{move ||subject.with(|subject|subject.as_ref().map(|subject|identity(subject,None)).unwrap_or_default())}</code></p>
            <ActionButton id="live-publication-read-readiness" locale=locale reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||subject.with(Option::is_none)) on_press=Callback::new(move |_|request_readiness.run(()))>{move ||text(Key::ReadReadiness,locale.get())}</ActionButton>
            <p role="status">{move ||readiness.with(|readiness|text(match readiness {None=>Key::ReadinessNotRead,Some(readiness) if readiness.blockers.is_empty()=>Key::ReadyToPublish,Some(_)=>Key::PublicationBlocked},locale.get()))}</p>
            <ul>{move ||readiness.with(|readiness|readiness.as_ref().map(|readiness|readiness.blockers.iter().map(|blocker|copy::publication_blocker(*blocker,locale.get()).to_owned()).collect::<Vec<_>>()).unwrap_or_default()).into_iter().map(|label|view!{<li>{label}</li>}).collect_view()}</ul>
            <details><summary>{move ||text(Key::InspectDocument,locale.get())}</summary><pre>{move ||facts.with(|facts|document(facts.publication.readiness.as_ref()))}</pre></details>
            <h4>{move ||text(Key::DistributionRights,locale.get())}</h4>
            <div class="live-grid">
                <Select id="live-publication-distribution-decision" label=Signal::derive(move ||text(Key::DistributionDecision,locale.get()).to_owned()) value=fields.rights_decision on_input=Callback::new(move |value|fields.rights_decision.set(value)) options=Signal::derive(move ||vec![SelectOption::new("",text(Key::Decision,locale.get())),SelectOption::new("cleared",text(Key::Cleared,locale.get())),SelectOption::new("revoked",text(Key::DistributionRevoked,locale.get()))]) class="live-field"/>
                <TextField id="live-publication-distribution-rights" label=Signal::derive(move ||text(Key::Rights,locale.get()).to_owned()) value=fields.rights on_input=Callback::new(move |value|fields.rights.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                <TextField id="live-publication-distribution-evidence" label=Signal::derive(move ||text(Key::Evidence,locale.get()).to_owned()) value=fields.evidence on_input=Callback::new(move |value|fields.evidence.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                <TextField id="live-publication-distribution-until" label=Signal::derive(move ||text(Key::ValidUntil,locale.get()).to_owned()) value=fields.until on_input=Callback::new(move |value|fields.until.set(value)) class="live-field" variant=TextFieldVariant::Floating density=EDITOR_FIELD_DENSITY autocomplete="off"/>
                <TextArea id="live-publication-distribution-notes" label=Signal::derive(move ||text(Key::DistributionNotes,locale.get()).to_owned()) value=fields.rights_notes on_input=Callback::new(move |value|fields.rights_notes.set(value)) class="live-field"/>
            </div><p>{move ||text(Key::ValidUntilHelp,locale.get())}</p>
            <ActionButton id="live-publication-record-distribution" locale=locale reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||subject.with(Option::is_none)) on_press=Callback::new(move |_|record_rights.run(()))>{move ||text(Key::RecordRights,locale.get())}</ActionButton>
            <details><summary>{move ||text(Key::PublicationReceipt,locale.get())}</summary><pre>{move ||facts.with(|facts|document(facts.publication.clearance_receipt.as_ref()))}</pre></details>
            <h4>{move ||text(Key::PublicationApproval,locale.get())}</h4><div class="live-grid">
                <Select id="live-publication-approval-decision" label=Signal::derive(move ||text(Key::Decision,locale.get()).to_owned()) value=fields.approval_decision on_input=Callback::new(move |value|fields.approval_decision.set(value)) options=Signal::derive(move ||vec![SelectOption::new("",text(Key::Decision,locale.get())),SelectOption::new("approved",text(Key::Approved,locale.get())),SelectOption::new("withdrawn",text(Key::Withdrawn,locale.get()))]) class="live-field"/>
                <TextArea id="live-publication-approval-notes" label=Signal::derive(move ||text(Key::ApprovalNotes,locale.get()).to_owned()) value=fields.approval_notes on_input=Callback::new(move |value|fields.approval_notes.set(value)) class="live-field"/>
            </div>
            <ActionButton id="live-publication-record-approval" locale=locale reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||readiness.with(|readiness|readiness.as_ref().is_none_or(|readiness|readiness.clearance.is_none()))) on_press=Callback::new(move |_|record_approval.run(()))>{move ||text(Key::RecordApproval,locale.get())}</ActionButton>
            <details><summary>{move ||text(Key::PublicationReceipt,locale.get())}</summary><pre>{move ||facts.with(|facts|document(facts.publication.approval_receipt.as_ref()))}</pre></details>
            <ActionButton id="live-publication-publish-release" locale=locale reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||readiness.with(|readiness|readiness.as_ref().is_none_or(|readiness|!readiness.blockers.is_empty()))) on_press=Callback::new(move |_|publish.run(()))>{move ||text(Key::PublishRelease,locale.get())}</ActionButton>
            <Show when=move ||facts.with(|facts|facts.publication.publish_receipt.is_some())><h4>{move ||text(Key::PublicationReceipt,locale.get())}</h4><p>{move ||text(Key::ReceiptNotCurrent,locale.get())}</p><pre>{move ||facts.with(|facts|document(facts.publication.publish_receipt.as_ref()))}</pre></Show>
            <h4>{move ||text(Key::ActiveRelease,locale.get())}</h4>
            <ActionButton id="live-publication-read-active-release" locale=locale reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {if blocked.get_untracked(){return;}facts.update(|facts|facts.publication.active=None);dispatch.send(Command::ActiveRelease {unit:facts.with_untracked(Facts::unit_id)});})>{move ||text(Key::ReadActiveRelease,locale.get())}</ActionButton>
            <p role="status">{move ||facts.with(|facts|match &facts.publication.active {None=>text(Key::ActiveNotRead,locale.get()).to_owned(),Some(active)=>active.release.as_ref().map(|release|copy::active_release(release,active.pointer_version)).unwrap_or_else(||text(Key::NoActiveRelease,locale.get()).to_owned())})}</p>
            <details><summary>{move ||text(Key::InspectDocument,locale.get())}</summary><pre>{move ||facts.with(|facts|document(facts.publication.active.as_ref()))}</pre></details>
            <TextArea id="live-publication-withdrawal-reason" label=Signal::derive(move ||text(Key::WithdrawalReason,locale.get()).to_owned()) value=fields.reason on_input=Callback::new(move |value|fields.reason.set(value)) class="live-field"/>
            <ActionButton id="live-publication-withdraw-release" locale=locale reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||fields.reason.with(|reason|reason.trim().is_empty())||facts.with(|facts|facts.publication.active.as_ref().is_none_or(|active|active.release.is_none()))) on_press=Callback::new(move |_|withdraw.run(()))>{move ||text(Key::WithdrawRelease,locale.get())}</ActionButton>
            <Show when=move ||facts.with(|facts|facts.publication.withdrawal_receipt.is_some())><h4>{move ||text(Key::WithdrawalReceipt,locale.get())}</h4><pre>{move ||facts.with(|facts|document(facts.publication.withdrawal_receipt.as_ref()))}</pre></Show>
            <p>{move ||if dirty.get() {text(Key::PublicationFormDirty,locale.get())}else{""}}</p>
            <ActionButton id="live-publication-reset-form" locale=locale reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                if blocked.get_untracked() {return;}
                if !dirty.get_untracked() || confirm(Key::ResetPublicationConfirm,"","",locale.get_untracked()) {fields.reset();ledger.set(FormLedger::default());}
            })>{move ||text(Key::ResetPublicationForm,locale.get())}</ActionButton>
        </section>}
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) use panel::PublicationPanel;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Command, Facts, Reply};
    fn subject() -> PublicationSubject {
        PublicationSubject {
            unit: "unit".into(),
            title: "Lời hẹn".into(),
            candidate: "candidate".into(),
            digest: "candidate-digest".into(),
            binding: AudioBinding {
                work_id: "work".into(),
                unit_id: "unit".into(),
                content_revision_id: "content".into(),
                content_revision_digest: "content-digest".into(),
                brief_id: "brief".into(),
                brief_version: 2,
                brief_digest: "brief-digest".into(),
                profile_id: "opus-mono-24-v1".into(),
            },
            rendition_digest: "rendition-digest".into(),
            sha256: "a".repeat(64),
        }
    }
    fn readiness() -> PublicationReadinessResponse {
        PublicationReadinessResponse {
            candidate_id: "candidate".into(),
            candidate_digest: "candidate-digest".into(),
            pointer_version: 7,
            active_release_id: Some("prior".into()),
            blockers: vec![],
            clearance: None,
            approval: None,
        }
    }
    #[test]
    fn publish_pins_exact_readiness_pointer_and_refuses_every_blocked_or_other_subject() {
        let subject = subject();
        let ready = readiness();
        let request = subject.publish_request(&ready, "operation".into()).unwrap();
        assert_eq!(request.expected_pointer_version, 7);
        assert_eq!(request.expected_active_release_id.as_deref(), Some("prior"));
        assert_eq!(request.candidate_digest, "candidate-digest");
        let mut changed = ready.clone();
        changed.candidate_id = "other".into();
        assert_eq!(subject.publish_request(&changed, "op".into()), None);
        changed = ready.clone();
        changed.candidate_digest = "other".into();
        assert_eq!(subject.publish_request(&changed, "op".into()), None);
        changed = ready;
        changed
            .blockers
            .push(PublicationBlocker::DeliveryUnavailable);
        assert_eq!(subject.publish_request(&changed, "op".into()), None);
    }
    #[test]
    fn decision_forms_are_explicit_and_bind_the_current_clearance() {
        let subject = subject();
        assert_eq!(RightsForm::default().request(&subject, "op".into()), None);
        let form = RightsForm {
            decision: "cleared".into(),
            rights: " r1, r2 ".into(),
            evidence: " proof ".into(),
            until: "12345".into(),
            notes: "attestation".into(),
        };
        let request = form.request(&subject, "op".into()).unwrap();
        assert_eq!(request.rights_refs, vec!["r1", "r2"]);
        assert_eq!(request.valid_until, 12345);
        let clearance = DistributionClearanceDocument {
            id: "clearance".into(),
            candidate_id: subject.candidate.clone(),
            actor_id: "owner".into(),
            sequence: 1,
            recorded_at: 1,
            request,
            digest: "clearance-digest".into(),
        };
        let approval = ApprovalForm {
            decision: "approved".into(),
            notes: "reviewed".into(),
        };
        assert_eq!(
            approval
                .request(&subject, &clearance, "op2".into())
                .unwrap()
                .clearance_digest,
            "clearance-digest"
        );
        let mut other = clearance.clone();
        other.candidate_id = "other".into();
        assert_eq!(approval.request(&subject, &other, "op2".into()), None);
        other = clearance;
        other.request.candidate_digest = "other".into();
        assert_eq!(approval.request(&subject, &other, "op2".into()), None);
    }
    #[test]
    fn section_acknowledgement_preserves_new_typing_and_other_unsaved_forms() {
        let original = RightsForm {
            decision: "revoked".into(),
            until: "12345".into(),
            ..RightsForm::default()
        };
        let ledger = FormLedger {
            saved: PublicationForm::default(),
            submitted: Some(SubmittedForm::Rights("op".into(), original.clone())),
        };
        let receipt = DistributionClearanceDocument {
            id: "clearance".into(),
            candidate_id: "candidate".into(),
            actor_id: "owner".into(),
            sequence: 1,
            recorded_at: 1,
            request: original.request(&subject(), "op".into()).unwrap(),
            digest: "digest".into(),
        };
        let acknowledged = ledger
            .acknowledge("op", &Reply::DistributionRights(receipt.clone()))
            .unwrap();
        let mut form = PublicationForm {
            rights: original,
            ..PublicationForm::default()
        };
        assert!(!acknowledged.dirty(&form));
        form.rights.notes = "later typing".into();
        assert!(acknowledged.dirty(&form));
        form = acknowledged.saved.clone();
        form.approval.notes = "unsaved approval".into();
        assert!(acknowledged.dirty(&form));
        let mut other = receipt;
        other.request.operation_id = "other".into();
        assert_eq!(
            ledger.acknowledge("op", &Reply::DistributionRights(other)),
            None
        );
    }
    fn work() -> CatalogWork {
        let create: CreateWorkRequest = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json")).unwrap();
        CatalogWork {
            id: create.id,
            kind: create.kind,
            owner_id: "owner".into(),
            details: create.details,
            lifecycle: "active".into(),
            revision: 7,
            created_at: 1,
            updated_at: 1,
        }
    }
    #[test]
    fn explicit_visibility_edit_carries_only_the_selected_field_and_exact_work_base() {
        let work = work();
        assert!(visibility_command(&work, "private", "operation".into()).is_none());
        assert!(visibility_command(&work, "unrecognized", "operation".into()).is_none());
        let command = visibility_command(&work, "public", "operation".into()).unwrap();
        let Command::PatchWork { work: id, request } = command else {
            panic!("wrong command")
        };
        assert_eq!(id, work.id);
        assert_eq!(request.expected_revision, 7);
        assert_eq!(request.operation_id, "operation");
        assert_eq!(
            serde_json::to_value(request.details).unwrap(),
            serde_json::json!({"visibility":"public"})
        );
    }
    #[test]
    fn receipt_without_operation_echo_requires_the_exact_completed_command_identity() {
        let mut work = work();
        work.details.visibility = "public".into();
        let receipt = Reply::Mutation(CatalogMutationResponse {
            work: work.clone(),
            units: vec![],
        });
        let ledger = FormLedger {
            saved: PublicationForm::default(),
            submitted: Some(SubmittedForm::Visibility {
                operation: "visibility-op".into(),
                work: work.id.clone(),
                value: "public".into(),
            }),
        };
        assert_eq!(ledger.acknowledge("other-op", &receipt), None);
        assert_eq!(
            ledger
                .acknowledge("visibility-op", &receipt)
                .unwrap()
                .saved
                .visibility,
            "public"
        );
        let ledger = FormLedger {
            saved: PublicationForm::default(),
            submitted: Some(SubmittedForm::Withdrawal(
                "withdraw-op".into(),
                "reason".into(),
            )),
        };
        let receipt = Reply::Withdrawal(AudioReleaseWithdrawalResponse {
            pointer_version: 8,
            withdrawn_release_id: "release".into(),
        });
        assert_eq!(ledger.acknowledge("other-op", &receipt), None);
        assert_eq!(
            ledger
                .acknowledge("withdraw-op", &receipt)
                .unwrap()
                .saved
                .withdrawal,
            "reason"
        );
    }
    #[test]
    fn approval_acknowledges_only_its_submitted_snapshot() {
        let form = ApprovalForm {
            decision: "approved".into(),
            notes: "named review".into(),
        };
        let ledger = FormLedger {
            saved: PublicationForm::default(),
            submitted: Some(SubmittedForm::Approval("approval-op".into(), form.clone())),
        };
        let receipt = Reply::PublicationApproval(PublicationApprovalDocument {
            id: "approval".into(),
            candidate_id: "candidate".into(),
            actor_id: "owner".into(),
            sequence: 1,
            recorded_at: 1,
            request: RecordPublicationApprovalRequest {
                operation_id: "approval-op".into(),
                candidate_digest: "candidate-digest".into(),
                clearance_digest: "clearance-digest".into(),
                decision: PublicationApprovalDecision::Approved,
                notes: form.notes.clone(),
            },
            digest: "digest".into(),
        });
        assert_eq!(ledger.acknowledge("other-op", &receipt), None);
        assert_eq!(
            ledger
                .acknowledge("approval-op", &receipt)
                .unwrap()
                .saved
                .approval,
            form
        );
    }
    #[test]
    fn original_publish_receipt_never_claims_a_current_active_pointer() {
        let response = ActiveAudioReleaseResponse {
            pointer_version: 8,
            release: None,
        };
        let mut facts = Facts::default();
        facts.publication.active = Some(ActiveAudioReleaseResponse {
            pointer_version: 9,
            release: None,
        });
        let command = Command::Publish {
            unit: "unit".into(),
            candidate: "candidate".into(),
            request: PublishAudioReleaseRequest {
                operation_id: "op".into(),
                candidate_digest: "candidate-digest".into(),
                expected_pointer_version: 7,
                expected_active_release_id: Some("prior".into()),
            },
        };
        let replayed =
            facts.apply_command(&command, Reply::ActiveRelease(Box::new(response.clone())));
        assert_eq!(replayed.publication.publish_receipt, Some(response.clone()));
        assert_eq!(replayed.publication.active, None);
        let current = replayed.apply_command(
            &Command::ActiveRelease {
                unit: "unit".into(),
            },
            Reply::ActiveRelease(Box::new(ActiveAudioReleaseResponse {
                pointer_version: 9,
                release: None,
            })),
        );
        assert_eq!(current.publication.active.unwrap().pointer_version, 9);
        assert_eq!(current.publication.publish_receipt, Some(response));
    }
    fn candidate() -> ReleaseCandidateResponse {
        let rendition: AudioRenditionDocument = serde_json::from_slice(include_bytes!("../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json")).unwrap();
        ReleaseCandidateResponse {
            candidate: ReleaseCandidateDocument {
                id: "candidate".into(),
                rendition_id: rendition.id,
                rendition_digest: rendition.digest,
                binding: rendition.binding,
                sha256: rendition.verified_media.sha256,
                work_metadata_revision: 1,
                unit_metadata_revision: 1,
                review_ids: vec![],
                created_by: "owner".into(),
                created_at: 1,
                operation_id: "op".into(),
                digest: "candidate-digest".into(),
            },
            blockers: vec![],
        }
    }
    #[test]
    fn new_candidate_and_rights_approval_withdrawal_invalidate_advisory_readiness() {
        let rights = RightsForm {
            decision: "cleared".into(),
            until: "12345".into(),
            ..RightsForm::default()
        };
        let clearance = DistributionClearanceDocument {
            id: "clearance".into(),
            candidate_id: "candidate".into(),
            actor_id: "owner".into(),
            sequence: 1,
            recorded_at: 1,
            request: rights.request(&subject(), "rights-op".into()).unwrap(),
            digest: "clearance-digest".into(),
        };
        let approval = PublicationApprovalDocument {
            id: "approval".into(),
            candidate_id: "candidate".into(),
            actor_id: "owner".into(),
            sequence: 1,
            recorded_at: 1,
            request: ApprovalForm {
                decision: "approved".into(),
                notes: "reviewed".into(),
            }
            .request(&subject(), &clearance, "approval-op".into())
            .unwrap(),
            digest: "approval-digest".into(),
        };
        for reply in [
            Reply::DistributionRights(clearance),
            Reply::PublicationApproval(approval),
            Reply::Withdrawal(AudioReleaseWithdrawalResponse {
                pointer_version: 8,
                withdrawn_release_id: "release".into(),
            }),
            Reply::Candidate(candidate()),
        ] {
            let mut facts = Facts::default();
            facts.publication.readiness = Some(readiness());
            facts.publication.active = Some(ActiveAudioReleaseResponse {
                pointer_version: 7,
                release: None,
            });
            let next = facts.apply(reply);
            assert_eq!(next.publication.readiness, None);
            assert_eq!(next.publication.active, None);
        }
    }
    #[test]
    fn publication_subject_refuses_another_unit_or_work_even_when_candidate_digest_matches() {
        let candidate = candidate();
        let binding = candidate.candidate.binding.clone();
        let unit = CatalogContentUnit {
            id: binding.unit_id.clone(),
            work_id: binding.work_id.clone(),
            kind: "chapter".into(),
            details: UnitDetailsDto {
                title: "Lời hẹn".into(),
                description: String::new(),
                source: None,
                script: None,
                content_digest: None,
            },
            position: 1,
            lifecycle: "active".into(),
            revision: 1,
            created_at: 1,
            updated_at: 1,
        };
        let mut facts = Facts {
            unit: Some(unit.clone()),
            candidate: Some(candidate),
            ..Facts::default()
        };
        assert_eq!(
            PublicationSubject::from_facts(&facts).unwrap().binding,
            binding
        );
        facts.unit.as_mut().unwrap().id = "another-unit".into();
        assert_eq!(PublicationSubject::from_facts(&facts), None);
        facts.unit = Some(unit);
        facts.unit.as_mut().unwrap().work_id = "another-work".into();
        assert_eq!(PublicationSubject::from_facts(&facts), None);
    }
    #[test]
    fn historical_content_inspection_preserves_but_current_head_read_invalidates_publication_facts()
    {
        let content:ContentRevisionResponse=serde_json::from_slice(include_bytes!("../../../../../services/cantos-server/contracts/studio/content/v1/fixtures/text-revision.json")).unwrap();
        let mut facts = Facts::default();
        facts.publication.readiness = Some(readiness());
        let history = facts.apply_command(
            &Command::ContentRevision {
                unit: content.unit_id.clone(),
                revision: content.id.clone(),
            },
            Reply::Content(content.clone()),
        );
        assert_eq!(history.publication.readiness, Some(readiness()));
        let head = history.apply_command(
            &Command::ContentHead {
                unit: content.unit_id.clone(),
            },
            Reply::Content(content),
        );
        assert_eq!(head.publication, PublicationFacts::default());
    }
}
