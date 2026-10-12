//! Verified-clip registry shell on the durable job authority.
//!
//! Registration happens inside `accept_step`'s transaction, so an accepted step always has its
//! clip row. Reuse is a separate, attempt-less way for a step to succeed: the worker asks for
//! candidates, probes their objects outside any transaction, then selects one under the same
//! fence, authority and job-state checks that gate an accepted result. A reused step has no
//! attempt, reservation, dispatch or observation row: a cache hit is never a provider send.

use std::{collections::BTreeMap, path::Path};

use cantos_contracts::{FrozenProductionDocument, ResolvedSynthesisInput};
use serde::Serialize;
use tokio_postgres::Row;
use uuid::Uuid;

use super::{authority, check_lease, is_authority_boundary, mode, run, DurableError, JobLease};
use crate::{
    contexts::synthesis::{
        fingerprint::{SpeechFingerprint, SpeechInput},
        jobs::{validate_execution_mode, ExecutionMode},
        media::clip_probe::{probe_candidates, ClipProber, VerifiedObject},
        receipt::{ReceiptOutcome, TtsReceipt},
        regeneration::{
            plan_regeneration, PlanGate, PlanInputs, ProductionShape, RegenerationPlan,
        },
        reuse::{
            resolve_reuse, ArtifactProbe, ClipCandidate, ClipRequest, RegisteredClip,
            ReuseDecision, RightsStanding, StoredClip,
        },
    },
    platform::postgres::{normalized_uuid, Store},
};

/// Bound on registry rows probed per fingerprint, oldest first. A fingerprint with more
/// registered clips than this simply misses; it can never probe without limit.
pub const MAX_CANDIDATES_PER_FINGERPRINT: i64 = 16;

const CLIP_COLUMNS: &str = "id,registration_seq,owner_id,speech_fingerprint,producer,voice_rights_record_id,output_contract,audio_sha256,attempt_id,output_path";

fn stored_clip(row: &Row) -> Result<StoredClip, DurableError> {
    let producer = row
        .get::<_, String>("producer")
        .parse::<ExecutionMode>()
        .map_err(|_| DurableError::Corrupt)?;
    Ok(StoredClip {
        clip: RegisteredClip {
            id: row.get("id"),
            sequence: row.get("registration_seq"),
            fingerprint: row
                .get::<_, String>("speech_fingerprint")
                .parse()
                .map_err(|_| DurableError::Corrupt)?,
            owner_id: row.get("owner_id"),
            producer,
            voice_rights_record_id: row.get("voice_rights_record_id"),
            output_contract: row.get("output_contract"),
            audio_sha256: row
                .get::<_, String>("audio_sha256")
                .parse()
                .map_err(|_| DurableError::Corrupt)?,
        },
        attempt_id: row.get("attempt_id"),
        output_path: row.get("output_path"),
    })
}

/// Record the audited bytes of an accepted attempt. A line with no position-free identity (an
/// unpinned model) is not registered and can never be reused.
#[allow(clippy::too_many_arguments)]
pub(super) async fn register_accepted_clip(
    tx: &deadpool_postgres::Transaction<'_>,
    run_row: &Row,
    step_fingerprint: Option<&str>,
    owner_id: &str,
    line: &ResolvedSynthesisInput,
    receipt: &TtsReceipt,
    audio: &[u8],
    output_path: Option<&str>,
    attempt: &str,
) -> Result<(), DurableError> {
    let Ok(input) = SpeechInput::from_resolved(line) else {
        return Ok(());
    };
    // Steps created before the registry have no position-free identity, and a step whose stored
    // identity disagrees with the recomputed one cannot be bound to a clip. Neither is an error:
    // acceptance proceeds and the line simply never becomes reusable.
    if step_fingerprint != Some(input.fingerprint().as_str()) {
        return Ok(());
    }
    let (Some(path), ReceiptOutcome::Output { audio_sha256 }) = (output_path, &receipt.outcome)
    else {
        return Err(DurableError::Corrupt);
    };
    let producer = mode(run_row)?;
    tx.execute(
        "INSERT INTO speech_clips(id,owner_id,speech_fingerprint,fingerprint_input,producer,voice_rights_record_id,output_contract,audio_sha256,audio_bytes,output_path,attempt_id) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (attempt_id) DO NOTHING",
        &[
            &Uuid::new_v4().to_string(),
            &owner_id,
            &input.fingerprint().as_str(),
            &input.canonical_bytes(),
            &producer.as_str(),
            &line.voice_rights_record_id,
            &line.output_contract,
            &audio_sha256,
            &(audio.len() as i64),
            &path,
            &attempt,
        ],
    )
    .await?;
    Ok(())
}

/// The facts a step needs before a reuse lookup, or `None` when reuse is not defined for it:
/// a step from before the registry, an unpinned model, an unsupported execution input, or any
/// provider attempt already on record (an ambiguous attempt is reconciled, never bypassed).
async fn request_for(
    tx: &deadpool_postgres::Transaction<'_>,
    run_row: &Row,
    document: &FrozenProductionDocument,
    step: &Row,
) -> Result<Option<ClipRequest>, DurableError> {
    let line = document
        .resolved
        .iter()
        .find(|line| line.dialogue_id == step.get::<_, String>("dialogue_id"))
        .ok_or(DurableError::Corrupt)?;
    let execution = mode(run_row)?;
    if validate_execution_mode(execution, line).is_err() {
        return Ok(None);
    }
    let Some(stored) = step.get::<_, Option<String>>("speech_fingerprint") else {
        return Ok(None);
    };
    let Ok(input) = SpeechInput::from_resolved(line) else {
        return Ok(None);
    };
    let fingerprint = input.fingerprint();
    // A stored identity that disagrees with the recomputed one (for example after an encoder
    // fix) means this step cannot be matched safely. Reuse is simply not defined for it: the
    // ordinary dispatch path has its own linkage check and takes over.
    if fingerprint.as_str() != stored {
        return Ok(None);
    }
    let attempted: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM durable_attempts WHERE step_id=$1)",
            &[&step.get::<_, String>("id")],
        )
        .await?
        .get(0);
    if attempted {
        return Ok(None);
    }
    Ok(Some(ClipRequest {
        fingerprint,
        owner_id: document.owner_id.clone(),
        producer: execution,
        voice_rights_record_id: line.voice_rights_record_id.clone(),
        output_contract: line.output_contract.clone(),
    }))
}

#[derive(Debug, Clone)]
pub struct ReusePrepared {
    pub request: ClipRequest,
    pub candidates: Vec<StoredClip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReuseSelected {
    pub clip_id: String,
    pub job_completed: bool,
}

/// Everything a planning command needs from PostgreSQL, gathered without file I/O.
#[derive(Debug)]
pub struct PlanFacts {
    pub owner_id: String,
    pub gate: PlanGate,
    pub previous: Option<ProductionShape>,
    pub next: ProductionShape,
    pub clips: BTreeMap<SpeechFingerprint, Vec<StoredClip>>,
}

/// Whether the plan's cache hits rest on a fresh look at the stored objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectProbes {
    Performed,
    /// No output root was supplied, so no object was verified and no clip counts as a hit.
    NotPerformed,
}

#[derive(Debug, Serialize)]
pub struct PlanReport {
    pub script_id: String,
    pub previous_snapshot_id: Option<String>,
    pub next_snapshot_id: String,
    pub execution_mode: ExecutionMode,
    pub object_probes: ObjectProbes,
    pub plan: RegenerationPlan,
}

impl Store {
    /// Registry candidates for a leased step, or `None` when reuse is not available for it. A
    /// lapsed authority also yields `None`: `prepare_step` then pauses the job exactly as it
    /// would without the registry, and no clip can widen a closed gate.
    pub async fn reuse_candidates(
        &self,
        lease: &JobLease,
    ) -> Result<Option<ReusePrepared>, DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &lease.run_id).await?;
        let document = match authority(&tx, &r).await {
            Ok(document) => document,
            Err(DurableError::Store(error)) if is_authority_boundary(&error) => return Ok(None),
            Err(error) => return Err(error),
        };
        let step = check_lease(&tx, lease).await?;
        let Some(request) = request_for(&tx, &r, &document, &step).await? else {
            return Ok(None);
        };
        // Scope is filtered before the bound so clips that could never be reused (another
        // producer, voice-rights record or contract) cannot crowd out valid ones. This only
        // narrows the set: `resolve_reuse` still decides, again, inside the selecting transaction.
        let rows = tx
            .query(
                &format!("SELECT {CLIP_COLUMNS} FROM speech_clips WHERE owner_id=$1 AND speech_fingerprint=$2 AND producer=$3 AND voice_rights_record_id=$4 AND output_contract=$5 ORDER BY registration_seq LIMIT $6"),
                &[
                    &request.owner_id,
                    &request.fingerprint.as_str(),
                    &request.producer.as_str(),
                    &request.voice_rights_record_id,
                    &request.output_contract,
                    &MAX_CANDIDATES_PER_FINGERPRINT,
                ],
            )
            .await?;
        let candidates = rows.iter().map(stored_clip).collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(Some(ReusePrepared {
            request,
            candidates,
        }))
    }

    /// Satisfy a leased step with an already registered clip. Authority, lease, job state, the
    /// absence of any provider attempt and the whole reuse decision are re-evaluated under the
    /// job lock with the shell's fresh probe result; any disagreement is a conflict and the
    /// worker falls back to the ordinary dispatch path. Creates no attempt or reservation.
    pub async fn select_reused_clip(
        &self,
        lease: &JobLease,
        verified: &VerifiedObject,
    ) -> Result<ReuseSelected, DurableError> {
        let clip_id =
            normalized_uuid(verified.clip_id()).map_err(|_| DurableError::InvalidRequest)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &lease.run_id).await?;
        // A lapse since the lookup is a disagreement like any other: the worker falls back to
        // `prepare_step`, which re-reads authority and pauses the job with its usual reason.
        let document = match authority(&tx, &r).await {
            Ok(document) => document,
            Err(DurableError::Store(error)) if is_authority_boundary(&error) => {
                return Err(DurableError::Conflict)
            }
            Err(error) => return Err(error),
        };
        let step = check_lease(&tx, lease).await?;
        let request = request_for(&tx, &r, &document, &step)
            .await?
            .ok_or(DurableError::Conflict)?;
        let row = tx
            .query_opt(
                &format!("SELECT {CLIP_COLUMNS} FROM speech_clips WHERE id=$1"),
                &[&clip_id],
            )
            .await?
            .ok_or(DurableError::Conflict)?;
        let stored = stored_clip(&row)?;
        let candidate = ClipCandidate {
            clip: stored.clip,
            probe: ArtifactProbe::Present {
                audio_sha256: verified.audio_sha256().clone(),
            },
        };
        match resolve_reuse(&request, RightsStanding::Current, &[candidate]) {
            ReuseDecision::Reuse {
                clip_id: chosen, ..
            } if chosen == clip_id => {}
            _ => return Err(DurableError::Conflict),
        }
        tx.execute(
            "UPDATE durable_steps SET status='succeeded',reused_clip_id=$2,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1",
            &[&lease.step_id, &clip_id],
        )
        .await?;
        let pending: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM durable_steps WHERE job_id=$1 AND status<>'succeeded')",
                &[&lease.job_id],
            )
            .await?
            .get(0);
        if !pending {
            tx.execute(
                "UPDATE durable_jobs SET status='completed',reason=NULL WHERE id=$1",
                &[&lease.job_id],
            )
            .await?;
        }
        match authority(&tx, &r).await {
            Ok(_) => {}
            Err(DurableError::Store(error)) if is_authority_boundary(&error) => {
                return Err(DurableError::Conflict)
            }
            Err(error) => return Err(error),
        }
        tx.commit().await?;
        Ok(ReuseSelected {
            clip_id,
            job_completed: !pending,
        })
    }

    /// Frozen shapes, the gate and registry rows for a regeneration plan. Read-only and
    /// advisory: the worker re-checks everything before it reuses anything.
    pub async fn plan_facts(
        &self,
        token: &str,
        script: &str,
        previous: Option<&str>,
        next: &str,
        execution: ExecutionMode,
    ) -> Result<PlanFacts, DurableError> {
        let next_snapshot = self.production_snapshot(token, script, next).await?;
        let previous_snapshot = match previous {
            Some(id) => Some(self.production_snapshot(token, script, id).await?),
            None => None,
        };
        let owner_id = next_snapshot.document.owner_id.clone();
        if previous_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.document.owner_id != owner_id)
        {
            return Err(DurableError::Conflict);
        }
        let next_shape = ProductionShape::from_document(&next_snapshot.document)
            .map_err(|_| DurableError::Corrupt)?;
        let previous_shape = previous_snapshot
            .as_ref()
            .map(|snapshot| ProductionShape::from_document(&snapshot.document))
            .transpose()
            .map_err(|_| DurableError::Corrupt)?;
        let fingerprints: Vec<String> = next_shape
            .fingerprints()
            .iter()
            .map(|fingerprint| fingerprint.to_string())
            .collect();
        let client = self.connection().await?;
        let rows = client
            .query(
                &format!("SELECT {CLIP_COLUMNS} FROM (SELECT c.*,row_number() OVER (PARTITION BY speech_fingerprint ORDER BY registration_seq) AS rn FROM speech_clips c WHERE c.owner_id=$1 AND c.speech_fingerprint=ANY($2) AND c.producer=$3) t WHERE rn<=$4 ORDER BY speech_fingerprint,registration_seq"),
                &[&owner_id, &fingerprints, &execution.as_str(), &MAX_CANDIDATES_PER_FINGERPRINT],
            )
            .await?;
        let mut clips: BTreeMap<SpeechFingerprint, Vec<StoredClip>> = BTreeMap::new();
        for row in &rows {
            let stored = stored_clip(row)?;
            clips
                .entry(stored.clip.fingerprint.clone())
                .or_default()
                .push(stored);
        }
        Ok(PlanFacts {
            owner_id,
            gate: PlanGate::from_eligibility(&next_snapshot.eligibility),
            previous: previous_shape,
            next: next_shape,
            clips,
        })
    }

    /// A read-only regeneration plan for `next`, relative to `previous` when given. Objects are
    /// probed only when an output root is supplied; otherwise no clip can count as a hit.
    pub async fn regeneration_plan(
        &self,
        token: &str,
        script: &str,
        previous: Option<&str>,
        next: &str,
        execution: ExecutionMode,
        output_root: Option<&Path>,
    ) -> Result<PlanReport, DurableError> {
        let prober = output_root
            .map(ClipProber::open)
            .transpose()
            .map_err(|_| DurableError::InvalidRequest)?;
        let facts = self
            .plan_facts(token, script, previous, next, execution)
            .await?;
        let performed = prober.is_some();
        let stored = facts.clips.clone();
        let cache = tokio::task::spawn_blocking(move || probe_candidates(prober.as_ref(), &stored))
            .await
            .map_err(|_| DurableError::Corrupt)?;
        let plan = plan_regeneration(&PlanInputs {
            previous: facts.previous.as_ref(),
            next: &facts.next,
            gate: facts.gate,
            owner_id: &facts.owner_id,
            producer: execution,
            cache: &cache,
        });
        Ok(PlanReport {
            script_id: normalized_uuid(script)?,
            previous_snapshot_id: previous.map(normalized_uuid).transpose()?,
            next_snapshot_id: normalized_uuid(next)?,
            execution_mode: execution,
            object_probes: if performed {
                ObjectProbes::Performed
            } else {
                ObjectProbes::NotPerformed
            },
            plan,
        })
    }
}
