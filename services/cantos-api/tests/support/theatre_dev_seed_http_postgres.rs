//! Development Theatre catalog seed behind `cargo xtask seed theatre`.
//!
//! Every Work, unit, content revision, brief, submission, private Opus intake, review,
//! clearance, approval, publication and withdrawal passes the actual Studio HTTP boundary and
//! app-role PostgreSQL. The only substitute is the test-only `VerifiedDeliveryFake` public
//! delivery observer; production hosts never compose it, so the seeded releases are
//! discoverable while playback stays `rendition_unavailable`. All text and audio is synthetic.
//!
//! Without `CANTOS_THEATRE_SEED_RECORD` this is an ordinary regression in the disposable
//! PostgreSQL gate. With it, the runner (`scripts/seed_theatre.py`) retains the database and
//! receives the record of stable identities it wrote.
use super::*;
use cantos_contracts::theatre::{PublicationManifest, UnitPage, WorkPage, WorkSummary};
use std::path::Path;

const RECORD_ENV: &str = "CANTOS_THEATRE_SEED_RECORD";
/// Long enough for a development session; the record says when to seed again.
const RIGHTS_WINDOW_MS: i64 = 30 * 86_400_000;
/// More than the default listener page (20), so Load more is exercised by real data.
const PODCAST_EPISODES: u16 = 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    /// Public Work; every unit is published unless the unit itself is withdrawn.
    Public,
    /// Public Work whose only release was published and then withdrawn.
    Withdrawn,
    /// Published release of a private Work: never anonymous.
    Private,
    /// Approved candidate that was never published.
    Unpublished,
}
impl Fate {
    fn name(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Withdrawn => "withdrawn",
            Self::Private => "private",
            Self::Unpublished => "unpublished",
        }
    }
}
struct SeedUnit {
    title: String,
    description: String,
    withdraw: bool,
    seconds: u8,
}
struct SeedWork {
    number: u16,
    kind: &'static str,
    title: &'static str,
    synopsis: &'static str,
    language: &'static str,
    fate: Fate,
    units: Vec<SeedUnit>,
}
fn work_id(number: u16) -> String {
    format!("5eed{number:04}-0000-4000-8000-000000000000")
}
fn unit_id(work: u16, unit: usize) -> String {
    format!("5eed{work:04}-{:04}-4000-8000-000000000000", unit + 1)
}
fn unit(title: &str, description: &str, seconds: u8) -> SeedUnit {
    SeedUnit {
        title: title.into(),
        description: description.into(),
        withdraw: false,
        seconds,
    }
}
/// Display order on the public shelf is newest first publication, so publish in reverse.
fn catalog() -> Vec<SeedWork> {
    let mut lantern = vec![
        unit(
            "Ngọn đèn đầu tiên",
            "Cô bé Thảo học cách dán khung tre từ bà ngoại.",
            3,
        ),
        unit(
            "Đêm rằm bị hoãn",
            "Chương đã rút khỏi phát hành để kiểm tra hàng bị ẩn.",
            2,
        ),
        unit(
            "Ánh sáng qua lớp giấy",
            "Chiếc đèn cuối cùng được thả trôi trên sông.",
            4,
        ),
    ];
    lantern[1].withdraw = true;
    vec![
        SeedWork {
            number: 1,
            kind: "novel",
            title: "Mùa gió chướng",
            synopsis: "Một gia đình làm muối ở cửa biển chờ người cha đi biển trở về, qua bốn mùa gió chướng thổi ngược dòng sông.",
            language: "vi-VN",
            fate: Fate::Public,
            units: vec![
                unit("Chương một: Bến cũ", "Ruộng muối, chiếc ghe và lời hứa trước mùa gió.", 3),
                unit("Chương hai: Muối kết hạt", "Nắng gắt làm hạt muối trắng và lòng người mặn.", 4),
                unit("Chương ba: Tin từ ngoài khơi", "Một chiếc radio cũ bắt được giọng nói quen.", 5),
                unit("Chương bốn: Gió đổi chiều", "Cả xóm ra bến đón những cánh buồm đầu tiên.", 3),
            ],
        },
        SeedWork {
            number: 2,
            kind: "story",
            title: "Chiếc đèn lồng giấy",
            synopsis: "Truyện ngắn về ba đêm làm đèn lồng trước Tết Trung thu ở một con hẻm Hội An.",
            language: "vi-VN",
            fate: Fate::Public,
            units: lantern,
        },
        SeedWork {
            number: 3,
            kind: "podcast",
            title: "Chuyện đêm khuya",
            synopsis: "Mỗi tập là một câu chuyện ngắn kể chậm cho người khó ngủ, từ những chuyến xe đêm đến tiếng mưa trên mái tôn.",
            language: "vi-VN",
            fate: Fate::Public,
            units: (1..=PODCAST_EPISODES)
                .map(|n| SeedUnit {
                    title: format!("Tập {n}: Chuyến xe số {}", 10 + n),
                    description: format!("Câu chuyện thứ {n} trên tuyến xe đêm qua thành phố."),
                    withdraw: false,
                    seconds: 2 + (n % 3) as u8,
                })
                .collect(),
        },
        SeedWork {
            number: 4,
            kind: "story",
            title: "The Lantern Keeper's Notes",
            synopsis: "A short English story about a keeper who writes one note for every lantern on the river.",
            language: "en-US",
            fate: Fate::Public,
            units: vec![
                unit("First note", "The keeper finds a lantern without a name.", 2),
                unit("Last note", "Every lantern returns to the riverbank at dawn.", 3),
            ],
        },
        SeedWork {
            number: 5,
            kind: "podcast",
            title: "Tín hiệu đã tắt",
            synopsis: "Podcast đã phát hành rồi rút lại; phải luôn trả not_found công khai.",
            language: "vi-VN",
            fate: Fate::Withdrawn,
            units: vec![unit("Tập duy nhất", "Bản phát hành đã bị rút.", 2)],
        },
        SeedWork {
            number: 6,
            kind: "novel",
            title: "Bản thảo riêng tư",
            synopsis: "Tác phẩm riêng tư có bản phát hành; khán giả ẩn danh không được thấy.",
            language: "vi-VN",
            fate: Fate::Private,
            units: vec![unit("Chương riêng tư", "Không bao giờ xuất hiện trong danh mục công khai.", 2)],
        },
        SeedWork {
            number: 7,
            kind: "story",
            title: "Truyện chưa phát hành",
            synopsis: "Ứng viên đã duyệt nhưng chưa xuất bản; không có bản phát hành công khai.",
            language: "vi-VN",
            fate: Fate::Unpublished,
            units: vec![unit("Chương chờ phát hành", "Chưa có bản phát hành.", 2)],
        },
    ]
}

struct Seeded {
    id: String,
    title: String,
    position: usize,
    publication: Option<String>,
    pointer_version: u64,
    withdrawn: bool,
}
struct SeedWorld {
    h: Harness,
    root: PrivateRoot,
}
impl SeedWorld {
    async fn new() -> Self {
        let mut h = Harness::new().await;
        // The same narrow grants the catalog walk uses; the default host can then read the
        // public projection and record withdrawals, but cannot publish (UnavailableDelivery).
        h.admin.batch_execute("GRANT SELECT,INSERT ON audio_submissions,audio_submission_revocations,audio_renditions,audio_reviews,audio_release_candidates,audio_distribution_clearances,audio_publication_approvals,audio_releases,audio_release_withdrawals,audio_publication_operations,audio_publication_events,audio_publication_pointers,audio_listener_works,audio_listener_units,audio_listener_releases TO cantos_app; GRANT UPDATE(active_release_id,version) ON audio_publication_pointers TO cantos_app; GRANT EXECUTE ON FUNCTION catalog_listener_discoverable(text,text),listener_actor_active(text) TO cantos_app;").await.unwrap();
        let root = PrivateRoot::new();
        h.store = h.store.clone().with_audio_storage(&root.0).unwrap();
        h.app = router_with_publication_delivery(
            AppState {
                store: h.store.clone(),
                web_origin: ORIGIN.into(),
            },
            "unused-public-root",
            Arc::new(VerifiedDeliveryFake),
        );
        Self { h, root }
    }
    /// A distinct synthetic Ogg/Opus signal per unit: real bytes for the actual intake check.
    fn signal(&self, seconds: u8, frequency: u32) -> Vec<u8> {
        let path = self.root.0.join(format!("seed-{}.opus", operation()));
        let source = format!("sine=frequency={frequency}:sample_rate=48000:duration={seconds}");
        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg(source)
            .args(["-ac", "1", "-c:a", "libopus", "-b:a", "24k", "-vbr", "off"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(path).unwrap()
    }
    async fn seed(&self, spec: &SeedWork, valid_until: i64) -> Vec<Seeded> {
        let id = work_id(spec.number);
        let mut request = work_request(spec.kind);
        request["id"] = json!(id);
        request["details"]["title"] = json!(spec.title);
        request["details"]["synopsis"] = json!(spec.synopsis);
        request["details"]["language"] = json!(spec.language);
        request["details"]["visibility"] = json!(if spec.fate == Fate::Private {
            "private"
        } else {
            "public"
        });
        request["details"]["rights_refs"] = json!(["catalog-rights"]);
        let mut work = self
            .h
            .json("POST", "/works", Some(ALICE), Some(request))
            .await
            .success(StatusCode::CREATED)["work"]
            .clone();
        for (index, unit) in spec.units.iter().enumerate() {
            let mut request = unit_request(&work, &unit.title);
            request["id"] = json!(unit_id(spec.number, index));
            request["details"]["description"] = json!(unit.description);
            work = self
                .h
                .json(
                    "POST",
                    &format!("/works/{id}/content-units"),
                    Some(ALICE),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED)["work"]
                .clone();
        }
        // Content writes advance the Work revision; finish them before any candidate exists so
        // every release pins the final metadata revision.
        let mut contents = Vec::new();
        for index in 0..spec.units.len() {
            let mut request = content_request(
                0,
                work["revision"].as_u64().unwrap(),
                json!({"kind":"recording"}),
            );
            request["details"]["language"] = json!(spec.language);
            let response = self
                .h
                .json(
                    "POST",
                    &format!(
                        "/content-units/{}/content/revisions",
                        unit_id(spec.number, index)
                    ),
                    Some(ALICE),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED);
            work["revision"] = response["work_revision"].clone();
            contents.push(response["content"].clone());
        }
        let mut seeded = Vec::new();
        for (index, (unit, content)) in spec.units.iter().zip(contents).enumerate() {
            let uid = unit_id(spec.number, index);
            let brief = self
                .h
                .json(
                    "POST",
                    &format!("/content-units/{uid}/production-briefs"),
                    Some(ALICE),
                    Some(brief_http::brief_request(&content, 0, true)),
                )
                .await
                .success(StatusCode::CREATED)["brief"]
                .clone();
            let bytes = self.signal(
                unit.seconds,
                220 + 20 * u32::from(spec.number) + 7 * index as u32,
            );
            // Upload capabilities are unique within the shared database.
            let token = format!("{:x}", Sha256::digest(operation().as_bytes()));
            let request = submission_request(&self.h, &uid, &brief, &bytes, &token).await;
            let submission = request["id"].as_str().unwrap().to_owned();
            self.h
                .json(
                    "POST",
                    &format!("/content-units/{uid}/audio-submissions"),
                    Some(ALICE),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED);
            let rendition = upload(self.h.app.clone(), &submission, &token, bytes)
                .await
                .success(StatusCode::OK);
            let (candidate, _, _) =
                approve_candidate(&self.h, &uid, &submission, &rendition, valid_until).await;
            let mut row = Seeded {
                id: uid.clone(),
                title: unit.title.clone(),
                position: index,
                publication: None,
                pointer_version: 0,
                withdrawn: false,
            };
            if spec.fate != Fate::Unpublished {
                let published = self.h.json("POST", &format!("/content-units/{uid}/release-candidates/{}/publish", candidate["id"].as_str().unwrap()), Some(ALICE), Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"expected_pointer_version":0,"expected_active_release_id":null}))).await.success(StatusCode::CREATED);
                row.pointer_version = published["pointer_version"].as_u64().unwrap();
                row.publication = Some(published["release"]["id"].as_str().unwrap().to_owned());
            }
            if unit.withdraw || spec.fate == Fate::Withdrawn {
                let withdrawn = self.h.json("POST", &format!("/content-units/{uid}/publication/withdraw"), Some(ALICE), Some(json!({"operation_id":operation(),"release_id":row.publication,"expected_pointer_version":row.pointer_version,"reason":"Development seed: withdrawn release must stay hidden"}))).await.success(StatusCode::OK);
                row.pointer_version = withdrawn["pointer_version"].as_u64().unwrap();
                row.withdrawn = true;
            }
            seeded.push(row);
        }
        seeded
    }
    async fn get(&self, path: &str) -> (StatusCode, Value) {
        theatre_call(self.h.app.clone(), "GET", path).await
    }
    /// Strict decoding through the shared v1 DTOs proves no extra (private) field leaked.
    async fn decode<T: serde::de::DeserializeOwned>(&self, path: &str) -> T {
        let (status, body) = self.get(path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        serde_json::from_value(body).unwrap_or_else(|e| panic!("{path}: {e}"))
    }
    async fn not_found(&self, path: &str) {
        let (status, body) = self.get(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(body, json!({"code":"not_found","retryable":false}));
    }
    async fn units(&self, work: &str) -> Vec<UnitPage> {
        let mut page: UnitPage = self.decode(&format!("/works/{work}/units")).await;
        let mut pages = Vec::new();
        for _ in 0..8 {
            assert_eq!(page.work_id, work);
            let next = page.next_cursor.clone();
            pages.push(page);
            let Some(cursor) = next else {
                return pages;
            };
            page = self
                .decode(&format!("/works/{work}/units?cursor={cursor}"))
                .await;
        }
        panic!("bounded seeded unit walk did not terminate");
    }
}

fn record(
    path: &Path,
    database: &str,
    valid_until: i64,
    specs: &[SeedWork],
    seeded: &[Vec<Seeded>],
) {
    let works: Vec<Value> = specs
        .iter()
        .zip(seeded)
        .map(|(spec, units)| {
            json!({
                "work_id": work_id(spec.number), "kind": spec.kind, "title": spec.title,
                "language": spec.language, "fate": spec.fate.name(),
                "units": units.iter().map(|u| json!({
                    "unit_id": u.id, "title": u.title, "studio_position": u.position,
                    "publication_id": u.publication, "pointer_version": u.pointer_version,
                    "withdrawn": u.withdrawn,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let value = json!({
        "format": 1,
        "database": database,
        "rights_valid_until_ms": valid_until,
        "studio_actor": "alice",
        "works": works,
    });
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn theatre_development_catalog_seed() {
    let world = SeedWorld::new().await;
    let valid_until = now(&world.h).await + RIGHTS_WINDOW_MS;
    let specs = catalog();
    let mut seeded = Vec::new();
    for spec in specs.iter().rev() {
        seeded.push(world.seed(spec, valid_until).await);
    }
    seeded.reverse();

    // The shelf: only the four public Works, newest first publication first.
    let public: Vec<String> = specs
        .iter()
        .filter(|s| s.fate == Fate::Public)
        .map(|s| work_id(s.number))
        .collect();
    let shelf: WorkPage = world.decode("/works?limit=20").await;
    assert_eq!(
        shelf
            .items
            .iter()
            .map(|w| w.work_id.clone())
            .collect::<Vec<_>>(),
        public
    );
    assert_eq!(shelf.next_cursor, None);
    let first: WorkPage = world.decode("/works?limit=2").await;
    let cursor = first
        .next_cursor
        .clone()
        .expect("four works need a second page of two");
    let second: WorkPage = world
        .decode(&format!("/works?limit=2&cursor={cursor}"))
        .await;
    assert_eq!(second.next_cursor, None);
    assert_eq!(
        first
            .items
            .iter()
            .chain(&second.items)
            .cloned()
            .collect::<Vec<_>>(),
        shelf.items
    );
    for (spec, units) in specs.iter().zip(&seeded) {
        let wid = work_id(spec.number);
        if spec.fate != Fate::Public {
            world.not_found(&format!("/works/{wid}")).await;
            world.not_found(&format!("/works/{wid}/units")).await;
            for unit in units {
                if let Some(publication) = &unit.publication {
                    world
                        .not_found(&format!("/publications/{publication}"))
                        .await;
                }
            }
            continue;
        }
        let detail: WorkSummary = world.decode(&format!("/works/{wid}")).await;
        assert_eq!(Some(&detail), shelf.items.iter().find(|w| w.work_id == wid));
        assert_eq!(
            (detail.title.as_str(), detail.language.as_str()),
            (spec.title, spec.language)
        );
        let pages = world.units(&wid).await;
        let rows: Vec<_> = pages.iter().flat_map(|p| p.items.clone()).collect();
        let expected: Vec<&Seeded> = units.iter().filter(|u| !u.withdrawn).collect();
        assert_eq!(
            rows.iter()
                .map(|r| (r.unit_id.as_str(), r.position as usize))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|u| (u.id.as_str(), u.position))
                .collect::<Vec<_>>(),
            "withdrawn units are omitted and positions keep their Studio order with gaps"
        );
        for (row, (unit, spec_unit)) in rows
            .iter()
            .zip(units.iter().zip(&spec.units).filter(|(u, _)| !u.withdrawn))
        {
            assert_eq!(Some(&row.publication_id), unit.publication.as_ref());
            assert_eq!(row.title, spec_unit.title);
            assert_eq!(row.description, spec_unit.description);
            let expected_ms = u64::from(spec_unit.seconds) * 1000;
            assert!(row.duration_ms.abs_diff(expected_ms) <= 100, "{row:?}");
        }
        if spec.kind == "podcast" {
            assert_eq!(pages.len(), 2, "23 episodes are two default pages");
            assert_eq!(pages[0].items.len(), 20);
        }
    }

    // A manifest is the exact public release; playback stays fail-closed on this host.
    let novel = &seeded[0][0];
    let publication = novel.publication.as_deref().unwrap();
    let manifest: PublicationManifest = world.decode(&format!("/publications/{publication}")).await;
    assert_eq!(
        (
            manifest.work_id.as_str(),
            manifest.unit_id.as_str(),
            manifest.publication_id.as_str()
        ),
        (work_id(1).as_str(), novel.id.as_str(), publication)
    );
    let (status, body) = theatre_call(
        world.h.app.clone(),
        "POST",
        &format!("/publications/{publication}/playback-authorizations"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body,
        json!({"code":"rendition_unavailable","retryable":true})
    );
    world
        .not_found("/works/5eed9999-0000-4000-8000-000000000000")
        .await;
    let (status, body) = world
        .get(&format!(
            "/works/{}/units?cursor=lc1_{}",
            work_id(3),
            "0".repeat(64)
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"code":"invalid_cursor","retryable":false}));

    if let Some(path) = env::var_os(RECORD_ENV) {
        // Keep the synthetic Studio session usable for documented withdrawal changes.
        world
            .h
            .admin
            .execute(
                "UPDATE sessions SET expires_at=to_timestamp($1::bigint/1000.0) WHERE actor_id='alice'",
                &[&valid_until],
            )
            .await
            .unwrap();
        record(
            Path::new(&path),
            &world.h.database,
            valid_until,
            &specs,
            &seeded,
        );
    }
}
