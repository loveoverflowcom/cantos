//! PostgreSQL durable local/synthetic worker shell. No commercial dispatch or media storage.
use super::production::{current_snapshot, durable_actor};
use super::{normalized_uuid, Store, StoreError};
use crate::jobs::{
    decide_next_step, evolve_run, input_fingerprint, validate_execution_mode, AttemptOutcome,
    ExecutionMode, LeaseDuration, RetryPolicy, RunEvent, RunStatus, StepDecision,
};
use crate::production::estimate_cost;
use crate::tts::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities,
};
use crate::tts::{
    audit_receipt, decode_document, decode_receipt, ChargeObservation, ReceiptEvidence,
    ReceiptOutcome, TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};
use cantos_contracts::{
    FrozenProductionDocument, ProductionCurrency, ProductionEstimate, ResolvedSynthesisInput,
};
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;
use uuid::Uuid;

#[derive(Debug)]
pub enum DurableError {
    Store(StoreError),
    InvalidRequest,
    Conflict,
    LeaseLost,
    BudgetExhausted,
    ReconciliationRequired,
    ProviderDisabled,
    Corrupt,
}
impl From<StoreError> for DurableError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}
impl From<tokio_postgres::Error> for DurableError {
    fn from(_: tokio_postgres::Error) -> Self {
        Self::Store(StoreError::Unavailable)
    }
}

#[derive(Debug, Clone)]
pub struct JobLease {
    pub run_id: String,
    pub job_id: String,
    pub step_id: String,
    pub worker_id: String,
    pub token: i64,
}
#[derive(Debug, Clone)]
pub struct AttemptWork {
    pub lease: JobLease,
    pub document: FrozenProductionDocument,
    pub line: ResolvedSynthesisInput,
    pub intent: TtsReceipt,
    pub mode: ExecutionMode,
    pub input_fingerprint: String,
}
#[derive(Debug, Clone)]
pub struct RecordedOutput {
    pub receipt: TtsReceipt,
    pub output_path: String,
}
#[derive(Debug)]
pub enum StepWork {
    Dispatch(Box<AttemptWork>),
    Recover(Box<AttemptWork>, Box<RecordedOutput>),
    Blocked,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSummary {
    pub run_id: String,
    pub job_id: String,
    pub status: RunStatus,
    pub reason: Option<String>,
    pub completed_steps: u64,
    pub total_steps: u64,
}

fn encode(value: &impl Serialize) -> Result<Vec<u8>, DurableError> {
    let bytes = serde_json::to_vec(value).map_err(|_| DurableError::InvalidRequest)?;
    if bytes.len() > 65536 {
        return Err(DurableError::InvalidRequest);
    }
    Ok(bytes)
}
fn status(row: &Row) -> Result<RunStatus, DurableError> {
    row.get::<_, String>("status")
        .parse()
        .map_err(|_| DurableError::Corrupt)
}
fn mode(row: &Row) -> Result<ExecutionMode, DurableError> {
    row.get::<_, String>("mode")
        .parse()
        .map_err(|_| DurableError::Corrupt)
}
fn valid_worker(worker: &str) -> bool {
    !worker.is_empty()
        && worker.len() <= 128
        && worker
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn currency(value: ProductionCurrency) -> &'static str {
    match value {
        ProductionCurrency::USD => "USD",
        ProductionCurrency::VND => "VND",
    }
}
fn outcome(receipt: Option<&TtsReceipt>, resolved: bool) -> AttemptOutcome {
    if resolved
        && receipt.is_some_and(|receipt| matches!(receipt.outcome, ReceiptOutcome::Output { .. }))
    {
        return AttemptOutcome::Unresolved;
    }
    if resolved {
        return AttemptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: DeliveryState::ConfirmedNotExecuted,
        };
    }
    match receipt.map(|r| &r.outcome) {
        None => AttemptOutcome::Unresolved,
        Some(ReceiptOutcome::Output { .. }) => AttemptOutcome::Output,
        Some(ReceiptOutcome::Failure { class, delivery }) => AttemptOutcome::Failure {
            class: *class,
            delivery: *delivery,
        },
    }
}
fn policy(row: &Row) -> Result<RetryPolicy, DurableError> {
    RetryPolicy::new(
        row.get::<_, i32>("max_attempts") as u32,
        row.get::<_, i64>("base_seconds") as u32,
        row.get::<_, i64>("cap_seconds") as u32,
    )
    .map_err(|_| DurableError::Corrupt)
}

async fn summary(
    tx: &deadpool_postgres::Transaction<'_>,
    run: &str,
) -> Result<JobSummary, DurableError> {
    let row=tx.query_opt("SELECT j.id,j.status,j.reason,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id) AS total,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id AND s.status='succeeded') AS done FROM durable_jobs j WHERE j.run_id=$1", &[&run]).await?.ok_or(DurableError::Store(StoreError::NotFound))?;
    Ok(JobSummary {
        run_id: run.into(),
        job_id: row.get("id"),
        status: status(&row)?,
        reason: row.get("reason"),
        total_steps: row.get::<_, i64>("total") as u64,
        completed_steps: row.get::<_, i64>("done") as u64,
    })
}
async fn run(tx: &deadpool_postgres::Transaction<'_>, id: &str) -> Result<Row, DurableError> {
    tx.query_opt("SELECT * FROM durable_runs WHERE id=$1", &[&id])
        .await?
        .ok_or(DurableError::Store(StoreError::NotFound))
}
async fn document(
    tx: &deadpool_postgres::Transaction<'_>,
    r: &Row,
) -> Result<FrozenProductionDocument, DurableError> {
    let snapshot: String = r.get("snapshot_id");
    let row = tx
        .query_one(
            "SELECT document_bytes FROM production_snapshots WHERE id=$1 AND script_id=$2",
            &[&snapshot, &r.get::<_, String>("script_id")],
        )
        .await?;
    let doc = decode_document(&row.get::<_, Vec<u8>>(0)).map_err(|_| DurableError::Corrupt)?;
    if doc.input_digest != r.get::<_, String>("input_digest")
        || doc.revision.script_id != r.get::<_, String>("script_id")
        || doc.owner_id != r.get::<_, String>("created_by")
    {
        return Err(DurableError::Corrupt);
    }
    Ok(doc)
}
async fn authority(
    tx: &deadpool_postgres::Transaction<'_>,
    r: &Row,
) -> Result<FrozenProductionDocument, DurableError> {
    let candidate = current_snapshot(
        tx,
        &r.get::<_, String>("created_by"),
        &r.get::<_, String>("script_id"),
        &r.get::<_, String>("snapshot_id"),
    )
    .await?;
    if candidate.document.input_digest != r.get::<_, String>("input_digest") {
        return Err(DurableError::Corrupt);
    }
    Ok(candidate.document)
}
async fn check_lease(
    tx: &deadpool_postgres::Transaction<'_>,
    lease: &JobLease,
) -> Result<Row, DurableError> {
    let job = tx
        .query_opt(
            "SELECT status FROM durable_jobs WHERE id=$1 AND run_id=$2 FOR UPDATE",
            &[&lease.job_id, &lease.run_id],
        )
        .await?
        .ok_or(DurableError::LeaseLost)?;
    if !matches!(status(&job)?, RunStatus::Queued | RunStatus::Running) {
        return Err(DurableError::LeaseLost);
    }
    tx.query_opt("SELECT * FROM durable_steps WHERE id=$1 AND job_id=$2 AND status='leased' AND lease_owner=$3 AND lease_token=$4 AND lease_expires_at>clock_timestamp() FOR UPDATE", &[&lease.step_id,&lease.job_id,&lease.worker_id,&lease.token]).await?.ok_or(DurableError::LeaseLost)
}
async fn stop_job(
    tx: &deadpool_postgres::Transaction<'_>,
    job: &str,
    new_status: &str,
    reason: &str,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_jobs SET status=$2,reason=$3 WHERE id=$1 AND status IN ('queued','running')", &[&job,&new_status,&reason]).await?;
    tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL WHERE job_id=$1 AND status='leased'", &[&job]).await?;
    Ok(())
}
async fn settle(
    tx: &deadpool_postgres::Transaction<'_>,
    reservation: &str,
    kind: &str,
    reported_amount: Option<i64>,
) -> Result<(), DurableError> {
    let row = tx
        .query_one(
            "SELECT run_id,amount_minor FROM durable_reservations WHERE id=$1",
            &[&reservation],
        )
        .await?;
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
    tx.query_one(
        "SELECT run_id FROM durable_budgets WHERE run_id=$1 FOR UPDATE",
        &[&run],
    )
    .await?;
    let n=tx.execute("INSERT INTO durable_settlements(reservation_id,kind,amount_minor) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&reservation,&kind,&settled]).await?;
    if n == 1 {
        tx.execute("UPDATE durable_budgets SET reserved_minor=reserved_minor-$2,settled_minor=settled_minor+$3 WHERE run_id=$1", &[&run,&amount,&settled]).await?;
    }
    Ok(())
}

impl Store {
    pub async fn enqueue_job(
        &self,
        token: &str,
        script: &str,
        snapshot: &str,
        operation: &str,
        execution: ExecutionMode,
        retry: RetryPolicy,
    ) -> Result<JobSummary, DurableError> {
        let script = normalized_uuid(script)?;
        let snapshot = normalized_uuid(snapshot)?;
        let operation = normalized_uuid(operation)?;
        let request = encode(&(
            snapshot.clone(),
            execution,
            retry.max_attempts(),
            retry.base_seconds(),
            retry.cap_seconds(),
        ))?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let candidate = current_snapshot(&tx, &actor, &script, &snapshot).await?;
        if let Some(prior)=tx.query_opt("SELECT id,request_bytes FROM durable_runs WHERE script_id=$1 AND created_by=$2 AND operation_id=$3", &[&script,&actor,&operation]).await? {
            if prior.get::<_,Vec<u8>>("request_bytes")!=request {return Err(DurableError::Conflict);}let result=summary(&tx,&prior.get::<_,String>("id")).await?;tx.commit().await?;return Ok(result);
        }
        let doc = &candidate.document;
        for line in &doc.resolved {
            validate_execution_mode(execution, line).map_err(|_| DurableError::ProviderDisabled)?;
        }
        if doc.resolved.is_empty() || doc.resolved.len() > 1000 {
            return Err(DurableError::InvalidRequest);
        }
        let run = Uuid::new_v4().to_string();
        let job = Uuid::new_v4().to_string();
        let budget = &doc.settings.settings.budget;
        tx.execute("INSERT INTO durable_runs(id,script_id,snapshot_id,created_by,operation_id,input_digest,mode,request_bytes,max_attempts,base_seconds,cap_seconds) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", &[&run,&script,&snapshot,&actor,&operation,&doc.input_digest,&execution.as_str(),&request,&(retry.max_attempts() as i32),&(retry.base_seconds() as i64),&(retry.cap_seconds() as i64)]).await?;
        tx.execute(
            "INSERT INTO durable_jobs(id,run_id,status) VALUES($1,$2,'queued')",
            &[&job, &run],
        )
        .await?;
        tx.execute("INSERT INTO durable_budgets(run_id,currency,limit_minor,reserved_minor,settled_minor) VALUES($1,$2,$3,0,0)", &[&run,&currency(budget.currency),&(budget.limit_minor as i64)]).await?;
        for line in &doc.resolved {
            let id = Uuid::new_v4().to_string();
            let fingerprint = input_fingerprint(line).map_err(|_| DurableError::Corrupt)?;
            tx.execute("INSERT INTO durable_steps(id,job_id,dialogue_id,input_fingerprint,status) VALUES($1,$2,$3,$4,'pending')", &[&id,&job,&line.dialogue_id,&fingerprint]).await?;
        }
        // Recheck time-sensitive eligibility immediately before commit under authority locks.
        current_snapshot(&tx, &actor, &script, &snapshot).await?;
        let result = summary(&tx, &run).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn job_status(&self, token: &str, id: &str) -> Result<JobSummary, DurableError> {
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let r = run(&tx, &id).await?;
        // Status is readable after approval expires; ownership still checked on every read.
        super::authorize(
            &tx,
            &actor,
            &r.get::<_, String>("script_id"),
            crate::revisions::Action::Review,
            false,
        )
        .await?;
        let result = summary(&tx, &id).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn claim_step(
        &self,
        worker: &str,
        execution: ExecutionMode,
        ttl: LeaseDuration,
    ) -> Result<Option<JobLease>, DurableError> {
        if !valid_worker(worker) {
            return Err(DurableError::InvalidRequest);
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let row=tx.query_opt("SELECT j.id,j.run_id,j.status FROM durable_jobs j JOIN durable_runs r ON r.id=j.run_id WHERE j.status IN ('queued','running') AND r.mode=$1 AND EXISTS(SELECT 1 FROM durable_steps s WHERE s.job_id=j.id AND ((s.status='pending' AND s.not_before<=clock_timestamp()) OR (s.status='leased' AND s.lease_expires_at<=clock_timestamp()))) ORDER BY j.created_at,j.id LIMIT 1 FOR UPDATE OF j SKIP LOCKED", &[&execution.as_str()]).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let job: String = row.get("id");
        let run: String = row.get("run_id");
        let step=tx.query_opt("SELECT id FROM durable_steps WHERE job_id=$1 AND ((status='pending' AND not_before<=clock_timestamp()) OR (status='leased' AND lease_expires_at<=clock_timestamp())) ORDER BY not_before,id LIMIT 1 FOR UPDATE SKIP LOCKED", &[&job]).await?;
        let Some(step) = step else {
            tx.commit().await?;
            return Ok(None);
        };
        let step: String = step.get("id");
        let leased=tx.query_one("UPDATE durable_steps SET status='leased',lease_owner=$2,lease_token=lease_token+1,lease_expires_at=clock_timestamp()+make_interval(secs=>$3::int) WHERE id=$1 RETURNING lease_token", &[&step,&worker,&(ttl.seconds() as i32)]).await?;
        let current = status(&row)?;
        let next = if current == RunStatus::Queued {
            evolve_run(current, RunEvent::Start).map_err(|_| DurableError::Conflict)?
        } else {
            current
        };
        tx.execute(
            "UPDATE durable_jobs SET status=$2,reason=NULL WHERE id=$1",
            &[&job, &next.as_str()],
        )
        .await?;
        let result = JobLease {
            run_id: run,
            job_id: job,
            step_id: step,
            worker_id: worker.into(),
            token: leased.get(0),
        };
        tx.commit().await?;
        Ok(Some(result))
    }
    pub async fn heartbeat_step(
        &self,
        lease: &JobLease,
        ttl: LeaseDuration,
    ) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        check_lease(&tx, lease).await?;
        tx.execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()+make_interval(secs=>$3::int) WHERE id=$1 AND lease_token=$2", &[&lease.step_id,&lease.token,&(ttl.seconds() as i32)]).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn prepare_step(&self, lease: &JobLease) -> Result<StepWork, DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &lease.run_id).await?;
        let doc = match authority(&tx, &r).await {
            Ok(document) => document,
            Err(DurableError::Store(
                StoreError::ProductionBlocked(_)
                | StoreError::Unauthenticated
                | StoreError::Forbidden
                | StoreError::NotFound
                | StoreError::StaleProductionInputs
                | StoreError::StaleRevision(_),
            )) => {
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
        let prior=tx.query_opt("SELECT a.*,o.receipt_bytes,o.output_path,EXISTS(SELECT 1 FROM durable_resolutions x WHERE x.attempt_id=a.id) AS resolved,EXISTS(SELECT 1 FROM durable_resolutions x WHERE x.attempt_id=a.id AND o.created_at>x.created_at) AS resolution_conflict FROM durable_attempts a LEFT JOIN durable_observations o ON o.attempt_id=a.id WHERE a.step_id=$1 ORDER BY attempt_number DESC LIMIT 1", &[&lease.step_id]).await?;
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
                    let waiting: bool=tx.query_one("SELECT COALESCE((SELECT created_at FROM durable_resolutions WHERE attempt_id=$1),(SELECT created_at FROM durable_observations WHERE attempt_id=$1))+make_interval(secs=>$2::int)>clock_timestamp()", &[&prior.get::<_,String>("id"),&(delay as i32)]).await?.get(0);
                    if waiting {
                        tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL,not_before=COALESCE((SELECT created_at FROM durable_resolutions WHERE attempt_id=$2),(SELECT created_at FROM durable_observations WHERE attempt_id=$2))+make_interval(secs=>$3::int) WHERE id=$1", &[&lease.step_id,&prior.get::<_,String>("id"),&(delay as i32)]).await?;
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
        let n=tx.execute("UPDATE durable_budgets SET reserved_minor=reserved_minor+$2 WHERE run_id=$1 AND currency=$3 AND $2<=limit_minor-settled_minor-reserved_minor", &[&lease.run_id,&amount,&currency(budget.currency)]).await?;
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
        tx.execute("INSERT INTO durable_reservations(id,run_id,step_id,amount_minor,currency) VALUES($1,$2,$3,$4,$5)", &[&reservation,&lease.run_id,&lease.step_id,&amount,&currency(budget.currency)]).await?;
        tx.execute("INSERT INTO durable_attempts(id,step_id,lease_token,operation_id,reservation_id,intent_bytes,attempt_number) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&attempt,&lease.step_id,&lease.token,&operation,&reservation,&encode(&intent)?,&((count+1) as i32)]).await?;
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

    /// Preserve an observed result when its private bytes cannot be safely recovered.
    /// This quarantines only the current job and never makes the attempt dispatchable again.
    pub async fn quarantine_recorded_output(
        &self,
        lease: &JobLease,
        invalid: bool,
    ) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        check_lease(&tx, lease).await?;
        let observed: bool = tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM durable_attempts a JOIN durable_observations o ON o.attempt_id=a.id WHERE a.step_id=$1 AND o.output_path IS NOT NULL)",
            &[&lease.step_id],
        ).await?.get(0);
        if !observed {
            return Err(DurableError::Conflict);
        }
        let reason = if invalid {
            "recorded_output_invalid"
        } else {
            "recorded_output_unavailable"
        };
        stop_job(&tx, &lease.job_id, "paused", reason).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Commit a single send boundary after rechecking authority. A duplicate caller must not send.
    pub async fn authorize_dispatch(&self, work: &AttemptWork) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let already_sent: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM durable_dispatches WHERE attempt_id=$1)",
                &[&work.intent.attempt_id],
            )
            .await?
            .get(0);
        if already_sent {
            return Err(DurableError::Conflict);
        }
        let r = run(&tx, &work.lease.run_id).await?;
        let doc = authority(&tx, &r).await?;
        let step = check_lease(&tx, &work.lease).await?;
        let row = tx.query_opt("SELECT intent_bytes FROM durable_attempts WHERE id=$1 AND step_id=$2 AND lease_token=$3", &[&work.intent.attempt_id, &work.lease.step_id, &work.lease.token]).await?.ok_or(DurableError::Conflict)?;
        let intent =
            decode_receipt(&row.get::<_, Vec<u8>>(0)).map_err(|_| DurableError::Corrupt)?;
        let line = doc
            .resolved
            .iter()
            .find(|line| line.dialogue_id == intent.dialogue_id)
            .ok_or(DurableError::Corrupt)?;
        if intent != work.intent
            || mode(&r)? != work.mode
            || input_fingerprint(line).map_err(|_| DurableError::Corrupt)?
                != step.get::<_, String>("input_fingerprint")
            || line != &work.line
        {
            return Err(DurableError::Conflict);
        }
        let known: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM durable_observations WHERE attempt_id=$1) OR EXISTS(SELECT 1 FROM durable_resolutions WHERE attempt_id=$1)", &[&intent.attempt_id]).await?.get(0);
        if known {
            return Err(DurableError::Conflict);
        }
        let n = tx.execute("INSERT INTO durable_dispatches(attempt_id,lease_token) VALUES($1,$2) ON CONFLICT DO NOTHING", &[&intent.attempt_id,&work.lease.token]).await?;
        if n != 1 {
            return Err(DurableError::Conflict);
        }
        authority(&tx, &r).await?;
        check_lease(&tx, &work.lease).await?;
        tx.commit().await?;
        Ok(())
    }

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
        let a=tx.query_opt("SELECT a.*,s.dialogue_id,s.input_fingerprint FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id JOIN durable_jobs j ON j.id=s.job_id WHERE a.id=$1 AND a.step_id=$2 AND s.job_id=$3 AND j.run_id=$4", &[&receipt.attempt_id,&work.lease.step_id,&work.lease.job_id,&work.lease.run_id]).await?.ok_or(DurableError::Conflict)?;
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
            let dispatched: bool = tx
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM durable_dispatches WHERE attempt_id=$1)",
                    &[&receipt.attempt_id],
                )
                .await?
                .get(0);
            if !dispatched {
                return Err(DurableError::Conflict);
            }
        }
        if matches!(receipt.charge, ChargeObservation::Reported { .. })
            && mode(&r)? != ExecutionMode::SyntheticFake
        {
            return Err(DurableError::InvalidRequest);
        }

        tx.query_one(
            "SELECT id FROM durable_jobs WHERE id=$1 FOR UPDATE",
            &[&work.lease.job_id],
        )
        .await?;
        // A denied duplicate caller cannot attest NotSent after another caller committed a send.
        if matches!(
            receipt.outcome,
            ReceiptOutcome::Failure {
                class: FailureClass::Cancelled,
                delivery: DeliveryState::NotSent
            }
        ) {
            let dispatched: bool = tx
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM durable_dispatches WHERE attempt_id=$1)",
                    &[&receipt.attempt_id],
                )
                .await?
                .get(0);
            if dispatched {
                return Err(DurableError::ReconciliationRequired);
            }
        }
        if let Some(prior) = tx
            .query_opt(
                "SELECT receipt_bytes,output_path FROM durable_observations WHERE attempt_id=$1",
                &[&receipt.attempt_id],
            )
            .await?
        {
            if prior.get::<_, Vec<u8>>(0) != bytes
                || prior.get::<_, Option<String>>(1).as_deref() != path
            {
                return Err(DurableError::Conflict);
            }
            tx.commit().await?;
            return Ok(());
        }
        tx.execute("INSERT INTO durable_observations(attempt_id,receipt_bytes,output_path) VALUES($1,$2,$3)", &[&receipt.attempt_id,&bytes,&path]).await?;
        let resolved: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM durable_resolutions WHERE attempt_id=$1)",
                &[&receipt.attempt_id],
            )
            .await?
            .get(0);
        if resolved {
            tx.execute("UPDATE durable_jobs SET status='paused',reason='resolution_contradiction' WHERE id=$1 AND status IN ('queued','running','paused')", &[&work.lease.job_id]).await?;
            tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL WHERE job_id=$1 AND status='leased'", &[&work.lease.job_id]).await?;
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
                    tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL,not_before=clock_timestamp()+make_interval(secs=>$2::int) WHERE id=$1", &[&work.lease.step_id,&(delay as i32)]).await?;
                }
                StepDecision::Accept | StepDecision::Dispatch => {}
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn accept_step(
        &self,
        lease: &JobLease,
        attempt: &str,
        audio: &[u8],
    ) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &lease.run_id).await?;
        let doc = authority(&tx, &r).await?;
        let step = check_lease(&tx, lease).await?;
        let resolved: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM durable_resolutions WHERE attempt_id=$1)",
                &[&attempt],
            )
            .await?
            .get(0);
        if resolved {
            return Err(DurableError::ReconciliationRequired);
        }
        let row=tx.query_opt("SELECT a.intent_bytes,o.receipt_bytes FROM durable_attempts a JOIN durable_observations o ON o.attempt_id=a.id WHERE a.id=$1 AND a.step_id=$2", &[&attempt,&lease.step_id]).await?.ok_or(DurableError::Conflict)?;
        let intent =
            decode_receipt(&row.get::<_, Vec<u8>>(0)).map_err(|_| DurableError::Corrupt)?;
        let receipt =
            decode_receipt(&row.get::<_, Vec<u8>>(1)).map_err(|_| DurableError::Corrupt)?;
        if !matches!(receipt.outcome, ReceiptOutcome::Output { .. })
            || receipt.attempt_id != attempt
            || receipt.operation_id != intent.operation_id
            || receipt.dialogue_id != step.get::<_, String>("dialogue_id")
        {
            return Err(DurableError::Conflict);
        }
        let line = doc
            .resolved
            .iter()
            .find(|l| l.dialogue_id == receipt.dialogue_id)
            .ok_or(DurableError::Corrupt)?;
        if input_fingerprint(line).map_err(|_| DurableError::Corrupt)?
            != step.get::<_, String>("input_fingerprint")
        {
            return Err(DurableError::Corrupt);
        }
        audit_receipt(&doc, &receipt, Some(audio)).map_err(|_| DurableError::Corrupt)?;
        tx.execute("UPDATE durable_steps SET status='succeeded',accepted_attempt_id=$2,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1", &[&lease.step_id,&attempt]).await?;
        let pending:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM durable_steps WHERE job_id=$1 AND status<>'succeeded')", &[&lease.job_id]).await?.get(0);
        if !pending {
            tx.execute(
                "UPDATE durable_jobs SET status='completed',reason=NULL WHERE id=$1",
                &[&lease.job_id],
            )
            .await?;
        }
        authority(&tx, &r).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn control_job(
        &self,
        token: &str,
        id: &str,
        operation: &str,
        event: RunEvent,
    ) -> Result<JobSummary, DurableError> {
        let id = normalized_uuid(id)?;
        let operation = normalized_uuid(operation)?;
        let action = match event {
            RunEvent::Pause => "pause",
            RunEvent::Resume => "resume",
            RunEvent::Cancel => "cancel",
            RunEvent::Retry => "retry",
            RunEvent::Start | RunEvent::Complete | RunEvent::Fail => {
                return Err(DurableError::InvalidRequest)
            }
        };
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let r = run(&tx, &id).await?;
        super::authorize(
            &tx,
            &actor,
            &r.get::<_, String>("script_id"),
            crate::revisions::Action::Review,
            true,
        )
        .await?;
        let job = tx
            .query_one(
                "SELECT * FROM durable_jobs WHERE run_id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let job_id: String = job.get("id");
        if let Some(prior)=tx.query_opt("SELECT action FROM durable_controls WHERE job_id=$1 AND actor_id=$2 AND operation_id=$3", &[&job_id,&actor,&operation]).await?{if prior.get::<_,String>(0)!=action{return Err(DurableError::Conflict);}let result=summary(&tx,&id).await?;tx.commit().await?;return Ok(result);}
        if matches!(event, RunEvent::Resume | RunEvent::Retry) {
            authority(&tx, &r).await?;
            let unresolved:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id LEFT JOIN durable_observations o ON o.attempt_id=a.id LEFT JOIN durable_resolutions x ON x.attempt_id=a.id WHERE s.job_id=$1 AND ((x.attempt_id IS NOT NULL AND o.created_at>x.created_at) OR (x.attempt_id IS NULL AND (o.attempt_id IS NULL OR convert_from(o.receipt_bytes,'UTF8')::jsonb#>>'{outcome,delivery}'='may_have_executed'))))", &[&job_id]).await?.get(0);
            if unresolved {
                return Err(DurableError::ReconciliationRequired);
            }
        }
        let next = evolve_run(status(&job)?, event).map_err(|_| DurableError::Conflict)?;
        let reason = if next == RunStatus::Paused {
            Some("creator_requested")
        } else {
            None
        };
        tx.execute("INSERT INTO durable_controls(id,job_id,actor_id,operation_id,action) VALUES($1,$2,$3,$4,$5)", &[&Uuid::new_v4().to_string(),&job_id,&actor,&operation,&action]).await?;
        tx.execute(
            "UPDATE durable_jobs SET status=$2,reason=$3 WHERE id=$1",
            &[&job_id, &next.as_str(), &reason],
        )
        .await?;
        let step_status = if next == RunStatus::Cancelled {
            "cancelled"
        } else {
            "pending"
        };
        tx.execute("UPDATE durable_steps SET status=$2,lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL WHERE job_id=$1 AND status IN ('pending','leased','failed')", &[&job_id,&step_status]).await?;
        let result = summary(&tx, &id).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Explicit owner attestation resolves only non-execution. Unknown results never auto-dispatch.
    pub async fn resolve_not_executed(
        &self,
        token: &str,
        id: &str,
        attempt: &str,
        evidence: &str,
    ) -> Result<JobSummary, DurableError> {
        if evidence.trim().is_empty()
            || evidence.len() > 2048
            || evidence.chars().any(char::is_control)
        {
            return Err(DurableError::InvalidRequest);
        }
        let id = normalized_uuid(id)?;
        let attempt = normalized_uuid(attempt)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let r = run(&tx, &id).await?;
        authority(&tx, &r).await?;
        if actor != r.get::<_, String>("created_by") {
            return Err(DurableError::Store(StoreError::Forbidden));
        }
        let job = tx
            .query_one(
                "SELECT id,status FROM durable_jobs WHERE run_id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let job_id: String = job.get("id");
        if status(&job)? != RunStatus::Paused {
            return Err(DurableError::Conflict);
        }
        let row=tx.query_opt("SELECT a.reservation_id,o.receipt_bytes FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id LEFT JOIN durable_observations o ON o.attempt_id=a.id WHERE a.id=$1 AND s.job_id=$2", &[&attempt,&job_id]).await?.ok_or(DurableError::Conflict)?;
        let receipt = row
            .get::<_, Option<Vec<u8>>>("receipt_bytes")
            .as_deref()
            .map(decode_receipt)
            .transpose()
            .map_err(|_| DurableError::Corrupt)?;
        if !matches!(
            outcome(receipt.as_ref(), false),
            AttemptOutcome::Unresolved
                | AttemptOutcome::Failure {
                    delivery: DeliveryState::MayHaveExecuted,
                    ..
                }
        ) {
            return Err(DurableError::Conflict);
        }
        if let Some(prior) = tx
            .query_opt(
                "SELECT evidence FROM durable_resolutions WHERE attempt_id=$1",
                &[&attempt],
            )
            .await?
        {
            if prior.get::<_, String>(0) != evidence {
                return Err(DurableError::Conflict);
            }
        } else {
            tx.execute("INSERT INTO durable_resolutions(attempt_id,resolved_by,evidence,kind) VALUES($1,$2,$3,'not_executed')", &[&attempt,&actor,&evidence]).await?;
        }
        settle(
            &tx,
            &row.get::<_, String>("reservation_id"),
            "released",
            None,
        )
        .await?;
        let result = summary(&tx, &id).await?;
        tx.commit().await?;
        Ok(result)
    }
}
