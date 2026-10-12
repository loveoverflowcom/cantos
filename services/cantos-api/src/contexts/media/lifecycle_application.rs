//! Content/Brief-bound Audio Lifecycle shell. All uploads share PR69's inspector/storage.
use super::{
    application::{inspect_and_store, storage},
    domain::lifecycle::*,
    postgres::lifecycle as sql,
};
use crate::contexts::{
    catalog::{domain::OwnerId, ProductionContent},
    production::{
        self,
        domain::brief::{fingerprint, ProductionBrief},
    },
};
use crate::platform::postgres::{
    authenticate, normalized_uuid, session_token_registered, LockMode, Store, StoreError, Tx,
};
use cantos_contracts as dto;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn invalid(error: AudioError) -> StoreError {
    let code = match error {
        AudioError::OperationReused => return StoreError::OperationReused,
        AudioError::StaleBrief => return StoreError::StaleProductionInputs,
        AudioError::PermissionRevoked | AudioError::PermissionExpired => {
            return StoreError::Unauthenticated
        }
        AudioError::Encoding => return StoreError::CorruptRevision,
        AudioError::InvalidId => "invalid_uuid",
        AudioError::InvalidDigest => "invalid_digest",
        AudioError::InvalidDeclaration => "invalid_declaration",
        AudioError::InvalidExpiry => "invalid_expiry",
        AudioError::InvalidSize => "invalid_size",
        AudioError::BindingMismatch => "binding_mismatch",
        AudioError::DigestMismatch => "digest_mismatch",
        AudioError::InvalidTransition => "invalid_transition",
    };
    StoreError::InvalidRequestFields(vec![dto::FieldIssue {
        path: "audio".into(),
        rule: code.into(),
    }])
}
fn hash(token: &str) -> Result<Vec<u8>, StoreError> {
    if !sha256_valid(token) {
        return Err(StoreError::Unauthenticated);
    }
    let mut h = Sha256::new();
    h.update(b"cantos/audio-upload/v1\n");
    h.update(token);
    Ok(h.finalize().to_vec())
}
async fn facts(
    tx: &Tx<'_>,
    token: Option<&str>,
    s: &AudioSubmission,
    lock: LockMode,
) -> Result<(ProductionContent, ProductionBrief, bool), StoreError> {
    let d = s.document();
    let owner = OwnerId::parse(&d.owner_id).map_err(|_| StoreError::CorruptRevision)?;
    let result = production::audio_brief(
        tx,
        token,
        Some(&owner),
        &d.binding.unit_id,
        d.binding.brief_version,
        lock,
    )
    .await?;
    if Binding::from_brief(&result.1)
        .map_err(|_| StoreError::CorruptRevision)?
        .raw()
        != &d.binding
        || result.0.work.owner_id().as_str() != d.owner_id
    {
        return Err(StoreError::CorruptRevision);
    }
    if d.declarations
        .rights_refs
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        != result
            .1
            .document()
            .work
            .rights_refs
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(result)
}
async fn verify(store: &Store, r: &AudioRendition) -> Result<(), StoreError> {
    let objects = storage(store)?;
    let m = r.document().verified_media.clone();
    tokio::task::spawn_blocking(move || objects.verify(&m.sha256, &m.sha256, m.byte_length))
        .await
        .map_err(|_| StoreError::Unavailable)?
}
async fn scoped(
    tx: &Tx<'_>,
    id: &str,
    bearer: &str,
    lock: LockMode,
) -> Result<(AudioSubmission, bool), StoreError> {
    let expected = hash(bearer)?;
    let row = sql::load(tx, id)
        .await?
        .ok_or(StoreError::Unauthenticated)?;
    if row.token_hash != expected {
        return Err(StoreError::Unauthenticated);
    }
    let (_, _, current) = facts(tx, None, &row.value, lock).await?;
    let revoked = sql::revoked(tx, id).await?;
    let d = row.value.document();
    decide_admission(
        &row.value,
        sql::now(tx).await?,
        revoked,
        current,
        &d.expected_sha256,
        d.expected_byte_length,
        None,
    )
    .map_err(invalid)?;
    Ok((row.value, current))
}
async fn owner(
    tx: &Tx<'_>,
    token: &str,
    unit: &str,
    id: &str,
    lock: LockMode,
) -> Result<(AudioSubmission, ProductionContent, bool), StoreError> {
    let actor = authenticate(tx, token).await?;
    let row = sql::load(tx, id).await?.ok_or(StoreError::NotFound)?;
    if row.value.document().owner_id != actor || row.value.document().binding.unit_id != unit {
        return Err(StoreError::NotFound);
    }
    let (facts, _, current) = facts(tx, Some(token), &row.value, lock).await?;
    Ok((row.value, facts, current))
}
async fn snapshot(
    store: &Store,
    tx: &Tx<'_>,
    s: AudioSubmission,
    current: bool,
) -> Result<dto::AudioSubmissionResponse, StoreError> {
    let revoked = sql::revoked(tx, &s.document().id).await?;
    let r = sql::rendition(tx, &s).await?;
    let reviews = if let Some(r) = &r {
        verify(store, r).await?;
        checked_reviews(tx, &s, r).await?
    } else {
        vec![]
    };
    let state = submission_state(
        sql::now(tx).await?,
        s.document().expires_at,
        revoked,
        current,
        r.is_some(),
    );
    Ok(dto::AudioSubmissionResponse {
        submission: s.document().clone(),
        state,
        rendition: r.map(|r| r.document().clone()),
        reviews: latest_reviews(&reviews).into_iter().cloned().collect(),
    })
}
async fn checked_reviews(
    tx: &Tx<'_>,
    s: &AudioSubmission,
    r: &AudioRendition,
) -> Result<Vec<dto::AudioReviewDocument>, StoreError> {
    let result = sql::reviews(tx, r).await?;
    if result.len() > 1000
        || result
            .iter()
            .any(|r| r.reviewer_id != s.document().owner_id)
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(result)
}
fn blockers(
    current: bool,
    revoked: bool,
    expired: bool,
    facts: &ProductionContent,
    reviews: &[dto::AudioReviewDocument],
    candidate: Option<&dto::ReleaseCandidateDocument>,
) -> Vec<dto::AudioReleaseBlocker> {
    release_blockers(
        current,
        revoked,
        expired,
        !facts.work.details().rights_refs.is_empty(),
        reviews,
        candidate,
        (facts.work.revision(), facts.unit.revision()),
    )
}
impl Store {
    /// The owner creates one exact expiring upload capability. Bearer is never returned/stored.
    pub async fn create_audio_submission(
        &self,
        token: &str,
        unit: &str,
        request: dto::CreateAudioSubmissionRequest,
    ) -> Result<dto::AudioSubmissionResponse, StoreError> {
        storage(self)?;
        AudioSubmissionId::parse(&request.id).map_err(invalid)?;
        normalized_uuid(&request.operation_id)?;
        if request.binding.unit_id != unit {
            return Err(invalid(AudioError::BindingMismatch));
        }
        let token_hash = hash(&request.upload_token).map_err(|_| StoreError::InvalidRequest)?;
        let request_digest = fingerprint(
            "as-request1",
            &(
                &request.id,
                &request.operation_id,
                &request.binding,
                &request.expected_sha256,
                request.expected_byte_length,
                &request.declarations,
                &token_hash,
                request.expires_at,
            ),
        )
        .map_err(|_| StoreError::CorruptRevision)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (facts, brief, current) = production::audio_brief(
            &tx,
            Some(token),
            None,
            unit,
            request.binding.brief_version,
            LockMode::Update,
        )
        .await?;
        let actor = facts.work.owner_id().as_str();
        if let Some(prior) = sql::prior(&tx, actor, &request.operation_id).await? {
            if prior.request_digest != request_digest {
                return Err(StoreError::OperationReused);
            }
            let (_, _, fresh) =
                self::facts(&tx, Some(token), &prior.value, LockMode::Update).await?;
            let response = snapshot(self, &tx, prior.value, fresh).await?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(response);
        }
        if !current {
            return Err(StoreError::StaleProductionInputs);
        }
        if session_token_registered(&tx, &request.upload_token).await?
            || production::producer_token_registered(&tx, &request.upload_token).await?
        {
            return Err(StoreError::InvalidRequestFields(vec![dto::FieldIssue {
                path: "upload_token".into(),
                rule: "credential_reuse".into(),
            }]));
        }
        if sql::load(&tx, &request.id).await?.is_some()
            || sql::token_exists(&tx, &token_hash).await?
        {
            return Err(StoreError::OperationReused);
        }
        let s = AudioSubmission::create(
            &request,
            &brief,
            facts.work.owner_id(),
            sql::now(&tx).await?,
        )
        .map_err(invalid)?;
        sql::insert(&tx, &s, &token_hash, &request_digest).await?;
        let reply = snapshot(self, &tx, s, current).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(reply)
    }
    pub async fn load_audio_submission(
        &self,
        token: &str,
        unit: &str,
        id: &str,
    ) -> Result<dto::AudioSubmissionResponse, StoreError> {
        AudioSubmissionId::parse(id).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, _, current) = owner(&tx, token, unit, id, LockMode::Share).await?;
        let reply = snapshot(self, &tx, s, current).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(reply)
    }
    pub async fn producer_audio_submission(
        &self,
        bearer: &str,
        id: &str,
    ) -> Result<dto::AudioSubmissionResponse, StoreError> {
        AudioSubmissionId::parse(id).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, current) = scoped(&tx, id, bearer, LockMode::Share).await?;
        let reply = snapshot(self, &tx, s, current).await?;
        scoped(&tx, id, bearer, LockMode::Share).await?;
        tx.commit().await?;
        Ok(reply)
    }
    pub async fn revoke_audio_submission(
        &self,
        token: &str,
        unit: &str,
        id: &str,
    ) -> Result<(), StoreError> {
        AudioSubmissionId::parse(id).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, _, _) = owner(&tx, token, unit, id, LockMode::Update).await?;
        transition(
            submission_state(
                sql::now(&tx).await?,
                s.document().expires_at,
                false,
                true,
                sql::rendition(&tx, &s).await?.is_some(),
            ),
            SubmissionEvent::Revoke,
        )
        .map_err(invalid)?;
        sql::revoke(&tx, id, &s.document().owner_id, sql::now(&tx).await?).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(())
    }
    /// PUT is finalization: exact bytes, real inspection, durable private object, reauthorization,
    /// then one immutable rendition commit. HTTP cancellation can leave only a private orphan.
    pub async fn upload_audio_submission(
        &self,
        bearer: &str,
        id: &str,
        bytes: Vec<u8>,
    ) -> Result<dto::AudioRenditionDocument, StoreError> {
        let objects = storage(self)?;
        if bytes.len() > super::audio_intake::MAX_AUDIO_BYTES {
            return Err(invalid(AudioError::InvalidSize));
        }
        AudioSubmissionId::parse(id).map_err(invalid)?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let length = bytes.len() as u64;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, current) = scoped(&tx, id, bearer, LockMode::Share).await?;
        let settled = sql::rendition(&tx, &s).await?;
        let decision = decide_admission(
            &s,
            sql::now(&tx).await?,
            false,
            current,
            &sha256,
            length,
            settled.as_ref(),
        )
        .map_err(invalid)?;
        if decision == AdmissionDecision::Replay {
            let r = settled.ok_or(StoreError::CorruptRevision)?;
            verify(self, &r).await?;
            scoped(&tx, id, bearer, LockMode::Share).await?;
            tx.commit().await?;
            return Ok(r.document().clone());
        }
        tx.commit().await?;
        drop(client);
        let (metadata, _stored) = inspect_and_store(objects, bytes, s.binding().profile()).await?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (locked, current) = scoped(&tx, id, bearer, LockMode::Update).await?;
        if locked != s {
            return Err(StoreError::CorruptRevision);
        }
        let prior = sql::rendition(&tx, &s).await?;
        decide_admission(
            &s,
            sql::now(&tx).await?,
            false,
            current,
            &metadata.sha256,
            metadata.byte_length,
            prior.as_ref(),
        )
        .map_err(invalid)?;
        if let Some(prior) = prior {
            if prior.document().verified_media != metadata {
                return Err(StoreError::OperationReused);
            }
            verify(self, &prior).await?;
            scoped(&tx, id, bearer, LockMode::Update).await?;
            tx.commit().await?;
            return Ok(prior.document().clone());
        }
        let r = AudioRendition::create(
            &s,
            metadata,
            AudioRenditionId::parse(&Uuid::new_v4().to_string()).map_err(invalid)?,
            sql::now(&tx).await?,
        )
        .map_err(invalid)?;
        transition(
            dto::AudioSubmissionState::AwaitingBytes,
            SubmissionEvent::Admit,
        )
        .map_err(invalid)?;
        sql::insert_rendition(&tx, &r).await?;
        scoped(&tx, id, bearer, LockMode::Update).await?;
        tx.commit().await?;
        Ok(r.document().clone())
    }
    pub async fn review_audio_submission(
        &self,
        token: &str,
        unit: &str,
        id: &str,
        request: dto::CreateAudioReviewRequest,
    ) -> Result<dto::AudioReviewDocument, StoreError> {
        AudioSubmissionId::parse(id).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, _, current) = owner(&tx, token, unit, id, LockMode::Update).await?;
        if !current || sql::revoked(&tx, id).await? {
            return Err(StoreError::StaleProductionInputs);
        }
        let r = sql::rendition(&tx, &s).await?.ok_or(StoreError::NotFound)?;
        verify(self, &r).await?;
        validate_review(&request, &r).map_err(invalid)?;
        let reviews = checked_reviews(&tx, &s, &r).await?;
        let actor = &s.document().owner_id;
        if let Some(subject) = sql::review_prior(&tx, actor, &request.operation_id).await? {
            if subject != r.document().id {
                return Err(StoreError::OperationReused);
            }
            let prior = reviews
                .iter()
                .find(|r| r.operation_id == request.operation_id)
                .ok_or(StoreError::CorruptRevision)?;
            if prior.request != request {
                return Err(StoreError::OperationReused);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.clone());
        }
        if reviews.len() >= 1000 {
            return Err(StoreError::InvalidRequest);
        }
        let sequence = reviews.iter().map(|r| r.sequence).max().unwrap_or(0) + 1;
        let review = AudioReview::record(
            dto::AudioReviewDocument {
                id: Uuid::new_v4().to_string(),
                rendition_id: r.document().id.clone(),
                operation_id: request.operation_id.clone(),
                sequence,
                reviewer_id: actor.clone(),
                reviewed_at: sql::now(&tx).await?,
                request,
                digest: String::new(),
            },
            &r,
        )
        .map_err(invalid)?;
        sql::insert_review(&tx, &review).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(review.document().clone())
    }
    pub async fn create_audio_release_candidate(
        &self,
        token: &str,
        unit: &str,
        id: &str,
        request: dto::CreateReleaseCandidateRequest,
    ) -> Result<dto::ReleaseCandidateResponse, StoreError> {
        normalized_uuid(&request.operation_id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, facts, current) = owner(&tx, token, unit, id, LockMode::Update).await?;
        let r = sql::rendition(&tx, &s).await?.ok_or(StoreError::NotFound)?;
        verify(self, &r).await?;
        if request.rendition_digest != r.document().digest {
            return Err(invalid(AudioError::DigestMismatch));
        }
        let reviews = checked_reviews(&tx, &s, &r).await?;
        let candidate = if let Some(prior) =
            sql::candidate_prior(&tx, &s.document().owner_id, &request.operation_id, &r).await?
        {
            prior
        } else {
            if !current || sql::revoked(&tx, id).await? {
                return Err(StoreError::StaleProductionInputs);
            }
            let candidate = ReleaseCandidate::record(
                dto::ReleaseCandidateDocument {
                    id: Uuid::new_v4().to_string(),
                    rendition_id: r.document().id.clone(),
                    rendition_digest: r.document().digest.clone(),
                    binding: s.document().binding.clone(),
                    sha256: r.document().verified_media.sha256.clone(),
                    work_metadata_revision: facts.work.revision(),
                    unit_metadata_revision: facts.unit.revision(),
                    review_ids: latest_reviews(&reviews)
                        .iter()
                        .map(|r| r.id.clone())
                        .collect(),
                    created_by: s.document().owner_id.clone(),
                    created_at: sql::now(&tx).await?,
                    operation_id: request.operation_id,
                    digest: String::new(),
                },
                &r,
            )
            .map_err(invalid)?;
            sql::insert_candidate(&tx, &candidate).await?;
            candidate.document().clone()
        };
        if candidate.created_by != s.document().owner_id {
            return Err(StoreError::CorruptRevision);
        }
        let blockers = blockers(
            current,
            sql::revoked(&tx, id).await?,
            sql::now(&tx).await? >= s.document().expires_at,
            &facts,
            &reviews,
            Some(&candidate),
        );
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(dto::ReleaseCandidateResponse {
            candidate,
            blockers,
        })
    }
    pub async fn load_audio_release_candidate(
        &self,
        token: &str,
        unit: &str,
        id: &str,
        candidate: &str,
    ) -> Result<dto::ReleaseCandidateResponse, StoreError> {
        ReleaseCandidateId::parse(candidate).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (s, facts, current) = owner(&tx, token, unit, id, LockMode::Share).await?;
        let r = sql::rendition(&tx, &s).await?.ok_or(StoreError::NotFound)?;
        verify(self, &r).await?;
        let candidate = sql::load_candidate(&tx, candidate, &r)
            .await?
            .ok_or(StoreError::NotFound)?;
        if candidate.created_by != s.document().owner_id {
            return Err(StoreError::CorruptRevision);
        }
        let reviews = checked_reviews(&tx, &s, &r).await?;
        let blockers = blockers(
            current,
            sql::revoked(&tx, id).await?,
            sql::now(&tx).await? >= s.document().expires_at,
            &facts,
            &reviews,
            Some(&candidate),
        );
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(dto::ReleaseCandidateResponse {
            candidate,
            blockers,
        })
    }
}

/// Concrete credential-separation consumer used by Production's read-grant issuer.
pub(crate) async fn upload_token_registered(tx: &Tx<'_>, token: &str) -> Result<bool, StoreError> {
    sql::token_exists(tx, &hash(token)?).await
}
