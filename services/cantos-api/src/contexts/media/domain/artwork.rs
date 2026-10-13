//! Work-cover selection belongs to Media; Catalog retains revision/lifecycle/time authority.
//! Pure plans append one immutable change, including explicit clear tombstones. The application
//! atomically persists the plan with Catalog's touched snapshot and the operation receipt.
use std::collections::BTreeSet;

use cantos_contracts as dto;
use serde::{Deserialize, Deserializer, Serialize};

use crate::contexts::catalog::domain::{CatalogError, OwnerId, Revision, Work, WorkId};

use super::image::{
    bounded_document, document_digest, operation_fingerprint, scheme_digest_valid, storage_time,
    ImageAsset, ImageAssetId, ImageError, ImageOperationId,
};

pub const MAX_ARTWORK_ALT_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtworkError {
    Image(ImageError),
    Catalog(CatalogError),
    BlankAltText,
    AltTextTooLong,
    AltTextControlCharacter,
    AssetBindingMismatch,
    MissingAsset,
    UnexpectedAsset,
    InvalidRecordedBy,
    DuplicateRevision,
    InvalidResponse,
}
impl From<ImageError> for ArtworkError {
    fn from(value: ImageError) -> Self {
        Self::Image(value)
    }
}
impl From<CatalogError> for ArtworkError {
    fn from(value: CatalogError) -> Self {
        Self::Catalog(value)
    }
}

/// An exact checked reference; existence, ownership and storage verification are separate facts.
/// ```compile_fail,E0616
/// use cantos_api::contexts::media::domain::artwork::WorkArtworkSelection;
/// fn overwrite(selection: &mut WorkArtworkSelection) { selection.alt_text = String::new(); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkArtworkSelection {
    image_asset_id: ImageAssetId,
    image_asset_digest: String,
    alt_text: String,
}
impl WorkArtworkSelection {
    pub fn new(
        image_asset_id: ImageAssetId,
        image_asset_digest: String,
        alt_text: String,
    ) -> Result<Self, ArtworkError> {
        if !scheme_digest_valid(&image_asset_digest, "ia-r1") {
            return Err(ImageError::InvalidDigest.into());
        }
        if alt_text.trim().is_empty() {
            return Err(ArtworkError::BlankAltText);
        }
        if alt_text.len() > MAX_ARTWORK_ALT_BYTES {
            return Err(ArtworkError::AltTextTooLong);
        }
        if alt_text
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
        {
            return Err(ArtworkError::AltTextControlCharacter);
        }
        Ok(Self {
            image_asset_id,
            image_asset_digest,
            alt_text,
        })
    }
    pub fn restore(raw: &dto::WorkArtworkSelection) -> Result<Self, ArtworkError> {
        Self::new(
            ImageAssetId::parse(&raw.image_asset_id)?,
            raw.image_asset_digest.clone(),
            raw.alt_text.clone(),
        )
    }
    pub fn image_asset_id(&self) -> &ImageAssetId {
        &self.image_asset_id
    }
    pub fn image_asset_digest(&self) -> &str {
        &self.image_asset_digest
    }
    pub fn alt_text(&self) -> &str {
        &self.alt_text
    }
    pub fn document(&self) -> dto::WorkArtworkSelection {
        dto::WorkArtworkSelection {
            image_asset_id: self.image_asset_id.as_str().into(),
            image_asset_digest: self.image_asset_digest.clone(),
            alt_text: self.alt_text.clone(),
        }
    }
    pub fn require_asset(&self, work: &Work, asset: &ImageAsset) -> Result<(), ArtworkError> {
        if self.image_asset_id != *asset.id()
            || self.image_asset_digest != asset.digest()
            || asset.work_id() != work.id()
            || asset.owner_id() != work.owner_id()
        {
            return Err(ArtworkError::AssetBindingMismatch);
        }
        Ok(())
    }
}

/// Clear is explicit. No missing-payload or implicit latest-image state is representable here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtworkChangeValue {
    Clear,
    Set(WorkArtworkSelection),
}
impl ArtworkChangeValue {
    pub fn selection(&self) -> Option<&WorkArtworkSelection> {
        match self {
            Self::Clear => None,
            Self::Set(selection) => Some(selection),
        }
    }
    fn document(&self) -> Option<dto::WorkArtworkSelection> {
        self.selection().map(WorkArtworkSelection::document)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetWorkArtwork {
    operation_id: ImageOperationId,
    expected_work_revision: Revision,
    change: ArtworkChangeValue,
    raw: dto::SetWorkArtworkRequest,
}
impl SetWorkArtwork {
    pub fn admit(raw: &dto::SetWorkArtworkRequest) -> Result<Self, ArtworkError> {
        bounded_document(raw)?;
        let operation_id = ImageOperationId::parse(&raw.operation_id)?;
        let expected_work_revision = Revision::new(raw.expected_work_revision)?;
        let change = match &raw.artwork {
            Some(selection) => ArtworkChangeValue::Set(WorkArtworkSelection::restore(selection)?),
            None => ArtworkChangeValue::Clear,
        };
        Ok(Self {
            operation_id,
            expected_work_revision,
            change,
            raw: raw.clone(),
        })
    }
    pub fn operation_id(&self) -> &ImageOperationId {
        &self.operation_id
    }
    pub const fn expected_work_revision(&self) -> Revision {
        self.expected_work_revision
    }
    pub fn change(&self) -> &ArtworkChangeValue {
        &self.change
    }
    pub fn document(&self) -> &dto::SetWorkArtworkRequest {
        &self.raw
    }
    pub fn fingerprint(&self, work_id: &WorkId) -> Result<String, ArtworkError> {
        Ok(operation_fingerprint("set_artwork", work_id, &self.raw)?)
    }
}

/// Durable raw change document. It is not an admitted change until restore succeeds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtworkChangeDocument {
    pub work_id: String,
    pub work_metadata_revision: u64,
    pub owner_id: String,
    #[serde(deserialize_with = "required_nullable")]
    pub artwork: Option<dto::WorkArtworkSelection>,
    pub recorded_by: String,
    pub recorded_at_ms: u64,
    pub digest: String,
}
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
#[derive(Serialize)]
struct ChangePreimage<'a> {
    work_id: &'a str,
    work_metadata_revision: u64,
    owner_id: &'a str,
    artwork: &'a Option<dto::WorkArtworkSelection>,
    recorded_by: &'a str,
    recorded_at_ms: u64,
}
fn change_digest(raw: &ArtworkChangeDocument) -> Result<String, ImageError> {
    document_digest(
        "wa-r1",
        &ChangePreimage {
            work_id: &raw.work_id,
            work_metadata_revision: raw.work_metadata_revision,
            owner_id: &raw.owner_id,
            artwork: &raw.artwork,
            recorded_by: &raw.recorded_by,
            recorded_at_ms: raw.recorded_at_ms,
        },
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkChange {
    work_id: WorkId,
    work_metadata_revision: Revision,
    owner_id: OwnerId,
    change: ArtworkChangeValue,
    recorded_at_ms: i64,
    raw: ArtworkChangeDocument,
}
impl ArtworkChange {
    fn create(work: &Work, change: ArtworkChangeValue) -> Result<Self, ArtworkError> {
        let mut raw = ArtworkChangeDocument {
            work_id: work.id().as_str().into(),
            work_metadata_revision: work.revision(),
            owner_id: work.owner_id().as_str().into(),
            artwork: change.document(),
            recorded_by: work.owner_id().as_str().into(),
            recorded_at_ms: work.updated_at() as u64,
            digest: String::new(),
        };
        raw.digest = change_digest(&raw)?;
        Self::restore(raw)
    }
    pub fn restore(raw: ArtworkChangeDocument) -> Result<Self, ArtworkError> {
        bounded_document(&raw)?;
        let work_id = WorkId::parse(&raw.work_id)?;
        let work_metadata_revision = Revision::new(raw.work_metadata_revision)?;
        let owner_id = OwnerId::parse(&raw.owner_id)?;
        if raw.recorded_by != owner_id.as_str() {
            return Err(ArtworkError::InvalidRecordedBy);
        }
        let change = match &raw.artwork {
            Some(selection) => ArtworkChangeValue::Set(WorkArtworkSelection::restore(selection)?),
            None => ArtworkChangeValue::Clear,
        };
        let recorded_at_ms = storage_time(raw.recorded_at_ms)?;
        if !scheme_digest_valid(&raw.digest, "wa-r1") || change_digest(&raw)? != raw.digest {
            return Err(ImageError::InvalidDigest.into());
        }
        Ok(Self {
            work_id,
            work_metadata_revision,
            owner_id,
            change,
            recorded_at_ms,
            raw,
        })
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub const fn work_metadata_revision(&self) -> Revision {
        self.work_metadata_revision
    }
    pub fn owner_id(&self) -> &OwnerId {
        &self.owner_id
    }
    pub fn change(&self) -> &ArtworkChangeValue {
        &self.change
    }
    pub const fn recorded_at_ms(&self) -> i64 {
        self.recorded_at_ms
    }
    pub fn document(&self) -> &ArtworkChangeDocument {
        &self.raw
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkMutation {
    work: Work,
    change: ArtworkChange,
    response: dto::WorkArtworkResponse,
}
impl ArtworkMutation {
    pub fn work(&self) -> &Work {
        &self.work
    }
    pub fn change(&self) -> &ArtworkChange {
        &self.change
    }
    pub fn response(&self) -> &dto::WorkArtworkResponse {
        &self.response
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtworkDecision {
    /// The shell returns exact original response bytes after current authorization.
    Replay,
    Apply(Box<ArtworkMutation>),
}
/// No in-place mutation occurs. Matching operations replay before draft/CAS/time checks; new
/// explicit set/clear requests always use Work.touch, even if the value already matches.
pub fn decide_artwork_change(
    work: &Work,
    request: &SetWorkArtwork,
    resolved_asset: Option<&ImageAsset>,
    prior_operation_fingerprint: Option<&str>,
    now: i64,
) -> Result<ArtworkDecision, ArtworkError> {
    let fingerprint = request.fingerprint(work.id())?;
    if let Some(prior) = prior_operation_fingerprint {
        return if prior == fingerprint {
            Ok(ArtworkDecision::Replay)
        } else {
            Err(ImageError::OperationReused.into())
        };
    }
    match (&request.change, resolved_asset) {
        (ArtworkChangeValue::Set(selection), Some(asset)) => {
            selection.require_asset(work, asset)?
        }
        (ArtworkChangeValue::Set(_), None) => return Err(ArtworkError::MissingAsset),
        (ArtworkChangeValue::Clear, Some(_)) => return Err(ArtworkError::UnexpectedAsset),
        (ArtworkChangeValue::Clear, None) => {}
    }
    let touched = work.touch(request.expected_work_revision.get(), now)?;
    let change = ArtworkChange::create(&touched, request.change.clone())?;
    let response = response(&touched, Some(&change));
    bounded_document(&response)?;
    Ok(ArtworkDecision::Apply(Box::new(ArtworkMutation {
        work: touched,
        change,
        response,
    })))
}

fn response(work: &Work, change: Option<&ArtworkChange>) -> dto::WorkArtworkResponse {
    dto::WorkArtworkResponse {
        work_id: work.id().as_str().into(),
        work_revision: work.revision(),
        artwork: change.and_then(|change| change.change.document()),
        changed_at_ms: change.map(|change| change.recorded_at_ms as u64),
    }
}

/// Resolve at an exact admitted Catalog snapshot. A clear row remains the newest change and
/// prevents resurrection; unrelated Catalog edits do not copy or rewrite prior selections.
/// Existence/authorization of the requested snapshot is the Catalog facade's obligation.
pub fn artwork_at(
    work: &Work,
    changes: &[ArtworkChange],
) -> Result<dto::WorkArtworkResponse, ArtworkError> {
    let mut seen = BTreeSet::new();
    let mut effective: Option<&ArtworkChange> = None;
    for change in changes {
        if change.work_id != *work.id() || change.owner_id != *work.owner_id() {
            return Err(ArtworkError::AssetBindingMismatch);
        }
        if !seen.insert(change.work_metadata_revision.get()) {
            return Err(ArtworkError::DuplicateRevision);
        }
        if change.work_metadata_revision.get() <= work.revision() {
            if change.recorded_at_ms < work.created_at()
                || change.recorded_at_ms > work.updated_at()
            {
                return Err(ImageError::InvalidTimestamp.into());
            }
            if effective.is_none_or(|prior| {
                prior.work_metadata_revision.get() < change.work_metadata_revision.get()
            }) {
                effective = Some(change);
            }
        }
    }
    Ok(response(work, effective))
}

/// Receipt integrity is not enough: callers restore response shape and exact scope as well.
pub fn restore_artwork_response(
    raw: dto::WorkArtworkResponse,
    work_id: &WorkId,
    revision: Revision,
) -> Result<dto::WorkArtworkResponse, ArtworkError> {
    bounded_document(&raw)?;
    if raw.work_id != work_id.as_str()
        || raw.work_revision != revision.get()
        || (raw.artwork.is_some() && raw.changed_at_ms.is_none())
    {
        return Err(ArtworkError::InvalidResponse);
    }
    if let Some(selection) = &raw.artwork {
        WorkArtworkSelection::restore(selection)?;
    }
    if let Some(at) = raw.changed_at_ms {
        storage_time(at)?;
    }
    Ok(raw)
}

#[cfg(test)]
#[path = "artwork_tests.rs"]
mod tests;
