//! Private caller contexts and submissions on the existing immutable revision authority.
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationContextRequest, AdaptationContextResponse, AdaptationCost,
    AdaptationProblem, AdaptationProposal, AdaptationProviderMetadata, AdaptationReviewResponse,
    AdaptationRunResponse, AdaptationStatus, AdaptationSubmissionReceipt,
    AdaptationSubmissionStatus, AdaptationUsage, CancelAdaptationRequest, FieldIssue,
    ImportOutcome, ImportResponse, RevisionResponse, SaveRevisionRequest, StartAdaptationRequest,
    SubmitAdaptationProposalRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_postgres::Row;
use uuid::Uuid;

use crate::contexts::editorial::{
    append_revision, authorize, export_digest, load_revision, register_evidence, script_exists,
    EvidenceKind,
};
use crate::platform::postgres::{
    authenticate, checked_number, normalized_uuid, LockMode, Store, StoreError,
};
use crate::{
    contexts::{
        adaptation::{
            admit_edited_proposal, admit_output, prepare_request, validate_caller_generation,
            TrustedBinding, CONTRACT_VERSION, MAX_REQUEST_BYTES, OUTPUT_SCHEMA, PROMPT_VERSION,
        },
        editorial::Action,
    },
    script_ir::read_canonical_script,
};

const RUN_COLUMNS: &str = "input_version,id,owner_id,operation_id,source_id,script_id,frozen_input::text AS frozen_input,input_digest,status,problem::text AS problem,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at,to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenAdaptation {
    request: StartAdaptationRequest,
    source: ImportResponse,
    base: Option<RevisionResponse>,
    provider: AdaptationProviderMetadata,
    generation_record_id: String,
    rights_record_id: String,
}

impl FrozenAdaptation {
    fn binding(&self, id: &str) -> TrustedBinding {
        TrustedBinding {
            source_id: self.source.id.clone(),
            source_sha256: self.source.sha256.clone(),
            generation_record_id: self.generation_record_id.clone(),
            rights_record_id: self.rights_record_id.clone(),
            id_namespace: id.replace('-', ""),
            base_script_json: self.base.as_ref().map(|base| base.script_json.clone()),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    problem: Option<AdaptationProblem>,
    usage: Option<AdaptationUsage>,
    cost: Option<AdaptationCost>,
    output_sha256: Option<String>,
    output_byte_len: Option<u64>,
}

fn problem(code: &str) -> AdaptationProblem {
    AdaptationProblem {
        code: code.into(),
        issues: vec![],
    }
}

fn typed_digest<T: Serialize>(label: &[u8], value: &T) -> Result<String, StoreError> {
    let mut digest = Sha256::new();
    digest.update(label);
    digest.update(serde_json::to_vec(value).map_err(|_| StoreError::CorruptRevision)?);
    Ok(format!("{:x}", digest.finalize()))
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

fn decode_frozen(row: &Row) -> Result<FrozenAdaptation, StoreError> {
    let frozen: FrozenAdaptation = serde_json::from_str(&row.get::<_, String>("frozen_input"))
        .map_err(|_| StoreError::CorruptRevision)?;
    if typed_digest(b"cantos/adaptation-input/a1\n", &frozen)?
        != row.get::<_, String>("input_digest")
        || frozen.request.operation_id != row.get::<_, String>("operation_id")
        || frozen.request.source_id != row.get::<_, String>("source_id")
        || frozen.request.script_id != row.get::<_, String>("script_id")
        || frozen.source.imported_by != row.get::<_, String>("owner_id")
        || frozen.source.id != frozen.request.source_id
        || !frozen.request.rights_authorization
        || frozen.request.expected_provider != frozen.provider
        || frozen.generation_record_id
            != format!("gen_{}", row.get::<_, String>("id").replace('-', ""))
        || frozen.rights_record_id
            != format!("rights_{}", row.get::<_, String>("id").replace('-', ""))
    {
        return Err(StoreError::CorruptRevision);
    }
    normalized_uuid(&frozen.request.operation_id).map_err(|_| StoreError::CorruptRevision)?;
    normalized_uuid(&frozen.request.script_id).map_err(|_| StoreError::CorruptRevision)?;
    match &frozen.base {
        Some(base) => {
            let script = read_canonical_script(base.script_json.as_bytes())
                .map_err(|_| StoreError::CorruptRevision)?;
            if base.script_id != frozen.request.script_id
                || base.revision != frozen.request.expected_revision
                || base.content_digest != script.content_digest().to_string()
                || base.export_digest != export_digest(base.script_json.as_bytes())
            {
                return Err(StoreError::CorruptRevision);
            }
        }
        None if frozen.request.expected_revision != 0 => return Err(StoreError::CorruptRevision),
        None => (),
    }
    Ok(frozen)
}

async fn source_in_transaction(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    id: &str,
) -> Result<ImportResponse, StoreError> {
    let query = format!(
        "SELECT {} FROM source_records WHERE owner_id=$1 AND imported_by=$1 AND id=$2",
        crate::contexts::ingestion::IMPORT_COLUMNS
    );
    let row = tx
        .query_opt(&query, &[&actor, &id])
        .await?
        .ok_or(StoreError::NotFound)?;
    crate::contexts::ingestion::decode_import(&row).map(|(source, _)| source)
}

async fn expire_dispatch(
    tx: &deadpool_postgres::Transaction<'_>,
    id: &str,
) -> Result<(), StoreError> {
    let reason = serde_json::to_string(&problem("dispatch_outcome_unknown"))
        .map_err(|_| StoreError::Unavailable)?;
    let changed = tx.execute("UPDATE adaptation_runs SET status='ambiguous',problem=$2::text::jsonb,dispatch_deadline=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='running' AND dispatch_deadline<=clock_timestamp()", &[&id,&reason]).await?;
    if changed == 1 {
        let observation = Observation {
            problem: Some(problem("dispatch_outcome_unknown")),
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

async fn run_in_transaction(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    id: &str,
) -> Result<(AdaptationRunResponse, FrozenAdaptation), StoreError> {
    let query =
        format!("SELECT {RUN_COLUMNS} FROM adaptation_runs WHERE id=$1 AND owner_id=$2 FOR UPDATE");
    // Lock before expiry so cancellation/completion/read recovery have one transition order.
    let initial = tx
        .query_opt(&query, &[&id, &actor])
        .await?
        .ok_or(StoreError::NotFound)?;
    if initial.get::<_, String>("input_version") != "a1" {
        return Err(StoreError::NotFound);
    }
    let frozen = decode_frozen(&initial)?;
    if source_in_transaction(tx, actor, &frozen.source.id).await? != frozen.source {
        return Err(StoreError::CorruptRevision);
    }
    expire_dispatch(tx, id).await?;
    let row = tx.query_one(&query, &[&id, &actor]).await?;
    let status = decode_status(&row.get::<_, String>("status"))?;
    let proposal = match tx
        .query_opt(
            "SELECT proposal::text,digest FROM adaptation_proposals WHERE run_id=$1",
            &[&id],
        )
        .await?
    {
        Some(row) => {
            let proposal: AdaptationProposal = serde_json::from_str(&row.get::<_, String>(0))
                .map_err(|_| StoreError::CorruptRevision)?;
            if typed_digest(b"cantos/adaptation-proposal/p1\n", &proposal)?
                != row.get::<_, String>(1)
            {
                return Err(StoreError::CorruptRevision);
            }
            read_canonical_script(proposal.script_json.as_bytes())
                .map_err(|_| StoreError::CorruptRevision)?;
            admit_edited_proposal(&proposal.script_json, &frozen.source, &frozen.binding(id))
                .map_err(|_| StoreError::CorruptRevision)?;
            Some(proposal)
        }
        None => None,
    };
    if matches!(
        status,
        AdaptationStatus::Succeeded | AdaptationStatus::Accepted
    ) && proposal.is_none()
    {
        return Err(StoreError::CorruptRevision);
    }
    let observation = tx.query_opt("SELECT observation::text FROM adaptation_observations WHERE run_id=$1 AND kind='provider_result'", &[&id]).await?.map(|row| serde_json::from_str::<Observation>(&row.get::<_,String>(0))).transpose().map_err(|_| StoreError::CorruptRevision)?;
    let accepted = tx
        .query_opt(
            "SELECT script_id,revision FROM adaptation_acceptances WHERE run_id=$1",
            &[&id],
        )
        .await?;
    let accepted_revision = if let Some(accepted) = accepted {
        Some(load_revision(tx, &accepted.get::<_, String>(0), accepted.get::<_, i64>(1)).await?)
    } else {
        None
    };
    if (status == AdaptationStatus::Accepted) != accepted_revision.is_some() {
        return Err(StoreError::CorruptRevision);
    }
    let run_problem = row
        .get::<_, Option<String>>("problem")
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|_| StoreError::CorruptRevision)?;
    let response = AdaptationRunResponse {
        id: id.into(),
        created_by: actor.into(),
        recorded_at: row.get("recorded_at"),
        updated_at: row.get("updated_at"),
        request: frozen.request.clone(),
        source_sha256: frozen.source.sha256.clone(),
        extractor_version: match &frozen.source.outcome {
            ImportOutcome::Parsed { extraction } => extraction.extractor_version.clone(),
            ImportOutcome::Failed { .. } => return Err(StoreError::CorruptRevision),
        },
        input_content_digest: frozen.base.as_ref().map(|base| base.content_digest.clone()),
        input_export_digest: frozen.base.as_ref().map(|base| base.export_digest.clone()),
        generation_record_id: frozen.generation_record_id.clone(),
        rights_record_id: frozen.rights_record_id.clone(),
        provider: frozen.provider.clone(),
        status,
        proposal,
        problem: run_problem,
        usage: observation.as_ref().and_then(|value| value.usage.clone()),
        cost: observation.and_then(|value| value.cost),
        accepted_revision,
    };
    Ok((response, frozen))
}

impl Store {
    /// The historical inference start endpoint is retired; existing receipts remain readable.
    pub async fn start_adaptation(
        &self,
        _token: &str,
        _request: StartAdaptationRequest,
    ) -> Result<AdaptationRunResponse, StoreError> {
        Err(StoreError::Unavailable)
    }

    pub fn adaptation_provider_metadata(&self) -> Option<AdaptationProviderMetadata> {
        None
    }

    pub async fn load_adaptation(
        &self,
        token: &str,
        id: &str,
    ) -> Result<AdaptationRunResponse, StoreError> {
        normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (response, _) = run_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn cancel_adaptation(
        &self,
        token: &str,
        id: &str,
        request: CancelAdaptationRequest,
    ) -> Result<AdaptationRunResponse, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-cancel:' || $1::text,0))",
            &[&actor],
        )
        .await?;
        let (response, _) = run_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        if let Some(row) = tx
            .query_opt(
                "SELECT run_id FROM adaptation_cancellations WHERE owner_id=$1 AND operation_id=$2",
                &[&actor, &request.operation_id],
            )
            .await?
        {
            if row.get::<_, String>(0) != id {
                return Err(StoreError::OperationReused);
            }
        } else {
            if response.status == AdaptationStatus::Accepted {
                return Err(StoreError::InvalidRequest);
            }
            if response.status != AdaptationStatus::Cancelled {
                tx.execute("UPDATE adaptation_runs SET status='cancelled',dispatch_deadline=NULL,updated_at=clock_timestamp() WHERE id=$1", &[&id]).await?;
            }
            tx.execute("INSERT INTO adaptation_cancellations(owner_id,operation_id,run_id) VALUES($1,$2,$3)", &[&actor,&request.operation_id,&id]).await?;
        }
        let (response, _) = run_in_transaction(&tx, &actor, id).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn accept_adaptation(
        &self,
        token: &str,
        id: &str,
        request: AcceptAdaptationRequest,
    ) -> Result<RevisionResponse, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        checked_number(request.expected_revision)?;
        if !request.reviewed_findings {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/reviewed_findings".into(),
                rule: "required_review".into(),
            }]));
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-accept:' || $1::text,0))",
            &[&actor],
        )
        .await?;
        let row = tx
            .query_opt(
                "SELECT input_version FROM adaptation_runs WHERE id=$1 AND owner_id=$2",
                &[&id, &actor],
            )
            .await?
            .ok_or(StoreError::NotFound)?;
        let (status, expected_revision, script_id, source, binding, accepted_revision) =
            match row.get::<_, String>(0).as_str() {
                "a1" => {
                    let (response, frozen) = run_in_transaction(&tx, &actor, id).await?;
                    let binding = frozen.binding(id);
                    (
                        response.status,
                        frozen.request.expected_revision,
                        frozen.request.script_id,
                        frozen.source,
                        binding,
                        response.accepted_revision,
                    )
                }
                "c1" => {
                    let (response, context) = context_in_transaction(&tx, &actor, id).await?;
                    let binding = context.binding(id);
                    (
                        response.status,
                        context.request.expected_revision,
                        context.request.script_id,
                        context.source,
                        binding,
                        response.accepted_revision,
                    )
                }
                _ => return Err(StoreError::CorruptRevision),
            };
        authenticate(&tx, token).await?;
        let prior = tx.query_opt(
            "SELECT run_id,request::text FROM adaptation_acceptances WHERE owner_id=$1 AND operation_id=$2",
            &[&actor, &request.operation_id],
        ).await?;
        if let Some(row) = prior {
            let prior: AcceptAdaptationRequest = serde_json::from_str(&row.get::<_, String>(1))
                .map_err(|_| StoreError::CorruptRevision)?;
            if row.get::<_, String>(0) != id || prior != request {
                return Err(StoreError::OperationReused);
            }
            let saved = accepted_revision.ok_or(StoreError::CorruptRevision)?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(saved);
        }
        if status != AdaptationStatus::Succeeded || request.expected_revision != expected_revision {
            return Err(StoreError::InvalidRequest);
        }
        let content = admit_edited_proposal(&request.script_json, &source, &binding)
            .map_err(|value| StoreError::InvalidRequestFields(value.issues))?;
        let saved = append_revision(
            &tx,
            &actor,
            &script_id,
            &SaveRevisionRequest {
                expected_revision: request.expected_revision,
                operation_id: request.operation_id.clone(),
                script_json: request.script_json.clone(),
            },
            &content,
        )
        .await?;
        let json = serde_json::to_string(&request).map_err(|_| StoreError::Unavailable)?;
        tx.execute("INSERT INTO adaptation_acceptances(run_id,owner_id,operation_id,request,script_id,revision) VALUES($1,$2,$3,$4::text::jsonb,$5,$6)", &[&id,&actor,&request.operation_id,&json,&saved.script_id,&checked_number(saved.revision)?]).await?;
        tx.execute("UPDATE adaptation_runs SET status='accepted',updated_at=clock_timestamp() WHERE id=$1 AND status='succeeded'", &[&id]).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(saved)
    }
}

/// c1 is separate from the preserved a1 preimage: no provider configuration or attestation.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenContext {
    request: AdaptationContextRequest,
    source: ImportResponse,
    base: Option<RevisionResponse>,
    context_version: String,
    prompt_version: String,
    contract_version: String,
    system_prompt: String,
    user_prompt: String,
    proposal_schema_json: String,
    generation_record_id: String,
    rights_record_id: String,
}

impl FrozenContext {
    fn binding(&self, id: &str) -> TrustedBinding {
        TrustedBinding {
            source_id: self.source.id.clone(),
            source_sha256: self.source.sha256.clone(),
            generation_record_id: self.generation_record_id.clone(),
            rights_record_id: self.rights_record_id.clone(),
            id_namespace: id.replace('-', ""),
            base_script_json: self.base.as_ref().map(|base| base.script_json.clone()),
        }
    }
}

fn decode_context(row: &Row) -> Result<FrozenContext, StoreError> {
    let context: FrozenContext = serde_json::from_str(&row.get::<_, String>("frozen_input"))
        .map_err(|_| StoreError::CorruptRevision)?;
    let extractor = match &context.source.outcome {
        ImportOutcome::Parsed { extraction } => &extraction.extractor_version,
        ImportOutcome::Failed { .. } => return Err(StoreError::CorruptRevision),
    };
    if row.get::<_, String>("input_version") != "c1"
        || context.context_version != "c1"
        || typed_digest(b"cantos/adaptation-input/c1\n", &context)?
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
        || context.generation_record_id
            != format!("gen_{}", row.get::<_, String>("id").replace('-', ""))
        || context.rights_record_id
            != format!("rights_{}", row.get::<_, String>("id").replace('-', ""))
    {
        return Err(StoreError::CorruptRevision);
    }
    normalized_uuid(&context.request.operation_id).map_err(|_| StoreError::CorruptRevision)?;
    normalized_uuid(&context.request.script_id).map_err(|_| StoreError::CorruptRevision)?;
    if let Some(base) = &context.base {
        let content = read_canonical_script(base.script_json.as_bytes())
            .map_err(|_| StoreError::CorruptRevision)?;
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

const SUBMISSION_COLUMNS: &str = "id,run_id,owner_id,operation_id,request::text AS request,receipt::text AS receipt,receipt_digest,to_char(submitted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS submitted_at";

fn decode_submission(
    row: &Row,
) -> Result<(AdaptationSubmissionReceipt, SubmitAdaptationProposalRequest), StoreError> {
    let receipt: AdaptationSubmissionReceipt =
        serde_json::from_str(&row.get::<_, String>("receipt"))
            .map_err(|_| StoreError::CorruptRevision)?;
    let request: SubmitAdaptationProposalRequest =
        serde_json::from_str(&row.get::<_, String>("request"))
            .map_err(|_| StoreError::CorruptRevision)?;
    if typed_digest(b"cantos/adaptation-submission/s1\n", &receipt)?
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
    normalized_uuid(&receipt.operation_id).map_err(|_| StoreError::CorruptRevision)?;
    Ok((receipt, request))
}

async fn context_in_transaction(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    id: &str,
) -> Result<(AdaptationContextResponse, FrozenContext), StoreError> {
    let query =
        format!("SELECT {RUN_COLUMNS} FROM adaptation_runs WHERE id=$1 AND owner_id=$2 FOR UPDATE");
    let row = tx
        .query_opt(&query, &[&id, &actor])
        .await?
        .ok_or(StoreError::NotFound)?;
    if row.get::<_, String>("input_version") != "c1" {
        return Err(StoreError::NotFound);
    }
    let context = decode_context(&row)?;
    if source_in_transaction(tx, actor, &context.source.id).await? != context.source {
        return Err(StoreError::CorruptRevision);
    }
    let status = decode_status(&row.get::<_, String>("status"))?;
    let proposal = match tx
        .query_opt(
            "SELECT proposal::text,digest FROM adaptation_proposals WHERE run_id=$1",
            &[&id],
        )
        .await?
    {
        Some(row) => {
            let proposal: AdaptationProposal = serde_json::from_str(&row.get::<_, String>(0))
                .map_err(|_| StoreError::CorruptRevision)?;
            if typed_digest(b"cantos/adaptation-proposal/p1\n", &proposal)?
                != row.get::<_, String>(1)
            {
                return Err(StoreError::CorruptRevision);
            }
            read_canonical_script(proposal.script_json.as_bytes())
                .map_err(|_| StoreError::CorruptRevision)?;
            admit_edited_proposal(&proposal.script_json, &context.source, &context.binding(id))
                .map_err(|_| StoreError::CorruptRevision)?;
            Some(proposal)
        }
        None => None,
    };
    if matches!(
        status,
        AdaptationStatus::Succeeded | AdaptationStatus::Accepted
    ) && proposal.is_none()
    {
        return Err(StoreError::CorruptRevision);
    }
    let query=format!("SELECT {SUBMISSION_COLUMNS} FROM adaptation_submissions WHERE run_id=$1 ORDER BY submission_number DESC LIMIT 1");
    let latest_submission = tx
        .query_opt(&query, &[&id])
        .await?
        .map(|row| decode_submission(&row).map(|(receipt, _)| receipt))
        .transpose()?;
    if let Some(submission) = &latest_submission {
        if submission.context_digest != row.get::<_, String>("input_digest")
            || !validate_caller_generation(&submission.generation, &context.prompt_version)
                .is_empty()
            || (submission.status == AdaptationSubmissionStatus::Valid
                && submission.proposal != proposal)
        {
            return Err(StoreError::CorruptRevision);
        }
    }
    if proposal.is_some()
        && latest_submission
            .as_ref()
            .is_none_or(|receipt| receipt.status != AdaptationSubmissionStatus::Valid)
    {
        return Err(StoreError::CorruptRevision);
    }
    let accepted = tx
        .query_opt(
            "SELECT script_id,revision FROM adaptation_acceptances WHERE run_id=$1",
            &[&id],
        )
        .await?;
    let accepted_revision = if let Some(accepted) = accepted {
        Some(load_revision(tx, &accepted.get::<_, String>(0), accepted.get::<_, i64>(1)).await?)
    } else {
        None
    };
    if (status == AdaptationStatus::Accepted) != accepted_revision.is_some() {
        return Err(StoreError::CorruptRevision);
    }
    let response = AdaptationContextResponse {
        id: id.into(),
        created_by: actor.into(),
        recorded_at: row.get("recorded_at"),
        updated_at: row.get("updated_at"),
        request: context.request.clone(),
        context_version: context.context_version.clone(),
        context_digest: row.get("input_digest"),
        prompt_version: context.prompt_version.clone(),
        contract_version: context.contract_version.clone(),
        source: context.source.clone(),
        input_revision: context.base.clone(),
        generation_record_id: context.generation_record_id.clone(),
        rights_record_id: context.rights_record_id.clone(),
        system_prompt: context.system_prompt.clone(),
        user_prompt: context.user_prompt.clone(),
        proposal_schema_json: context.proposal_schema_json.clone(),
        status,
        proposal,
        latest_submission,
        accepted_revision,
    };
    Ok((response, context))
}

async fn current_base(
    tx: &deadpool_postgres::Transaction<'_>,
    actor: &str,
    script: &str,
    expected: u64,
) -> Result<Option<RevisionResponse>, StoreError> {
    if !script_exists(tx, script).await? {
        return if expected == 0 {
            Ok(None)
        } else {
            Err(StoreError::NotFound)
        };
    }
    let (owner, head) = authorize(tx, actor, script, Action::Review, LockMode::Update).await?;
    if owner != actor {
        return Err(StoreError::Forbidden);
    }
    if head != expected {
        return Err(StoreError::StaleRevision(head));
    }
    Ok(Some(
        load_revision(tx, script, checked_number(expected)?).await?,
    ))
}

impl Store {
    pub async fn create_adaptation_context(
        &self,
        token: &str,
        request: AdaptationContextRequest,
    ) -> Result<AdaptationContextResponse, StoreError> {
        normalized_uuid(&request.operation_id)?;
        normalized_uuid(&request.script_id)?;
        checked_number(request.expected_revision)?;
        if !request.rights_authorization {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/rights_authorization".into(),
                rule: "required_authorization".into(),
            }]));
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation:' || $1::text,0))",
            &[&actor],
        )
        .await?;
        authenticate(&tx, token).await?;
        if let Some(prior)=tx.query_opt("SELECT id,input_version FROM adaptation_runs WHERE owner_id=$1 AND operation_id=$2",&[&actor,&request.operation_id]).await? {
            if prior.get::<_,String>(1)!="c1" {return Err(StoreError::OperationReused);}
            let (response,context)=context_in_transaction(&tx,&actor,&prior.get::<_,String>(0)).await?;
            if context.request!=request {return Err(StoreError::OperationReused);}
            authenticate(&tx,token).await?;tx.commit().await?;return Ok(response);
        }
        let source = source_in_transaction(&tx, &actor, &request.source_id).await?;
        let extractor = match &source.outcome {
            ImportOutcome::Parsed { extraction } => &extraction.extractor_version,
            ImportOutcome::Failed { .. } => return Err(StoreError::InvalidRequest),
        };
        let mut issues = Vec::new();
        if request.source_sha256 != source.sha256 {
            issues.push(FieldIssue {
                path: "/source_sha256".into(),
                rule: "source_identity_mismatch".into(),
            });
        }
        if &request.extractor_version != extractor {
            issues.push(FieldIssue {
                path: "/extractor_version".into(),
                rule: "extraction_identity_mismatch".into(),
            });
        }
        for (value, path) in [
            (&source.metadata.rights_holder, "/source/rights_holder"),
            (
                &source.metadata.permission_evidence,
                "/source/permission_evidence",
            ),
            (&source.metadata.usage_scope, "/source/usage_scope"),
        ] {
            if value.as_deref().is_none_or(|value| value.trim().is_empty()) {
                issues.push(FieldIssue {
                    path: path.into(),
                    rule: "required_claim".into(),
                });
            }
        }
        if !issues.is_empty() {
            return Err(StoreError::InvalidRequestFields(issues));
        }
        let base = current_base(&tx, &actor, &request.script_id, request.expected_revision).await?;
        let prepared =
            prepare_request(&source, base.as_ref().map(|base| base.script_json.as_str()))
                .map_err(|error| StoreError::InvalidRequestFields(error.issues))?;
        let id = Uuid::new_v4();
        let context = FrozenContext {
            request: request.clone(),
            source,
            base,
            context_version: "c1".into(),
            prompt_version: PROMPT_VERSION.into(),
            contract_version: CONTRACT_VERSION.into(),
            system_prompt: prepared.system,
            user_prompt: prepared.prompt,
            proposal_schema_json: OUTPUT_SCHEMA.into(),
            generation_record_id: format!("gen_{}", id.simple()),
            rights_record_id: format!("rights_{}", id.simple()),
        };
        for (kind, evidence, description) in [
            (
                EvidenceKind::Generation,
                &context.generation_record_id,
                "Caller adaptation submission binding; generation provenance is declared and unverified",
            ),
            (
                EvidenceKind::Rights,
                &context.rights_record_id,
                "Creator source/export/adaptation-use assertion pinned to source checksum; legal and publication eligibility remain unknown",
            ),
        ] {
            register_evidence(&tx, &actor, kind, evidence, description).await?;
        }
        let digest = typed_digest(b"cantos/adaptation-input/c1\n", &context)?;
        let json = serde_json::to_string(&context).map_err(|_| StoreError::Unavailable)?;
        let id = id.to_string();
        tx.execute("INSERT INTO adaptation_runs(id,owner_id,operation_id,source_id,script_id,frozen_input,input_digest,status,input_version) VALUES($1,$2,$3,$4,$5,$6::text::jsonb,$7,'awaiting_proposal','c1')",&[&id,&actor,&request.operation_id,&request.source_id,&request.script_id,&json,&digest]).await?;
        let (response, _) = context_in_transaction(&tx, &actor, &id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn load_adaptation_context(
        &self,
        token: &str,
        id: &str,
    ) -> Result<AdaptationContextResponse, StoreError> {
        normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (response, _) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn load_adaptation_review(
        &self,
        token: &str,
        id: &str,
    ) -> Result<AdaptationReviewResponse, StoreError> {
        normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let version = tx
            .query_opt(
                "SELECT input_version FROM adaptation_runs WHERE id=$1 AND owner_id=$2",
                &[&id, &actor],
            )
            .await?
            .ok_or(StoreError::NotFound)?
            .get::<_, String>(0);
        let response = match version.as_str() {
            "c1" => {
                let (context, _) = context_in_transaction(&tx, &actor, id).await?;
                AdaptationReviewResponse::Caller {
                    context: Box::new(context),
                }
            }
            "a1" => {
                let (run, frozen) = run_in_transaction(&tx, &actor, id).await?;
                AdaptationReviewResponse::Legacy {
                    run: Box::new(run),
                    source: Box::new(frozen.source),
                    input_revision: frozen.base.map(Box::new),
                }
            }
            _ => return Err(StoreError::CorruptRevision),
        };
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn submit_adaptation_proposal(
        &self,
        token: &str,
        id: &str,
        request: SubmitAdaptationProposalRequest,
    ) -> Result<AdaptationSubmissionReceipt, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        if request.proposal_json.len() > 1024 * 1024 {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/proposal_json".into(),
                rule: "byte_length".into(),
            }]));
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-submit:' || $1::text,0))",
            &[&actor],
        )
        .await?;
        let (response, context) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        let query=format!("SELECT {SUBMISSION_COLUMNS} FROM adaptation_submissions WHERE owner_id=$1 AND operation_id=$2");
        if let Some(row) = tx
            .query_opt(&query, &[&actor, &request.operation_id])
            .await?
        {
            let (receipt, prior) = decode_submission(&row)?;
            if receipt.run_id != id || prior != request {
                return Err(StoreError::OperationReused);
            }
            if !validate_caller_generation(&receipt.generation, &context.prompt_version).is_empty()
                || receipt.context_digest != response.context_digest
            {
                return Err(StoreError::CorruptRevision);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(receipt);
        }
        if response.context_digest != request.context_digest {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/context_digest".into(),
                rule: "immutable_context".into(),
            }]));
        }
        let issues = validate_caller_generation(&request.generation, &context.prompt_version);
        if !issues.is_empty() {
            return Err(StoreError::InvalidRequestFields(issues));
        }
        if response.status != AdaptationStatus::AwaitingProposal {
            return Err(StoreError::ProposalAlreadySubmitted);
        }
        current_base(
            &tx,
            &actor,
            &context.request.script_id,
            context.request.expected_revision,
        )
        .await?;
        let admitted = admit_output(
            request.proposal_json.as_bytes(),
            &context.source,
            &context.binding(id),
        );
        let (status, proposal, problem) = match admitted {
            Ok(proposal) => (AdaptationSubmissionStatus::Valid, Some(proposal), None),
            Err(error) => (AdaptationSubmissionStatus::Invalid, None, Some(error)),
        };
        let submitted_at:String=tx.query_one("SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",&[]).await?.get(0);
        let receipt = AdaptationSubmissionReceipt {
            id: Uuid::new_v4().to_string(),
            run_id: id.into(),
            operation_id: request.operation_id.clone(),
            context_digest: request.context_digest.clone(),
            submitted_by: actor.clone(),
            submitted_at,
            output_sha256: format!("{:x}", Sha256::digest(request.proposal_json.as_bytes())),
            generation: request.generation.clone(),
            status,
            problem,
            proposal: proposal.clone(),
        };
        let number:i64=tx.query_one("SELECT coalesce(max(submission_number),0)+1 FROM adaptation_submissions WHERE run_id=$1",&[&id]).await?.get(0);
        let request_json = serde_json::to_string(&request).map_err(|_| StoreError::Unavailable)?;
        let receipt_json = serde_json::to_string(&receipt).map_err(|_| StoreError::Unavailable)?;
        let digest = typed_digest(b"cantos/adaptation-submission/s1\n", &receipt)?;
        tx.execute("INSERT INTO adaptation_submissions(id,run_id,owner_id,operation_id,submission_number,request,receipt,receipt_digest,submitted_at) VALUES($1,$2,$3,$4,$5,$6::text::jsonb,$7::text::jsonb,$8,$9::text::timestamptz)",&[&receipt.id,&id,&actor,&request.operation_id,&number,&request_json,&receipt_json,&digest,&receipt.submitted_at]).await?;
        if let Some(proposal) = proposal {
            let json = serde_json::to_string(&proposal).map_err(|_| StoreError::Unavailable)?;
            let digest = typed_digest(b"cantos/adaptation-proposal/p1\n", &proposal)?;
            tx.execute("INSERT INTO adaptation_proposals(run_id,proposal,digest) VALUES($1,$2::text::jsonb,$3)",&[&id,&json,&digest]).await?;
            let changed=tx.execute("UPDATE adaptation_runs SET status='succeeded',updated_at=clock_timestamp() WHERE id=$1 AND input_version='c1' AND status='awaiting_proposal'",&[&id]).await?;
            if changed != 1 {
                return Err(StoreError::ProposalAlreadySubmitted);
            }
        }
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(receipt)
    }
}

impl Store {
    /// A cancelled caller context cannot receive a corrected output or accept its proposal.
    pub async fn cancel_adaptation_context(
        &self,
        token: &str,
        id: &str,
        request: CancelAdaptationRequest,
    ) -> Result<AdaptationContextResponse, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('adaptation-cancel:' || $1::text,0))",
            &[&actor],
        )
        .await?;
        let (response, _) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        if let Some(prior) = tx
            .query_opt(
                "SELECT run_id FROM adaptation_cancellations WHERE owner_id=$1 AND operation_id=$2",
                &[&actor, &request.operation_id],
            )
            .await?
        {
            if prior.get::<_, String>(0) != id {
                return Err(StoreError::OperationReused);
            }
        } else {
            if response.status == AdaptationStatus::Accepted {
                return Err(StoreError::InvalidRequest);
            }
            if response.status != AdaptationStatus::Cancelled {
                tx.execute("UPDATE adaptation_runs SET status='cancelled',updated_at=clock_timestamp() WHERE id=$1",&[&id]).await?;
            }
            tx.execute("INSERT INTO adaptation_cancellations(owner_id,operation_id,run_id) VALUES($1,$2,$3)",&[&actor,&request.operation_id,&id]).await?;
        }
        let (response, _) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }
}
