# Theatre seeded catalog: PostgreSQL → API → Web/CMP integration record

Source: branch `feat/theatre-seeded-catalog-integration` from `develop@0d6725a`, 2026-10-17.
Scope: the development seed ([runbook](../deployment/theatre-seeded-catalog.md)), Theatre Web
live Work detail and the CMP production catalog adapter. This is an observation record for that
source; it is not production publication, playback, deployment or native-device acceptance.

## Environment actually used

The development machine has no PostgreSQL client or server binaries, so `cargo xtask verify
postgres` is `UNSUPPORTED` there. For this record the agent started two **disposable, labeled,
loopback-only Podman containers** from the already present `postgres:17.11-bookworm` image
(PostgreSQL 17.11) and removed them afterwards:

- `cantos-agent-seed-pg` (127.0.0.1:55433, tmpfs data) served as the disposable cluster for the
  seed and `catalog_http_postgres` tests (`CANTOS_TEST_CLUSTER_URL` with `cantos_test_admin`).
- `cantos-agent-retained-pg` (host network, 127.0.0.1:55434) ran with its `PGDATA` bind-mounted at
  the same absolute path under `target/revision-evidence/cluster-<n>/pgdata`, so
  `SHOW data_directory` matched a retained-cluster record. That record was written by hand to
  emulate `verify postgres --keep`; `psql` was a shim running the container's `psql` 17.

Ports 8080/8082 were occupied by a pre-existing local stack that the agent did not own, so
`cargo xtask run cantos-theatre --web --test-cluster` correctly refused to start and was not
observed serving. The same `cantos-server` binary was started directly on `127.0.0.1:8090`
(`CANTOS_ENV=development`, default composition, `CANTOS_WEB_DIST` = the Theatre `trunk build`
output), serving Web and API from one origin.

## Results by layer

| Layer | What ran | Result |
| --- | --- | --- |
| CONTRACT | Shared Rust DTO strict decoding in the seed test; Web decoders against `packages/contracts/fixtures/theatre/discovery/v1`; CMP DTO drift test against the v1 schema | PASS (see CMP row for its own result) |
| POSTGRES | `theatre_development_catalog_seed` against PostgreSQL 17.11 (41.5 s): 7 Works, 34 releases through Studio HTTP, app-role SQL, actual Opus intake, reviews, clearance, approval, publish and withdraw; asserts shelf order, Work pagination, unit walks with gaps, withdrawn/private/unpublished `not_found`, manifest identity, `rendition_unavailable` playback, invalid cursor | PASS |
| POSTGRES | Full `catalog_http_postgres` suite (existing walks, publication, audio, brief tests plus the seed) on the same server | PASS: 52 passed, 1 skipped by the runner's own `--skip catalog_http_recovery_after_server_restart` (161 s) |
| XTASK | `cargo xtask seed theatre` end to end on the retained-path server: seed, reseed (same Work/unit IDs, 34 new publication IDs, previous seed database dropped), `--reset` (seed database dropped, record removed) | PASS |
| XTASK | Refusals: no retained record (usage refusal, exit 2); record pointing at a real PostgreSQL whose `data_directory` differs (`REFUSED`, nothing written) | PASS |
| HTTP | Real socket on 8090: `/works?limit=20` (4 Works, `no-store`, `nosniff`), podcast units 20 + cursor, 404 `not_found` for withdrawn/private/unpublished/absent Works | PASS |
| HTTP | Documented Studio withdrawal of episode 23 through the running host (pointer 1 → 2); public list then ends at 22 and the old manifest is `not_found` | PASS |
| BROWSER | Built-in browser on `http://127.0.0.1:8090/`: Home shows the 4 seeded Works; card → detail by `work_id`; 20 episodes then **Tải thêm tập** → 23 with focus on episode 21; after the withdrawal a reload shows 22; story with a withdrawn chapter shows 2 rows; withdrawn/private Works show the merged "not public" copy; `en-US` Work keeps `lang="en-US"`; API stopped → "Chưa mở được tác phẩm" + **Thử lại**, no mock data; API restarted → Retry recovers; Back restores focus to the opening card; Listen disabled with `aria-describedby`; 375 px has no horizontal overflow; `?mode=mock` still shows only fixtures | PASS (interaction-tested; not a full visual/a11y matrix) |
| CMP / JVM | `cargo xtask theatre-demo verify`: shared 197 tests (drift check against the v1 schema and fixtures, domain, reducer, controller, adapter, Compose JVM renders), sandbox 218 tests; JVM transport against a JDK loopback server (mock-integration) | PASS |
| CMP / HTTP | `CANTOS_THEATRE_LIVE_ORIGIN=http://127.0.0.1:8090 cargo xtask theatre-demo verify`: the production Kotlin adapter, JVM transport and controller walked the real seeded API (4 Works, 30 units after the withdrawal); its per-Work `work_id`/kind/count/first/last `unit_id` lines equal an independent Python walk of the same API and the rows seen in the browser | PASS |
| ANDROID / packaging | `cargo xtask theatre-host android-release` (release boundary and artifact inspection: no sandbox/mock/test content, no INTERNET); `cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080` built the debug APK, whose `aapt2` manifest has INTERNET, `CantosApiOrigin` and the loopback-only network security config; a non-loopback `http://192.168.1.5:8080` origin is refused with exit 2 | PASS (static packaging only) |
| ANDROID / runtime | No device or emulator attached and no system image installed; install, `adb reverse`, launch, TalkBack and visual checks did not run | NOT_RUN |
| IOS | No Xcode or device on this Linux machine | NOT_RUN |
| REAL_MEDIA | Public playback delivery is not composed by the host; playback authorization answers `rendition_unavailable` | NOT_RUN (by design, #170) |

## Review fixes after these observations

The PR #226 review found and fixed, with failing-first regressions: a multi-page walk after an
`invalid_cursor` restart did not derive Web `unavailable` rows; the CMP reducer appended ids it
had not shown to the end after a restart instead of server order; Web showed stored corruption
(`500`, `retryable:false`) as a connection problem; a deep-linked Web detail did not reset
Home's scroll on Back; a blocked unsafe link left the detail loading; CMP admitted an `[::1]`
debug origin that its network security config rejects. These changes are covered by unit,
JVM and WASM build checks; the browser walk and the live JVM walk above were not repeated.

## Not established

Production publication or delivery, playback, progress/sync, offline, identity, retained
releases, Android/iOS runtime behavior, screen readers, 200% text, light/dark visual review and
the W01–W03 / M01–M06 visual matrices. The `repository` gate fails identically on a clean
`develop@0d6725a` checkout (stale Leptos/CMP preview qualifications), so its result here is not
attributable to this change.
