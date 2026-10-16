# Local Theatre catalog with seeded development data

Status: **implemented development tooling** for a disposable local database. It is not a
production seed, publication path, playback path or deployment acceptance. Evidence for the
first run is in [the integration record](../evidence/theatre-seeded-catalog-integration.md).

## What `cargo xtask seed theatre` does

It creates a **new database in the disposable cluster** retained by
`cargo xtask verify postgres --keep` and runs one PostgreSQL test,
`theatre_development_catalog_seed`, against it. Every record goes through the real Studio HTTP
routes and app-role SQL: Work, unit, content revision, brief, submission, actual Ogg/Opus intake,
QC/rights/editorial reviews, distribution clearance, publication approval, publish and withdraw.
The only substitute is the test-only public-delivery observer (`VerifiedDeliveryFake`), which
lives under `services/cantos-api/tests/` and is never composed by `cantos-server`. No row is
inserted around rights, approvals, publication eligibility or revision immutability.

The command then writes `target/revision-evidence/theatre-seed.json` (ignored by Git) and drops
the previous seed database recorded for the same cluster. `cargo xtask run cantos-theatre --web
--test-cluster` serves the recorded seed database when the record matches the retained cluster,
otherwise the existing empty Theatre fixture.

| Work ID | Kind | Title | Public result |
| --- | --- | --- | --- |
| `5eed0001-0000-4000-8000-000000000000` | novel | Mùa gió chướng | 4 chapters |
| `5eed0002-0000-4000-8000-000000000000` | story | Chiếc đèn lồng giấy | 2 of 3 chapters; the second is withdrawn, so positions are 0 and 2 |
| `5eed0003-0000-4000-8000-000000000000` | podcast | Chuyện đêm khuya | 23 episodes: a default page of 20 plus a next page of 3 |
| `5eed0004-0000-4000-8000-000000000000` | story | The Lantern Keeper's Notes | 2 chapters, `en-US` |
| `5eed0005-0000-4000-8000-000000000000` | podcast | Tín hiệu đã tắt | published then withdrawn: `not_found` |
| `5eed0006-0000-4000-8000-000000000000` | novel | Bản thảo riêng tư | private Work with a release: `not_found` |
| `5eed0007-0000-4000-8000-000000000000` | story | Truyện chưa phát hành | approved, never published: `not_found` |

Unit IDs are `5eed<work>-<unit>-4000-8000-000000000000` with one-based four-digit numbers, for
example `5eed0003-0021-4000-8000-000000000000` for the 21st episode. Work and unit IDs are the
same after every reseed. `publication_id` values are minted by the publication use case, so a
reseed creates new releases with new IDs; clients must treat them as new releases. Audio
durations are 2–5 seconds of synthetic sine tones, and the rights window ends 30 days after
seeding; seed again after that.

## Run it

Prerequisites: Rust 1.98.1, Python 3, PostgreSQL server tools (`initdb`, `pg_ctl`, `psql`) on
`PATH`, `ffmpeg`/`ffprobe`, and Trunk for Web. Nothing here installs tools, starts Podman or uses
cloud credentials, Firebase, Kanidm, Gemini, Forge or S3.

```bash
cargo xtask verify postgres --keep
```

```bash
cargo xtask seed theatre
```

```bash
cargo xtask run cantos-theatre --web --test-cluster
```

The API listens on `127.0.0.1:8080` and Trunk on `127.0.0.1:8082`. Both origins must return
the same page:

```bash
curl -s 'http://127.0.0.1:8080/api/v1/theatre/works?limit=20'
```

```bash
curl -s 'http://127.0.0.1:8082/api/v1/theatre/works?limit=20'
```

Open `http://127.0.0.1:8082/`, choose a Work and use **Tải thêm** on _Chuyện đêm khuya_.
`?mode=mock` and `?mode=membership-preview` remain separate offline previews; live mode never
falls back to them.

### Android (Theatre production host)

The production host (`androidProductionApp`) composes the live catalog in its **debug** build
when an API origin is configured. Its release build keeps the unavailable state: the release
inspection admits no network permission yet. Debug cleartext is limited to `127.0.0.1` and
`localhost`; any build accepts `https://`. With an Android 7+ device attached over USB:

```bash
cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080
```

```bash
adb install -r apps/cantos-theatre/mobile/androidProductionApp/build/outputs/apk/debug/androidProductionApp-debug.apk
```

```bash
adb reverse tcp:8080 tcp:8080
```

Open **Cantos**: the list, a Work and its chapters or episodes use the same `work_id`, `unit_id`
and `publication_id` values as Web; there is no play control. Without a device, the same Kotlin
adapter, transport and controller can walk the running API on the JVM (skipped unless the
variable is set):

```bash
CANTOS_THEATRE_LIVE_ORIGIN=http://127.0.0.1:8080 cargo xtask theatre-demo verify
```

### Observe a change

The seed extends the synthetic Studio session of `alice` (token: 64 lowercase `a`) for the
rights window, so a withdrawal can be made through the real Studio route of the running API.
Read the release ID and pointer version of a unit from `theatre-seed.json`, then:

```bash
curl -sS -X POST "http://127.0.0.1:8080/api/v1/studio/content-units/5eed0003-0023-4000-8000-000000000000/publication/withdraw" -H 'Origin: http://127.0.0.1:8082' -H 'Content-Type: application/json' -H 'Accept: application/json' -b "cantos_session=$(printf 'a%.0s' $(seq 64))" -d '{"operation_id":"'"$(python3 -c 'import uuid;print(uuid.uuid4())')"'","release_id":"RELEASE_ID","expected_pointer_version":1,"reason":"Local seed change"}'
```

Refresh the Web detail or the Android list: episode 23 disappears and the page ends after 22.
Publishing through the running host is not possible because its delivery adapter is
unavailable; reseed to restore the original catalog.

### Reset

```bash
cargo xtask seed theatre --reset
```

It drops only the database named by the seed record for the same retained cluster and removes
the record; `run --test-cluster` then serves the empty fixture again.

## Safety rules the primitive enforces

- It never reads `DATABASE_URL` or inherited `PG*`/`CANTOS_*` variables, and builds its admin URL
  only from the retained record's loopback port.
- The record must say `retained: true`, name a `cluster-<number>` directory directly under this
  checkout's `target/revision-evidence/` with a `pgdata/PG_VERSION`, and include the Theatre
  fixture. Otherwise it refuses and asks for `cargo xtask verify postgres --keep`.
- Before any write it asks the server for `SHOW data_directory` and refuses unless it is that
  retained `pgdata`. A different PostgreSQL on the same port is never touched.
- It drops only `cantos_test_` plus 32 hex databases named by a seed record for the same cluster
  and port, never a verification fixture named by the run record. The drop uses
  `WITH (FORCE)`, so stop `run … --test-cluster` before reseeding or resetting; a host still
  serving the old seed database loses its connections and must be restarted anyway.

## Troubleshooting

| Symptom | Cause and action |
| --- | --- |
| `seed theatre needs the disposable cluster retained by …` | No `run.json`; run `cargo xtask verify postgres --keep` |
| `REFUSED: retained cluster … is not reachable` | The cluster was stopped or the machine restarted; run `verify postgres --keep` again, then seed |
| `REFUSED: … served by another PostgreSQL data directory` | The port belongs to another server; nothing was changed |
| `UNSUPPORTED … missing psql` | Install the PostgreSQL client and server tools yourself; xtask never installs them |
| `run` says `127.0.0.1:8080 … is already in use` | Another process owns the port; stop it yourself or use `app cantos-theatre --web` against an API you own |
| Empty Home after seeding | The API was started before seeding, or the seed record belongs to an older cluster; restart `run … --test-cluster` and check its first line |
| Catalog became empty after a month | The seeded rights window expired; seed again |
| Play is unavailable | Expected: discovery data only. Playback authorization answers `rendition_unavailable` until #170 is accepted |

## Not established by this tooling

Public media delivery, playback, progress/sync, offline downloads, identity, retained-release
access and any production publication. A seeded catalog is synthetic development data: it says
nothing about real rights, human listening or provider behavior.
