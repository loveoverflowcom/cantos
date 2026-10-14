# Integrated private storage review — 14 October 2026

## Scope and provenance

PR90 was refreshed from `9306d22954b44aa9be390495aa0b2581c0738d22` by merging freshly
fetched develop `c645e498ede50a87434fed6727d28b479109300b` in the clean existing
`/private/tmp/cantos-provider-storage` worktree. The primary checkout and other feature
worktrees were not moved. PR101, PR103 and PR105 were not adopted or merged.

Review covers the final storage feature contribution against that develop revision, its
Catalog/Editorial/Media/Publication consumers, HTTP/session boundaries, migrations, config,
contracts and verification scripts. Inherited UI/native changes are outside this backend review;
ordinary preflight still selects their existing gates because Cargo inputs broaden impact.
[The earlier record](provider-independent-storage.md) retains the original implementation and
failed/unavailable checks. The results here supersede those results for this integrated tree.

Invariant: accepted private audio keeps its original target/key/hash/length, current owner access
is checked after I/O, and object failure or target switching cannot silently retarget receipts,
renditions or publication observations. A private read or format admission grants no public access.

## Review and repairs

Independent Media/storage, HTTP/auth, PostgreSQL and S3/signing passes reviewed construction,
actual-byte admission, conditional writes, exact recorded locations, replay, error translation,
stream/pin bounds and grants. A separate final read-only pass reviewed publication changes.
Final assessment: `no-actionable-findings` within that declared scope, not production acceptance.

- Resolved manifest/lock, module/router, migration registry, architecture checker and docs
  conflicts while preserving merged Studio identity and the Protobuf opt-in/JSON default.
- Kept composition-owned Store and its Studio OIDC handle; private reads now select the correct
  Store-aware cookie namespace. Producer credentials and other surfaces confer no Studio preview.
- Publication resolves the rendition's recorded target. Approval, readiness, publish and active
  reads release initial SQL transactions before object/delivery I/O, then recheck current
  authority and exact rendition/location in the final transaction. Release DTOs and digests stay
  unchanged. Added a real target-switch/no-fallback/history regression.
- Added exact safe StorageError HTTP mapping and Studio-only route/auth regressions. The
  capability enum is available to that test through the narrow facade only in test builds;
  direct domain/adapter imports remain rejected by the architecture checker.
- Corrected the cross-surface negative preview oracle: Theatre keeps its own closed
  `code/retryable` error, while Studio continues to validate its ApiError schema.

Migration registry contains versions1–20 and25. All21 original SQL files were byte-compared
against their published refs:0015 against the original PR90 head; the others against refreshed
develop. No SQL checksum or historical receipt/rendition body was changed. Migration0023 remains
reserved for the separate artwork slice.

## Independent assurance ledger

| Claim / owner | Required by | Actual status and evidence level | Verified scope / residual |
| --- | --- | --- | --- |
| Dependency direction and authority / composition, Media facade | AGENTS and engineering decoupling | PASS; documented independent review, statically-checked | Narrow safe facade; domain and adapters stay behind owning applications |
| Immutable IDs, integrity, owner decisions and ranges / Media domain | Private content reads v1 | PASS; type-enforced, example-tested, property-tested, differentially-tested | Core input/error partitions and independent bounded range oracle; no vendor guarantee |
| Private create/read orchestration / Media and storage | Object-storage architecture | PASS; mock-integration-tested, integration-tested | Production registry with RecordingPrivateStorage doubles; real Unix filesystem and controlled loopback S3 transport/signing/pin/range/cancellation tests |
| Current Studio authority and binary transport / HTTP and read use cases | Private content reads v1 | PASS; example-tested, integration-tested | Studio cookie selection, producer/foreign surface denial, exact bytes, HEAD, range/ETag/error contracts, safe response shape |
| Durable targets, replay, transactions and publication / PostgreSQL and Media | Persistence and publication boundaries | PASS; integration-tested, fault-injected | Additive upgrade, two-writer/replay/rollback, target switch/no fallback, unchanged release snapshot; actual API kill + PG restart |
| Full provider/deployment and playback acceptance | Provider and owning capability gates | NOT_RUN | No provider-live, public delivery, browser/native playback or subjective listening claim |

Final ordinary Rust run passes format, `-D warnings` lint and workspace tests:288 API unit tests,
9 surface-route tests, existing property/controlled-peer tests and type-barrier doctests. Repository
passes typed service/static architecture/link checks and329 Python tests. Their printed Kani fixture
lines are verifier-adapter test output, not execution of a new bounded proof.

The real PostgreSQL17.4 run passes Studio OIDC18, identity9, revisions21, imports9, adaptations21,
production11, Catalog20 plus its restart test, Catalog HTTP47 plus its restart test, Library7 and
private intake21 tests. HTTP/process recovery verifies exact GET/HEAD byte identity, first/middle/
suffix/final ranges,416 and protected303 refresh, then compares observations after API kill and
PostgreSQL restart. The publication target-switch test exercises approval/readiness/publish/active
against an empty new target, then rejects fallback when the recorded object is removed even though
identical bytes exist at the active write target. Its stored release snapshot remains identical.

Publication delivery in these tests uses VerifiedDeliveryFake; it establishes local transaction
and gate behavior, not actual public storage/CDN readiness. S3 tests use controlled peers, not
Long Vân, R2, B2 or a deployed S3 service. Media fixtures are owned synthetic signals outside Git.

## Reproducible commands and retained output

Working directory: `/private/tmp/cantos-provider-storage`. Every verification command uses:

```sh
export RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export TMPDIR=/private/tmp GRADLE_USER_HOME=/private/tmp/cantos-gradle-user-home
export PATH=/private/tmp/cantos-gradle-8.14.3/gradle-8.14.3/bin:/private/tmp/cantos-audio-tools:$PATH
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify repository
cargo xtask verify postgres
cargo xtask preflight --changed-only
```

Installed Rust, Gradle8.14.3, Java24, PostgreSQL17.4 and fixture ffmpeg6.0 were reused; no installer,
Podman service, credential/client provisioning or OS/network/TLS setting was invoked. One heavy
verification lane runs at a time. RUST_TEST_THREADS=1 prevents the known ephemeral-port fixture
flake; explicit concurrent database/request tests still execute their interleavings.

Raw records are under ignored `artifacts/agent-runs/pr90-review/`:
`verify-rust-final-fixed.log`, `verify-contracts.log`, `verify-repository-fixed.log`,
`verify-postgres-fixed.log` and `preflight-final.log`. The PR records the final changed-only
preflight summary. Source/diff fingerprints and independent S3 file hashes are retained with
those logs; each PG run's fixed-name output files are snapshotted before the next run.
Disposable clusters/WAL remain at their unique original paths; their owned servers are stopped.
The inherited Homebrew PostgreSQL server is never used or stopped.

Actual failures are retained: the first new mapper test imported a domain type directly
(`verify-repository.log`), its unconditional test-only reexport then failed strict unused-import
lint (`verify-rust-final.log`), and the old preview test applied Studio's schema to Theatre's404
(`verify-postgres.log`). These test/integration defects were repaired without skipping tests,
weakening warnings, modifying migration bytes or changing a production error contract.

## Deferred acceptance and handoff

| Remaining check | Status / reason | Prerequisite and next action | Next owner / risk |
| --- | --- | --- | --- |
| Live private S3 behavior, anonymous denial, conditional create, versions, grants, CORS and capacity | NOT_RUN; outside authorized local scope | Operator authorization, isolated target and supplied least-privilege secrets; run the architecture's provider acceptance checklist | Unassigned — assignment required before provider activation; controlled peers do not prove vendor behavior |
| Retained historical provider version | NOT_RUN; not implemented | Separately reviewed retained-version capability/migration and provider retention policy | Storage owner; persisted identity is target/key/SHA/length, with a version/ETag pin selected after each current-byte verification |
| Load/latency and read cost | NOT_RUN; unmeasured | Authorized disposable representative workload, EXPLAIN/API measurements and a stated performance budget | Unassigned; full hashing precedes even a small range; indexed point reads do not justify DBSP here |
| Revocation while preview/grant I/O is paused | NOT_RUN as a named race regression | Add a deterministic paused-I/O test at the existing post-I/O reauthorization seam | Media/identity owner; source reauthorization and current-session negative cases are tested, but this exact race is not separately asserted |
| Public delivery, native/browser playback, screen readers, named listening | NOT_RUN; separate capability acceptance | Owning Publication/Listening/client/operator gates and real permitted runtime | Owning issues; development host and private grants remain insufficient for activation |
| Optional Kani/fuzz campaign | NOT_RUN; no changed pure kernel requires a new proof | Separately selected registered kernel/target with installed supported verifier/fuzzer | No new proof/fuzz claim |

PR105 can extend `PrivateStorage::put_if_absent(VerifiedAudio) -> StorageFuture<()>` and
`StorageRegistry::persist` inside Media with a sealed verified image payload and actual MIME,
then reuse verify/open_verified/temporary_read and original-target resolution. Preserve the
32MiB audio envelope, immutable receipts, request lanes and reauthorization; do not construct
fake audio proof for image bytes. S3 currently writes audio/ogg beneath private/audio; adapt
that narrow boundary rather than introducing another S3 adapter. No image feature or migration
is included here.

Residual risk: accepted provider version is not persisted. Changed current bytes fail integrity;
recreated identical bytes remain eligible, and a deleted current object cannot be recovered from
an old version through this API. Issued bearer grants can remain usable until expiry. Mutable
same-UID filesystem access, private orphan retention, global Catalog fencing and production
performance remain operational limits. Real provider, identity/TLS and public isolation acceptance
remain prerequisites; a local PASS does not accept production or the broader listening issues.
