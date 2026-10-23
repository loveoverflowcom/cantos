//! External Brief use cases. The shell obtains locked facts, then calls deterministic rules.
use super::super::{domain::brief::*, postgres::briefs as sql};
use crate::contexts::catalog::{
    self,
    domain::{
        content::{ContentBody, ContentRevision, ContentRevisionId},
        ContentUnitId, Lifecycle, OwnerId,
    },
    CatalogStoreError, ProductionContent,
};
use crate::contexts::{editorial, ingestion};
use crate::contract::studio as dto;
use crate::platform::store::{authenticate, normalized_uuid, LockMode, Store, StoreError, Tx};
use sha2::{Digest, Sha256};

fn catalog_error(error: CatalogStoreError) -> StoreError {
    match error {
        CatalogStoreError::Storage(error) => error,
        _ => StoreError::CorruptRevision,
    }
}
fn invalid(error: BriefError) -> StoreError {
    let rule = match error {
        BriefError::InvalidId => "invalid_uuid",
        BriefError::InvalidVersion => "invalid_version",
        BriefError::InvalidGuidance => "invalid_guidance",
        BriefError::UnsupportedProfile => "unsupported_profile",
        BriefError::ContentMismatch => "content_mismatch",
        BriefError::InvalidInstructions => "invalid_instructions",
        BriefError::InvalidDigest => "invalid_digest",
        BriefError::ScopeMismatch => "scope_mismatch",
        BriefError::ExpiredGrant => "invalid_expiry",
        BriefError::RevokedGrant => "revoked_grant",
        BriefError::Incompatible(_) => "unsupported_required_capability",
        BriefError::Encoding => return StoreError::CorruptRevision,
    };
    StoreError::InvalidRequestFields(vec![dto::FieldIssue {
        path: "brief".into(),
        rule: rule.into(),
    }])
}
fn unit(value: &str) -> Result<ContentUnitId, StoreError> {
    let id = ContentUnitId::parse(value).map_err(|_| StoreError::InvalidRequest)?;
    if id.as_str() != value {
        return Err(StoreError::InvalidRequest);
    }
    Ok(id)
}
fn checked_version(value: u64) -> Result<(), StoreError> {
    if value == 0 || value > i64::MAX as u64 {
        return Err(StoreError::InvalidRequest);
    }
    Ok(())
}
/// Token hashes are domain-separated from Studio sessions; no cookie can become a grant.
fn producer_hash(token: &str) -> Result<Vec<u8>, StoreError> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(StoreError::Unauthenticated);
    }
    let mut hash = Sha256::new();
    hash.update(b"cantos/producer-read/v1\n");
    hash.update(token);
    Ok(hash.finalize().to_vec())
}
pub(in crate::contexts::production) fn restore(
    raw: dto::ProductionBriefDocument,
) -> Result<ProductionBrief, StoreError> {
    let content =
        catalog::restore_content(raw.content.clone()).map_err(|_| StoreError::CorruptRevision)?;
    let admitted =
        ProductionBrief::admit(raw.clone(), &content).map_err(|_| StoreError::CorruptRevision)?;
    if admitted.document() != &raw {
        return Err(StoreError::CorruptRevision);
    }
    Ok(admitted)
}

async fn instructions(
    tx: &Tx<'_>,
    owner: &str,
    content: &ContentRevision,
) -> Result<dto::ProductionInstructions, StoreError> {
    // Optional provenance sources on Text/Script are revalidated, but are never appended to
    // spoken text. Source and Script bodies each use their exact owning-context records.
    if let Some(source) = content.details().body().source() {
        if ingestion::source_checksum(tx, owner, source.id())
            .await?
            .as_deref()
            != Some(source.sha256())
        {
            return Err(StoreError::EvidenceUnavailable);
        }
    }
    match content.details().body() {
        ContentBody::Recording => Ok(dto::ProductionInstructions::Recording {}),
        ContentBody::Text { text, .. } => Ok(dto::ProductionInstructions::Text {
            text: text.as_str().into(),
        }),
        ContentBody::Source { source } => {
            // Legacy text-only references have no immutable extraction receipt. Refuse them
            // explicitly; an owner can append normalized Text or import the preserved source.
            let receipt = ingestion::imported_source(tx, owner, source.id())
                .await?
                .ok_or(StoreError::EvidenceUnavailable)?;
            let receipt_digest = ingestion::domain::receipt::receipt_digest(
                &receipt.metadata,
                &receipt.outcome,
                &receipt.sha256,
            )
            .map_err(|_| StoreError::CorruptRevision)?;
            Ok(dto::ProductionInstructions::Source {
                receipt: Box::new(receipt),
                receipt_digest,
            })
        }
        ContentBody::Script { script, .. } => {
            let revision =
                editorial::owned_revision(tx, owner, script.id(), script.revision() as i64)
                    .await?
                    .ok_or(StoreError::EvidenceUnavailable)?;
            if revision.export_digest != script.export_digest() {
                return Err(StoreError::EvidenceUnavailable);
            }
            Ok(dto::ProductionInstructions::Script {
                revision: editorial::document::revision(revision),
            })
        }
    }
}
async fn validate_pinned(tx: &Tx<'_>, brief: &ProductionBrief) -> Result<(), StoreError> {
    let doc = brief.document();
    let pinned = catalog::pinned_content(
        tx,
        &unit(&doc.content.unit_id)?,
        &ContentRevisionId::parse(&doc.content.id).map_err(|_| StoreError::CorruptRevision)?,
    )
    .await
    .map_err(|_| StoreError::CorruptRevision)?;
    if catalog::content::wire::response(&pinned).map_err(|_| StoreError::CorruptRevision)?
        != doc.content
        || instructions(tx, &doc.work.owner_id, &pinned).await? != doc.instructions
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}
async fn response(
    tx: &Tx<'_>,
    facts: &ProductionContent,
    brief: ProductionBrief,
) -> Result<dto::ProductionBriefResponse, StoreError> {
    validate_pinned(tx, &brief).await?;
    if brief.document().work.owner_id != facts.work.owner_id().as_str()
        || brief.document().work.id != facts.work.id().as_str()
    {
        return Err(StoreError::CorruptRevision);
    }
    let latest = sql::latest(tx, facts.unit.id().as_str()).await?;
    let freshness = brief
        .freshness(
            &facts.head,
            &facts.work.details().rights_refs,
            latest,
            facts.work.lifecycle() == Lifecycle::Archived
                || facts.unit.lifecycle() == Lifecycle::Archived,
        )
        .map_err(invalid)?;
    Ok(dto::ProductionBriefResponse {
        brief: brief.document().clone(),
        freshness,
    })
}
fn current(response: &dto::ProductionBriefResponse) -> Result<(), StoreError> {
    if response.freshness != dto::BriefFreshness::Current {
        return Err(StoreError::StaleProductionInputs);
    }
    Ok(())
}

impl Store {
    pub async fn create_production_brief(
        &self,
        token: &str,
        unit_id: &str,
        request: dto::CreateProductionBriefRequest,
    ) -> Result<dto::ProductionBriefResponse, StoreError> {
        let unit = unit(unit_id)?;
        ProductionBriefId::parse(&request.id).map_err(invalid)?;
        ContentRevisionId::parse(&request.content_revision_id)
            .map_err(|_| StoreError::InvalidRequest)?;
        let operation = normalized_uuid(&request.operation_id)?;
        if request.expected_version > i64::MAX as u64 {
            return Err(StoreError::InvalidRequest);
        }
        let guidance = Guidance::admit(request.guidance.clone()).map_err(invalid)?;
        let mut canonical_request = request.clone();
        canonical_request.guidance = guidance.as_raw().clone();
        canonical_request.operation_id = operation.clone();
        let request_digest =
            fingerprint("pb-request1", &(unit_id, &canonical_request)).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        authenticate(&tx, token).await?;
        let facts = catalog::production_content(&tx, Some(token), None, &unit, LockMode::Update)
            .await
            .map_err(catalog_error)?;
        let owner = facts.work.owner_id().as_str();
        if let Some((digest, brief)) = sql::prior(&tx, owner, &operation).await? {
            if digest != request_digest || brief.document().content.unit_id != unit_id {
                return Err(StoreError::OperationReused);
            }
            let reply = response(&tx, &facts, brief).await?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(reply);
        }
        if facts.work.lifecycle() == Lifecycle::Archived
            || facts.unit.lifecycle() == Lifecycle::Archived
        {
            return Err(StoreError::StaleProductionInputs);
        }
        if facts.head.id().as_str() != request.content_revision_id
            || facts
                .head
                .revision_digest()
                .map_err(|_| StoreError::CorruptRevision)?
                .as_str()
                != request.content_revision_digest
        {
            return Err(StoreError::StaleProductionInputs);
        }
        if let Some(existing_owner) = sql::brief_id_owner(&tx, &request.id).await? {
            return Err(if existing_owner == owner {
                StoreError::OperationReused
            } else {
                StoreError::NotFound
            });
        }
        let latest = sql::latest(&tx, unit_id).await?;
        let version = next_brief_version(latest, request.expected_version)
            .map_err(|_| StoreError::StaleRevision(latest))?;
        let now = sql::now_ms(&tx).await?.max(facts.head.recorded_at());
        let work = dto::BriefWork {
            id: facts.work.id().as_str().into(),
            owner_id: owner.into(),
            kind: facts.work.kind().as_str().into(),
            title: facts.work.details().title.clone(),
            rights_refs: facts.work.details().rights_refs.clone(),
            rights_digest: rights_digest(&facts.work.details().rights_refs).map_err(invalid)?,
        };
        let document = dto::ProductionBriefDocument {
            schema_version: "1".into(),
            id: request.id,
            version,
            work,
            content: catalog::content::wire::response(&facts.head).map_err(catalog_error)?,
            instructions: instructions(&tx, owner, &facts.head).await?,
            guidance: guidance.as_raw().clone(),
            created_by: owner.into(),
            created_at: now,
            digest: String::new(),
        };
        let brief = ProductionBrief::admit(document, &facts.head).map_err(invalid)?;
        sql::insert(&tx, &brief, &operation, &request_digest).await?;
        let reply = response(&tx, &facts, brief).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(reply)
    }
    pub async fn production_brief(
        &self,
        token: &str,
        unit_id: &str,
        version: u64,
    ) -> Result<dto::ProductionBriefResponse, StoreError> {
        let unit = unit(unit_id)?;
        checked_version(version)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        authenticate(&tx, token).await?;
        let facts = catalog::production_content(&tx, Some(token), None, &unit, LockMode::Share)
            .await
            .map_err(catalog_error)?;
        let brief = sql::load(&tx, unit_id, version)
            .await?
            .ok_or(StoreError::NotFound)?;
        let reply = response(&tx, &facts, brief).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(reply)
    }
    pub async fn create_producer_grant(
        &self,
        token: &str,
        unit_id: &str,
        version: u64,
        request: dto::CreateProducerGrantRequest,
    ) -> Result<dto::ProducerGrantResponse, StoreError> {
        let unit = unit(unit_id)?;
        checked_version(version)?;
        ProducerGrantId::parse(&request.id).map_err(invalid)?;
        BriefDigest::parse(&request.brief_digest).map_err(invalid)?;
        let hash = producer_hash(&request.token).map_err(|_| StoreError::InvalidRequest)?;
        let operation = normalized_uuid(&request.operation_id)?;
        // Never persist the bearer token in operation payloads, fingerprints or receipts.
        let digest = fingerprint(
            "pb-grant1",
            &(
                unit_id,
                version,
                &request.id,
                &request.brief_digest,
                &hash,
                request.expires_at,
                &request.capabilities,
            ),
        )
        .map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        authenticate(&tx, token).await?;
        let facts = catalog::production_content(&tx, Some(token), None, &unit, LockMode::Update)
            .await
            .map_err(catalog_error)?;
        let owner = facts.work.owner_id().as_str();
        if crate::platform::store::session_token_registered(&tx, &request.token).await? {
            return Err(StoreError::InvalidRequestFields(vec![dto::FieldIssue {
                path: "token".into(),
                rule: "studio_token_reuse".into(),
            }]));
        }
        if crate::contexts::media::upload_token_registered(&tx, &request.token).await? {
            return Err(StoreError::InvalidRequestFields(vec![dto::FieldIssue {
                path: "token".into(),
                rule: "credential_reuse".into(),
            }]));
        }
        if let Some(prior) = sql::grant_prior(&tx, owner, &operation).await? {
            if prior.request_digest != digest {
                return Err(StoreError::OperationReused);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.response);
        }
        let brief = sql::load(&tx, unit_id, version)
            .await?
            .ok_or(StoreError::NotFound)?;
        if brief.digest().as_str() != request.brief_digest {
            return Err(StoreError::StaleProductionInputs);
        }
        current(&response(&tx, &facts, brief.clone()).await?)?;
        Guidance::admit(brief.document().guidance.clone())
            .map_err(invalid)?
            .check_capabilities(&request.capabilities)
            .map_err(invalid)?;
        let now = sql::now_ms(&tx).await?;
        validate_grant_expiry(now, request.expires_at).map_err(invalid)?;
        let reply = dto::ProducerGrantResponse {
            id: request.id,
            work_id: facts.work.id().as_str().into(),
            unit_id: unit_id.into(),
            brief_id: brief.id().as_str().into(),
            brief_digest: brief.digest().as_str().into(),
            permission: "read_instructions".into(),
            expires_at: request.expires_at,
        };
        ProducerScope::admit(&reply)
            .map_err(invalid)?
            .check(&brief, now, false)
            .map_err(invalid)?;
        if sql::by_id(&tx, &reply.id).await?.is_some() || sql::by_token(&tx, &hash).await?.is_some()
        {
            return Err(StoreError::OperationReused);
        }
        sql::insert_grant(
            &tx,
            &sql::GrantRow {
                response: reply.clone(),
                owner: owner.into(),
                token_hash: hash,
                operation,
                request_digest: digest,
            },
            now,
        )
        .await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(reply)
    }
    pub async fn revoke_producer_grant(
        &self,
        token: &str,
        unit_id: &str,
        grant_id: &str,
    ) -> Result<(), StoreError> {
        let unit = unit(unit_id)?;
        ProducerGrantId::parse(grant_id).map_err(invalid)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        authenticate(&tx, token).await?;
        let facts = catalog::production_content(&tx, Some(token), None, &unit, LockMode::Update)
            .await
            .map_err(catalog_error)?;
        let grant = sql::by_id(&tx, grant_id)
            .await?
            .ok_or(StoreError::NotFound)?;
        if grant.owner != facts.work.owner_id().as_str() || grant.response.unit_id != unit_id {
            return Err(StoreError::NotFound);
        }
        sql::revoke(&tx, grant_id, grant.owner.as_str(), sql::now_ms(&tx).await?).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn producer_instructions(
        &self,
        bearer: &str,
        unit_id: &str,
        version: u64,
    ) -> Result<dto::ProductionBriefDocument, StoreError> {
        let unit = unit(unit_id)?;
        checked_version(version)?;
        let hash = producer_hash(bearer)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let grant = sql::by_token(&tx, &hash)
            .await?
            .ok_or(StoreError::Unauthenticated)?;
        if grant.response.unit_id != unit_id {
            return Err(StoreError::NotFound);
        }
        let owner = OwnerId::parse(&grant.owner).map_err(|_| StoreError::CorruptRevision)?;
        let facts = catalog::production_content(&tx, None, Some(&owner), &unit, LockMode::Share)
            .await
            .map_err(catalog_error)?;
        let brief = sql::load(&tx, unit_id, version)
            .await?
            .ok_or(StoreError::NotFound)?;
        let scope =
            ProducerScope::admit(&grant.response).map_err(|_| StoreError::CorruptRevision)?;
        let revoked = sql::revoked(&tx, scope.grant_id().as_str()).await?;
        scope
            .check(&brief, sql::now_ms(&tx).await?, revoked)
            .map_err(|_| StoreError::Unauthenticated)?;
        let reply = response(&tx, &facts, brief.clone()).await?;
        current(&reply)?;
        scope
            .check(&brief, sql::now_ms(&tx).await?, revoked)
            .map_err(|_| StoreError::Unauthenticated)?;
        tx.commit().await?;
        Ok(reply.brief)
    }
}

/// Concrete Media consumer: owning contexts revalidate exact historical instructions and
/// current Content/rights/guidance under the same Catalog fence used for mutations.
pub(crate) async fn audio_brief(
    tx: &Tx<'_>,
    token: Option<&str>,
    expected_owner: Option<&OwnerId>,
    unit_id: &str,
    version: u64,
    lock: LockMode,
) -> Result<(ProductionContent, ProductionBrief, bool), StoreError> {
    let id = unit(unit_id)?;
    checked_version(version)?;
    let facts = catalog::production_content(tx, token, expected_owner, &id, lock)
        .await
        .map_err(catalog_error)?;
    let brief = sql::load(tx, unit_id, version)
        .await?
        .ok_or(StoreError::NotFound)?;
    for reference in &facts.work.details().rights_refs {
        if !editorial::evidence_exists(tx, facts.work.owner_id().as_str(), "rights", reference)
            .await?
        {
            return Err(StoreError::EvidenceUnavailable);
        }
    }
    let reply = response(tx, &facts, brief.clone()).await?;
    Ok((
        facts,
        brief,
        reply.freshness == dto::BriefFreshness::Current,
    ))
}

pub(crate) async fn producer_token_registered(
    tx: &Tx<'_>,
    token: &str,
) -> Result<bool, StoreError> {
    Ok(sql::by_token(tx, &producer_hash(token)?).await?.is_some())
}
