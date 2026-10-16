//! Test-only orchestration around the production-compiled REST adapter. The process-local
//! fence is not a Repository implementation and establishes no durable concurrency,
//! transaction, crash-recovery, cancellation rollback or production enrollment guarantee.
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use super::core::{self, ActionProof, Binding, ConfirmationError, Record, Refusal};
use crate::platform::theatre_registration::firebase_action::{
    ActionCode, ActionError, BindingField as ActionBindingField, ConsumptionError, DedicatedUid,
    ExpectedActionBinding, FirebaseActionClient, FirebaseProjectId, MAX_RESPONSE_BYTES,
};

struct CandidateTransport(FirebaseActionClient);

impl CandidateTransport {
    fn fixture(address: SocketAddr, timeout: Duration) -> Result<Self, Refusal> {
        FirebaseActionClient::fixture(address, timeout)
            .map(Self)
            .map_err(map_failure)
    }

    async fn confirm(
        &self,
        local: &LocalFence,
        binding: &Binding,
        code: ActionCode,
        clock: &FixtureClock,
    ) -> Result<ActionProof, ConfirmationError> {
        let expected = ExpectedActionBinding::new(
            binding.registration().clone(),
            FirebaseProjectId::parse(binding.project())
                .map_err(|error| ConfirmationError::BeforeConsume(map_failure(error)))?,
            DedicatedUid::parse(binding.uid())
                .map_err(|error| ConfirmationError::BeforeConsume(map_failure(error)))?,
        );
        {
            let mut record = local
                .0
                .lock()
                .map_err(|_| ConfirmationError::BeforeConsume(Refusal::Protocol))?;
            *record = core::reserve(&record, binding, clock.now())
                .map_err(ConfirmationError::BeforeConsume)?;
        }
        let inspected = self.0.inspect(expected, code).await;
        let (checked, inspected) = {
            let mut record = local
                .0
                .lock()
                .map_err(|_| ConfirmationError::BeforeConsume(Refusal::Protocol))?;
            let result = inspected.map_err(map_failure).and_then(|inspected| {
                // Production inspection already established purpose/email. The pure
                // prototype check here only exercises its local freshness fence.
                core::inspect(
                    &record,
                    binding,
                    clock.now(),
                    "VERIFY_EMAIL",
                    binding.registration().email().as_str(),
                )
                .map(|(next, checked)| (next, checked, inspected))
            });
            match result {
                Ok((next, checked, inspected)) => {
                    *record = next;
                    (checked, inspected)
                }
                Err(error) => {
                    *record = record.release_inspection(binding);
                    return Err(ConfirmationError::BeforeConsume(error));
                }
            }
        };
        // No retry: a dropped future leaves the local Consuming state fenced.
        let consumed = self.0.consume(inspected).await;
        let mut record = local
            .0
            .lock()
            .map_err(|_| ConfirmationError::ConsumeUncertain(Refusal::Protocol))?;
        let result = consumed
            .map_err(|error| match error {
                ConsumptionError::BeforeConsume(error)
                | ConsumptionError::ConsumeUncertain(error) => map_failure(error),
            })
            .and_then(|_| {
                // Production consumption already checked exact UID/email/optional
                // false. This pure step adds only local phase and freshness checks.
                core::finish(
                    &record,
                    checked,
                    clock.now(),
                    binding.uid(),
                    binding.registration().email().as_str(),
                    None,
                )
            });
        match result {
            Ok((next, proof)) => {
                *record = next;
                Ok(proof)
            }
            Err(error) => {
                *record = record.consume_uncertain(binding);
                Err(ConfirmationError::ConsumeUncertain(error))
            }
        }
    }
}

struct LocalFence(Mutex<Record>);
struct FixtureClock(AtomicU64);
impl FixtureClock {
    fn now(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

fn map_failure(error: ActionError) -> Refusal {
    use super::core::BindingField;
    match error {
        ActionError::InvalidBinding(field) => Refusal::InvalidBinding(match field {
            ActionBindingField::Project => BindingField::Project,
            ActionBindingField::Uid => BindingField::Uid,
            ActionBindingField::Email => BindingField::Email,
        }),
        ActionError::InvalidCode => Refusal::InvalidCode,
        ActionError::WrongPurpose => Refusal::WrongPurpose,
        ActionError::Unverified => Refusal::Unverified,
        ActionError::Expired => Refusal::Expired,
        ActionError::InvalidApiKey
        | ActionError::WrongClient
        | ActionError::Configuration
        | ActionError::Protocol => Refusal::Protocol,
        ActionError::TooLarge => Refusal::TooLarge,
        ActionError::NotJson => Refusal::NotJson,
        ActionError::Redirect => Refusal::Redirect,
        ActionError::Timeout => Refusal::Timeout,
        ActionError::Network => Refusal::Network,
        ActionError::Quota => Refusal::Quota,
        ActionError::Provider | ActionError::ProviderDisabled => Refusal::Provider,
        ActionError::Rejected => Refusal::Rejected,
    }
}

#[cfg(test)]
mod fixture_tests;
