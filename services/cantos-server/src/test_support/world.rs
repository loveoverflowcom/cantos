//! A published catalog written into a real file-backed `server.db` for the CMP business API
//! tests (#333). Test support only: it builds *valid domain values* (identities, digests, canonical
//! snapshots) with the production constructors and stores them in the real tables, so the serving
//! path under test (production handlers, eligibility, the media route, the Library) reads them
//! exactly as it reads production rows. It is not a creator -> publication journey: no Studio
//! route, review, rights decision or publication transaction ran, and the media bytes are
//! content-addressed transport bytes, not inspected Opus.
//!
//! Identities are derived from a per-work `tag`, so two works in one file never collide, and every
//! time is relative to the fixture's clock so rights windows are meaningful against it.
use cantos_contracts as dto;
use sha2::{Digest, Sha256};

use super::ServerFixture;
use crate::{
    contexts::{
        catalog::{
            content::wire,
            domain::{
                content::{
                    ContentBody, ContentDetails, ContentRevision, ContentRevisionFacts,
                    ContentRevisionId,
                },
                ContentUnit, ContentUnitId, ContentUnitKind, NewContentUnit, NewWork, OwnerId,
                UnitDetails, Visibility, Work, WorkDetails, WorkId, WorkKind,
            },
            restore_content,
        },
        media::{
            domain::{
                lifecycle::{
                    AudioRendition, AudioRenditionId, AudioSubmission, Binding, ReleaseCandidate,
                },
                publication::{
                    decide, AudioRelease, DeliveryEvidence, DeliveryObservation,
                    DistributionClearance, ObjectObservation, PublicationApproval,
                    PublicationFacts, ReleaseId, Sha256 as ObjectSha,
                },
            },
            index_release, FilesystemPublicationStorage,
        },
        production::domain::brief::{rights_digest, ProductionBrief},
    },
    platform::store::{checked_number, Tx},
};

/// Every `rights_refs` list in the chain; producer declarations, the brief, the Work and the
/// owner's clearance must name the same references.
const RIGHTS: &str = "synthetic-rights";

/// Whether a seeded unit's chain ends in an active release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Candidate, clearance and approval exist, but nothing was ever published (staged).
    Staged,
    /// A release is the active pointer target and is indexed for listeners.
    Published,
}

#[derive(Clone, Debug)]
pub struct UnitSeed {
    pub title: String,
    pub stage: Stage,
    /// Rights end this many milliseconds after the fixture's "now"; negative = already ended
    /// (the release was valid when it was published an hour earlier).
    pub rights_until_from_now_ms: i64,
}

impl UnitSeed {
    pub fn published(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            stage: Stage::Published,
            rights_until_from_now_ms: 86_400_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorkSeed {
    pub owner: String,
    /// Distinguishes every identity of this work from other works in the same file.
    pub tag: u32,
    pub kind: WorkKind,
    pub title: String,
    pub synopsis: String,
    pub language: String,
    pub visibility: Visibility,
    pub units: Vec<UnitSeed>,
}

impl WorkSeed {
    pub fn podcast(tag: u32, title: &str, units: &[&str]) -> Self {
        Self {
            owner: "alice".into(),
            tag,
            kind: WorkKind::Podcast,
            title: title.into(),
            synopsis: "Tác phẩm tổng hợp để kiểm tra danh mục công khai.".into(),
            language: "vi-VN".into(),
            visibility: Visibility::Public,
            units: units
                .iter()
                .map(|title| UnitSeed::published(title))
                .collect(),
        }
    }
}

/// One audio chain: the identities a test asserts against.
#[derive(Clone, Debug)]
pub struct Chain {
    pub submission_id: String,
    pub rendition_id: String,
    pub candidate_id: String,
    pub release_id: Option<String>,
    pub content_revision_id: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub object_id: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct SeededUnit {
    pub unit_id: String,
    pub position: u32,
    pub title: String,
    pub chain: Chain,
}

#[derive(Clone, Debug)]
pub struct SeededWork {
    pub work_id: String,
    pub owner: String,
    pub tag: u32,
    pub title: String,
    pub units: Vec<SeededUnit>,
    /// The fixture's "now" (Unix ms) when it was seeded.
    pub now_ms: i64,
}

/// Deterministic per-work identities: `{tag}-0000-4000-8000-{n}`.
struct Ids {
    tag: u32,
    next: u64,
}

impl Ids {
    fn id(&mut self) -> String {
        self.next += 1;
        format!("{:08x}-0000-4000-8000-{:012x}", self.tag, self.next)
    }
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn stored<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("a domain value encodes")
}

fn unit_kind(kind: WorkKind) -> ContentUnitKind {
    match kind {
        WorkKind::Podcast => ContentUnitKind::Episode,
        WorkKind::Novel | WorkKind::Story => ContentUnitKind::Chapter,
    }
}

/// Deterministic transport bytes for one chain; distinct per `(tag, unit, chain)`.
fn media_bytes(tag: u32, unit: usize, chain: u32) -> Vec<u8> {
    (0..4096_u32)
        .map(|i| {
            u8::try_from(
                (i.wrapping_mul(31)
                    .wrapping_add(tag)
                    .wrapping_add(unit as u32)
                    .wrapping_add(chain.wrapping_mul(7)))
                    % 251,
            )
            .expect("the value is below 251")
        })
        .collect()
}

impl ServerFixture {
    /// Seed a Work with its Units and audio chains. `storage` receives each chain's bytes (the
    /// publication class an origin-media host serves) when the test serves media.
    pub async fn seed_work(
        &self,
        storage: Option<&FilesystemPublicationStorage>,
        seed: &WorkSeed,
    ) -> SeededWork {
        self.seed_actors(&[seed.owner.as_str()]).await;
        let now = i64::try_from(self.clock_now_micros().div_euclid(1000)).expect("the clock fits");
        let t0 = now - 3_600_000;
        let mut ids = Ids {
            tag: seed.tag,
            next: 0,
        };
        let owner = OwnerId::parse(&seed.owner).expect("a valid owner");
        let work = Work::new(NewWork {
            id: WorkId::parse(&ids.id()).expect("a valid work id"),
            kind: seed.kind,
            owner_id: owner.clone(),
            details: WorkDetails {
                title: seed.title.clone(),
                synopsis: seed.synopsis.clone(),
                language: seed.language.clone(),
                visibility: seed.visibility,
                rights_refs: vec![RIGHTS.into()],
            },
            at: t0 + 1000,
        })
        .expect("a valid work");
        let mut units = Vec::new();
        for (index, unit_seed) in seed.units.iter().enumerate() {
            units.push((
                index,
                unit_seed,
                ContentUnit::new(
                    &work,
                    NewContentUnit {
                        id: ContentUnitId::parse(&ids.id()).expect("a valid unit id"),
                        kind: unit_kind(seed.kind),
                        details: UnitDetails {
                            title: unit_seed.title.clone(),
                            description: "Tập thử nghiệm".into(),
                            source: None,
                            script: None,
                            content_digest: None,
                        },
                        position: u32::try_from(index + 1).expect("few units"),
                        at: t0 + 1000,
                    },
                )
                .expect("a valid unit"),
            ));
        }
        let mut conn = self.store().connection().await.expect("a connection");
        let tx = conn.transaction().await.expect("the writer");
        insert_work(&tx, &work, &seed.owner).await;
        let mut seeded = Vec::new();
        for (index, unit_seed, unit) in &units {
            insert_unit(&tx, unit, &seed.owner).await;
            let content = content_for(&mut ids, &work, unit, &owner, t0);
            insert_content(&tx, &content).await;
            let chain = publish_chain(
                &tx,
                storage,
                &mut ids,
                ChainFacts {
                    seed,
                    work: &work,
                    unit,
                    unit_index: *index,
                    chain_no: 1,
                    unit_seed,
                    content: &content,
                    t0,
                    now,
                    brief_version: 1,
                },
            )
            .await;
            seeded.push(SeededUnit {
                unit_id: unit.id().as_str().to_owned(),
                position: unit.position(),
                title: unit_seed.title.clone(),
                chain,
            });
        }
        tx.commit().await.expect("the seed commits");
        SeededWork {
            work_id: work.id().as_str().to_owned(),
            owner: seed.owner.clone(),
            tag: seed.tag,
            title: seed.title.clone(),
            units: seeded,
            now_ms: now,
        }
    }

    /// Publish a second, later release of `unit` (a new brief version, submission, rendition,
    /// candidate, clearance and approval). The pointer moves to it, which supersedes the first.
    pub async fn publish_replacement(
        &self,
        storage: Option<&FilesystemPublicationStorage>,
        work: &SeededWork,
        unit: &SeededUnit,
    ) -> Chain {
        let mut conn = self.store().connection().await.expect("a connection");
        let tx = conn.transaction().await.expect("the writer");
        // Fresh identities for the replacement chain, disjoint from the first (`tag + 1 << 16`).
        let mut ids = Ids {
            tag: work.tag.wrapping_add(0x1_0000),
            next: 0,
        };
        let now = i64::try_from(self.clock_now_micros().div_euclid(1000)).expect("the clock fits");
        let t0 = now - 3_600_000;
        let seed = WorkSeed {
            owner: work.owner.clone(),
            tag: work.tag,
            kind: WorkKind::Podcast,
            title: work.title.clone(),
            synopsis: String::new(),
            language: "vi-VN".into(),
            visibility: Visibility::Public,
            units: vec![],
        };
        let (work_row, unit_row, content) = load_for_replacement(&tx, work, unit).await;
        let chain = publish_chain(
            &tx,
            storage,
            &mut ids,
            ChainFacts {
                seed: &seed,
                work: &work_row,
                unit: &unit_row,
                unit_index: usize::try_from(unit.position).expect("a position") + 100,
                chain_no: 2,
                unit_seed: &UnitSeed::published(&unit.title),
                content: &content,
                t0,
                now,
                brief_version: 2,
            },
        )
        .await;
        tx.commit().await.expect("the replacement commits");
        chain
    }

    /// Withdraw the active release of `unit`: the withdrawal fact, then the pointer moves to NULL.
    pub async fn withdraw_release(&self, owner: &str, unit: &SeededUnit) {
        let release = unit
            .chain
            .release_id
            .as_deref()
            .expect("the unit has a release");
        let at = i64::try_from(self.clock_now_micros().div_euclid(1000)).expect("the clock fits");
        self.exec(
            "INSERT INTO audio_release_withdrawals(release_id,actor_id,recorded_at,reason) VALUES(?1,?2,?3,'Synthetic withdrawal')",
            &[&release, &owner, &at],
        )
        .await
        .expect("the withdrawal is recorded");
        self.exec(
            "UPDATE audio_publication_pointers SET active_release_id=NULL,version=version+1 WHERE unit_id=?1",
            &[&unit.unit_id.as_str()],
        )
        .await
        .expect("the pointer withdraws");
    }

    /// Revoke the producer capability behind the unit's submission.
    pub async fn revoke_submission(&self, owner: &str, unit: &SeededUnit) {
        let at = i64::try_from(self.clock_now_micros().div_euclid(1000)).expect("the clock fits");
        self.exec(
            "INSERT INTO audio_submission_revocations(submission_id,owner_id,revoked_at) VALUES(?1,?2,?3)",
            &[&unit.chain.submission_id.as_str(), &owner, &at],
        )
        .await
        .expect("the revocation is recorded");
    }

    /// Record a later Work revision with `visibility`, as a Studio edit would.
    pub async fn edit_work_visibility(&self, work: &SeededWork, visibility: Visibility) {
        self.edit_work(work, |details| details.visibility = visibility)
            .await;
    }

    /// Record a later Work revision after `change`, as a Studio edit would: the next revision
    /// row, then the head, in one unit. Released revisions pinned by a release do not move.
    pub async fn edit_work(&self, work: &SeededWork, change: impl FnOnce(&mut WorkDetails)) {
        let rows = self
            .rows(
                "SELECT r.snapshot FROM catalog_works w JOIN catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision WHERE w.id=?1",
                &[&work.work_id.as_str()],
            )
            .await
            .expect("the head is read");
        let bytes: Vec<u8> = rows[0].get(0);
        let current =
            crate::contexts::catalog::decode_work_snapshot(&bytes).expect("the work decodes");
        let mut details = current.details().clone();
        change(&mut details);
        let at = i64::try_from(self.clock_now_micros().div_euclid(1000)).expect("the clock fits");
        let next = current
            .edit(current.revision(), details, at)
            .expect("the edit is admitted");
        let snapshot = stored(&next);
        let revision = i64::try_from(next.revision()).expect("a revision");
        let digest = hex(&snapshot);
        let mut conn = self.store().connection().await.expect("a connection");
        let tx = conn.transaction().await.expect("the writer");
        tx.execute(
            "INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,?2,?3,?4,?5)",
            &[&work.work_id.as_str(), &revision, &snapshot, &digest.as_str(), &work.owner.as_str()],
        )
        .await
        .expect("the revision is recorded");
        tx.execute(
            "UPDATE catalog_works SET head_revision=?2 WHERE id=?1",
            &[&work.work_id.as_str(), &revision],
        )
        .await
        .expect("the head advances");
        tx.commit().await.expect("the edit commits");
    }

    /// The fixture clock's reading in microseconds.
    pub(crate) fn clock_now_micros(&self) -> i64 {
        self.clock_reading().expect("the clock reads")
    }
}

struct ChainFacts<'a> {
    seed: &'a WorkSeed,
    work: &'a Work,
    unit: &'a ContentUnit,
    unit_index: usize,
    chain_no: u32,
    unit_seed: &'a UnitSeed,
    content: &'a ContentRevision,
    t0: i64,
    now: i64,
    brief_version: u64,
}

async fn insert_work(tx: &Tx<'_>, work: &Work, actor: &str) {
    let snapshot = stored(work);
    let revision = i64::try_from(work.revision()).expect("a revision");
    tx.execute(
        "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES(?1,?2,?3,?4,?5)",
        &[
            &work.id().as_str(),
            &work.owner_id().as_str(),
            &work.kind().as_str(),
            &work.lifecycle().as_str(),
            &revision,
        ],
    )
    .await
    .expect("the work row is written");
    tx.execute(
        "INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,?2,?3,?4,?5)",
        &[
            &work.id().as_str(),
            &revision,
            &snapshot,
            &hex(&snapshot).as_str(),
            &actor,
        ],
    )
    .await
    .expect("the work revision is written");
}

async fn insert_unit(tx: &Tx<'_>, unit: &ContentUnit, actor: &str) {
    let snapshot = stored(unit);
    let revision = i64::try_from(unit.revision()).expect("a revision");
    let position = i32::try_from(unit.position()).expect("a position");
    tx.execute(
        "INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,\"position\",lifecycle) SELECT ?1,w.id,w.kind,?3,?4,?5,?6 FROM catalog_works w WHERE w.id=?2",
        &[
            &unit.id().as_str(),
            &unit.work_id().as_str(),
            &unit.kind().as_str(),
            &revision,
            &position,
            &unit.lifecycle().as_str(),
        ],
    )
    .await
    .expect("the unit row is written");
    tx.execute(
        "INSERT INTO catalog_unit_revisions(unit_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,?2,?3,?4,?5)",
        &[
            &unit.id().as_str(),
            &revision,
            &snapshot,
            &hex(&snapshot).as_str(),
            &actor,
        ],
    )
    .await
    .expect("the unit revision is written");
}

fn content_for(
    ids: &mut Ids,
    work: &Work,
    unit: &ContentUnit,
    owner: &OwnerId,
    t0: i64,
) -> ContentRevision {
    ContentRevision::new(ContentRevisionFacts {
        id: ContentRevisionId::parse(&ids.id()).expect("a content id"),
        work_id: work.id().clone(),
        unit_id: unit.id().clone(),
        revision: 1,
        details: ContentDetails::new(
            work.details().language.clone(),
            ContentBody::Recording,
            vec![],
            "Synthetic recording fixture".into(),
        )
        .expect("valid content details"),
        recorded_by: owner.clone(),
        recorded_at: t0 + 1000,
    })
    .expect("a valid content revision")
}

async fn insert_content(tx: &Tx<'_>, content: &ContentRevision) {
    let snapshot = stored(&wire::response(content).expect("content encodes"));
    let revision = i64::try_from(content.revision()).expect("a revision");
    let digest = content.revision_digest().expect("a digest");
    tx.execute(
        "INSERT INTO catalog_content_revisions(id,unit_id,revision,snapshot,revision_digest,recorded_by) VALUES(?1,?2,?3,?4,?5,?6)",
        &[
            &content.id().as_str(),
            &content.unit_id().as_str(),
            &revision,
            &snapshot,
            &digest.as_str(),
            &content.recorded_by().as_str(),
        ],
    )
    .await
    .expect("the content revision is written");
    if revision == 1 {
        tx.execute(
            "INSERT INTO catalog_content_heads(unit_id,revision_id,revision) VALUES(?1,?2,?3)",
            &[
                &content.unit_id().as_str(),
                &content.id().as_str(),
                &revision,
            ],
        )
        .await
        .expect("the content head is written");
    }
}

/// Re-read what a replacement chain binds: the stored Work, Unit and first content revision.
async fn load_for_replacement(
    tx: &Tx<'_>,
    work: &SeededWork,
    unit: &SeededUnit,
) -> (Work, ContentUnit, ContentRevision) {
    use crate::contexts::catalog::{
        decode_unit_snapshot as decode_unit, decode_work_snapshot as decode_work,
    };
    let w: Vec<u8> = tx
        .query_one(
            "SELECT r.snapshot FROM catalog_works w JOIN catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision WHERE w.id=?1",
            &[&work.work_id.as_str()],
        )
        .await
        .expect("the work is read")
        .get(0);
    let work_row = decode_work(&w).expect("the work decodes");
    let u: Vec<u8> = tx
        .query_one(
            "SELECT r.snapshot FROM catalog_units u JOIN catalog_unit_revisions r ON r.unit_id=u.id AND r.revision=u.head_revision WHERE u.id=?1",
            &[&unit.unit_id.as_str()],
        )
        .await
        .expect("the unit is read")
        .get(0);
    let unit_row = decode_unit(&u, work_row.kind()).expect("the unit decodes");
    let c: Vec<u8> = tx
        .query_one(
            "SELECT snapshot FROM catalog_content_revisions WHERE id=?1",
            &[&unit.chain.content_revision_id.as_str()],
        )
        .await
        .expect("the content is read")
        .get(0);
    let content = restore_content(serde_json::from_slice(&c).expect("content json"))
        .expect("the content restores");
    (work_row, unit_row, content)
}

#[allow(clippy::too_many_lines)]
async fn publish_chain(
    tx: &Tx<'_>,
    storage: Option<&FilesystemPublicationStorage>,
    ids: &mut Ids,
    f: ChainFacts<'_>,
) -> Chain {
    let owner = OwnerId::parse(&f.seed.owner).expect("a valid owner");
    let t0 = f.t0;
    // The brief of this unit's content revision.
    let request: dto::CreateProductionBriefRequest = serde_json::from_str(include_str!(
        "../../../../packages/contracts/fixtures/briefs/v1/create-recording.json"
    ))
    .expect("the brief fixture parses");
    let brief_id = ids.id();
    let brief = ProductionBrief::admit(
        dto::ProductionBriefDocument {
            schema_version: "1".into(),
            id: brief_id.clone(),
            version: f.brief_version,
            work: dto::BriefWork {
                id: f.work.id().as_str().into(),
                owner_id: f.seed.owner.clone(),
                kind: f.work.kind().as_str().into(),
                title: f.work.details().title.clone(),
                rights_refs: vec![RIGHTS.into()],
                rights_digest: rights_digest(&[RIGHTS.into()]).expect("a rights digest"),
            },
            content: wire::response(f.content).expect("content encodes"),
            instructions: dto::ProductionInstructions::Recording {},
            guidance: request.guidance,
            created_by: f.seed.owner.clone(),
            created_at: t0 + 2000,
            digest: String::new(),
        },
        f.content,
    )
    .expect("a valid brief");
    let brief_doc = brief.document();
    let brief_operation = ids.id();
    tx.execute(
        "INSERT INTO production_briefs(id,unit_id,content_revision_id,owner_id,version,operation_id,request_digest,digest,snapshot,content_sequence) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        &[
            &brief_doc.id.as_str(),
            &brief_doc.content.unit_id.as_str(),
            &brief_doc.content.id.as_str(),
            &brief_doc.work.owner_id.as_str(),
            &i64::try_from(brief_doc.version).expect("a version"),
            &brief_operation.as_str(),
            &format!("pb-request1:sha256:{}", hex(brief_operation.as_bytes())).as_str(),
            &brief_doc.digest.as_str(),
            &stored(brief_doc),
            &i64::try_from(brief_doc.content.revision).expect("a revision"),
        ],
    )
    .await
    .expect("the brief row is written");

    // The submission, bound to that exact brief.
    let bytes = media_bytes(f.seed.tag, f.unit_index, f.chain_no);
    let sha256 = hex(&bytes);
    let submission_id = ids.id();
    let submission_operation = ids.id();
    let upload_token = hex(format!("upload-{}", ids.id()));
    let submission = AudioSubmission::create(
        &dto::CreateAudioSubmissionRequest {
            operation_id: submission_operation.clone(),
            id: submission_id.clone(),
            binding: Binding::from_brief(&brief)
                .expect("a binding")
                .raw()
                .clone(),
            expected_sha256: sha256.clone(),
            expected_byte_length: bytes.len() as u64,
            declarations: dto::ProducerDeclarations {
                producer: "Synthetic producer".into(),
                method: "recording".into(),
                provenance: "Original owned test signal".into(),
                rights_refs: vec![RIGHTS.into()],
                quality_report: "NOT_RUN listening".into(),
            },
            upload_token: upload_token.clone(),
            expires_at: t0 + 5000,
        },
        &brief,
        &owner,
        t0 + 3000,
    )
    .expect("a valid submission");
    let sd = submission.document();
    tx.execute(
        "INSERT INTO audio_submissions(id,unit_id,owner_id,brief_id,brief_digest,operation_id,request_digest,token_hash,snapshot,digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        &[
            &sd.id.as_str(),
            &sd.binding.unit_id.as_str(),
            &sd.owner_id.as_str(),
            &sd.binding.brief_id.as_str(),
            &sd.binding.brief_digest.as_str(),
            &sd.operation_id.as_str(),
            &format!("as-request1:sha256:{}", hex(sd.operation_id.as_bytes())).as_str(),
            &Sha256::digest(upload_token.as_bytes()).to_vec(),
            &stored(sd),
            &sd.digest.as_str(),
        ],
    )
    .await
    .expect("the submission row is written");

    // The rendition the producer's bytes were admitted as.
    let rendition_id = ids.id();
    let rendition = AudioRendition::create(
        &submission,
        dto::AudioMediaMetadata {
            sha256: sha256.clone(),
            byte_length: bytes.len() as u64,
            container: "ogg".into(),
            codec: "opus".into(),
            channels: 1,
            sample_rate_hz: 48000,
            duration_ms: 60_000,
            audio_bitrate_bps: 24000,
            profile_id: "opus-mono-24-v1".into(),
        },
        AudioRenditionId::parse(&rendition_id).expect("a rendition id"),
        t0 + 4000,
    )
    .expect("a valid rendition");
    let rd = rendition.document();
    tx.execute(
        "INSERT INTO audio_renditions(id,submission_id,object_key,snapshot,digest,storage_target_id) VALUES(?1,?2,?3,?4,?5,?6)",
        &[
            &rd.id.as_str(),
            &rd.submission_id.as_str(),
            &rd.verified_media.sha256.as_str(),
            &stored(rd),
            &rd.digest.as_str(),
            &"legacy-filesystem",
        ],
    )
    .await
    .expect("the rendition row is written");

    // Reviews are evidence inside the release manifest; the review rows themselves belong to the
    // Media write path and are not part of this slice.
    let reviews: Vec<_> = [
        dto::AudioReviewKind::Qc,
        dto::AudioReviewKind::Rights,
        dto::AudioReviewKind::Editorial,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| {
        let id = ids.id();
        crate::contexts::media::domain::lifecycle::AudioReview::record(
            dto::AudioReviewDocument {
                id: id.clone(),
                rendition_id: rd.id.clone(),
                operation_id: id.clone(),
                sequence: i as u64 + 1,
                reviewer_id: f.seed.owner.clone(),
                reviewed_at: t0 + 4500,
                request: dto::CreateAudioReviewRequest {
                    operation_id: id,
                    rendition_digest: rd.digest.clone(),
                    sha256: rd.verified_media.sha256.clone(),
                    kind,
                    decision: dto::AudioReviewDecision::Approved,
                    notes: "Synthetic approval; no listening claim".into(),
                    evidence_refs: vec!["fixture-only".into()],
                },
                digest: String::new(),
            },
            &rendition,
        )
        .expect("a valid review")
    })
    .collect();

    // The candidate the owner then clears and approves.
    let candidate_id = ids.id();
    let candidate_operation = ids.id();
    let candidate = ReleaseCandidate::record(
        dto::ReleaseCandidateDocument {
            id: candidate_id.clone(),
            rendition_id: rd.id.clone(),
            rendition_digest: rd.digest.clone(),
            binding: rd.binding.clone(),
            sha256: sha256.clone(),
            work_metadata_revision: f.work.revision(),
            unit_metadata_revision: f.unit.revision(),
            review_ids: reviews.iter().map(|r| r.document().id.clone()).collect(),
            created_by: f.seed.owner.clone(),
            created_at: t0 + 4500,
            operation_id: candidate_operation.clone(),
            digest: String::new(),
        },
        &rendition,
    )
    .expect("a valid candidate");
    let cd = candidate.document();
    tx.execute(
        "INSERT INTO audio_release_candidates VALUES(?1,?2,?3,?4,?5,?6)",
        &[
            &cd.id.as_str(),
            &cd.rendition_id.as_str(),
            &cd.created_by.as_str(),
            &cd.operation_id.as_str(),
            &stored(cd),
            &cd.digest.as_str(),
        ],
    )
    .await
    .expect("the candidate row is written");

    let rights_id = ids.id();
    let rights = DistributionClearance::record(
        dto::DistributionClearanceDocument {
            id: rights_id.clone(),
            candidate_id: cd.id.clone(),
            actor_id: f.seed.owner.clone(),
            sequence: 1,
            recorded_at: t0 + 4600,
            request: dto::RecordDistributionRightsRequest {
                operation_id: rights_id,
                candidate_digest: cd.digest.clone(),
                decision: dto::DistributionDecision::Cleared,
                rights_refs: vec![RIGHTS.into()],
                valid_until: f.now + f.unit_seed.rights_until_from_now_ms,
                evidence_refs: vec!["synthetic-owner-reviewed-permission".into()],
                notes: "Fixture permission for distribution".into(),
            },
            digest: String::new(),
        },
        &candidate,
    )
    .expect("a valid clearance");
    let dc = rights.document();
    tx.execute(
        "INSERT INTO audio_distribution_clearances(id,candidate_id,owner_id,operation_id,sequence,snapshot,digest) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        &[
            &dc.id.as_str(),
            &dc.candidate_id.as_str(),
            &dc.actor_id.as_str(),
            &dc.request.operation_id.as_str(),
            &checked_number(dc.sequence).expect("a sequence"),
            &stored(dc),
            &dc.digest.as_str(),
        ],
    )
    .await
    .expect("the clearance row is written");

    let approval_id = ids.id();
    let approval = PublicationApproval::record(
        dto::PublicationApprovalDocument {
            id: approval_id.clone(),
            candidate_id: cd.id.clone(),
            actor_id: f.seed.owner.clone(),
            sequence: 1,
            recorded_at: t0 + 4700,
            request: dto::RecordPublicationApprovalRequest {
                operation_id: approval_id,
                candidate_digest: cd.digest.clone(),
                clearance_digest: dc.digest.clone(),
                decision: dto::PublicationApprovalDecision::Approved,
                notes: "Approve this exact synthetic candidate".into(),
            },
            digest: String::new(),
        },
        &candidate,
    )
    .expect("a valid approval");
    let pa = approval.document();
    tx.execute(
        "INSERT INTO audio_publication_approvals(id,candidate_id,owner_id,operation_id,sequence,snapshot,digest) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        &[
            &pa.id.as_str(),
            &pa.candidate_id.as_str(),
            &pa.actor_id.as_str(),
            &pa.request.operation_id.as_str(),
            &checked_number(pa.sequence).expect("a sequence"),
            &stored(pa),
            &pa.digest.as_str(),
        ],
    )
    .await
    .expect("the approval row is written");

    // The pointer exists from the first candidate on, at version 0 with nothing active.
    tx.execute(
        "INSERT INTO audio_publication_pointers SELECT ?1,?2,NULL,0 WHERE NOT EXISTS (SELECT 1 FROM audio_publication_pointers WHERE unit_id=?1)",
        &[&f.unit.id().as_str(), &f.seed.owner.as_str()],
    )
    .await
    .expect("the pointer row is written");

    let object_id = format!("publication-{}-{}", rd.id, sha256);
    let mut release_id = None;
    if f.unit_seed.stage == Stage::Published {
        let object = ObjectObservation::Verified {
            sha256: ObjectSha::parse(&sha256).expect("a digest"),
            byte_length: bytes.len() as u64,
        };
        let delivery = DeliveryObservation::Verified(
            DeliveryEvidence::new(
                object_id.clone(),
                ObjectSha::parse(&sha256).expect("a digest"),
                bytes.len() as u64,
                t0 + 4800,
                t0 + 9000,
            )
            .expect("valid delivery evidence"),
        );
        let facts = PublicationFacts {
            candidate: &candidate,
            rendition: &rendition,
            work: f.work,
            unit: f.unit,
            content: f.content,
            upload_expires_at: t0 + 8000,
            actor: &owner,
            current_brief: true,
            upload_revoked: false,
            reviews: &reviews,
            rights: Some(&rights),
            approval: Some(&approval),
            object: &object,
            delivery: &delivery,
            now: t0 + 5000,
        };
        let witness = decide(&facts).expect("the exact chain is publishable");
        let id = ids.id();
        let release = AudioRelease::publish(
            ReleaseId::parse(&id).expect("a release id"),
            &owner,
            t0 + 5000,
            None,
            witness,
        )
        .expect("a valid release");
        let d = release.document();
        let m = &d.manifest;
        tx.execute(
            "INSERT INTO audio_releases(id,unit_id,owner_id,candidate_id,rendition_id,snapshot,manifest_digest,digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            &[
                &d.id.as_str(),
                &m.unit_id.as_str(),
                &d.published_by.as_str(),
                &m.candidate_id.as_str(),
                &m.rendition_id.as_str(),
                &stored(d),
                &m.digest.as_str(),
                &d.digest.as_str(),
            ],
        )
        .await
        .expect("the release row is written");
        // The pointer activates the release (one version forward), then the production indexer
        // writes the listener discovery rows for it.
        tx.execute(
            "UPDATE audio_publication_pointers SET active_release_id=?1,version=version+1 WHERE unit_id=?2",
            &[&d.id.as_str(), &f.unit.id().as_str()],
        )
        .await
        .expect("the pointer activates");
        index_release(tx, &release)
            .await
            .expect("the release is indexed");
        release_id = Some(d.id.clone());
    }
    if let Some(storage) = storage {
        storage
            .place_transport_bytes(&bytes)
            .expect("the publication bytes are placed");
    }
    Chain {
        submission_id,
        rendition_id,
        candidate_id,
        release_id,
        content_revision_id: f.content.id().as_str().to_owned(),
        sha256,
        bytes,
        object_id,
        duration_ms: 60_000,
    }
}
