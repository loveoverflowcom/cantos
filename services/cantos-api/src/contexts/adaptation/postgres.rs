//! Adaptation tables: runs with their frozen inputs, caller submissions, admitted proposals,
//! provider observations, cancellations and acceptance links. Each decoder re-checks the stored
//! record against its typed digest and the columns that index it.
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationProblem, AdaptationProposal, AdaptationStatus,
    AdaptationSubmissionReceipt, AdaptationSubmissionStatus, ImportOutcome,
    SubmitAdaptationProposalRequest,
};
use sha2::{Digest, Sha256};
use tokio_postgres::Row;

use super::domain::{
    generation_record_id, rights_record_id, typed_digest, FrozenAdaptation, FrozenContext,
    Observation, INPUT_A1_DIGEST, INPUT_C1_DIGEST, MAX_REQUEST_BYTES, PROPOSAL_DIGEST,
    SUBMISSION_DIGEST,
};
use crate::contexts::editorial::export_digest;
use crate::platform::postgres::{normalized_uuid, StoreError, Tx};
use crate::script_ir::read_canonical_script;

const RUN_COLUMNS: &str = "input_version,id,owner_id,operation_id,source_id,script_id,frozen_input::text AS frozen_input,input_digest,status,problem::text AS problem,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at,to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";
const SUBMISSION_COLUMNS: &str = "id,run_id,owner_id,operation_id,request::text AS request,receipt::text AS receipt,receipt_digest,to_char(submitted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS submitted_at";

fn corrupt<T>(_: T) -> StoreError {
    StoreError::CorruptRevision
}

/// Per-actor advisory locks that serialize one kind of adaptation operation. A hash collision
/// only adds contention; queries still enforce the exact owner.
#[derive(Clone, Copy)]
pub(super) enum ActorLock {
    Context,
    Submit,
    Accept,
    Cancel,
}

pub(super) async fn lock_actor(
    tx: &Tx<'_>,
    lock: ActorLock,
    actor: &str,
) -> Result<(), StoreError> {
    let query = match lock {
        ActorLock::Context => {
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation:' || $1::text,0))"
        }
        ActorLock::Submit => {
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-submit:' || $1::text,0))"
        }
        ActorLock::Accept => {
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-accept:' || $1::text,0))"
        }
        ActorLock::Cancel => {
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-cancel:' || $1::text,0))"
        }
    };
    tx.query_one(query, &[&actor]).await?;
    Ok(())
}

fn decode_status(status: &str) -> Result<AdaptationStatus, StoreError> {
    match status {
        "awaiting_proposal" => Ok(AdaptationStatus::AwaitingProposal),
        "queued" => Ok(AdaptationStatus::Queued),
        "running" => Ok(AdaptationStatus::Running),
        "succeeded" => Ok(AdaptationStatus::Succeeded),
        "invalid_output" => Ok(AdaptationStatus::InvalidOutput),
        "failed" => Ok(AdaptationStatus::Failed),
        "ambiguous" => Ok(AdaptationStatus::Ambiguous),
        "cancelled" => Ok(AdaptationStatus::Cancelled),
        "accepted" => Ok(AdaptationStatus::Accepted),
        _ => Err(StoreError::CorruptRevision),
    }
}

/// One `adaptation_runs` row, owned by the requesting actor and locked for update.
pub(super) struct RunRow(Row);

impl RunRow {
    pub(super) fn input_version(&self) -> String {
        self.0.get("input_version")
    }

    pub(super) fn input_digest(&self) -> String {
        self.0.get("input_digest")
    }

    pub(super) fn recorded_at(&self) -> String {
        self.0.get("recorded_at")
    }

    pub(super) fn updated_at(&self) -> String {
        self.0.get("updated_at")
    }

    pub(super) fn status(&self) -> Result<AdaptationStatus, StoreError> {
        decode_status(&self.0.get::<_, String>("status"))
    }

    pub(super) fn problem(&self) -> Result<Option<AdaptationProblem>, StoreError> {
        self.0
            .get::<_, Option<String>>("problem")
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(corrupt)
    }

    /// The retired `a1` preimage, bound to this row's identities.
    pub(super) fn frozen_adaptation(&self) -> Result<FrozenAdaptation, StoreError> {
        let row = &self.0;
        let frozen: FrozenAdaptation =
            serde_json::from_str(&row.get::<_, String>("frozen_input")).map_err(corrupt)?;
        if typed_digest(INPUT_A1_DIGEST, &frozen).map_err(corrupt)?
            != row.get::<_, String>("input_digest")
            || frozen.request.operation_id != row.get::<_, String>("operation_id")
            || frozen.request.source_id != row.get::<_, String>("source_id")
            || frozen.request.script_id != row.get::<_, String>("script_id")
            || frozen.source.imported_by != row.get::<_, String>("owner_id")
            || frozen.source.id != frozen.request.source_id
            || !frozen.request.rights_authorization
            || frozen.request.expected_provider != frozen.provider
            || frozen.generation_record_id != generation_record_id(&row.get::<_, String>("id"))
            || frozen.rights_record_id != rights_record_id(&row.get::<_, String>("id"))
        {
            return Err(StoreError::CorruptRevision);
        }
        normalized_uuid(&frozen.request.operation_id).map_err(corrupt)?;
        normalized_uuid(&frozen.request.script_id).map_err(corrupt)?;
        match &frozen.base {
            Some(base) => {
                let script = read_canonical_script(base.script_json.as_bytes()).map_err(corrupt)?;
                if base.script_id != frozen.request.script_id
                    || base.revision != frozen.request.expected_revision
                    || base.content_digest != script.content_digest().to_string()
                    || base.export_digest != export_digest(base.script_json.as_bytes())
                {
                    return Err(StoreError::CorruptRevision);
                }
            }
            None if frozen.request.expected_revision != 0 => {
                return Err(StoreError::CorruptRevision)
            }
            None => (),
        }
        Ok(frozen)
    }

    /// The caller-owned `c1` context, bound to this row's identities.
    pub(super) fn frozen_context(&self) -> Result<FrozenContext, StoreError> {
        let row = &self.0;
        let context: FrozenContext =
            serde_json::from_str(&row.get::<_, String>("frozen_input")).map_err(corrupt)?;
        let extractor = match &context.source.outcome {
            ImportOutcome::Parsed { extraction } => &extraction.extractor_version,
            ImportOutcome::Failed { .. } => return Err(StoreError::CorruptRevision),
        };
        if row.get::<_, String>("input_version") != "c1"
            || context.context_version != "c1"
            || typed_digest(INPUT_C1_DIGEST, &context).map_err(corrupt)?
                != row.get::<_, String>("input_digest")
            || context.request.operation_id != row.get::<_, String>("operation_id")
            || context.request.script_id != row.get::<_, String>("script_id")
            || context.request.source_id != row.get::<_, String>("source_id")
            || context.request.source_id != context.source.id
            || context.request.source_sha256 != context.source.sha256
            || &context.request.extractor_version != extractor
            || context.source.imported_by != row.get::<_, String>("owner_id")
            || !context.request.rights_authorization
            || context.system_prompt.len() + context.user_prompt.len() > MAX_REQUEST_BYTES
            || context.system_prompt.is_empty()
            || context.user_prompt.is_empty()
            || context.proposal_schema_json.len() > 65536
            || context.generation_record_id != generation_record_id(&row.get::<_, String>("id"))
            || context.rights_record_id != rights_record_id(&row.get::<_, String>("id"))
        {
            return Err(StoreError::CorruptRevision);
        }
        normalized_uuid(&context.request.operation_id).map_err(corrupt)?;
        normalized_uuid(&context.request.script_id).map_err(corrupt)?;
        if let Some(base) = &context.base {
            let content = read_canonical_script(base.script_json.as_bytes()).map_err(corrupt)?;
            if base.script_id != context.request.script_id
                || base.revision != context.request.expected_revision
                || base.content_digest != content.content_digest().to_string()
                || base.export_digest != export_digest(base.script_json.as_bytes())
            {
                return Err(StoreError::CorruptRevision);
            }
        } else if context.request.expected_revision != 0 {
            return Err(StoreError::CorruptRevision);
        }
        Ok(context)
    }
}

/// Locks the actor's run before any expiry or transition, so cancellation, completion and read
/// recovery share one transition order.
pub(super) async fn lock_run(
    tx: &Tx<'_>,
    actor: &str,
    id: &str,
) -> Result<Option<RunRow>, StoreError> {
    let query =
        format!("SELECT {RUN_COLUMNS} FROM adaptation_runs WHERE id=$1 AND owner_id=$2 FOR UPDATE");
    Ok(tx.query_opt(&query, &[&id, &actor]).await?.map(RunRow))
}

/// The same locked row again, after this transaction changed it.
pub(super) async fn reload_run(tx: &Tx<'_>, actor: &str, id: &str) -> Result<RunRow, StoreError> {
    let query =
        format!("SELECT {RUN_COLUMNS} FROM adaptation_runs WHERE id=$1 AND owner_id=$2 FOR UPDATE");
    Ok(RunRow(tx.query_one(&query, &[&id, &actor]).await?))
}

pub(super) async fn input_version(
    tx: &Tx<'_>,
    actor: &str,
    id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT input_version FROM adaptation_runs WHERE id=$1 AND owner_id=$2",
            &[&id, &actor],
        )
        .await?
        .map(|row| row.get(0)))
}

pub(super) async fn run_by_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<(String, String)>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT id,input_version FROM adaptation_runs WHERE owner_id=$1 AND operation_id=$2",
            &[&actor, &operation],
        )
        .await?
        .map(|row| (row.get(0), row.get(1))))
}

pub(super) struct NewContextRun<'a> {
    pub id: &'a str,
    pub actor: &'a str,
    pub operation: &'a str,
    pub source: &'a str,
    pub script: &'a str,
    pub frozen_json: &'a str,
    pub digest: &'a str,
}

pub(super) async fn insert_context_run(
    tx: &Tx<'_>,
    new: NewContextRun<'_>,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO adaptation_runs(id,owner_id,operation_id,source_id,script_id,frozen_input,input_digest,status,input_version) VALUES($1,$2,$3,$4,$5,$6::text::jsonb,$7,'awaiting_proposal','c1')",&[&new.id,&new.actor,&new.operation,&new.source,&new.script,&new.frozen_json,&new.digest]).await?;
    Ok(())
}

/// A retired provider dispatch whose deadline passed has an unknown outcome: it becomes
/// ambiguous with an interruption observation, and is never resent.
pub(super) async fn expire_dispatch(tx: &Tx<'_>, id: &str) -> Result<(), StoreError> {
    let problem = || AdaptationProblem {
        code: "dispatch_outcome_unknown".into(),
        issues: vec![],
    };
    let reason = serde_json::to_string(&problem()).map_err(|_| StoreError::Unavailable)?;
    let changed = tx.execute("UPDATE adaptation_runs SET status='ambiguous',problem=$2::text::jsonb,dispatch_deadline=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='running' AND dispatch_deadline<=clock_timestamp()", &[&id,&reason]).await?;
    if changed == 1 {
        let observation = Observation {
            problem: Some(problem()),
            usage: None,
            cost: None,
            output_sha256: None,
            output_byte_len: None,
        };
        let json = serde_json::to_string(&observation).map_err(|_| StoreError::Unavailable)?;
        tx.execute("INSERT INTO adaptation_observations(run_id,kind,observation) VALUES($1,'interrupted',$2::text::jsonb)", &[&id,&json]).await?;
    }
    Ok(())
}

/// The admitted proposal with its stored digest, if one was recorded.
pub(super) async fn proposal(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<(AdaptationProposal, String)>, StoreError> {
    tx.query_opt(
        "SELECT proposal::text,digest FROM adaptation_proposals WHERE run_id=$1",
        &[&id],
    )
    .await?
    .map(|row| {
        let proposal: AdaptationProposal =
            serde_json::from_str(&row.get::<_, String>(0)).map_err(corrupt)?;
        Ok((proposal, row.get(1)))
    })
    .transpose()
}

pub(super) async fn insert_proposal(
    tx: &Tx<'_>,
    id: &str,
    proposal: &AdaptationProposal,
) -> Result<(), StoreError> {
    let json = serde_json::to_string(proposal).map_err(|_| StoreError::Unavailable)?;
    let digest = typed_digest(PROPOSAL_DIGEST, proposal).map_err(corrupt)?;
    tx.execute(
        "INSERT INTO adaptation_proposals(run_id,proposal,digest) VALUES($1,$2::text::jsonb,$3)",
        &[&id, &json, &digest],
    )
    .await?;
    Ok(())
}

/// Moves a caller context from awaiting a proposal to succeeded; false if it had moved already.
pub(super) async fn mark_proposal_received(tx: &Tx<'_>, id: &str) -> Result<bool, StoreError> {
    let changed=tx.execute("UPDATE adaptation_runs SET status='succeeded',updated_at=clock_timestamp() WHERE id=$1 AND input_version='c1' AND status='awaiting_proposal'",&[&id]).await?;
    Ok(changed == 1)
}

pub(super) async fn provider_observation(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<Observation>, StoreError> {
    tx.query_opt("SELECT observation::text FROM adaptation_observations WHERE run_id=$1 AND kind='provider_result'", &[&id]).await?.map(|row| serde_json::from_str::<Observation>(&row.get::<_,String>(0))).transpose().map_err(corrupt)
}

/// The script revision an acceptance of this run created, if any.
pub(super) async fn acceptance_link(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<(String, i64)>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT script_id,revision FROM adaptation_acceptances WHERE run_id=$1",
            &[&id],
        )
        .await?
        .map(|row| (row.get(0), row.get(1))))
}

pub(super) async fn acceptance_by_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<(String, AcceptAdaptationRequest)>, StoreError> {
    tx.query_opt(
        "SELECT run_id,request::text FROM adaptation_acceptances WHERE owner_id=$1 AND operation_id=$2",
        &[&actor, &operation],
    )
    .await?
    .map(|row| {
        let prior: AcceptAdaptationRequest =
            serde_json::from_str(&row.get::<_, String>(1)).map_err(corrupt)?;
        Ok((row.get(0), prior))
    })
    .transpose()
}

pub(super) struct NewAcceptance<'a> {
    pub id: &'a str,
    pub actor: &'a str,
    pub request: &'a AcceptAdaptationRequest,
    pub script: &'a str,
    pub revision: i64,
}

pub(super) async fn insert_acceptance(
    tx: &Tx<'_>,
    new: NewAcceptance<'_>,
) -> Result<(), StoreError> {
    let json = serde_json::to_string(new.request).map_err(|_| StoreError::Unavailable)?;
    tx.execute("INSERT INTO adaptation_acceptances(run_id,owner_id,operation_id,request,script_id,revision) VALUES($1,$2,$3,$4::text::jsonb,$5,$6)", &[&new.id,&new.actor,&new.request.operation_id,&json,&new.script,&new.revision]).await?;
    Ok(())
}

pub(super) async fn mark_accepted(tx: &Tx<'_>, id: &str) -> Result<(), StoreError> {
    tx.execute("UPDATE adaptation_runs SET status='accepted',updated_at=clock_timestamp() WHERE id=$1 AND status='succeeded'", &[&id]).await?;
    Ok(())
}

pub(super) async fn cancellation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT run_id FROM adaptation_cancellations WHERE owner_id=$1 AND operation_id=$2",
            &[&actor, &operation],
        )
        .await?
        .map(|row| row.get(0)))
}

pub(super) async fn insert_cancellation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
    id: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO adaptation_cancellations(owner_id,operation_id,run_id) VALUES($1,$2,$3)",
        &[&actor, &operation, &id],
    )
    .await?;
    Ok(())
}

/// A retired provider run also drops its dispatch deadline.
pub(super) async fn cancel_legacy_run(tx: &Tx<'_>, id: &str) -> Result<(), StoreError> {
    tx.execute("UPDATE adaptation_runs SET status='cancelled',dispatch_deadline=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&id]).await?;
    Ok(())
}

pub(super) async fn cancel_context(tx: &Tx<'_>, id: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE adaptation_runs SET status='cancelled',updated_at=clock_timestamp() WHERE id=$1",
        &[&id],
    )
    .await?;
    Ok(())
}

/// A stored submission and the request that produced it, checked against its receipt digest.
fn decode_submission(
    row: &Row,
) -> Result<(AdaptationSubmissionReceipt, SubmitAdaptationProposalRequest), StoreError> {
    let receipt: AdaptationSubmissionReceipt =
        serde_json::from_str(&row.get::<_, String>("receipt")).map_err(corrupt)?;
    let request: SubmitAdaptationProposalRequest =
        serde_json::from_str(&row.get::<_, String>("request")).map_err(corrupt)?;
    if typed_digest(SUBMISSION_DIGEST, &receipt).map_err(corrupt)?
        != row.get::<_, String>("receipt_digest")
        || receipt.id != row.get::<_, String>("id")
        || receipt.run_id != row.get::<_, String>("run_id")
        || receipt.submitted_by != row.get::<_, String>("owner_id")
        || receipt.operation_id != row.get::<_, String>("operation_id")
        || receipt.submitted_at != row.get::<_, String>("submitted_at")
        || request.operation_id != receipt.operation_id
        || request.context_digest != receipt.context_digest
        || request.generation != receipt.generation
        || format!("{:x}", Sha256::digest(request.proposal_json.as_bytes()))
            != receipt.output_sha256
        || match receipt.status {
            AdaptationSubmissionStatus::Valid => {
                receipt.proposal.is_none() || receipt.problem.is_some()
            }
            AdaptationSubmissionStatus::Invalid => {
                receipt.proposal.is_some() || receipt.problem.is_none()
            }
        }
    {
        return Err(StoreError::CorruptRevision);
    }
    normalized_uuid(&receipt.operation_id).map_err(corrupt)?;
    Ok((receipt, request))
}

pub(super) async fn latest_submission(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<AdaptationSubmissionReceipt>, StoreError> {
    let query=format!("SELECT {SUBMISSION_COLUMNS} FROM adaptation_submissions WHERE run_id=$1 ORDER BY submission_number DESC LIMIT 1");
    tx.query_opt(&query, &[&id])
        .await?
        .map(|row| decode_submission(&row).map(|(receipt, _)| receipt))
        .transpose()
}

pub(super) async fn submission_by_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<(AdaptationSubmissionReceipt, SubmitAdaptationProposalRequest)>, StoreError> {
    let query=format!("SELECT {SUBMISSION_COLUMNS} FROM adaptation_submissions WHERE owner_id=$1 AND operation_id=$2");
    tx.query_opt(&query, &[&actor, &operation])
        .await?
        .map(|row| decode_submission(&row))
        .transpose()
}

/// The database clock, formatted like every stored timestamp.
pub(super) async fn submission_time(tx: &Tx<'_>) -> Result<String, StoreError> {
    Ok(tx.query_one("SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",&[]).await?.get(0))
}

pub(super) async fn next_submission_number(tx: &Tx<'_>, id: &str) -> Result<i64, StoreError> {
    Ok(tx.query_one("SELECT coalesce(max(submission_number),0)+1 FROM adaptation_submissions WHERE run_id=$1",&[&id]).await?.get(0))
}

pub(super) struct NewSubmission<'a> {
    pub id: &'a str,
    pub actor: &'a str,
    pub number: i64,
    pub request: &'a SubmitAdaptationProposalRequest,
    pub receipt: &'a AdaptationSubmissionReceipt,
}

pub(super) async fn insert_submission(
    tx: &Tx<'_>,
    new: NewSubmission<'_>,
) -> Result<(), StoreError> {
    let request_json = serde_json::to_string(new.request).map_err(|_| StoreError::Unavailable)?;
    let receipt_json = serde_json::to_string(new.receipt).map_err(|_| StoreError::Unavailable)?;
    let digest = typed_digest(SUBMISSION_DIGEST, new.receipt).map_err(corrupt)?;
    tx.execute("INSERT INTO adaptation_submissions(id,run_id,owner_id,operation_id,submission_number,request,receipt,receipt_digest,submitted_at) VALUES($1,$2,$3,$4,$5,$6::text::jsonb,$7::text::jsonb,$8,$9::text::timestamptz)",&[&new.receipt.id,&new.id,&new.actor,&new.request.operation_id,&new.number,&request_json,&receipt_json,&digest,&new.receipt.submitted_at]).await?;
    Ok(())
}
