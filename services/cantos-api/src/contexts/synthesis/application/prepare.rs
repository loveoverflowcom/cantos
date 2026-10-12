//! Prepare a recoverable step or persist its next intent and reservation atomically.
//! Retry, ambiguity, budget and exact-input decisions occur under authority and lease locks.
use super::super::postgres::{
    attempts, authority, check_lease, currency, encode, is_authority_boundary, mode, outcome,
    policy, run, stop_job,
};
use crate::{
    contexts::{
        production::estimate_cost,
        synthesis::{
            attempt::{
                ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities,
            },
            jobs::{
                decide_next_step, input_fingerprint, validate_execution_mode, ExecutionMode,
                StepDecision,
            },
            receipt::{
                audit_receipt, decode_receipt, ChargeObservation, ReceiptEvidence, ReceiptOutcome,
                TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
            },
            AttemptWork, DurableError, JobLease, RecordedOutput, StepWork,
        },
    },
    platform::postgres::Store,
};
use cantos_contracts::ProductionEstimate;
use uuid::Uuid;

impl Store {
    pub async fn prepare_step(&self, lease: &JobLease) -> Result<StepWork, DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &lease.run_id).await?;
        let doc = match authority(&tx, &r).await {
            Ok(document) => document,
            Err(DurableError::Store(error)) if is_authority_boundary(&error) => {
                check_lease(&tx, lease).await?;
                stop_job(&tx, &lease.job_id, "paused", "authorization_invalid").await?;
                tx.commit().await?;
                return Ok(StepWork::Blocked);
            }
            Err(error) => return Err(error),
        };
        let step = check_lease(&tx, lease).await?;
        let line = doc
            .resolved
            .iter()
            .find(|line| line.dialogue_id == step.get::<_, String>("dialogue_id"))
            .ok_or(DurableError::Corrupt)?
            .clone();
        let execution = mode(&r)?;
        validate_execution_mode(execution, &line).map_err(|_| DurableError::ProviderDisabled)?;
        let fingerprint = input_fingerprint(&line).map_err(|_| DurableError::Corrupt)?;
        if fingerprint != step.get::<_, String>("input_fingerprint") {
            return Err(DurableError::Corrupt);
        }
        let prior = attempts::latest_attempt(&tx, &lease.step_id).await?;
        let retry = policy(&r)?;
        let count = prior
            .as_ref()
            .map_or(0, |row| row.get::<_, i32>("attempt_number") as u32);
        if let Some(prior) = &prior {
            let receipt = prior
                .get::<_, Option<Vec<u8>>>("receipt_bytes")
                .as_deref()
                .map(decode_receipt)
                .transpose()
                .map_err(|_| DurableError::Corrupt)?;
            let decision = if prior.get::<_, bool>("resolution_conflict") {
                StepDecision::Reconcile
            } else {
                decide_next_step(
                    outcome(receipt.as_ref(), prior.get("resolved")),
                    count,
                    &retry,
                )
            };
            match decision {
                StepDecision::Accept => {
                    let intent = decode_receipt(&prior.get::<_, Vec<u8>>("intent_bytes"))
                        .map_err(|_| DurableError::Corrupt)?;
                    let output = RecordedOutput {
                        receipt: receipt.ok_or(DurableError::Corrupt)?,
                        output_path: prior
                            .get::<_, Option<String>>("output_path")
                            .ok_or(DurableError::Corrupt)?,
                    };
                    let work = AttemptWork {
                        lease: lease.clone(),
                        document: doc,
                        line,
                        intent,
                        mode: execution,
                        input_fingerprint: fingerprint,
                    };
                    tx.commit().await?;
                    return Ok(StepWork::Recover(Box::new(work), Box::new(output)));
                }
                StepDecision::Reconcile => {
                    stop_job(&tx, &lease.job_id, "paused", "uncertain_attempt").await?;
                    tx.commit().await?;
                    return Ok(StepWork::Blocked);
                }
                StepDecision::Fail => {
                    stop_job(
                        &tx,
                        &lease.job_id,
                        "failed",
                        "retry_limit_or_permanent_failure",
                    )
                    .await?;
                    tx.commit().await?;
                    return Ok(StepWork::Blocked);
                }
                StepDecision::RetryAfter(_) => {
                    let delay =
                        retry.retry_delay_seconds(count, &prior.get::<_, String>("operation_id"));
                    let waiting =
                        attempts::retry_waiting(&tx, &prior.get::<_, String>("id"), delay as i32)
                            .await?;
                    if waiting {
                        attempts::defer_retry(
                            &tx,
                            &lease.step_id,
                            &prior.get::<_, String>("id"),
                            delay as i32,
                        )
                        .await?;
                        tx.commit().await?;
                        return Ok(StepWork::Blocked);
                    }
                }
                StepDecision::Dispatch => {}
            }
        }
        let budget = &doc.settings.settings.budget;
        let amount = match execution {
            ExecutionMode::LocalEspeak => 0,
            ExecutionMode::SyntheticFake => {
                match estimate_cost(budget.currency, line.text.chars().count() as u64, budget) {
                    ProductionEstimate::Known { amount_minor, .. } => amount_minor as i64,
                    ProductionEstimate::Unavailable { .. } => {
                        return Err(DurableError::BudgetExhausted)
                    }
                }
            }
        };
        let n =
            attempts::reserve_budget(&tx, &lease.run_id, amount, currency(budget.currency)).await?;
        if n == 0 {
            stop_job(&tx, &lease.job_id, "paused", "budget_exhausted").await?;
            tx.commit().await?;
            return Ok(StepWork::Blocked);
        }
        let attempt = Uuid::new_v4().to_string();
        let operation = Uuid::new_v4().to_string();
        let reservation = Uuid::new_v4().to_string();
        let intent = TtsReceipt {
            version: RECEIPT_VERSION.into(),
            evidence: match execution {
                ExecutionMode::LocalEspeak => ReceiptEvidence::CallerReported,
                ExecutionMode::SyntheticFake => ReceiptEvidence::SyntheticFixture,
            },
            attempt_id: attempt.clone(),
            operation_id: operation.clone(),
            provider_request_id: None,
            idempotency_key: None,
            input_digest: doc.input_digest.clone(),
            dialogue_id: line.dialogue_id.clone(),
            provider: TtsProviderIdentity {
                provider_id: line.provider_id.clone(),
                model_id: line.model_id.clone(),
                voice_id: line.voice_id.clone(),
                adapter_version: line.adapter_version.clone(),
            },
            reconciliation: ReconciliationCapabilities::new(
                ContractCapability::Unavailable,
                ContractCapability::Unavailable,
                ContractCapability::Unavailable,
            ),
            usage: UsageObservation::Unknown {},
            charge: ChargeObservation::Unknown {},
            outcome: ReceiptOutcome::Failure {
                class: FailureClass::Timeout,
                delivery: DeliveryState::MayHaveExecuted,
            },
        };
        audit_receipt(&doc, &intent, None).map_err(|_| DurableError::Corrupt)?;
        attempts::insert_reservation(
            &tx,
            &reservation,
            &lease.run_id,
            &lease.step_id,
            amount,
            currency(budget.currency),
        )
        .await?;
        attempts::insert_attempt(
            &tx,
            attempts::NewAttempt {
                id: &attempt,
                lease,
                operation: &operation,
                reservation: &reservation,
                intent_bytes: &encode(&intent)?,
                number: (count + 1) as i32,
            },
        )
        .await?;
        authority(&tx, &r).await?;
        check_lease(&tx, lease).await?;
        let work = AttemptWork {
            lease: lease.clone(),
            document: doc,
            line,
            intent,
            mode: execution,
            input_fingerprint: fingerprint,
        };
        tx.commit().await?;
        Ok(StepWork::Dispatch(Box::new(work)))
    }
}
