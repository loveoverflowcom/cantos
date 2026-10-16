//! One bounded generation attempt: reserve, call the provider once, record the outcome.
//!
//! The order is the safety property. The reservation is appended to the durable history
//! **before** the provider is called, only the invocation whose append succeeded may call, and
//! whatever happens afterwards (a timeout, a lost response, a result that cannot be written,
//! a process that dies) is recorded or left open, never answered with a second call. A new call
//! needs a new attempt, and a new attempt needs an explicit regeneration. Retrying a request
//! returns the recorded state.
//!
//! Provider cost is never exactly-once: an aborted request does not prove the provider stopped
//! or did not charge, and an open attempt stays open until an operator decides what it was.

use std::{num::NonZeroU64, slice};

use crate::domain::{
    attempt::{
        regenerate, settle, submit, Acknowledgement, AttemptNumber, Cost, Event, Lookup, Operation,
        Policy, ProviderAudio, Rejection, ReplayError, ReserveRefusal, SettleRefusal, Settlement,
        Status, Submission, UncertainReason,
    },
    digest::Sha256Digest,
    generation::{GenerationRequest, RequestFingerprint},
    label::OperationKey,
};

use super::ports::{AttemptStore, AttemptStoreError, Provider, ProviderResult, Reserved, Store};

/// The largest provider response Forge keeps for this first, single-segment slice. It bounds
/// memory and scratch space; it is not a Cantos intake limit and not a streaming design.
pub const MAX_PROVIDER_AUDIO_BYTES: u64 = 32 * 1024 * 1024;

/// What the operator's policy says one attempt of this request costs and how much may be spent.
/// A request cannot supply or change either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    pub policy: Policy,
    pub cost: Cost,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Produced {
    /// This invocation won the reservation, made the one provider call and recorded its outcome.
    /// The status may be an uncertain or rejected attempt: that is an outcome, not an error.
    Executed(Status),
    /// A retry, or a lost race against an identical request: the recorded state. This
    /// invocation made no provider call.
    Recorded(Status),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProduceError {
    /// Reading the history failed. Nothing was reserved or called.
    Store(AttemptStoreError),
    /// The history cannot have been produced by any sequence of decisions. Nothing was called.
    Corrupt(ReplayError),
    /// The store returned the history of another operation.
    ForeignHistory { found: OperationKey },
    /// The same key was used for a different request.
    Conflict { recorded: RequestFingerprint },
    /// A rule refused the reservation. Nothing was reserved or called.
    Refused(ReserveRefusal),
    /// Regeneration or recovery named a key with no history.
    UnknownOperation,
    /// The reservation could not be confirmed, so **no provider call was made**. The event may
    /// still exist (a lost response); a retry returns the recorded state and never calls.
    NotReserved(AttemptStoreError),
    /// The provider was called, but its outcome could not be recorded. The attempt stays open
    /// or was settled by someone else, and no further call is allowed for it. Reconcile it
    /// (recover, then an explicit regeneration) instead of replaying.
    OutcomeNotRecorded {
        attempt: AttemptNumber,
        error: AttemptStoreError,
    },
    /// An outcome could not be settled: the latest attempt already has one, or the number is not
    /// the latest.
    Settle(SettleRefusal),
}

/// The ports one run needs, borrowed for the duration of a call.
pub struct Producer<'a, A, S, P> {
    pub attempts: &'a A,
    /// Where the provider's bytes are kept, addressed by their digest.
    pub blobs: &'a S,
    pub provider: &'a P,
}

impl<A: AttemptStore, S: Store, P: Provider> Producer<'_, A, S, P> {
    /// Runs the request named by `key`, or returns what is already recorded for it.
    pub fn generate(
        &self,
        key: &OperationKey,
        request: &GenerationRequest,
        terms: Terms,
    ) -> Result<Produced, ProduceError> {
        let existing = self.load(key)?;
        let lookup = match &existing {
            Some(operation) => Lookup::Found(operation),
            None => Lookup::Absent(key),
        };
        match submit(lookup, request.fingerprint(), terms.cost, terms.policy) {
            Submission::Replay(status) => Ok(Produced::Recorded(status)),
            Submission::Conflict { recorded } => Err(ProduceError::Conflict { recorded }),
            Submission::Refused(refusal) => Err(ProduceError::Refused(refusal)),
            Submission::Reserve(event) => self.run(key, request, existing, event),
        }
    }

    /// An explicit new attempt for an operation whose request is unchanged. After an uncertain
    /// attempt it needs the operator's [`Acknowledgement`] of exactly that attempt.
    pub fn regenerate(
        &self,
        key: &OperationKey,
        request: &GenerationRequest,
        acknowledgement: Option<Acknowledgement>,
        terms: Terms,
    ) -> Result<Produced, ProduceError> {
        let operation = self.load(key)?.ok_or(ProduceError::UnknownOperation)?;
        if request.fingerprint() != operation.fingerprint() {
            return Err(ProduceError::Conflict {
                recorded: operation.fingerprint(),
            });
        }
        let event = regenerate(&operation, acknowledgement, terms.cost, terms.policy)
            .map_err(ProduceError::Refused)?;
        self.run(key, request, Some(operation), event)
    }

    /// Records that the open attempt's owner is gone, making its effect *uncertain*. Only the
    /// caller can know that: an owner that is alive finds its own outcome write refused, and
    /// whatever it produced is kept only as bytes under their digest.
    pub fn recover(&self, key: &OperationKey) -> Result<Status, ProduceError> {
        let operation = self.load(key)?.ok_or(ProduceError::UnknownOperation)?;
        let attempt = operation.ledger().attempts();
        let event = settle(
            &operation,
            attempt,
            Settlement::Uncertain(UncertainReason::Abandoned),
        )
        .map_err(ProduceError::Settle)?;
        self.attempts
            .append(key, operation.next_index(), &event)
            .map_err(ProduceError::Store)?;
        operation
            .apply(&event)
            .map(|settled| settled.status())
            .map_err(ProduceError::Corrupt)
    }

    fn load(&self, key: &OperationKey) -> Result<Option<Operation>, ProduceError> {
        let events = self.attempts.events(key).map_err(ProduceError::Store)?;
        if events.is_empty() {
            return Ok(None);
        }
        let operation = Operation::replay(&events).map_err(ProduceError::Corrupt)?;
        if operation.key() != key {
            return Err(ProduceError::ForeignHistory {
                found: operation.key().clone(),
            });
        }
        Ok(Some(operation))
    }

    /// Appends the reservation `event`; only if that append succeeds, makes the one call.
    fn run(
        &self,
        key: &OperationKey,
        request: &GenerationRequest,
        base: Option<Operation>,
        event: Event,
    ) -> Result<Produced, ProduceError> {
        let index = base.as_ref().map_or(0, Operation::next_index);
        match self.attempts.append(key, index, &event) {
            Ok(()) => {}
            // Another invocation reserved first: it owns the call, this one only reports.
            Err(AttemptStoreError::Conflict) => return self.recorded(key, request.fingerprint()),
            Err(error) => return Err(ProduceError::NotReserved(error)),
        }
        let operation = match base {
            Some(operation) => operation.apply(&event),
            None => Operation::replay(slice::from_ref(&event)),
        }
        .map_err(ProduceError::Corrupt)?;
        let attempt = operation.ledger().attempts();
        let reserved = Reserved::new(attempt);
        let settlement = match self.provider.synthesize(&reserved, request) {
            ProviderResult::Audio(bytes) => self.keep(&bytes),
            ProviderResult::Rejected(reason) => Settlement::Rejected(reason),
            ProviderResult::Unknown(reason) => Settlement::Uncertain(reason),
        };
        self.record(key, &operation, attempt, settlement)
    }

    /// Keeps the provider's bytes under their digest, or says why they cannot be a result.
    fn keep(&self, bytes: &[u8]) -> Settlement {
        let Some(length) = u64::try_from(bytes.len())
            .ok()
            .filter(|length| *length <= MAX_PROVIDER_AUDIO_BYTES)
            .and_then(NonZeroU64::new)
        else {
            return Settlement::Rejected(Rejection::InvalidAudio);
        };
        let digest = Sha256Digest::of(bytes);
        match self.blobs.put_blob(digest, bytes) {
            Ok(()) => Settlement::Succeeded(ProviderAudio {
                digest,
                bytes: length,
            }),
            // The provider answered and may have charged, but Forge cannot keep what it got.
            Err(_) => Settlement::Uncertain(UncertainReason::ResultNotRecorded),
        }
    }

    /// Writes the attempt's one outcome. A lost response is healed by reading the history back:
    /// only the reservation's owner records a provider outcome, so finding exactly this event
    /// at this position in an intact history with the same prefix proves the write happened.
    /// A later valid regeneration is allowed; a matching event in an unrelated or corrupt
    /// history proves nothing. (A reservation cannot be healed this way: two identical
    /// requests produce identical reservation events.)
    fn record(
        &self,
        key: &OperationKey,
        operation: &Operation,
        attempt: AttemptNumber,
        settlement: Settlement,
    ) -> Result<Produced, ProduceError> {
        let event = settle(operation, attempt, settlement).map_err(ProduceError::Settle)?;
        let index = operation.next_index();
        if let Err(error) = self.attempts.append(key, index, &event) {
            let written = self.attempts.events(key).is_ok_and(|events| {
                events.get(index) == Some(&event)
                    && Operation::replay(&events).is_ok()
                    && events
                        .get(..index)
                        .and_then(|prefix| Operation::replay(prefix).ok())
                        .as_ref()
                        == Some(operation)
            });
            if !written {
                return Err(ProduceError::OutcomeNotRecorded { attempt, error });
            }
        }
        operation
            .apply(&event)
            .map(|settled| Produced::Executed(settled.status()))
            .map_err(ProduceError::Corrupt)
    }

    fn recorded(
        &self,
        key: &OperationKey,
        fingerprint: RequestFingerprint,
    ) -> Result<Produced, ProduceError> {
        let operation = self.load(key)?.ok_or(ProduceError::UnknownOperation)?;
        // The competing reservation may pin a different request under the same key.
        if operation.fingerprint() != fingerprint {
            return Err(ProduceError::Conflict {
                recorded: operation.fingerprint(),
            });
        }
        Ok(Produced::Recorded(operation.status()))
    }
}
