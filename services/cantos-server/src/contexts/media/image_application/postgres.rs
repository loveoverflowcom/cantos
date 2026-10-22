//! Concrete short PostgreSQL transactions for private artwork. There is no byte/decoder/
//! object-storage call in this module, and no live transaction is returned to the service.
use cantos_contracts as dto;

use super::{
    AdmissionContext, ArtworkApplicationError, ArtworkContext, ArtworkReceipt, ArtworkRepository,
    MetadataFuture, OwnerWorkScope, RecordedImageLocation,
};
use crate::{
    contexts::{
        catalog::{
            self,
            domain::{Lifecycle, OwnerId, Revision},
            OwnerWork,
        },
        editorial,
        media::{
            domain::{
                artwork::{
                    artwork_at, decide_artwork_change, ArtworkChangeValue, ArtworkDecision,
                    ArtworkError, SetWorkArtwork,
                },
                image::{
                    decide_image_admission, decide_image_prepare, ImageAdmissionDecision,
                    ImageAsset, ImageAssetId, ImageError, ImageIntent, ImageIntentId,
                    ImageMetadata, ImagePrepareDecision, PrepareImage,
                },
            },
            postgres::{artwork as sql, image as image_sql},
        },
    },
    platform::store::{LockMode, Store, StoreError, Tx},
};

#[derive(Clone)]
pub(super) struct PgArtworkRepository(pub Store);
impl ArtworkRepository for PgArtworkRepository {
    fn prepare(
        &self,
        scope: OwnerWorkScope,
        request: PrepareImage,
    ) -> MetadataFuture<ArtworkReceipt<dto::ImageIntentDocument>> {
        let store = self.0.clone();
        Box::pin(async move { prepare(&store, &scope, request).await })
    }
    fn intent(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
    ) -> MetadataFuture<dto::ImageIntentResponse> {
        let store = self.0.clone();
        Box::pin(async move { intent(&store, &scope, &id).await })
    }
    fn admission_context(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
    ) -> MetadataFuture<AdmissionContext> {
        let store = self.0.clone();
        Box::pin(async move { admission_context(&store, &scope, &id).await })
    }
    fn finalize(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
        metadata: ImageMetadata,
        location: RecordedImageLocation,
    ) -> MetadataFuture<ArtworkReceipt<dto::ImageAssetDocument>> {
        let store = self.0.clone();
        Box::pin(async move { finalize(&store, &scope, &id, metadata, location).await })
    }
    fn asset(
        &self,
        scope: OwnerWorkScope,
        id: ImageAssetId,
    ) -> MetadataFuture<image_sql::StoredImageAsset> {
        let store = self.0.clone();
        Box::pin(async move { asset(&store, &scope, &id).await })
    }
    fn artwork(
        &self,
        scope: OwnerWorkScope,
        revision: Option<Revision>,
    ) -> MetadataFuture<dto::WorkArtworkResponse> {
        let store = self.0.clone();
        Box::pin(async move { artwork(&store, &scope, revision).await })
    }
    fn artwork_context(
        &self,
        scope: OwnerWorkScope,
        request: SetWorkArtwork,
    ) -> MetadataFuture<ArtworkContext> {
        let store = self.0.clone();
        Box::pin(async move { artwork_context(&store, &scope, &request).await })
    }
    fn set_artwork(
        &self,
        scope: OwnerWorkScope,
        request: SetWorkArtwork,
        observed_asset: Option<image_sql::StoredImageAsset>,
    ) -> MetadataFuture<ArtworkReceipt<dto::WorkArtworkResponse>> {
        let store = self.0.clone();
        Box::pin(
            async move { set_artwork(&store, &scope, &request, observed_asset.as_ref()).await },
        )
    }
}

fn require_draft(owner: &OwnerWork) -> Result<(), ArtworkApplicationError> {
    if owner.work.lifecycle() != Lifecycle::Draft {
        return Err(ArtworkError::from(catalog::domain::CatalogError::WorkArchived).into());
    }
    Ok(())
}
async fn rights(
    tx: &Tx<'_>,
    owner: &OwnerId,
    refs: &[String],
) -> Result<(), ArtworkApplicationError> {
    for evidence in refs {
        if !editorial::evidence_exists(tx, owner.as_str(), "rights", evidence).await? {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    Ok(())
}
async fn scoped_intent(
    tx: &Tx<'_>,
    owner: &OwnerWork,
    id: &ImageIntentId,
) -> Result<image_sql::StoredImageIntent, ArtworkApplicationError> {
    image_sql::load_intent(tx, &owner.actor, owner.work.id(), id)
        .await?
        .ok_or_else(|| StoreError::NotFound.into())
}
async fn selected_asset(
    tx: &Tx<'_>,
    owner: &OwnerWork,
    request: &SetWorkArtwork,
) -> Result<Option<image_sql::StoredImageAsset>, ArtworkApplicationError> {
    match request.change() {
        ArtworkChangeValue::Clear => Ok(None),
        ArtworkChangeValue::Set(selection) => {
            let stored = image_sql::load_asset_by_id(
                tx,
                &owner.actor,
                owner.work.id(),
                selection.image_asset_id(),
            )
            .await?
            .ok_or(StoreError::NotFound)?;
            selection.require_asset(&owner.work, &stored.asset)?;
            Ok(Some(stored))
        }
    }
}
async fn prior_artwork(
    tx: &Tx<'_>,
    token: &str,
    owner: &OwnerWork,
    request: &SetWorkArtwork,
) -> Result<Option<ArtworkReceipt<dto::WorkArtworkResponse>>, ArtworkApplicationError> {
    let Some(operation) = sql::prior_operation(tx, &owner.actor, request.operation_id()).await?
    else {
        return Ok(None);
    };
    if operation.work_id != *owner.work.id()
        || operation.kind != sql::ArtworkOperationKind::SetArtwork
        || operation.fingerprint != request.fingerprint(owner.work.id())?
    {
        return Err(ImageError::OperationReused.into());
    }
    let value = operation.artwork_response()?;
    if value.work_revision
        != request
            .expected_work_revision()
            .get()
            .checked_add(1)
            .ok_or(StoreError::CorruptRevision)?
        || value.artwork.as_ref()
            != request
                .change()
                .selection()
                .map(|value| value.document())
                .as_ref()
    {
        return Err(StoreError::CorruptRevision.into());
    }
    // Original response must remain anchored to its exact immutable Catalog/Media history.
    let revision = Revision::new(value.work_revision).map_err(|_| StoreError::CorruptRevision)?;
    let historical = catalog::owner_work_revision(tx, token, owner.work.id(), Some(revision))
        .await
        .map_err(|error| match error {
            catalog::CatalogStoreError::Storage(StoreError::NotFound) => {
                ArtworkApplicationError::Storage(StoreError::CorruptRevision)
            }
            other => ArtworkApplicationError::Catalog(other),
        })?;
    let change = sql::effective(tx, &owner.actor, owner.work.id(), revision)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    if change.work_metadata_revision() != revision
        || artwork_at(&historical.work, &[change]).map_err(|_| StoreError::CorruptRevision)?
            != value
    {
        return Err(StoreError::CorruptRevision.into());
    }
    Ok(Some(ArtworkReceipt {
        value,
        json_bytes: operation.response_bytes,
    }))
}

async fn prepare(
    store: &Store,
    scope: &OwnerWorkScope,
    request: PrepareImage,
) -> Result<ArtworkReceipt<dto::ImageIntentDocument>, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Update).await?;
    let fingerprint = request.fingerprint(owner.work.id())?;
    let prior = sql::prior_operation(&tx, &owner.actor, request.operation_id()).await?;
    let existing = image_sql::load_intent(&tx, &owner.actor, owner.work.id(), request.id()).await?;
    let decision = decide_image_prepare(
        &request,
        owner.work.id(),
        prior.as_ref().map(|value| value.fingerprint.as_str()),
        existing.as_ref().map(|value| &value.intent),
    )?;
    let receipt = match decision {
        ImagePrepareDecision::Replay => {
            let prior = prior.ok_or(StoreError::CorruptRevision)?;
            if prior.kind != sql::ArtworkOperationKind::PrepareImage
                || prior.work_id != *owner.work.id()
            {
                return Err(ImageError::OperationReused.into());
            }
            let intent = prior.image_intent()?;
            let existing = existing.ok_or(StoreError::CorruptRevision)?;
            if intent.document() != existing.intent.document()
                || prior.response_bytes != existing.document_bytes
                || intent.request().document() != request.document()
            {
                return Err(StoreError::CorruptRevision.into());
            }
            ArtworkReceipt {
                value: intent.document().clone(),
                json_bytes: prior.response_bytes,
            }
        }
        ImagePrepareDecision::Create => {
            require_draft(&owner)?;
            if image_sql::intent_id_exists(&tx, request.id()).await? {
                return Err(ImageError::DuplicateIntent.into());
            }
            rights(&tx, &owner.actor, request.declarations().rights_refs()).await?;
            let now = catalog::owner_work_now(&tx).await?;
            let intent = ImageIntent::create(request, owner.work.id(), &owner.actor, now)?;
            let bytes = image_sql::insert_intent(&tx, &intent).await?;
            sql::record_operation(
                &tx,
                &owner.actor,
                intent.request().operation_id(),
                owner.work.id(),
                sql::ArtworkOperationKind::PrepareImage,
                &fingerprint,
                &bytes,
            )
            .await?;
            ArtworkReceipt {
                value: intent.document().clone(),
                json_bytes: bytes,
            }
        }
    };
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(receipt)
}

async fn intent(
    store: &Store,
    scope: &OwnerWorkScope,
    id: &ImageIntentId,
) -> Result<dto::ImageIntentResponse, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Share).await?;
    let stored = scoped_intent(&tx, &owner, id).await?;
    let asset = image_sql::load_asset(&tx, &stored.intent).await?;
    let now = catalog::owner_work_now(&tx).await?;
    let state = stored
        .intent
        .state(asset.as_ref().map(|value| &value.asset), now)
        .map_err(|_| StoreError::CorruptRevision)?;
    let value = dto::ImageIntentResponse {
        intent: stored.intent.document().clone(),
        asset: asset.map(|value| value.asset.document().clone()),
        state,
    };
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(value)
}

async fn admission_context(
    store: &Store,
    scope: &OwnerWorkScope,
    id: &ImageIntentId,
) -> Result<AdmissionContext, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Share).await?;
    let intent = scoped_intent(&tx, &owner, id).await?;
    let asset = image_sql::load_asset(&tx, &intent.intent).await?;
    let now = catalog::owner_work_now(&tx).await?;
    if asset.is_none() {
        require_draft(&owner)?;
        intent.intent.require_pending(now)?;
        rights(
            &tx,
            &owner.actor,
            intent.intent.request().declarations().rights_refs(),
        )
        .await?;
    } else {
        intent
            .intent
            .state(asset.as_ref().map(|value| &value.asset), now)
            .map_err(|_| StoreError::CorruptRevision)?;
    }
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(AdmissionContext {
        work: owner.work,
        intent,
        asset,
        now,
    })
}

async fn finalize(
    store: &Store,
    scope: &OwnerWorkScope,
    id: &ImageIntentId,
    metadata: ImageMetadata,
    location: RecordedImageLocation,
) -> Result<ArtworkReceipt<dto::ImageAssetDocument>, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Update).await?;
    let intent = scoped_intent(&tx, &owner, id).await?;
    let asset = image_sql::load_asset(&tx, &intent.intent).await?;
    let now = catalog::owner_work_now(&tx).await?;
    let decision = decide_image_admission(
        &intent.intent,
        &metadata,
        asset.as_ref().map(|value| &value.asset),
        now,
    )?;
    let receipt = match decision {
        ImageAdmissionDecision::Replay => {
            let asset = asset.ok_or(StoreError::CorruptRevision)?;
            ArtworkReceipt {
                value: asset.asset.document().clone(),
                json_bytes: asset.document_bytes,
            }
        }
        ImageAdmissionDecision::Create => {
            require_draft(&owner)?;
            rights(
                &tx,
                &owner.actor,
                intent.intent.request().declarations().rights_refs(),
            )
            .await?;
            let now = catalog::owner_work_now(&tx).await?;
            let asset = ImageAsset::create(&intent.intent, metadata, now)?;
            let bytes = image_sql::insert_asset(
                &tx,
                &asset,
                &location.storage_target,
                &location.object_key,
            )
            .await?;
            // Time can advance while awaiting INSERT/locks; expire before commit and roll back
            // metadata while retaining only the immutable private orphan from earlier I/O.
            intent
                .intent
                .require_pending(catalog::owner_work_now(&tx).await?)?;
            ArtworkReceipt {
                value: asset.document().clone(),
                json_bytes: bytes,
            }
        }
    };
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(receipt)
}

async fn asset(
    store: &Store,
    scope: &OwnerWorkScope,
    id: &ImageAssetId,
) -> Result<image_sql::StoredImageAsset, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Share).await?;
    let stored = image_sql::load_asset_by_id(&tx, &owner.actor, owner.work.id(), id)
        .await?
        .ok_or(StoreError::NotFound)?;
    if stored.asset.admitted_at_ms() > catalog::owner_work_now(&tx).await? {
        return Err(StoreError::CorruptRevision.into());
    }
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(stored)
}

async fn artwork(
    store: &Store,
    scope: &OwnerWorkScope,
    revision: Option<Revision>,
) -> Result<dto::WorkArtworkResponse, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work_revision(&tx, &scope.token, &scope.work_id, revision).await?;
    let revision = Revision::new(owner.work.revision()).map_err(|_| StoreError::CorruptRevision)?;
    let change = sql::effective(&tx, &owner.actor, owner.work.id(), revision).await?;
    let value = artwork_at(&owner.work, &change.into_iter().collect::<Vec<_>>())
        .map_err(|_| StoreError::CorruptRevision)?;
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(value)
}

async fn artwork_context(
    store: &Store,
    scope: &OwnerWorkScope,
    request: &SetWorkArtwork,
) -> Result<ArtworkContext, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Share).await?;
    let replay = prior_artwork(&tx, &scope.token, &owner, request).await?;
    let asset = if replay.is_some() {
        None
    } else {
        let asset = selected_asset(&tx, &owner, request).await?;
        let now = catalog::owner_work_now(&tx)
            .await?
            .max(owner.work.updated_at());
        decide_artwork_change(
            &owner.work,
            request,
            asset.as_ref().map(|value| &value.asset),
            None,
            now,
        )?;
        if let Some(asset) = &asset {
            rights(&tx, &owner.actor, asset.asset.declarations().rights_refs()).await?;
        }
        asset
    };
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(ArtworkContext {
        work: owner.work,
        asset,
        replay,
    })
}

async fn set_artwork(
    store: &Store,
    scope: &OwnerWorkScope,
    request: &SetWorkArtwork,
    observed: Option<&image_sql::StoredImageAsset>,
) -> Result<ArtworkReceipt<dto::WorkArtworkResponse>, ArtworkApplicationError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(StoreError::from)?;
    let owner = catalog::owner_work(&tx, &scope.token, &scope.work_id, LockMode::Update).await?;
    if let Some(replay) = prior_artwork(&tx, &scope.token, &owner, request).await? {
        catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
        tx.commit().await.map_err(StoreError::from)?;
        return Ok(replay);
    }
    let asset = selected_asset(&tx, &owner, request).await?;
    match (&asset, observed) {
        (None, None) => {}
        (Some(current), Some(observed)) if super::same_stored_asset(current, observed) => {}
        _ => return Err(StoreError::CorruptRevision.into()),
    }
    if let Some(asset) = &asset {
        rights(&tx, &owner.actor, asset.asset.declarations().rights_refs()).await?;
    }
    let at = catalog::owner_work_now(&tx)
        .await?
        .max(owner.work.updated_at());
    let mutation = match decide_artwork_change(
        &owner.work,
        request,
        asset.as_ref().map(|value| &value.asset),
        None,
        at,
    )? {
        ArtworkDecision::Apply(mutation) => mutation,
        ArtworkDecision::Replay => return Err(StoreError::CorruptRevision.into()),
    };
    let response = mutation.response().clone();
    let bytes = serde_json::to_vec(&response).map_err(|_| StoreError::CorruptRevision)?;
    let updated = catalog::advance_owner_work(
        &tx,
        &scope.token,
        owner.work.id(),
        request.expected_work_revision(),
        at,
    )
    .await?;
    if &updated != mutation.work() {
        return Err(StoreError::CorruptRevision.into());
    }
    sql::insert_change(&tx, mutation.change()).await?;
    sql::record_operation(
        &tx,
        &owner.actor,
        request.operation_id(),
        owner.work.id(),
        sql::ArtworkOperationKind::SetArtwork,
        &request.fingerprint(owner.work.id())?,
        &bytes,
    )
    .await?;
    catalog::recheck_owner_work(&tx, &scope.token, &owner).await?;
    tx.commit().await.map_err(StoreError::from)?;
    Ok(ArtworkReceipt {
        value: response,
        json_bytes: bytes,
    })
}
