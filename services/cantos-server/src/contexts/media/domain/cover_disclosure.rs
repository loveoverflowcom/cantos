//! Accepted ADR 0033 rule 2: the current active Work owner's explicit public-display decision
//! for one exact admitted image asset digest. Append-only and separate from private admission,
//! Work visibility, distribution clearance and audio approval: none implies it, and it implies
//! none of them. Pure: the shell supplies ownership, the asset binding, the order and the clock.
use crate::contract::studio as dto;

use super::{
    image::{operation_fingerprint, scheme_digest_valid, ImageError, ImageOperationId},
    public_cover::CoverDisclosure,
};
use crate::contexts::catalog::domain::{OwnerId, Work, WorkId};

/// Largest admitted exclusive expiry: the JSON-safe integer bound clients can represent.
pub const MAX_DISCLOSURE_UNTIL_MS: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverDisclosureError {
    Image(ImageError),
    /// `cleared` without an expiry, or `revoked` with one.
    InvalidDecision,
    /// A clearance that is not strictly after its recording time, or beyond the JSON-safe bound.
    InvalidExpiry,
    /// The digest names no admitted image of this Work and owner.
    UnknownImage,
    Corrupt,
}
impl From<ImageError> for CoverDisclosureError {
    fn from(value: ImageError) -> Self {
        Self::Image(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisclosureDecision {
    Cleared { valid_until_ms: i64 },
    Revoked,
}
impl DisclosureDecision {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Cleared { .. } => "cleared",
            Self::Revoked => "revoked",
        }
    }
    pub const fn valid_until_ms(self) -> Option<i64> {
        match self {
            Self::Cleared { valid_until_ms } => Some(valid_until_ms),
            Self::Revoked => None,
        }
    }
    /// Restore stored columns; a clearance must carry an expiry and a revocation none.
    pub fn restore(kind: &str, valid_until_ms: Option<i64>) -> Result<Self, CoverDisclosureError> {
        match (kind, valid_until_ms) {
            ("cleared", Some(valid_until_ms)) => Ok(Self::Cleared { valid_until_ms }),
            ("revoked", None) => Ok(Self::Revoked),
            _ => Err(CoverDisclosureError::Corrupt),
        }
    }
}

/// An admitted request: exact operation identity, image digest and decision shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCoverDisclosure {
    operation_id: ImageOperationId,
    image_asset_digest: String,
    decision: DisclosureDecision,
    raw: dto::RecordCoverDisclosureRequest,
}
impl RecordCoverDisclosure {
    pub fn admit(raw: &dto::RecordCoverDisclosureRequest) -> Result<Self, CoverDisclosureError> {
        let operation_id = ImageOperationId::parse(&raw.operation_id)?;
        if !scheme_digest_valid(&raw.image_asset_digest, "ia-r1") {
            return Err(ImageError::InvalidDigest.into());
        }
        let decision = match (raw.decision, raw.valid_until_ms) {
            (dto::CoverDisclosureKind::Cleared, Some(until)) => {
                let valid_until_ms =
                    i64::try_from(until).map_err(|_| CoverDisclosureError::InvalidExpiry)?;
                if !(1..=MAX_DISCLOSURE_UNTIL_MS).contains(&valid_until_ms) {
                    return Err(CoverDisclosureError::InvalidExpiry);
                }
                DisclosureDecision::Cleared { valid_until_ms }
            }
            (dto::CoverDisclosureKind::Revoked, None) => DisclosureDecision::Revoked,
            _ => return Err(CoverDisclosureError::InvalidDecision),
        };
        Ok(Self {
            operation_id,
            image_asset_digest: raw.image_asset_digest.clone(),
            decision,
            raw: raw.clone(),
        })
    }
    pub fn operation_id(&self) -> &ImageOperationId {
        &self.operation_id
    }
    pub fn image_asset_digest(&self) -> &str {
        &self.image_asset_digest
    }
    pub const fn decision(&self) -> DisclosureDecision {
        self.decision
    }
    pub fn fingerprint(&self, work: &WorkId) -> Result<String, CoverDisclosureError> {
        Ok(operation_fingerprint("cover_disclosure", work, &self.raw)?)
    }
}

/// One recorded decision. Fields are private: only `decide` and checked `restore` build it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverDisclosureRecord {
    work_id: WorkId,
    sequence: u64,
    owner_id: OwnerId,
    image_asset_digest: String,
    decision: DisclosureDecision,
    recorded_at_ms: i64,
    operation_id: ImageOperationId,
    fingerprint: String,
}
/// Raw stored facts; restore re-checks every rule the writer enforced.
pub struct StoredDisclosure {
    pub work_id: String,
    pub sequence: i64,
    pub owner_id: String,
    pub image_asset_digest: String,
    pub decision: String,
    pub valid_until_ms: Option<i64>,
    pub recorded_at_ms: i64,
    pub operation_id: String,
    pub fingerprint: String,
}
impl CoverDisclosureRecord {
    pub fn restore(raw: StoredDisclosure) -> Result<Self, CoverDisclosureError> {
        let decision = DisclosureDecision::restore(&raw.decision, raw.valid_until_ms)?;
        let sequence = u64::try_from(raw.sequence).map_err(|_| CoverDisclosureError::Corrupt)?;
        if sequence == 0
            || raw.recorded_at_ms < 0
            || !scheme_digest_valid(&raw.image_asset_digest, "ia-r1")
            || !scheme_digest_valid(&raw.fingerprint, "image-operation1")
            || decision
                .valid_until_ms()
                .is_some_and(|until| until <= raw.recorded_at_ms || until > MAX_DISCLOSURE_UNTIL_MS)
        {
            return Err(CoverDisclosureError::Corrupt);
        }
        Ok(Self {
            work_id: WorkId::parse(&raw.work_id).map_err(|_| CoverDisclosureError::Corrupt)?,
            sequence,
            owner_id: OwnerId::parse(&raw.owner_id).map_err(|_| CoverDisclosureError::Corrupt)?,
            image_asset_digest: raw.image_asset_digest,
            decision,
            recorded_at_ms: raw.recorded_at_ms,
            operation_id: ImageOperationId::parse(&raw.operation_id)
                .map_err(|_| CoverDisclosureError::Corrupt)?,
            fingerprint: raw.fingerprint,
        })
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn owner_id(&self) -> &OwnerId {
        &self.owner_id
    }
    pub fn image_asset_digest(&self) -> &str {
        &self.image_asset_digest
    }
    pub const fn decision(&self) -> DisclosureDecision {
        self.decision
    }
    pub const fn recorded_at_ms(&self) -> i64 {
        self.recorded_at_ms
    }
    pub fn operation_id(&self) -> &ImageOperationId {
        &self.operation_id
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// The kernel's view of this decision, for the pinned image of the same Work and owner.
    pub fn as_cover_disclosure(&self) -> CoverDisclosure<'_> {
        match self.decision {
            DisclosureDecision::Cleared { valid_until_ms } => CoverDisclosure::Cleared {
                image_asset_digest: &self.image_asset_digest,
                valid_until_ms,
            },
            DisclosureDecision::Revoked => CoverDisclosure::Revoked {
                image_asset_digest: &self.image_asset_digest,
            },
        }
    }
    pub fn response(&self) -> dto::CoverDisclosureResponse {
        dto::CoverDisclosureResponse {
            work_id: self.work_id.as_str().into(),
            sequence: self.sequence,
            image_asset_digest: self.image_asset_digest.clone(),
            decision: match self.decision {
                DisclosureDecision::Cleared { .. } => dto::CoverDisclosureKind::Cleared,
                DisclosureDecision::Revoked => dto::CoverDisclosureKind::Revoked,
            },
            // Admitted values are within 0..=MAX_DISCLOSURE_UNTIL_MS.
            valid_until_ms: self
                .decision
                .valid_until_ms()
                .map(|until| until.unsigned_abs()),
            recorded_at_ms: self.recorded_at_ms.unsigned_abs(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverDisclosureDecision {
    /// The same operation was already recorded with the same request: return it unchanged.
    Replay(CoverDisclosureRecord),
    Apply(CoverDisclosureRecord),
}

/// Shell-resolved facts. `work` is the current head authorized for its active owner.
pub struct DisclosureFacts<'a> {
    pub work: &'a Work,
    /// The decision this owner already recorded under the request's operation ID, if any.
    pub prior: Option<&'a CoverDisclosureRecord>,
    /// Whether the digest names an admitted image of exactly this Work and owner.
    pub image_bound: bool,
    pub next_sequence: u64,
    pub now_ms: i64,
}

/// Matching operations replay before any time or binding check; a reused operation ID with a
/// different request is refused. Nothing here can make a decision about another Work's image.
pub fn decide_cover_disclosure(
    request: &RecordCoverDisclosure,
    facts: DisclosureFacts<'_>,
) -> Result<CoverDisclosureDecision, CoverDisclosureError> {
    let fingerprint = request.fingerprint(facts.work.id())?;
    if let Some(prior) = facts.prior {
        return if prior.fingerprint == fingerprint && prior.work_id == *facts.work.id() {
            Ok(CoverDisclosureDecision::Replay(prior.clone()))
        } else {
            Err(ImageError::OperationReused.into())
        };
    }
    if !facts.image_bound {
        return Err(CoverDisclosureError::UnknownImage);
    }
    if facts.now_ms < 0 || facts.next_sequence == 0 {
        return Err(CoverDisclosureError::Corrupt);
    }
    if let DisclosureDecision::Cleared { valid_until_ms } = request.decision {
        if valid_until_ms <= facts.now_ms {
            return Err(CoverDisclosureError::InvalidExpiry);
        }
    }
    Ok(CoverDisclosureDecision::Apply(CoverDisclosureRecord {
        work_id: facts.work.id().clone(),
        sequence: facts.next_sequence,
        owner_id: facts.work.owner_id().clone(),
        image_asset_digest: request.image_asset_digest.clone(),
        decision: request.decision,
        recorded_at_ms: facts.now_ms,
        operation_id: request.operation_id.clone(),
        fingerprint,
    }))
}

#[cfg(test)]
#[path = "cover_disclosure_tests.rs"]
mod tests;
