use uuid::Uuid;

use super::super::domain::PendingRegistration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BindingField {
    Operation,
    Project,
    Uid,
    Email,
    Epoch,
    Expiry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    InvalidBinding(BindingField),
    InvalidCode,
    WrongPurpose,
    Unverified,
    Expired,
    Cancelled,
    InFlight,
    Consumed,
    Uncertain,
    Protocol,
    TooLarge,
    NotJson,
    Redirect,
    Timeout,
    Network,
    Quota,
    Provider,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConfirmationError {
    BeforeConsume(Refusal),
    ConsumeUncertain(Refusal),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectId(String);

#[derive(Clone, PartialEq, Eq)]
struct DedicatedUid(String);

impl std::fmt::Debug for DedicatedUid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DedicatedUid([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Epoch(u64);

/// Reuses the existing signup identity/email policy. This is a local immutable binding,
/// not a claim that Firebase returns an operation, project or epoch in either REST reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Binding {
    registration: PendingRegistration,
    project: ProjectId,
    uid: DedicatedUid,
    epoch: Epoch,
    expires_at: u64,
}

impl Binding {
    pub(super) fn candidate(
        registration: PendingRegistration,
        project: &str,
        uid: &str,
        epoch: u64,
        expires_at: u64,
    ) -> Result<Self, Refusal> {
        if project.is_empty()
            || project.len() > 128
            || !project
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Refusal::InvalidBinding(BindingField::Project));
        }
        if uid.is_empty() || uid.len() > 128 || !uid.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(Refusal::InvalidBinding(BindingField::Uid));
        }
        if epoch == 0 {
            return Err(Refusal::InvalidBinding(BindingField::Epoch));
        }
        if expires_at == 0 {
            return Err(Refusal::InvalidBinding(BindingField::Expiry));
        }
        Ok(Self {
            registration,
            project: ProjectId(project.to_owned()),
            uid: DedicatedUid(uid.to_owned()),
            epoch: Epoch(epoch),
            expires_at,
        })
    }

    fn matches(&self, other: &Self) -> Result<(), Refusal> {
        let fields = [
            (
                self.registration.id() == other.registration.id(),
                BindingField::Operation,
            ),
            (self.project == other.project, BindingField::Project),
            (self.uid == other.uid, BindingField::Uid),
            (
                self.registration.email() == other.registration.email(),
                BindingField::Email,
            ),
            (self.epoch == other.epoch, BindingField::Epoch),
            (self.expires_at == other.expires_at, BindingField::Expiry),
        ];
        for (equal, field) in fields {
            if !equal {
                return Err(Refusal::InvalidBinding(field));
            }
        }
        Ok(())
    }

    // Exact provider text comparison is deliberate: Email already trims input and
    // lowercases the domain, while preserving local-part spelling. The deduplication
    // canonical_key is NOT mailbox-action authority; no case/dot/plus alias is accepted.
    fn matches_email(&self, email: &str) -> Result<(), Refusal> {
        if email != self.registration.email().as_str() {
            return Err(Refusal::InvalidBinding(BindingField::Email));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    AwaitingAction,
    Inspecting,
    Consuming,
    Confirmed,
    Uncertain,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Record {
    binding: Binding,
    phase: Phase,
}

impl Record {
    pub(super) fn new(binding: Binding) -> Self {
        Self {
            binding,
            phase: Phase::AwaitingAction,
        }
    }

    pub(super) fn with_phase(&self, phase: Phase) -> Self {
        Self {
            binding: self.binding.clone(),
            phase,
        }
    }

    pub(super) fn phase(&self) -> Phase {
        self.phase
    }

    pub(super) fn release_inspection(&self, binding: &Binding) -> Self {
        if self.binding == *binding && self.phase == Phase::Inspecting {
            self.with_phase(Phase::AwaitingAction)
        } else {
            self.clone()
        }
    }

    pub(super) fn consume_uncertain(&self, binding: &Binding) -> Self {
        if self.binding == *binding && self.phase == Phase::Consuming {
            self.with_phase(Phase::Uncertain)
        } else {
            self.clone()
        }
    }

    fn check(&self, binding: &Binding, now: u64, expected: Phase) -> Result<(), Refusal> {
        self.binding.matches(binding)?;
        match self.phase {
            Phase::Cancelled => return Err(Refusal::Cancelled),
            Phase::Confirmed => return Err(Refusal::Consumed),
            Phase::Uncertain => return Err(Refusal::Uncertain),
            Phase::AwaitingAction | Phase::Inspecting | Phase::Consuming => {}
        }
        if now >= self.binding.expires_at {
            return Err(Refusal::Expired);
        }
        if self.phase != expected {
            return Err(Refusal::InFlight);
        }
        Ok(())
    }
}

/// Kept private to this module; it cannot be deserialized or forged from a bool.
pub(super) struct CheckedAction {
    binding: Binding,
}

/// Evidence of this candidate action only. No conversion to CreatedPerson, setup or session.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ActionProof {
    binding: Binding,
}

pub(super) fn reserve(record: &Record, binding: &Binding, now: u64) -> Result<Record, Refusal> {
    record.check(binding, now, Phase::AwaitingAction)?;
    Ok(record.with_phase(Phase::Inspecting))
}

pub(super) fn inspect(
    record: &Record,
    binding: &Binding,
    now: u64,
    purpose: &str,
    email: &str,
) -> Result<(Record, CheckedAction), Refusal> {
    record.check(binding, now, Phase::Inspecting)?;
    if purpose != "VERIFY_EMAIL" {
        return Err(Refusal::WrongPurpose);
    }
    binding.matches_email(email)?;
    Ok((
        record.with_phase(Phase::Consuming),
        CheckedAction {
            binding: binding.clone(),
        },
    ))
}

pub(super) fn finish(
    record: &Record,
    action: CheckedAction,
    now: u64,
    uid: &str,
    email: &str,
    email_verified: Option<bool>,
) -> Result<(Record, ActionProof), Refusal> {
    record.check(&action.binding, now, Phase::Consuming)?;
    action.binding.matches_email(email)?;
    if uid != action.binding.uid.0 {
        return Err(Refusal::InvalidBinding(BindingField::Uid));
    }
    // The official confirmation sample omits this optional field. The successful
    // consumption of the same purpose-checked action is essential; true alone is not.
    if email_verified == Some(false) {
        return Err(Refusal::Unverified);
    }
    Ok((
        record.with_phase(Phase::Confirmed),
        ActionProof {
            binding: action.binding,
        },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PollStatus {
    ActionRequired,
    Pending,
    ActionAccepted,
    Unavailable,
}

pub(super) fn poll(record: &Record) -> PollStatus {
    match record.phase {
        Phase::AwaitingAction => PollStatus::ActionRequired,
        Phase::Inspecting | Phase::Consuming => PollStatus::Pending,
        Phase::Confirmed => PollStatus::ActionAccepted,
        Phase::Cancelled | Phase::Uncertain => PollStatus::Unavailable,
    }
}

/// A can know this receipt while V controls the mailbox. It carries no proof authority.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct InitiationReceipt(Uuid);

pub(super) fn receipt(binding: &Binding) -> InitiationReceipt {
    InitiationReceipt(binding.registration.id())
}

#[cfg(test)]
mod core_tests {
    use super::*;
    use crate::platform::theatre_registration::domain::Email;

    fn binding() -> Binding {
        Binding::candidate(
            PendingRegistration::new(
                Uuid::from_u128(1),
                Email::parse("Victim@EXAMPLE.test").unwrap(),
            )
            .unwrap(),
            "candidate-project",
            "dedicated-new-uid",
            1,
            200,
        )
        .unwrap()
    }

    #[test]
    fn every_local_binding_field_is_exact() {
        let original = binding();
        let mut variants = Vec::new();
        let mut changed = original.clone();
        changed.registration =
            PendingRegistration::new(Uuid::from_u128(2), original.registration.email().clone())
                .unwrap();
        variants.push((changed, BindingField::Operation));
        let mut changed = original.clone();
        changed.project = ProjectId("other-project".into());
        variants.push((changed, BindingField::Project));
        let mut changed = original.clone();
        changed.uid = DedicatedUid("other-uid".into());
        variants.push((changed, BindingField::Uid));
        let mut changed = original.clone();
        changed.registration = PendingRegistration::new(
            original.registration.id(),
            Email::parse("victim@example.test").unwrap(),
        )
        .unwrap();
        variants.push((changed, BindingField::Email));
        let mut changed = original.clone();
        changed.epoch = Epoch(2);
        variants.push((changed, BindingField::Epoch));
        let mut changed = original.clone();
        changed.expires_at = 201;
        variants.push((changed, BindingField::Expiry));
        let record = Record::new(original);
        for (candidate, field) in variants {
            assert_eq!(
                reserve(&record, &candidate, 100),
                Err(Refusal::InvalidBinding(field))
            );
            assert_eq!(record.phase(), Phase::AwaitingAction);
        }
    }

    #[test]
    fn local_terminal_and_expiry_fences_precede_effects() {
        let binding = binding();
        let record = Record::new(binding.clone());
        for (phase, reason) in [
            (Phase::Cancelled, Refusal::Cancelled),
            (Phase::Confirmed, Refusal::Consumed),
            (Phase::Uncertain, Refusal::Uncertain),
            (Phase::Inspecting, Refusal::InFlight),
            (Phase::Consuming, Refusal::InFlight),
        ] {
            assert_eq!(
                reserve(&record.with_phase(phase), &binding, 100),
                Err(reason)
            );
        }
        assert_eq!(reserve(&record, &binding, 200), Err(Refusal::Expired));
    }

    #[test]
    fn purpose_and_exact_email_are_checked_before_consumption() {
        let binding = binding();
        let record = reserve(&Record::new(binding.clone()), &binding, 100).unwrap();
        for purpose in [
            "PASSWORD_RESET",
            "EMAIL_SIGNIN",
            "RECOVER_EMAIL",
            "verify_email",
            "",
        ] {
            assert_eq!(
                inspect(&record, &binding, 100, purpose, "Victim@example.test").err(),
                Some(Refusal::WrongPurpose)
            );
        }
        for email in [
            "victim@example.test",
            "Victim@EXAMPLE.test",
            "other@example.test",
            " Victim@example.test",
        ] {
            assert_eq!(
                inspect(&record, &binding, 100, "VERIFY_EMAIL", email).err(),
                Some(Refusal::InvalidBinding(BindingField::Email))
            );
        }
        assert_eq!(record.phase(), Phase::Inspecting);
    }

    #[test]
    fn completion_requires_same_checked_binding_and_exact_uid_email() {
        let binding = binding();
        let inspecting = reserve(&Record::new(binding.clone()), &binding, 100).unwrap();
        for (uid, email, verified, reason) in [
            (
                "other",
                "Victim@example.test",
                Some(true),
                Refusal::InvalidBinding(BindingField::Uid),
            ),
            (
                "dedicated-new-uid",
                "victim@example.test",
                Some(true),
                Refusal::InvalidBinding(BindingField::Email),
            ),
            (
                "dedicated-new-uid",
                "Victim@example.test",
                Some(false),
                Refusal::Unverified,
            ),
        ] {
            let (consuming, action) = inspect(
                &inspecting,
                &binding,
                100,
                "VERIFY_EMAIL",
                "Victim@example.test",
            )
            .unwrap();
            assert_eq!(
                finish(&consuming, action, 100, uid, email, verified),
                Err(reason)
            );
        }
        for verified in [None, Some(true)] {
            let (consuming, action) = inspect(
                &inspecting,
                &binding,
                100,
                "VERIFY_EMAIL",
                "Victim@example.test",
            )
            .unwrap();
            let (done, proof) = finish(
                &consuming,
                action,
                100,
                "dedicated-new-uid",
                "Victim@example.test",
                verified,
            )
            .unwrap();
            assert_eq!(done.phase(), Phase::Confirmed);
            assert_eq!(proof.binding, binding);
        }
    }

    #[test]
    fn freshness_is_rechecked_at_both_await_boundaries() {
        let binding = binding();
        let inspecting = reserve(&Record::new(binding.clone()), &binding, 100).unwrap();
        assert_eq!(
            inspect(
                &inspecting,
                &binding,
                200,
                "VERIFY_EMAIL",
                "Victim@example.test"
            )
            .err(),
            Some(Refusal::Expired)
        );
        let (consuming, action) = inspect(
            &inspecting,
            &binding,
            100,
            "VERIFY_EMAIL",
            "Victim@example.test",
        )
        .unwrap();
        assert_eq!(
            finish(
                &consuming,
                action,
                200,
                "dedicated-new-uid",
                "Victim@example.test",
                Some(true)
            ),
            Err(Refusal::Expired)
        );
    }

    #[test]
    fn a_initiates_v_verifies_a_polls_without_acquiring_authority() {
        let binding = binding();
        let record = Record::new(binding.clone());
        let initiator_receipt = receipt(&binding);
        assert_eq!(initiator_receipt, InitiationReceipt(Uuid::from_u128(1)));
        for _ in 0..3 {
            assert_eq!(poll(&record), PollStatus::ActionRequired);
        }
        let inspecting = reserve(&record, &binding, 100).unwrap();
        let (consuming, v_action) = inspect(
            &inspecting,
            &binding,
            100,
            "VERIFY_EMAIL",
            "Victim@example.test",
        )
        .unwrap();
        let (confirmed, _v_only_proof) = finish(
            &consuming,
            v_action,
            100,
            "dedicated-new-uid",
            "Victim@example.test",
            Some(true),
        )
        .unwrap();
        assert_eq!(poll(&confirmed), PollStatus::ActionAccepted);
        // PollStatus and InitiationReceipt contain no ActionProof or setup/session material.
        assert_eq!(reserve(&confirmed, &binding, 100), Err(Refusal::Consumed));
    }
}
