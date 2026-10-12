//! Append observations even from stale workers, then settle cost and evolve only current leases.
//! The same transaction protects receipt linkage, resolution conflicts and reservation accounting.
use super::super::postgres::{
    attempts, check_lease, document, encode, mode, observations, outcome, policy, run, stop_job,
};
use crate::{
    contexts::synthesis::{
        attempt::{DeliveryState, FailureClass},
        jobs::{decide_next_step, ExecutionMode, StepDecision},
        receipt::{audit_receipt, decode_receipt, ChargeObservation, ReceiptOutcome, TtsReceipt},
        AttemptWork, DurableError,
    },
    platform::postgres::Store,
};

pub(super) async fn settle(
    tx: &deadpool_postgres::Transaction<'_>,
    reservation: &str,
    kind: &str,
    reported_amount: Option<i64>,
) -> Result<(), DurableError> {
    let row = observations::reservation(tx, reservation).await?;
    let run: String = row.get("run_id");
    let amount: i64 = row.get("amount_minor");
    let settled = if kind == "released" {
        0
    } else {
        reported_amount.unwrap_or(amount)
    };
    if settled < 0 || settled > amount {
        return Err(DurableError::InvalidRequest);
    }
    // Lock budget before inserting ledger entry; idempotent observation can race with resolution.
    observations::lock_budget(tx, &run).await?;
    let n = observations::record_settlement(tx, reservation, kind, settled).await?;
    if n == 1 {
        observations::update_budget(tx, &run, amount, settled).await?;
    }
    Ok(())
}

impl Store {
    pub async fn observe_attempt(
        &self,
        work: &AttemptWork,
        receipt: &TtsReceipt,
        path: Option<&str>,
        audio: Option<&[u8]>,
    ) -> Result<(), DurableError> {
        if path.is_some() != matches!(receipt.outcome, ReceiptOutcome::Output { .. })
            || path.is_some() != audio.is_some()
        {
            return Err(DurableError::InvalidRequest);
        }
        if let Some(path) = path {
            if path.is_empty() || path.len() > 4096 || !std::path::Path::new(path).is_absolute() {
                return Err(DurableError::InvalidRequest);
            }
        }
        let bytes = encode(receipt)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &work.lease.run_id).await?;
        let doc = document(&tx, &r).await?;
        let a = observations::attempt(&tx, &receipt.attempt_id, &work.lease).await?;
        let intent = decode_receipt(&a.get::<_, Vec<u8>>("intent_bytes"))
            .map_err(|_| DurableError::Corrupt)?;
        if receipt.attempt_id != intent.attempt_id
            || receipt.operation_id != intent.operation_id
            || receipt.input_digest != intent.input_digest
            || receipt.dialogue_id != intent.dialogue_id
            || receipt.provider != intent.provider
            || receipt.evidence != intent.evidence
            || receipt.reconciliation != intent.reconciliation
            || receipt.provider_request_id != intent.provider_request_id
            || receipt.idempotency_key != intent.idempotency_key
        {
            return Err(DurableError::Conflict);
        }
        audit_receipt(&doc, receipt, audio).map_err(|_| DurableError::Corrupt)?;
        if matches!(receipt.outcome, ReceiptOutcome::Output { .. }) {
            let dispatched = attempts::has_dispatch(&tx, &receipt.attempt_id).await?;
            if !dispatched {
                return Err(DurableError::Conflict);
            }
        }
        if matches!(receipt.charge, ChargeObservation::Reported { .. })
            && mode(&r)? != ExecutionMode::SyntheticFake
        {
            return Err(DurableError::InvalidRequest);
        }

        observations::lock_job(&tx, &work.lease.job_id).await?;
        // A denied duplicate caller cannot attest NotSent after another caller committed a send.
        if matches!(
            receipt.outcome,
            ReceiptOutcome::Failure {
                class: FailureClass::Cancelled,
                delivery: DeliveryState::NotSent
            }
        ) {
            let dispatched = attempts::has_dispatch(&tx, &receipt.attempt_id).await?;
            if dispatched {
                return Err(DurableError::ReconciliationRequired);
            }
        }
        if let Some(prior) = observations::prior_observation(&tx, &receipt.attempt_id).await? {
            if prior.receipt_bytes != bytes || prior.output_path.as_deref() != path {
                return Err(DurableError::Conflict);
            }
            tx.commit().await?;
            return Ok(());
        }
        observations::insert_observation(&tx, &receipt.attempt_id, &bytes, path).await?;
        let resolved = observations::has_resolution(&tx, &receipt.attempt_id).await?;
        if resolved {
            observations::pause_resolution_conflict(&tx, &work.lease.job_id).await?;
            tx.commit().await?;
            return Ok(());
        }
        let decision = decide_next_step(
            outcome(Some(receipt), false),
            a.get::<_, i32>("attempt_number") as u32,
            &policy(&r)?,
        );
        let reservation: String = a.get("reservation_id");
        match decision {
            StepDecision::Reconcile => {}
            StepDecision::Accept | StepDecision::Fail | StepDecision::RetryAfter(_) => {
                match &receipt.charge {
                    ChargeObservation::Reported {
                        currency: charge_currency,
                        amount_minor,
                        ..
                    } => {
                        if *charge_currency != doc.settings.settings.budget.currency {
                            return Err(DurableError::InvalidRequest);
                        }
                        let amount = i64::try_from(*amount_minor)
                            .map_err(|_| DurableError::InvalidRequest)?;
                        settle(&tx, &reservation, "reported_synthetic", Some(amount)).await?;
                    }
                    ChargeObservation::Unknown {} => {
                        let kind = if decision == StepDecision::Accept {
                            "provisional_local"
                        } else {
                            "released"
                        };
                        settle(&tx, &reservation, kind, None).await?;
                    }
                }
            }
            StepDecision::Dispatch => return Err(DurableError::Corrupt),
        }
        // Real observations survive stale fences; only control pointers are fenced.
        let lease_current = match check_lease(&tx, &work.lease).await {
            Ok(_) => true,
            Err(DurableError::LeaseLost) => false,
            Err(error) => return Err(error),
        };
        if lease_current {
            match decision {
                StepDecision::Reconcile => {
                    stop_job(&tx, &work.lease.job_id, "paused", "uncertain_attempt").await?
                }
                StepDecision::Fail => {
                    stop_job(
                        &tx,
                        &work.lease.job_id,
                        "failed",
                        "retry_limit_or_permanent_failure",
                    )
                    .await?
                }
                StepDecision::RetryAfter(_) => {
                    let delay = policy(&r)?.retry_delay_seconds(
                        a.get::<_, i32>("attempt_number") as u32,
                        &intent.operation_id,
                    );
                    observations::defer_after_observation(&tx, &work.lease.step_id, delay as i32)
                        .await?;
                }
                StepDecision::Accept | StepDecision::Dispatch => {}
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
