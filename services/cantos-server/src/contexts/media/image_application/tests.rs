//! L4 mock integration: the production ArtworkService and sealed PNG/JPEG inspector are
//! exercised with FakeMetadataRepository, a fixed clock and FakePrivateStorage. These
//! tests do not establish SQL locking/constraints, real session auth or S3 behavior.
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
};

use futures_util::{stream, StreamExt};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex as AsyncMutex, Notify};

use super::*;
use crate::contexts::{
    catalog::domain::{CatalogError, NewWork, OwnerId, Visibility, WorkDetails, WorkKind},
    media::{
        domain::{
            artwork::{artwork_at, decide_artwork_change, ArtworkChange, ArtworkDecision},
            image::{
                decide_image_admission, decide_image_prepare, ImageAdmissionDecision, ImageAsset,
                ImageIntent, ImagePrepareDecision,
            },
        },
        turso::image::StoredImageIntent,
    },
};

// Independently produced fixtures and hashes are shared with the actual decoder tests.
const PNG: &[u8] = include_bytes!("../image_intake/fixtures/python-rgb-2x3.png");
const JPEG: &[u8] = include_bytes!("../image_intake/fixtures/pillow-rgb-2x3.jpg");
const PNG_HASH: &str = "7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276";
const JPEG_HASH: &str = "a259cc25075434554da937e24a8056b5f9db0608a448c06effac4ba9c950eb46";
const WORK: &str = "00000000-0000-0000-0000-000000000001";
const OTHER_WORK: &str = "00000000-0000-0000-0000-000000000002";
const OWNER: &str = "00000000-0000-0000-0000-000000000010";
const OTHER_OWNER: &str = "00000000-0000-0000-0000-000000000011";
const NOW: i64 = 100;
static TEST_LANES: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

fn id(number: u32) -> String {
    format!("00000000-0000-0000-0000-{number:012}")
}
fn request(number: u32, jpeg: bool) -> dto::CreateImageIntentRequest {
    dto::CreateImageIntentRequest {
        id: id(number),
        operation_id: id(number + 1000),
        expected_sha256: if jpeg { JPEG_HASH } else { PNG_HASH }.into(),
        expected_byte_length: if jpeg { JPEG.len() } else { PNG.len() } as u64,
        content_type: if jpeg {
            dto::ImageMime::Jpeg
        } else {
            dto::ImageMime::Png
        },
        declarations: dto::ImageDeclarations {
            provenance: "Externally prepared synthetic cover".into(),
            rights_refs: vec!["rights-a".into()],
        },
    }
}
fn set(
    number: u32,
    revision: u64,
    asset: &dto::ImageAssetDocument,
    alt: &str,
) -> dto::SetWorkArtworkRequest {
    dto::SetWorkArtworkRequest {
        operation_id: id(number),
        expected_work_revision: revision,
        artwork: Some(dto::WorkArtworkSelection {
            image_asset_id: asset.id.clone(),
            image_asset_digest: asset.digest.clone(),
            alt_text: alt.into(),
        }),
    }
}
fn clear(number: u32, revision: u64) -> dto::SetWorkArtworkRequest {
    dto::SetWorkArtworkRequest {
        operation_id: id(number),
        expected_work_revision: revision,
        artwork: None,
    }
}
fn receipt<T: serde::Serialize + Clone>(value: &T) -> ArtworkReceipt<T> {
    ArtworkReceipt {
        value: value.clone(),
        json_bytes: serde_json::to_vec(value).unwrap(),
    }
}

#[derive(Clone)]
enum Operation {
    Prepare {
        work: WorkId,
        fingerprint: String,
        receipt: ArtworkReceipt<dto::ImageIntentDocument>,
    },
    Set {
        work: WorkId,
        fingerprint: String,
        receipt: ArtworkReceipt<dto::WorkArtworkResponse>,
    },
}
#[derive(Clone, Copy)]
enum Change {
    Revoke,
    ExpireSession,
    ExpireIntent,
    Archive,
    RemoveRights,
    CatalogEdit,
    EqualAdmissionOnOriginalTarget,
}
#[derive(Clone)]
struct Pause {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}
impl Pause {
    fn new() -> Self {
        Self {
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }
}

#[derive(Clone)]
struct FakeState {
    work: Work,
    history: BTreeMap<u64, Work>,
    intents: BTreeMap<String, StoredImageIntent>,
    assets: BTreeMap<String, StoredImageAsset>,
    changes: Vec<ArtworkChange>,
    operations: BTreeMap<String, Operation>,
    rights: BTreeMap<String, String>,
    now: i64,
    session_active: bool,
    session_expires: i64,
    tx_active: bool,
    events: Vec<String>,
    context_calls: usize,
    context_change: Option<(usize, Change)>,
    object_bytes: BTreeMap<(String, String), Vec<u8>>,
    active_target: String,
    persist_change: Option<Change>,
    verify_change: Option<Change>,
    open_change: Option<Change>,
    persist_error: Option<ImageObjectError>,
    readback_error: Option<ImageObjectError>,
    verify_error: Option<ImageObjectError>,
    open_error: Option<ImageObjectError>,
    open_length: Option<u64>,
    persist_pause: Option<Pause>,
    stream_leases: usize,
}
impl FakeState {
    fn new() -> Self {
        let work = Work::new(NewWork {
            id: WorkId::parse(WORK).unwrap(),
            kind: WorkKind::Story,
            owner_id: OwnerId::parse(OWNER).unwrap(),
            at: NOW,
            details: WorkDetails {
                title: "Mùa thu".into(),
                synopsis: String::new(),
                language: "vi".into(),
                visibility: Visibility::Private,
                rights_refs: vec!["rights-a".into()],
            },
        })
        .unwrap();
        Self {
            history: BTreeMap::from([(1, work.clone())]),
            work,
            intents: BTreeMap::new(),
            assets: BTreeMap::new(),
            changes: Vec::new(),
            operations: BTreeMap::new(),
            rights: BTreeMap::from([("rights-a".into(), OWNER.into())]),
            now: NOW,
            session_active: true,
            session_expires: i64::MAX,
            tx_active: false,
            events: Vec::new(),
            context_calls: 0,
            context_change: None,
            object_bytes: BTreeMap::new(),
            active_target: "first-private".into(),
            persist_change: None,
            verify_change: None,
            open_change: None,
            persist_error: None,
            readback_error: None,
            verify_error: None,
            open_error: None,
            open_length: None,
            persist_pause: None,
            stream_leases: 0,
        }
    }
    fn authorize(&self, scope: &OwnerWorkScope) -> Result<(), ArtworkApplicationError> {
        if scope.token == "missing" || !self.session_active || self.now >= self.session_expires {
            return Err(StoreError::Unauthenticated.into());
        }
        if scope.token != "owner" || scope.work_id != *self.work.id() {
            return Err(StoreError::NotFound.into());
        }
        Ok(())
    }
    fn draft(&self) -> Result<(), ArtworkApplicationError> {
        if self.work.lifecycle() != Lifecycle::Draft {
            return Err(ArtworkError::Catalog(CatalogError::WorkArchived).into());
        }
        Ok(())
    }
    fn rights(&self, refs: &[String]) -> Result<(), ArtworkApplicationError> {
        if refs.iter().any(|reference| {
            self.rights.get(reference).map(String::as_str) != Some(self.work.owner_id().as_str())
        }) {
            return Err(StoreError::EvidenceUnavailable.into());
        }
        Ok(())
    }
    fn change(&mut self, change: Change) {
        match change {
            Change::Revoke => self.session_active = false,
            Change::ExpireSession => self.session_expires = self.now,
            Change::ExpireIntent => {
                self.now = self.intents.values().next().unwrap().intent.expires_at_ms()
            }
            Change::Archive => {
                self.work = self.work.archive(self.work.revision(), self.now).unwrap();
                self.history.insert(self.work.revision(), self.work.clone());
            }
            Change::RemoveRights => {
                self.rights.clear();
            }
            Change::CatalogEdit => {
                self.work = self.work.touch(self.work.revision(), self.now).unwrap();
                self.history.insert(self.work.revision(), self.work.clone());
            }
            Change::EqualAdmissionOnOriginalTarget => {
                unreachable!("handled with inspected storage witness")
            }
        }
    }
    fn stored_intent(
        &self,
        id: &ImageIntentId,
    ) -> Result<StoredImageIntent, ArtworkApplicationError> {
        self.intents
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| StoreError::NotFound.into())
    }
    fn stored_asset(&self, id: &ImageAssetId) -> Result<StoredImageAsset, ArtworkApplicationError> {
        let value = self
            .assets
            .get(id.as_str())
            .cloned()
            .ok_or(StoreError::NotFound)?;
        if value.asset.work_id() != self.work.id() || value.asset.owner_id() != self.work.owner_id()
        {
            return Err(StoreError::NotFound.into());
        }
        if value.asset.admitted_at_ms() > self.now {
            return Err(StoreError::CorruptRevision.into());
        }
        Ok(value)
    }
    fn selected(
        &self,
        request: &SetWorkArtwork,
    ) -> Result<Option<StoredImageAsset>, ArtworkApplicationError> {
        match request.change() {
            ArtworkChangeValue::Clear => Ok(None),
            ArtworkChangeValue::Set(selection) => {
                let stored = self.stored_asset(selection.image_asset_id())?;
                selection.require_asset(&self.work, &stored.asset)?;
                Ok(Some(stored))
            }
        }
    }
    fn prior_set(
        &self,
        request: &SetWorkArtwork,
    ) -> Result<Option<ArtworkReceipt<dto::WorkArtworkResponse>>, ArtworkApplicationError> {
        match self.operations.get(request.operation_id().as_str()) {
            None => Ok(None),
            Some(Operation::Set {
                work,
                fingerprint,
                receipt,
            }) if work == self.work.id()
                && fingerprint == &request.fingerprint(self.work.id())? =>
            {
                Ok(Some(receipt.clone()))
            }
            Some(_) => Err(ImageError::OperationReused.into()),
        }
    }
}

#[derive(Clone)]
struct FakeMetadataRepository {
    state: Arc<Mutex<FakeState>>,
}
impl FakeMetadataRepository {
    fn transaction<T>(
        &self,
        name: &str,
        scope: &OwnerWorkScope,
        execute: impl FnOnce(&mut FakeState) -> Result<T, ArtworkApplicationError>,
    ) -> Result<T, ArtworkApplicationError> {
        let mut state = self.state.lock().unwrap();
        assert!(!state.tx_active, "a metadata transaction escaped its port");
        state.tx_active = true;
        state.events.push(format!("{name}:begin"));
        let old = state.clone();
        let result = state
            .authorize(scope)
            .and_then(|()| {
                state.events.push(format!("{name}:owner"));
                execute(&mut state)
            })
            .and_then(|value| {
                state.authorize(scope)?;
                state.events.push(format!("{name}:reauth"));
                Ok(value)
            });
        let events = state.events.clone();
        if result.is_err() {
            *state = old;
            state.events = events;
        }
        state.events.push(format!(
            "{name}:{}",
            if result.is_ok() { "commit" } else { "rollback" }
        ));
        state.tx_active = false;
        result
    }
}
impl ArtworkRepository for FakeMetadataRepository {
    fn prepare(
        &self,
        scope: OwnerWorkScope,
        request: PrepareImage,
    ) -> MetadataFuture<ArtworkReceipt<dto::ImageIntentDocument>> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("prepare", &scope, |state| {
                let fingerprint = request.fingerprint(state.work.id())?;
                let prior = state.operations.get(request.operation_id().as_str());
                let prior_fingerprint = prior.map(|operation| match operation {
                    Operation::Prepare { fingerprint, .. } | Operation::Set { fingerprint, .. } => {
                        fingerprint.as_str()
                    }
                });
                let existing = state.intents.get(request.id().as_str());
                match decide_image_prepare(
                    &request,
                    state.work.id(),
                    prior_fingerprint,
                    existing.map(|stored| &stored.intent),
                )? {
                    ImagePrepareDecision::Replay => match prior {
                        Some(Operation::Prepare { work, receipt, .. })
                            if work == state.work.id() =>
                        {
                            Ok(receipt.clone())
                        }
                        _ => Err(ImageError::OperationReused.into()),
                    },
                    ImagePrepareDecision::Create => {
                        state.draft()?;
                        state.rights(request.declarations().rights_refs())?;
                        let operation = request.operation_id().as_str().to_owned();
                        let intent = ImageIntent::create(
                            request,
                            state.work.id(),
                            state.work.owner_id(),
                            state.now,
                        )?;
                        let accepted = receipt(intent.document());
                        state.intents.insert(
                            intent.id().as_str().into(),
                            StoredImageIntent {
                                intent,
                                document_bytes: accepted.json_bytes.clone(),
                            },
                        );
                        state.operations.insert(
                            operation,
                            Operation::Prepare {
                                work: state.work.id().clone(),
                                fingerprint,
                                receipt: accepted.clone(),
                            },
                        );
                        Ok(accepted)
                    }
                }
            })
        })
    }
    fn intent(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
    ) -> MetadataFuture<dto::ImageIntentResponse> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("intent", &scope, |state| {
                let intent = state.stored_intent(&id)?.intent;
                let asset = state.assets.get(id.as_str()).map(|stored| &stored.asset);
                Ok(dto::ImageIntentResponse {
                    state: intent.state(asset, state.now)?,
                    intent: intent.document().clone(),
                    asset: asset.map(|asset| asset.document().clone()),
                })
            })
        })
    }
    fn admission_context(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
    ) -> MetadataFuture<AdmissionContext> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("context", &scope, |state| {
                state.context_calls += 1;
                if state
                    .context_change
                    .as_ref()
                    .is_some_and(|(call, _)| *call == state.context_calls)
                {
                    let (_, change) = state.context_change.take().unwrap();
                    state.change(change);
                    state.authorize(&scope)?;
                }
                let intent = state.stored_intent(&id)?;
                let asset = state.assets.get(id.as_str()).cloned();
                if asset.is_none() {
                    state.draft()?;
                    intent.intent.require_pending(state.now)?;
                    state.rights(intent.intent.request().declarations().rights_refs())?;
                } else {
                    intent
                        .intent
                        .state(asset.as_ref().map(|stored| &stored.asset), state.now)?;
                }
                Ok(AdmissionContext {
                    work: state.work.clone(),
                    intent,
                    asset,
                    now: state.now,
                })
            })
        })
    }
    fn finalize(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
        metadata: ImageMetadata,
        location: RecordedImageLocation,
    ) -> MetadataFuture<ArtworkReceipt<dto::ImageAssetDocument>> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("finalize", &scope, |state| {
                let intent = state.stored_intent(&id)?.intent;
                let existing = state.assets.get(id.as_str());
                match decide_image_admission(
                    &intent,
                    &metadata,
                    existing.map(|stored| &stored.asset),
                    state.now,
                )? {
                    ImageAdmissionDecision::Replay => {
                        let stored = existing.unwrap();
                        Ok(ArtworkReceipt {
                            value: stored.asset.document().clone(),
                            json_bytes: stored.document_bytes.clone(),
                        })
                    }
                    ImageAdmissionDecision::Create => {
                        state.draft()?;
                        state.rights(intent.request().declarations().rights_refs())?;
                        let asset = ImageAsset::create(&intent, metadata, state.now)?;
                        let accepted = receipt(asset.document());
                        state.assets.insert(
                            id.as_str().into(),
                            StoredImageAsset {
                                asset,
                                storage_target: location.storage_target,
                                object_key: location.object_key,
                                document_bytes: accepted.json_bytes.clone(),
                            },
                        );
                        Ok(accepted)
                    }
                }
            })
        })
    }
    fn asset(&self, scope: OwnerWorkScope, id: ImageAssetId) -> MetadataFuture<StoredImageAsset> {
        let repo = self.clone();
        Box::pin(async move { repo.transaction("asset", &scope, |state| state.stored_asset(&id)) })
    }
    fn artwork(
        &self,
        scope: OwnerWorkScope,
        revision: Option<Revision>,
    ) -> MetadataFuture<dto::WorkArtworkResponse> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("artwork", &scope, |state| {
                let revision = revision.map_or(state.work.revision(), Revision::get);
                let work = state.history.get(&revision).ok_or(StoreError::NotFound)?;
                Ok(artwork_at(work, &state.changes)?)
            })
        })
    }
    fn artwork_context(
        &self,
        scope: OwnerWorkScope,
        request: SetWorkArtwork,
    ) -> MetadataFuture<ArtworkContext> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("artwork-context", &scope, |state| {
                let replay = state.prior_set(&request)?;
                let asset = if replay.is_some() {
                    None
                } else {
                    let asset = state.selected(&request)?;
                    decide_artwork_change(
                        &state.work,
                        &request,
                        asset.as_ref().map(|stored| &stored.asset),
                        None,
                        state.now.max(state.work.updated_at()),
                    )?;
                    if let Some(stored) = &asset {
                        state.rights(stored.asset.declarations().rights_refs())?;
                    }
                    asset
                };
                Ok(ArtworkContext {
                    work: state.work.clone(),
                    asset,
                    replay,
                })
            })
        })
    }
    fn set_artwork(
        &self,
        scope: OwnerWorkScope,
        request: SetWorkArtwork,
        observed: Option<StoredImageAsset>,
    ) -> MetadataFuture<ArtworkReceipt<dto::WorkArtworkResponse>> {
        let repo = self.clone();
        Box::pin(async move {
            repo.transaction("set", &scope, |state| {
                if let Some(replay) = state.prior_set(&request)? {
                    return Ok(replay);
                }
                let asset = state.selected(&request)?;
                match (&asset, &observed) {
                    (None, None) => {}
                    (Some(current), Some(observed)) if same_stored_asset(current, observed) => {}
                    _ => return Err(StoreError::CorruptRevision.into()),
                }
                if let Some(stored) = &asset {
                    state.rights(stored.asset.declarations().rights_refs())?;
                }
                match decide_artwork_change(
                    &state.work,
                    &request,
                    asset.as_ref().map(|stored| &stored.asset),
                    None,
                    state.now.max(state.work.updated_at()),
                )? {
                    ArtworkDecision::Replay => Err(StoreError::CorruptRevision.into()),
                    ArtworkDecision::Apply(mutation) => {
                        let accepted = receipt(mutation.response());
                        state.work = mutation.work().clone();
                        state
                            .history
                            .insert(state.work.revision(), state.work.clone());
                        state.changes.push(mutation.change().clone());
                        state.operations.insert(
                            request.operation_id().as_str().into(),
                            Operation::Set {
                                work: state.work.id().clone(),
                                fingerprint: request.fingerprint(state.work.id())?,
                                receipt: accepted.clone(),
                            },
                        );
                        Ok(accepted)
                    }
                }
            })
        })
    }
}

#[derive(Clone)]
struct FakePrivateStorage {
    state: Arc<Mutex<FakeState>>,
}
fn stored_bytes<'a>(
    state: &'a FakeState,
    asset: &StoredImageAsset,
) -> Result<&'a [u8], ImageObjectError> {
    let bytes = state
        .object_bytes
        .get(&(asset.storage_target.clone(), asset.object_key.clone()))
        .ok_or(ImageObjectError::NotFound)?;
    if bytes.len() as u64 != asset.asset.metadata().byte_length()
        || format!("{:x}", Sha256::digest(bytes)) != asset.asset.metadata().sha256()
    {
        return Err(ImageObjectError::Corrupt);
    }
    Ok(bytes)
}
impl ImageObjects for FakePrivateStorage {
    fn persist(&self, image: VerifiedImage) -> ImageObjectFuture<RecordedImageLocation> {
        let storage = self.clone();
        Box::pin(async move {
            let pause = {
                let state = storage.state.lock().unwrap();
                assert!(!state.tx_active, "persist inside metadata transaction");
                state.persist_pause.clone()
            };
            if let Some(pause) = pause {
                pause.entered.notify_one();
                pause.release.notified().await;
            }
            let mut state = storage.state.lock().unwrap();
            assert!(!state.tx_active, "persist inside metadata transaction");
            // Receiving VerifiedImage proves actual complete decoder execution preceded this effect.
            state.events.push("objects:sealed-witness".into());
            state.events.push("objects:persist".into());
            if let Some(error) = state.persist_error {
                return Err(error);
            }
            let key = image.metadata().sha256().to_owned();
            let target = state.active_target.clone();
            state
                .object_bytes
                .entry((target.clone(), key.clone()))
                .or_insert_with(|| image.bytes().to_vec());
            assert_eq!(format!("{:x}", Sha256::digest(image.bytes())), key);
            state.events.push("objects:readback".into());
            if let Some(error) = state.readback_error {
                return Err(error);
            }
            if let Some(change) = state.persist_change.take() {
                if matches!(change, Change::EqualAdmissionOnOriginalTarget) {
                    // Deterministic named race: another equal admission committed first on the
                    // retained original target while this writer used a switched active target.
                    let intent = state.intents.values().next().unwrap().intent.clone();
                    let asset =
                        ImageAsset::create(&intent, image.metadata().clone(), state.now).unwrap();
                    let bytes = serde_json::to_vec(asset.document()).unwrap();
                    state.object_bytes.insert(
                        ("first-private".into(), key.clone()),
                        image.bytes().to_vec(),
                    );
                    state.assets.insert(
                        asset.id().as_str().into(),
                        StoredImageAsset {
                            asset,
                            storage_target: "first-private".into(),
                            object_key: key.clone(),
                            document_bytes: bytes,
                        },
                    );
                    state.events.push("rival:accepted-first-private".into());
                } else {
                    state.change(change);
                }
            }
            Ok(RecordedImageLocation {
                storage_target: target,
                object_key: key,
            })
        })
    }
    fn verify(&self, asset: StoredImageAsset) -> ImageObjectFuture<()> {
        let storage = self.clone();
        Box::pin(async move {
            let mut state = storage.state.lock().unwrap();
            assert!(!state.tx_active, "verify inside metadata transaction");
            state
                .events
                .push(format!("objects:verify:{}", asset.storage_target));
            if let Some(error) = state.verify_error {
                return Err(error);
            }
            stored_bytes(&state, &asset)?;
            if let Some(change) = state.verify_change.take() {
                state.change(change);
            }
            Ok(())
        })
    }
    fn open(&self, asset: StoredImageAsset) -> ImageObjectFuture<OpenImage> {
        let storage = self.clone();
        Box::pin(async move {
            let mut state = storage.state.lock().unwrap();
            assert!(!state.tx_active, "open inside metadata transaction");
            state
                .events
                .push(format!("objects:open:{}", asset.storage_target));
            if let Some(error) = state.open_error {
                return Err(error);
            }
            let bytes = stored_bytes(&state, &asset)?.to_vec();
            let length = state.open_length.unwrap_or(bytes.len() as u64);
            if let Some(change) = state.open_change.take() {
                state.change(change);
            }
            state.stream_leases += 1;
            let lease = StreamLease(storage.state.clone());
            let stream = stream::once(async move {
                let _lease = lease;
                Ok(Bytes::from(bytes))
            });
            Ok(OpenImage {
                stream: Box::pin(stream),
                byte_length: length,
            })
        })
    }
}
struct StreamLease(Arc<Mutex<FakeState>>);
impl Drop for StreamLease {
    fn drop(&mut self) {
        self.0.lock().unwrap().stream_leases -= 1;
    }
}
struct Harness {
    state: Arc<Mutex<FakeState>>,
    service: ArtworkService,
}
impl Harness {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(FakeState::new()));
        let service = ArtworkService {
            repository: Arc::new(FakeMetadataRepository {
                state: state.clone(),
            }),
            objects: Arc::new(FakePrivateStorage {
                state: state.clone(),
            }),
        };
        Self { state, service }
    }
    fn update(&self, execute: impl FnOnce(&mut FakeState)) {
        execute(&mut self.state.lock().unwrap());
    }
    fn events(&self) -> Vec<String> {
        self.state.lock().unwrap().events.clone()
    }
    fn counts(&self) -> (usize, usize, usize, usize, u64) {
        let state = self.state.lock().unwrap();
        (
            state.intents.len(),
            state.assets.len(),
            state.changes.len(),
            state.operations.len(),
            state.work.revision(),
        )
    }
    async fn prepared(&self, number: u32, jpeg: bool) -> dto::CreateImageIntentRequest {
        let raw = request(number, jpeg);
        self.service
            .prepare("owner", WORK, raw.clone())
            .await
            .unwrap();
        raw
    }
    async fn admitted(&self, number: u32, jpeg: bool) -> ArtworkReceipt<dto::ImageAssetDocument> {
        let raw = self.prepared(number, jpeg).await;
        self.service
            .begin_upload("owner", WORK, &raw.id, raw.content_type)
            .await
            .unwrap()
            .admit(if jpeg { JPEG } else { PNG }.to_vec())
            .await
            .unwrap()
    }
}
fn assert_image_error<T>(result: Result<T, ArtworkApplicationError>, expected: ImageError) {
    assert!(matches!(result, Err(ArtworkApplicationError::Image(actual)) if actual == expected));
}
fn assert_storage_error<T>(
    result: Result<T, ArtworkApplicationError>,
    expected: impl FnOnce(StoreError) -> bool,
) {
    match result {
        Err(ArtworkApplicationError::Storage(error)) => assert!(expected(error)),
        _ => panic!("expected exact storage error"),
    }
}
fn assert_object_error<T>(result: Result<T, ArtworkApplicationError>, expected: ImageObjectError) {
    assert!(matches!(result, Err(ArtworkApplicationError::Object(actual)) if actual == expected));
}

#[tokio::test]
async fn complete_private_prepare_admit_select_replace_clear_and_history_keep_exact_receipts() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let empty = harness.service.artwork("owner", WORK, None).await.unwrap();
    assert_eq!(
        empty,
        dto::WorkArtworkResponse {
            work_id: WORK.into(),
            work_revision: 1,
            artwork: None,
            changed_at_ms: None
        }
    );
    let prepare = request(100, false);
    let first = harness
        .service
        .prepare("owner", WORK, prepare.clone())
        .await
        .unwrap();
    let original_prepare = first.json_bytes().to_vec();
    assert_eq!(first.value().work_id, WORK);
    assert_eq!(first.value().owner_id, OWNER);
    assert_eq!(first.value().created_at_ms, NOW as u64);
    assert_eq!(first.value().expires_at_ms, 1_800_100);
    assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
    assert_eq!(
        harness
            .service
            .intent("owner", WORK, &prepare.id)
            .await
            .unwrap()
            .state,
        dto::ImageIntentState::Pending
    );
    let png = harness
        .service
        .begin_upload("owner", WORK, &prepare.id, dto::ImageMime::Png)
        .await
        .unwrap()
        .admit(PNG.to_vec())
        .await
        .unwrap();
    let original_asset = png.json_bytes().to_vec();
    assert_eq!(png.value().verified_media.sha256, PNG_HASH);
    assert_eq!(
        (
            png.value().verified_media.width_px,
            png.value().verified_media.height_px
        ),
        (2, 3)
    );
    assert_eq!(harness.counts(), (1, 1, 0, 1, 1));
    let status = harness
        .service
        .intent("owner", WORK, &prepare.id)
        .await
        .unwrap();
    assert_eq!(status.state, dto::ImageIntentState::Admitted);
    assert_eq!(status.asset.as_ref(), Some(png.value()));
    let mut preview = harness
        .service
        .preview("owner", WORK, &prepare.id)
        .await
        .unwrap();
    assert_eq!(preview.metadata.byte_length(), PNG.len() as u64);
    assert_eq!(
        preview.stream.next().await.unwrap().unwrap(),
        Bytes::from_static(PNG)
    );
    assert!(preview.stream.next().await.is_none());
    let selected_raw = set(2100, 1, png.value(), "Bìa mùa thu");
    let selected = harness
        .service
        .set_artwork("owner", WORK, selected_raw.clone())
        .await
        .unwrap();
    let original_selection = selected.json_bytes().to_vec();
    assert_eq!(selected.value().work_revision, 2);
    assert_eq!(
        selected.value().artwork.as_ref().unwrap().alt_text,
        "Bìa mùa thu"
    );
    let jpeg = harness.admitted(101, true).await;
    let replaced = harness
        .service
        .set_artwork("owner", WORK, set(2101, 2, jpeg.value(), "Bìa thay thế"))
        .await
        .unwrap();
    assert_eq!(replaced.value().work_revision, 3);
    let cleared = harness
        .service
        .set_artwork("owner", WORK, clear(2102, 3))
        .await
        .unwrap();
    // This independently written response checks the canonical JSON body including explicit null.
    assert_eq!(cleared.json_bytes(), b"{\"work_id\":\"00000000-0000-0000-0000-000000000001\",\"work_revision\":4,\"artwork\":null,\"changed_at_ms\":100}");
    assert_eq!(
        harness.service.artwork("owner", WORK, None).await.unwrap(),
        *cleared.value()
    );
    assert_eq!(
        harness
            .service
            .artwork("owner", WORK, Some(1))
            .await
            .unwrap(),
        empty
    );
    assert_eq!(
        harness
            .service
            .artwork("owner", WORK, Some(2))
            .await
            .unwrap(),
        *selected.value()
    );
    assert_eq!(
        harness
            .service
            .artwork("owner", WORK, Some(3))
            .await
            .unwrap(),
        *replaced.value()
    );
    assert_eq!(
        harness
            .service
            .artwork("owner", WORK, Some(4))
            .await
            .unwrap(),
        *cleared.value()
    );
    assert_storage_error(
        harness.service.artwork("owner", WORK, Some(5)).await,
        |error| matches!(error, StoreError::NotFound),
    );
    assert_eq!(
        harness
            .service
            .prepare("owner", WORK, prepare.clone())
            .await
            .unwrap()
            .json_bytes(),
        original_prepare
    );
    assert_eq!(
        harness
            .service
            .asset("owner", WORK, &prepare.id)
            .await
            .unwrap()
            .json_bytes(),
        original_asset
    );
    assert_eq!(
        harness
            .service
            .set_artwork("owner", WORK, selected_raw)
            .await
            .unwrap()
            .json_bytes(),
        original_selection
    );
    assert_eq!(harness.counts(), (2, 2, 3, 5, 4));
    for bytes in [&original_prepare, &original_asset, &original_selection] {
        let text = std::str::from_utf8(bytes).unwrap();
        assert!(!text.contains("first-private"));
        assert!(!text.contains("object_key"));
        assert!(!text.contains("storage_target"));
    }
}

#[tokio::test]
async fn upload_effect_trace_ends_transactions_before_sealed_storage_then_reauthenticates() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let raw = harness.prepared(100, false).await;
    harness.update(|state| state.events.clear());
    harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap()
        .admit(PNG.to_vec())
        .await
        .unwrap();
    assert_eq!(
        harness.events(),
        [
            "context:begin",
            "context:owner",
            "context:reauth",
            "context:commit",
            "context:begin",
            "context:owner",
            "context:reauth",
            "context:commit",
            "context:begin",
            "context:owner",
            "context:reauth",
            "context:commit",
            "objects:sealed-witness",
            "objects:persist",
            "objects:readback",
            "finalize:begin",
            "finalize:owner",
            "finalize:reauth",
            "finalize:commit",
            "asset:begin",
            "asset:owner",
            "asset:reauth",
            "asset:commit",
            "objects:verify:first-private",
            "asset:begin",
            "asset:owner",
            "asset:reauth",
            "asset:commit",
        ]
    );
}

#[tokio::test]
async fn owner_only_paths_fail_closed_for_foreign_owner_members_wrong_work_and_missing_session() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    let before = harness.counts();
    harness.update(|state| state.events.clear());
    for (token, work, unauthenticated) in [
        ("other-owner", WORK, false),
        ("editor", WORK, false),
        ("reader", WORK, false),
        ("owner", OTHER_WORK, false),
        ("missing", WORK, true),
    ] {
        let check = |error| {
            if unauthenticated {
                matches!(error, StoreError::Unauthenticated)
            } else {
                matches!(error, StoreError::NotFound)
            }
        };
        assert_storage_error(
            harness
                .service
                .prepare(token, work, request(101, false))
                .await,
            check,
        );
        assert_storage_error(
            harness.service.intent(token, work, &asset.value().id).await,
            check,
        );
        assert_storage_error(
            harness
                .service
                .begin_upload(token, work, &asset.value().id, dto::ImageMime::Png)
                .await,
            check,
        );
        assert_storage_error(
            harness.service.asset(token, work, &asset.value().id).await,
            check,
        );
        assert_storage_error(
            harness
                .service
                .preview(token, work, &asset.value().id)
                .await,
            check,
        );
        assert_storage_error(harness.service.artwork(token, work, None).await, check);
        assert_storage_error(
            harness
                .service
                .set_artwork(token, work, set(2100, 1, asset.value(), "Cover"))
                .await,
            check,
        );
    }
    assert_eq!(harness.counts(), before);
    assert!(!harness
        .events()
        .iter()
        .any(|event| event.starts_with("objects:")));
}

#[tokio::test]
async fn missing_or_foreign_rights_reject_prepare_and_pending_upload_without_object_effects() {
    let _lanes = TEST_LANES.lock().await;
    for foreign in [false, true] {
        let harness = Harness::new();
        harness.update(|state| {
            if foreign {
                state.rights.insert("rights-a".into(), OTHER_OWNER.into());
            } else {
                state.rights.clear();
            }
        });
        assert_storage_error(
            harness
                .service
                .prepare("owner", WORK, request(100, false))
                .await,
            |error| matches!(error, StoreError::EvidenceUnavailable),
        );
        assert_eq!(harness.counts(), (0, 0, 0, 0, 1));
        harness.update(|state| {
            state.rights.insert("rights-a".into(), OWNER.into());
        });
        let raw = harness.prepared(100, false).await;
        harness.update(|state| {
            state.rights.clear();
            state.events.clear();
        });
        assert_storage_error(
            harness
                .service
                .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
                .await,
            |error| matches!(error, StoreError::EvidenceUnavailable),
        );
        assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
        assert!(!harness
            .events()
            .iter()
            .any(|event| event.starts_with("objects:")));
    }
}

#[tokio::test]
async fn declared_hash_length_mime_and_actual_format_mismatch_never_persist() {
    let _lanes = TEST_LANES.lock().await;
    for mismatch in [
        ImageError::ExpectedHashMismatch,
        ImageError::ExpectedLengthMismatch,
        ImageError::ExpectedMimeMismatch,
    ] {
        let harness = Harness::new();
        let mut raw = request(100, false);
        match mismatch {
            ImageError::ExpectedHashMismatch => raw.expected_sha256 = "0".repeat(64),
            ImageError::ExpectedLengthMismatch => raw.expected_byte_length += 1,
            ImageError::ExpectedMimeMismatch => raw.content_type = dto::ImageMime::Jpeg,
            _ => unreachable!(),
        }
        harness
            .service
            .prepare("owner", WORK, raw.clone())
            .await
            .unwrap();
        if mismatch == ImageError::ExpectedMimeMismatch {
            assert_image_error(
                harness
                    .service
                    .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
                    .await,
                mismatch,
            );
        } else {
            let ticket = harness
                .service
                .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
                .await
                .unwrap();
            assert_image_error(ticket.admit(PNG.to_vec()).await, mismatch);
        }
        assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
        assert!(harness.state.lock().unwrap().object_bytes.is_empty());
    }
    for (bytes, hash, error) in [
        (JPEG, JPEG_HASH, ImageIntakeError::FormatMismatch),
        (
            &PNG[..33],
            "b63d79a8d91b094bc10d48eecbdc4cfc29898570a3736da320f4a92c437330ca",
            ImageIntakeError::MalformedImage,
        ),
    ] {
        let harness = Harness::new();
        let mut raw = request(100, false);
        // These encoded facts match exactly, so this reaches the actual decoder. The prefix
        // hash is independently computed by Python hashlib, as are the full fixture hashes.
        raw.expected_sha256 = hash.into();
        raw.expected_byte_length = bytes.len() as u64;
        harness
            .service
            .prepare("owner", WORK, raw.clone())
            .await
            .unwrap();
        let ticket = harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await
            .unwrap();
        assert!(
            matches!(ticket.admit(bytes.to_vec()).await, Err(ArtworkApplicationError::Intake(actual)) if actual == error)
        );
        assert!(harness.state.lock().unwrap().object_bytes.is_empty());
    }
}

#[tokio::test]
async fn archive_expiry_and_revocation_after_lane_acquisition_are_checked_before_body_buffering() {
    let _lanes = TEST_LANES.lock().await;
    for change in [
        Change::Archive,
        Change::ExpireIntent,
        Change::Revoke,
        Change::ExpireSession,
    ] {
        let harness = Harness::new();
        let raw = harness.prepared(100, false).await;
        harness.update(|state| state.context_change = Some((2, change)));
        let result = harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await;
        match change {
            Change::Archive => assert!(matches!(
                result,
                Err(ArtworkApplicationError::Artwork(ArtworkError::Catalog(
                    CatalogError::WorkArchived
                )))
            )),
            Change::ExpireIntent => assert_image_error(result, ImageError::IntentExpired),
            Change::Revoke | Change::ExpireSession => {
                assert_storage_error(result, |error| matches!(error, StoreError::Unauthenticated))
            }
            _ => unreachable!(),
        }
        assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
        assert!(harness.state.lock().unwrap().object_bytes.is_empty());
        // Failed second authorization releases the owned reservation.
        let one = ImageAdmission::try_acquire().unwrap();
        let two = ImageAdmission::try_acquire().unwrap();
        drop((one, two));
    }
}

#[tokio::test]
async fn changes_after_private_persist_leave_only_an_orphan_and_no_asset_receipt() {
    let _lanes = TEST_LANES.lock().await;
    for change in [
        Change::Revoke,
        Change::ExpireSession,
        Change::ExpireIntent,
        Change::Archive,
        Change::RemoveRights,
    ] {
        let harness = Harness::new();
        let raw = harness.prepared(100, false).await;
        let ticket = harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await
            .unwrap();
        harness.update(|state| state.persist_change = Some(change));
        let result = ticket.admit(PNG.to_vec()).await;
        match change {
            Change::Revoke | Change::ExpireSession => {
                assert_storage_error(result, |error| matches!(error, StoreError::Unauthenticated))
            }
            Change::ExpireIntent => assert_image_error(result, ImageError::IntentExpired),
            Change::Archive => assert!(matches!(
                result,
                Err(ArtworkApplicationError::Artwork(ArtworkError::Catalog(
                    CatalogError::WorkArchived
                )))
            )),
            Change::RemoveRights => assert_storage_error(result, |error| {
                matches!(error, StoreError::EvidenceUnavailable)
            }),
            _ => unreachable!(),
        }
        let state = harness.state.lock().unwrap();
        assert_eq!(state.assets.len(), 0);
        assert_eq!(state.changes.len(), 0);
        assert_eq!(state.operations.len(), 1);
        assert_eq!(state.object_bytes.len(), 1);
        assert_eq!(state.object_bytes.values().next().unwrap(), PNG);
        assert!(state.events.contains(&"finalize:rollback".into()));
    }
}

#[tokio::test]
async fn admitted_retries_preserve_bytes_and_original_time_after_expiry_archive_and_target_switch()
{
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let accepted = harness.admitted(100, false).await;
    let original = accepted.json_bytes().to_vec();
    harness.update(|state| {
        state.now = 1_800_100;
        state.change(Change::Archive);
        state.active_target = "second-private".into();
        state.events.clear();
    });
    let replay = harness
        .service
        .begin_upload("owner", WORK, &accepted.value().id, dto::ImageMime::Png)
        .await
        .unwrap()
        .admit(PNG.to_vec())
        .await
        .unwrap();
    assert_eq!(replay.json_bytes(), original);
    assert_eq!(replay.value().admitted_at_ms, NOW as u64);
    assert_eq!(
        harness
            .service
            .intent("owner", WORK, &accepted.value().id)
            .await
            .unwrap()
            .state,
        dto::ImageIntentState::Admitted
    );
    let mut preview = harness
        .service
        .preview("owner", WORK, &accepted.value().id)
        .await
        .unwrap();
    assert_eq!(
        preview.stream.next().await.unwrap().unwrap(),
        Bytes::from_static(PNG)
    );
    assert!(!harness.events().contains(&"objects:persist".into()));
    assert!(harness
        .events()
        .contains(&"objects:verify:first-private".into()));
    assert!(harness
        .events()
        .contains(&"objects:open:first-private".into()));
    assert!(harness
        .state
        .lock()
        .unwrap()
        .object_bytes
        .keys()
        .all(|(target, _)| target == "first-private"));
}

#[tokio::test]
async fn stale_selection_and_late_catalog_edit_never_overwrite_or_append_artwork() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    harness.update(|state| state.verify_change = Some(Change::CatalogEdit));
    let result = harness
        .service
        .set_artwork("owner", WORK, set(2100, 1, asset.value(), "Cover"))
        .await;
    assert!(matches!(
        result,
        Err(ArtworkApplicationError::Artwork(ArtworkError::Catalog(
            CatalogError::RevisionConflict {
                current_revision: 2,
                expected_revision: 1
            }
        )))
    ));
    assert_eq!(harness.counts(), (1, 1, 0, 1, 2));
    let empty = harness.service.artwork("owner", WORK, None).await.unwrap();
    assert_eq!(empty.artwork, None);
    harness.update(|state| state.events.clear());
    let result = harness
        .service
        .set_artwork("owner", WORK, set(2101, 1, asset.value(), "Different"))
        .await;
    assert!(matches!(
        result,
        Err(ArtworkApplicationError::Artwork(ArtworkError::Catalog(
            CatalogError::RevisionConflict {
                current_revision: 2,
                expected_revision: 1
            }
        )))
    ));
    assert_eq!(harness.counts(), (1, 1, 0, 1, 2));
    assert!(!harness
        .events()
        .iter()
        .any(|event| event.starts_with("objects:")));
}

#[tokio::test]
async fn selection_rechecks_owner_lifecycle_and_rights_after_object_verification() {
    let _lanes = TEST_LANES.lock().await;
    for change in [
        Change::Revoke,
        Change::ExpireSession,
        Change::Archive,
        Change::RemoveRights,
    ] {
        let harness = Harness::new();
        let asset = harness.admitted(100, false).await;
        harness.update(|state| state.verify_change = Some(change));
        let result = harness
            .service
            .set_artwork("owner", WORK, set(2100, 1, asset.value(), "Cover"))
            .await;
        match change {
            Change::Revoke | Change::ExpireSession => {
                assert_storage_error(result, |error| matches!(error, StoreError::Unauthenticated))
            }
            Change::Archive => assert!(matches!(
                result,
                Err(ArtworkApplicationError::Artwork(ArtworkError::Catalog(
                    CatalogError::WorkArchived
                )))
            )),
            Change::RemoveRights => assert_storage_error(result, |error| {
                matches!(error, StoreError::EvidenceUnavailable)
            }),
            _ => unreachable!(),
        }
        let state = harness.state.lock().unwrap();
        assert!(state.changes.is_empty());
        assert_eq!(state.operations.len(), 1);
    }
}

#[tokio::test]
async fn verification_corruption_missing_or_unavailable_never_selects_and_replay_verifies_objects()
{
    let _lanes = TEST_LANES.lock().await;
    for failure in [
        ImageObjectError::Corrupt,
        ImageObjectError::NotFound,
        ImageObjectError::Unavailable,
    ] {
        let harness = Harness::new();
        let asset = harness.admitted(100, false).await;
        harness.update(|state| state.verify_error = Some(failure));
        assert_object_error(
            harness
                .service
                .set_artwork("owner", WORK, set(2100, 1, asset.value(), "Cover"))
                .await,
            failure,
        );
        assert_object_error(
            harness
                .service
                .begin_upload("owner", WORK, &asset.value().id, dto::ImageMime::Png)
                .await
                .unwrap()
                .admit(PNG.to_vec())
                .await,
            failure,
        );
        assert_eq!(harness.counts(), (1, 1, 0, 1, 1));
    }
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    harness.update(|state| state.object_bytes.values_mut().next().unwrap()[0] ^= 1);
    assert_object_error(
        harness
            .service
            .preview("owner", WORK, &asset.value().id)
            .await,
        ImageObjectError::Corrupt,
    );
    assert_object_error(
        harness
            .service
            .set_artwork("owner", WORK, set(2100, 1, asset.value(), "Cover"))
            .await,
        ImageObjectError::Corrupt,
    );
    assert_eq!(harness.counts(), (1, 1, 0, 1, 1));
}

#[tokio::test]
async fn accepted_equal_concurrent_admission_verifies_authoritative_original_target() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let raw = harness.prepared(100, false).await;
    harness.update(|state| {
        state.active_target = "second-private".into();
        state.persist_change = Some(Change::EqualAdmissionOnOriginalTarget);
        state.events.clear();
    });
    let accepted = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap()
        .admit(PNG.to_vec())
        .await
        .unwrap();
    assert_eq!(accepted.value().id, raw.id);
    assert_eq!(harness.counts(), (1, 1, 0, 1, 1));
    let state = harness.state.lock().unwrap();
    assert_eq!(
        state.assets.values().next().unwrap().storage_target,
        "first-private"
    );
    assert!(state
        .events
        .contains(&"rival:accepted-first-private".into()));
    assert!(state
        .events
        .contains(&"objects:verify:first-private".into()));
    assert!(!state
        .events
        .contains(&"objects:verify:second-private".into()));
    assert_eq!(state.object_bytes.len(), 2);
}

#[tokio::test]
async fn prepare_and_artwork_operation_reuse_and_changed_payload_are_atomic_conflicts() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    let mut changed_prepare = request(100, false);
    changed_prepare.declarations.provenance = "Another source".into();
    assert_image_error(
        harness
            .service
            .prepare("owner", WORK, changed_prepare)
            .await,
        ImageError::OperationReused,
    );
    let mut duplicate = request(100, false);
    duplicate.operation_id = id(9999);
    assert_image_error(
        harness.service.prepare("owner", WORK, duplicate).await,
        ImageError::DuplicateIntent,
    );
    let mut cross_kind = clear(1100, 1);
    cross_kind.operation_id = request(100, false).operation_id;
    assert_image_error(
        harness.service.set_artwork("owner", WORK, cross_kind).await,
        ImageError::OperationReused,
    );
    let selected_raw = set(2100, 1, asset.value(), "Cover");
    let selected = harness
        .service
        .set_artwork("owner", WORK, selected_raw.clone())
        .await
        .unwrap();
    let before = harness.counts();
    let mut changed = selected_raw.clone();
    changed.artwork.as_mut().unwrap().alt_text = "Changed".into();
    assert_image_error(
        harness.service.set_artwork("owner", WORK, changed).await,
        ImageError::OperationReused,
    );
    assert_eq!(harness.counts(), before);
    harness.update(|state| {
        state.now += 1;
        state.change(Change::CatalogEdit);
        state.verify_error = Some(ImageObjectError::Unavailable);
        state.events.clear();
    });
    assert_eq!(
        harness
            .service
            .set_artwork("owner", WORK, selected_raw)
            .await
            .unwrap()
            .json_bytes(),
        selected.json_bytes()
    );
    assert!(!harness
        .events()
        .iter()
        .any(|event| event.starts_with("objects:")));
}

#[tokio::test]
async fn preview_reauthenticates_after_open_and_drops_stream_on_error_or_caller_drop() {
    let _lanes = TEST_LANES.lock().await;
    for change in [Change::Revoke, Change::ExpireSession] {
        let harness = Harness::new();
        let asset = harness.admitted(100, false).await;
        harness.update(|state| state.open_change = Some(change));
        assert_storage_error(
            harness
                .service
                .preview("owner", WORK, &asset.value().id)
                .await,
            |error| matches!(error, StoreError::Unauthenticated),
        );
        assert_eq!(harness.state.lock().unwrap().stream_leases, 0);
    }
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    harness.update(|state| state.open_length = Some(PNG.len() as u64 + 1));
    assert_object_error(
        harness
            .service
            .preview("owner", WORK, &asset.value().id)
            .await,
        ImageObjectError::Corrupt,
    );
    assert_eq!(harness.state.lock().unwrap().stream_leases, 0);
    harness.update(|state| state.open_length = None);
    let preview = harness
        .service
        .preview("owner", WORK, &asset.value().id)
        .await
        .unwrap();
    assert_eq!(harness.state.lock().unwrap().stream_leases, 1);
    drop(preview);
    assert_eq!(harness.state.lock().unwrap().stream_leases, 0);
}

#[tokio::test]
async fn busy_upload_drop_rejected_body_and_cancelled_storage_release_owned_body_lanes() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let raw = harness.prepared(100, false).await;
    let one = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap();
    let two = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap();
    assert!(matches!(
        harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await,
        Err(ArtworkApplicationError::Intake(
            ImageIntakeError::AdmissionBusy
        ))
    ));
    drop(one);
    drop(two);
    let ticket = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap();
    assert!(matches!(
        ticket.admit(Vec::new()).await,
        Err(ArtworkApplicationError::Image(ImageError::EmptyImage))
    ));
    let pause = Pause::new();
    harness.update(|state| state.persist_pause = Some(pause.clone()));
    let ticket = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap();
    let running = tokio::spawn(ticket.admit(PNG.to_vec()));
    pause.entered.notified().await;
    let second = ImageAdmission::try_acquire().unwrap();
    assert!(matches!(
        ImageAdmission::try_acquire(),
        Err(ImageIntakeError::AdmissionBusy)
    ));
    running.abort();
    assert!(matches!(running.await, Err(error) if error.is_cancelled()));
    drop(second);
    let one = ImageAdmission::try_acquire().unwrap();
    let two = ImageAdmission::try_acquire().unwrap();
    drop((one, two));
    assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
    assert!(harness.state.lock().unwrap().object_bytes.is_empty());
}

#[tokio::test]
async fn persist_failure_is_typed_and_does_not_fabricate_an_admitted_asset() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let raw = harness.prepared(100, false).await;
    harness.update(|state| state.persist_error = Some(ImageObjectError::Unavailable));
    assert_object_error(
        harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await
            .unwrap()
            .admit(PNG.to_vec())
            .await,
        ImageObjectError::Unavailable,
    );
    assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
    assert_eq!(
        harness
            .service
            .intent("owner", WORK, &raw.id)
            .await
            .unwrap()
            .state,
        dto::ImageIntentState::Pending
    );
    assert!(harness.state.lock().unwrap().object_bytes.is_empty());
}

#[tokio::test]
async fn lost_admission_response_reconciles_committed_receipt_and_retry_without_second_write() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let raw = harness.prepared(100, false).await;
    // The accepted row is durable before this post-commit availability failure. There is
    // deliberately no assumption that an error means a private write/metadata commit failed.
    harness.update(|state| state.verify_error = Some(ImageObjectError::Unavailable));
    assert_object_error(
        harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await
            .unwrap()
            .admit(PNG.to_vec())
            .await,
        ImageObjectError::Unavailable,
    );
    assert_eq!(harness.counts(), (1, 1, 0, 1, 1));
    let status = harness
        .service
        .intent("owner", WORK, &raw.id)
        .await
        .unwrap();
    assert_eq!(status.state, dto::ImageIntentState::Admitted);
    let original = harness.service.asset("owner", WORK, &raw.id).await.unwrap();
    harness.update(|state| {
        state.verify_error = None;
        state.active_target = "second-private".into();
        state.now = 1_800_100;
        state.events.clear();
    });
    let retried = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap()
        .admit(PNG.to_vec())
        .await
        .unwrap();
    assert_eq!(retried.json_bytes(), original.json_bytes());
    assert!(!harness.events().contains(&"objects:persist".into()));
    assert_eq!(harness.state.lock().unwrap().object_bytes.len(), 1);
}

#[tokio::test]
async fn absent_cross_work_cross_owner_or_wrong_digest_asset_is_not_a_selection_capability() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    let mut nonexistent = set(2100, 1, asset.value(), "Cover");
    nonexistent.artwork.as_mut().unwrap().image_asset_id = id(9999);
    assert_storage_error(
        harness
            .service
            .set_artwork("owner", WORK, nonexistent)
            .await,
        |error| matches!(error, StoreError::NotFound),
    );
    let mut wrong_digest = set(2101, 1, asset.value(), "Cover");
    wrong_digest.artwork.as_mut().unwrap().image_asset_digest =
        format!("ia-r1:sha256:{}", "0".repeat(64));
    assert!(matches!(
        harness
            .service
            .set_artwork("owner", WORK, wrong_digest)
            .await,
        Err(ArtworkApplicationError::Artwork(
            ArtworkError::AssetBindingMismatch
        ))
    ));
    for (foreign_work, foreign_owner, number) in
        [(OTHER_WORK, OWNER, 101), (WORK, OTHER_OWNER, 102)]
    {
        harness.update(|state| {
            let intent = ImageIntent::create(
                PrepareImage::admit(&request(number, false)).unwrap(),
                &WorkId::parse(foreign_work).unwrap(),
                &OwnerId::parse(foreign_owner).unwrap(),
                NOW,
            )
            .unwrap();
            let metadata = state
                .assets
                .get(&asset.value().id)
                .unwrap()
                .asset
                .metadata()
                .clone();
            let foreign = ImageAsset::create(&intent, metadata, NOW).unwrap();
            state.assets.insert(
                foreign.id().as_str().into(),
                StoredImageAsset {
                    document_bytes: serde_json::to_vec(foreign.document()).unwrap(),
                    asset: foreign,
                    storage_target: "first-private".into(),
                    object_key: PNG_HASH.into(),
                },
            );
        });
        let foreign = harness
            .state
            .lock()
            .unwrap()
            .assets
            .get(&id(number))
            .unwrap()
            .asset
            .document()
            .clone();
        harness.update(|state| state.events.clear());
        assert_storage_error(
            harness
                .service
                .set_artwork("owner", WORK, set(2200 + number, 1, &foreign, "Cover"))
                .await,
            |error| matches!(error, StoreError::NotFound),
        );
        assert!(!harness
            .events()
            .iter()
            .any(|event| event.starts_with("objects:")));
    }
    let state = harness.state.lock().unwrap();
    assert!(state.changes.is_empty());
    assert_eq!(state.work.revision(), 1);
    assert_eq!(state.operations.len(), 1);
}

#[tokio::test]
async fn explicit_clear_needs_no_object_io_and_tombstones_survive_unrelated_catalog_edits() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let asset = harness.admitted(100, false).await;
    let selected = harness
        .service
        .set_artwork("owner", WORK, set(2100, 1, asset.value(), "Cover"))
        .await
        .unwrap();
    harness.update(|state| {
        state.verify_error = Some(ImageObjectError::Unavailable);
        state.open_error = Some(ImageObjectError::Unavailable);
        state.events.clear();
    });
    let cleared = harness
        .service
        .set_artwork("owner", WORK, clear(2101, 2))
        .await
        .unwrap();
    assert_eq!(cleared.value().work_revision, 3);
    assert_eq!(cleared.value().artwork, None);
    assert!(!harness
        .events()
        .iter()
        .any(|event| event.starts_with("objects:")));
    harness.update(|state| state.change(Change::CatalogEdit));
    let effective = harness.service.artwork("owner", WORK, None).await.unwrap();
    assert_eq!(effective.work_revision, 4);
    assert_eq!(effective.artwork, None);
    assert_eq!(effective.changed_at_ms, Some(NOW as u64));
    assert_eq!(
        harness
            .service
            .artwork("owner", WORK, Some(2))
            .await
            .unwrap(),
        *selected.value()
    );
    assert_eq!(
        harness
            .service
            .artwork("owner", WORK, Some(3))
            .await
            .unwrap(),
        *cleared.value()
    );
    let second_clear = harness
        .service
        .set_artwork("owner", WORK, clear(2102, 4))
        .await
        .unwrap();
    assert_eq!(second_clear.value().work_revision, 5);
    assert_eq!(harness.counts(), (1, 1, 3, 4, 5));
}

#[tokio::test]
async fn ambiguous_write_or_failed_readback_leaves_private_bytes_but_no_asset_and_can_reconcile() {
    let _lanes = TEST_LANES.lock().await;
    for failure in [ImageObjectError::Unavailable, ImageObjectError::Corrupt] {
        let harness = Harness::new();
        let raw = harness.prepared(100, false).await;
        harness.update(|state| state.readback_error = Some(failure));
        assert_object_error(
            harness
                .service
                .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
                .await
                .unwrap()
                .admit(PNG.to_vec())
                .await,
            failure,
        );
        assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
        assert_eq!(harness.state.lock().unwrap().object_bytes.len(), 1);
        assert_eq!(
            harness
                .service
                .intent("owner", WORK, &raw.id)
                .await
                .unwrap()
                .state,
            dto::ImageIntentState::Pending
        );
        harness.update(|state| state.readback_error = None);
        let accepted = harness
            .service
            .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
            .await
            .unwrap()
            .admit(PNG.to_vec())
            .await
            .unwrap();
        assert_eq!(accepted.value().verified_media.sha256, PNG_HASH);
        assert_eq!(harness.counts(), (1, 1, 0, 1, 1));
        assert_eq!(harness.state.lock().unwrap().object_bytes.len(), 1);
    }
}

#[tokio::test]
async fn declared_encoded_hash_is_checked_before_expensive_decoder_on_malformed_body() {
    let _lanes = TEST_LANES.lock().await;
    let harness = Harness::new();
    let mut raw = request(100, false);
    // Valid declared PNG type/length, wrong expected hash and an undecodable PNG prefix.
    // ExpectedHashMismatch independently proves cheap encoded integrity preceded decoding.
    raw.expected_byte_length = 33;
    raw.expected_sha256 = "0".repeat(64);
    harness
        .service
        .prepare("owner", WORK, raw.clone())
        .await
        .unwrap();
    let ticket = harness
        .service
        .begin_upload("owner", WORK, &raw.id, dto::ImageMime::Png)
        .await
        .unwrap();
    assert_image_error(
        ticket.admit(PNG[..33].to_vec()).await,
        ImageError::ExpectedHashMismatch,
    );
    assert_eq!(harness.counts(), (1, 0, 0, 1, 1));
    assert!(harness.state.lock().unwrap().object_bytes.is_empty());
}
