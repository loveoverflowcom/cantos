//! Append-only effective artwork and exact operation receipts. Catalog owns the Work clock,
//! history and authorization; this adapter owns only Media's tables and checked row admission.
use crate::contract::studio as dto;
use crate::platform::store::Row;
use sha2::{Digest, Sha256};

use super::image::{corrupt, decode_document, document_bytes, matches};
use crate::contexts::catalog::domain::{OwnerId, Revision, WorkId};
use crate::contexts::media::domain::{
    artwork::{restore_artwork_response, ArtworkChange, ArtworkChangeDocument},
    image::{sha256_valid, ImageIntent, ImageOperationId, MAX_IMAGE_DOCUMENT_BYTES},
};
use crate::platform::store::{checked_number, StoreError, Tx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::contexts::media) enum ArtworkOperationKind {
    PrepareImage,
    SetArtwork,
}
impl ArtworkOperationKind {
    pub(in crate::contexts::media) const fn as_str(self) -> &'static str {
        match self {
            Self::PrepareImage => "prepare_image",
            Self::SetArtwork => "set_artwork",
        }
    }
    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "prepare_image" => Ok(Self::PrepareImage),
            "set_artwork" => Ok(Self::SetArtwork),
            _ => Err(StoreError::CorruptRevision),
        }
    }
}

pub(in crate::contexts::media) struct ArtworkOperation {
    pub work_id: WorkId,
    pub kind: ArtworkOperationKind,
    pub fingerprint: String,
    pub response_bytes: Vec<u8>,
    owner_id: OwnerId,
    operation_id: ImageOperationId,
}
impl ArtworkOperation {
    /// Decode only after the application compares command kind/path/fingerprint. Original
    /// bytes remain the JSON replay; checked documents serve the optional Protobuf transport.
    pub(in crate::contexts::media) fn image_intent(&self) -> Result<ImageIntent, StoreError> {
        if self.kind != ArtworkOperationKind::PrepareImage {
            return Err(StoreError::CorruptRevision);
        }
        let raw: dto::ImageIntentDocument = decode_document(&self.response_bytes)?;
        let intent = ImageIntent::restore(raw).map_err(|_| StoreError::CorruptRevision)?;
        if intent.work_id() != &self.work_id
            || intent.owner_id() != &self.owner_id
            || intent.request().operation_id() != &self.operation_id
            || intent
                .request()
                .fingerprint(&self.work_id)
                .map_err(|_| StoreError::CorruptRevision)?
                != self.fingerprint
        {
            return Err(StoreError::CorruptRevision);
        }
        Ok(intent)
    }
    pub(in crate::contexts::media) fn artwork_response(
        &self,
    ) -> Result<dto::WorkArtworkResponse, StoreError> {
        if self.kind != ArtworkOperationKind::SetArtwork {
            return Err(StoreError::CorruptRevision);
        }
        let raw: dto::WorkArtworkResponse = decode_document(&self.response_bytes)?;
        let revision = Revision::new(raw.work_revision).map_err(|_| StoreError::CorruptRevision)?;
        if raw.changed_at_ms.is_none() {
            return Err(StoreError::CorruptRevision);
        }
        restore_artwork_response(raw, &self.work_id, revision)
            .map_err(|_| StoreError::CorruptRevision)
    }
}
fn fingerprint_valid(value: &str) -> bool {
    value
        .strip_prefix("image-operation1:sha256:")
        .is_some_and(sha256_valid)
}
fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn change_row(row: &Row) -> Result<ArtworkChange, StoreError> {
    let bytes: Vec<u8> = corrupt(row.try_get("document_bytes"))?;
    let raw: ArtworkChangeDocument = decode_document(&bytes)?;
    let change = ArtworkChange::restore(raw).map_err(|_| StoreError::CorruptRevision)?;
    let d = change.document();
    for (column, value) in [
        ("work_id", d.work_id.as_str()),
        ("owner_id", d.owner_id.as_str()),
        ("recorded_by", d.recorded_by.as_str()),
        ("digest", d.digest.as_str()),
    ] {
        matches(row, column, value)?;
    }
    if corrupt(row.try_get::<_, i64>("work_metadata_revision"))?
        != checked_number(d.work_metadata_revision).map_err(|_| StoreError::CorruptRevision)?
        || corrupt(row.try_get::<_, i64>("recorded_at_ms"))? != change.recorded_at_ms()
    {
        return Err(StoreError::CorruptRevision);
    }
    let selection = change.change().selection();
    for (column, expected) in [
        (
            "image_asset_id",
            selection.map(|s| s.image_asset_id().as_str()),
        ),
        (
            "image_asset_digest",
            selection.map(|s| s.image_asset_digest()),
        ),
        ("alt_text", selection.map(|s| s.alt_text())),
    ] {
        if corrupt(row.try_get::<_, Option<String>>(column))?.as_deref() != expected {
            return Err(StoreError::CorruptRevision);
        }
    }
    Ok(change)
}

/// The (work_id,work_metadata_revision) primary key supports one backward index scan. A
/// clear row is returned intact; no filtering can resurrect a previously selected cover.
pub(in crate::contexts::media) async fn effective(
    tx: &Tx<'_>,
    owner: &OwnerId,
    work: &WorkId,
    revision: Revision,
) -> Result<Option<ArtworkChange>, StoreError> {
    tx.query_opt(
        "SELECT * FROM media_work_artwork_changes WHERE work_id=?1 AND owner_id=?2 AND work_metadata_revision<=?3 ORDER BY work_metadata_revision DESC LIMIT 1",
        &[&work.as_str(),&owner.as_str(),&checked_number(revision.get())?],
    ).await?.as_ref().map(change_row).transpose()
}
pub(in crate::contexts::media) async fn insert_change(
    tx: &Tx<'_>,
    value: &ArtworkChange,
) -> Result<Vec<u8>, StoreError> {
    let d = value.document();
    let bytes = document_bytes(d)?;
    let selection = value.change().selection();
    tx.execute(
        "INSERT INTO media_work_artwork_changes(work_id,work_metadata_revision,owner_id,image_asset_id,image_asset_digest,alt_text,recorded_by,recorded_at_ms,document_bytes,digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        &[&d.work_id,&checked_number(d.work_metadata_revision)?,&d.owner_id,
          &selection.map(|s|s.image_asset_id().as_str()),&selection.map(|s|s.image_asset_digest()),
          &selection.map(|s|s.alt_text()),&d.recorded_by,&value.recorded_at_ms(),&bytes,&d.digest],
    ).await.map_err(|error| {
        if error.constraint()==Some(cantos_store_turso::error::ConstraintKind::Unique) {StoreError::OperationReused}
        else {StoreError::from(error)}
    })?;
    Ok(bytes)
}
pub(in crate::contexts::media) async fn prior_operation(
    tx: &Tx<'_>,
    owner: &OwnerId,
    operation: &ImageOperationId,
) -> Result<Option<ArtworkOperation>, StoreError> {
    let row = tx.query_opt(
        "SELECT owner_id,operation_id,work_id,kind,fingerprint,response_bytes,response_checksum FROM media_artwork_operations WHERE owner_id=?1 AND operation_id=?2",
        &[&owner.as_str(),&operation.as_str()],
    ).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let work: String = corrupt(row.try_get("work_id"))?;
    let work_id = WorkId::parse(&work).map_err(|_| StoreError::CorruptRevision)?;
    matches(&row, "owner_id", owner.as_str())?;
    matches(&row, "operation_id", operation.as_str())?;
    let kind = ArtworkOperationKind::parse(&corrupt(row.try_get::<_, String>("kind"))?)?;
    let fingerprint: String = corrupt(row.try_get("fingerprint"))?;
    let response_bytes: Vec<u8> = corrupt(row.try_get("response_bytes"))?;
    let response_checksum: String = corrupt(row.try_get("response_checksum"))?;
    if !fingerprint_valid(&fingerprint)
        || response_bytes.is_empty()
        || response_bytes.len() > MAX_IMAGE_DOCUMENT_BYTES
        || !sha256_valid(&response_checksum)
        || checksum(&response_bytes) != response_checksum
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(Some(ArtworkOperation {
        work_id,
        kind,
        fingerprint,
        response_bytes,
        owner_id: owner.clone(),
        operation_id: operation.clone(),
    }))
}
pub(in crate::contexts::media) async fn record_operation(
    tx: &Tx<'_>,
    owner: &OwnerId,
    operation: &ImageOperationId,
    work: &WorkId,
    kind: ArtworkOperationKind,
    fingerprint: &str,
    response_bytes: &[u8],
) -> Result<(), StoreError> {
    if !fingerprint_valid(fingerprint) {
        return Err(StoreError::CorruptRevision);
    }
    let admitted = ArtworkOperation {
        work_id: work.clone(),
        kind,
        fingerprint: fingerprint.into(),
        response_bytes: response_bytes.into(),
        owner_id: owner.clone(),
        operation_id: operation.clone(),
    };
    match kind {
        ArtworkOperationKind::PrepareImage => {
            admitted.image_intent()?;
        }
        ArtworkOperationKind::SetArtwork => {
            admitted.artwork_response()?;
        }
    }
    tx.execute(
        "INSERT INTO media_artwork_operations(owner_id,operation_id,work_id,kind,fingerprint,response_bytes,response_checksum) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        &[&owner.as_str(),&operation.as_str(),&work.as_str(),&kind.as_str(),&fingerprint,&response_bytes,&checksum(response_bytes)],
    ).await.map_err(|error| {
        if error.constraint()==Some(cantos_store_turso::error::ConstraintKind::Unique) {StoreError::OperationReused}
        else {StoreError::from(error)}
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../test_support/artwork_turso_tests.rs"]
pub(in crate::contexts::media) mod tests;
