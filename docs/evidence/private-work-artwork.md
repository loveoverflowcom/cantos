# Private Work artwork implementation evidence

## Current integration checkpoint — 14 October 2026 UTC

The separately reviewed storage dependency is now incorporated from develop
`edbd7c7f6f0494e0d01dc5b4139134052b07ed07`, tree
`6bc5900899e44afd706b717756a91867dc54dc75` (PR90 final head
`ea7aee5857577f12752e0d66a55bac08a72a00d3`). Integration preserves the published PR105
checkpoint `cd1097d6bc76e3b584ddaa43516bbe844afb3ea4`, tree
`047f2453535d8223dd209b4a524d4ec871d0fa2b`, as the first parent rather than rewriting its
six logical commits. No unrelated PR is merged by this task.

A dot filesystem snapshot replacement removed the original local worktree, original local
commit objects, tool caches and ignored raw artifacts. The remote checkpoint recovered every
committed product source, fixture, plan and evidence document; no unique unfinished product
code was lost. The original local-to-published commit mapping below remains historical.
October 13 execution claims describe their original inspected/running snapshots; missing raw
logs cannot be rechecked now and their printed PASS rows are not fresh integrated evidence.
New tools and final source evidence are regenerated in the recovered isolated worktree.

### Implemented shared-storage and identity integration

- The closed `VerifiedPayload` carries only actual `VerifiedAudio` or `VerifiedImage` witnesses.
  Integrity and actual Ogg/Opus, PNG or JPEG MIME are derived from inspection. No raw payload,
  forged audio witness, second S3 adapter or caller-selected object key is introduced.
- Image persistence reuses the reviewed registry, private filesystem and S3 adapters,
  conditional-create/readback rules and existing registry/filesystem concurrency bounds.
  Image admission retains its separate two-lane/8 MiB encoded/64 MiB decoded policy.
  Owner-backed, zero-copy transport `Bytes` retain `Arc<VerifiedImage>` until their last clone
  drops, while another owner survives readback. Filesystem blocking ownership survives caller
  cancellation. These are ownership/resource regressions, not a hard CPU/total-RSS proof.
- Stored image target/key/SHA/length become the shared checked `PrivateObjectRef`. Reads use
  the original target; missing targets never fall back to the active write target. Missing or
  mismatched admitted bytes fail as corruption. Typed transport failures remain safe outcomes.
- **No provider version is persisted at admission.** Each S3 read verifies current bytes and
  then pins that read by its observed version/ETag. Recreated identical bytes are eligible;
  retrieval of deleted/noncurrent historical versions and provider retention remain separate.
- Default current Studio artwork routes activate only when existing private storage is
  configured. Every private-artwork composition disables static serving. No legacy, Theatre,
  Admin, producer or public image path is added. Final Studio cookie namespace and session
  header/query fences run before extraction/effects, and application reauthorization remains
  after inspection/storage and before mutation/response.
- The registry is additive and sorted: 22 entries, versions 1–20, 23 and 25. Migration 0023 is still
  this slice's reserved sidecar. Every one of the 22 migration files is byte-identical to its
  proper published parent, including PR90's 0015 and Studio identity's 0020/0025. Nothing is
  renumbered or rewritten.

### Fresh independent-layer execution

All commands below execute on the recovered integrated source. Full new logs are retained in
ignored `artifacts/agent-runs/issue104-integration/`; old missing logs are not substituted.

| Scope | Actual result | Evidence boundary |
| --- | --- | --- |
| Media pure domain | PASS | 86 tests, including actual image/artwork core and reviewed storage identities/ranges |
| PNG/JPEG inspection | PASS | 17 real decoder, malformed/metadata/resource and cancellation cases |
| Image application | PASS | 26 passed; 21 actual-service cases use named repository/storage fakes, five bridge cases exercise real private filesystem bytes; one real SQL case ignored |
| Shared storage | PASS | 54 tests, retaining all reviewed PR90 cases plus eight image regressions; real synthetic filesystem and controlled loopback S3 peers, no live provider |
| Artwork HTTP | PASS | Nine adapter/extractor unit probes plus four actual production-router composition probes; lazy pools do not establish authenticated owner SQL |
| Actual repository gate | PASS | 570 text files/18 skills and 337 Python tests; initial forbidden fixture import and lexical alias findings fixed in source without weakening the checker |
| All API test targets | PASS | Fresh `cargo test -p cantos-api --tests --locked --no-run`, including four new ignored production HTTP/service/SQL/filesystem tests; compilation only |
| Fresh reviewed-base service fixtures | FAIL | Clean detached edbd7c7, own base-built xtask target: 8 passed/17 failed; same private-input checkout guard failures, no feature source and no weakened guard |
| Canonical contracts | PASS | Fresh Script IR reference, Python/prost metadata oracle, 91 Rust contract tests, actual WAV oracle and two Ogg/Opus inspection cases |
| Canonical Rust aggregate | FAIL | Format/all-target workspace lint PASS; API library 376 passed/65 ignored and four artwork composition tests passed; xtask 134 passed/17 known private-fixture failures independently reproduced above |
| Preflight planning | PASS | Actual changed-only dry-run selected all 17 gates conservatively; planning is not gate execution |
| Final preflight/Kani | NOT_RUN | Fresh execution/proof outcomes will be appended after actual completion; the October 13 rows below are historical |
| Disposable PostgreSQL/authorized HTTP/restart | NOT_RUN | Prior Unix-socket startup denial remains unresolved; no retry, alternate TCP, inherited DB or mock substitution |

The first repository run failed the prohibited inline HTTP fixture import and a test alias
ambiguity; both were corrected, and the canonical rerun passed. The four new ignored acceptance
sources call actual production router/service/repository/registry paths, use the original three
synthetic fixtures, assert exact receipts/history/authorization/session fences and a two-PID
Catalog clock/CAS race. Their reopen asserts a new Store/pool/directory descriptor, **not** a
process kill or PostgreSQL restart. They are registered in the existing disposable runner and
remain authored/compiled only while that startup is denied.

The committed historical records identify the denied action as disposable PostgreSQL Unix-socket
creation; the original detailed reviewer rationale/raw event did not survive this restored
snapshot. A permitted disposable executor is still required for upgrade/constraints/grants,
real races/read plans, authenticated HTTP/rights/session acceptance, host-kill and PG-restart
faults. Restored PG17.4 executable availability and Doctor's runnable row do not resolve access.
Live provider, issuer/TLS/native runtime and public delivery remain outside this acceptance.

Independent source review inspected the shared bridge/payload/ownership/MIME, original-target
reads, configured-storage activation, final identity fences and all 22 unchanged migrations.
At 03:00:56 UTC its bounded 47-file path+NUL+bytes+NUL hash was
`13c11606424ca3b4816b3e470cef6202a4bdb22a3a8823ff08453c7607f1a304`.
It found no remaining confirmed source defect after the inline fixture correction and ran no
build, database, provider or proof. This is a source snapshot pin, separate from execution.

A targeted decoder dependency metadata review used the actual normal-edge `cargo tree -p image`
closure (19 locked package/version pairs) and official RustSec database commit
`6aed84668c23d7f535077a9fe918bf1fa0f6a5bc`. Its published records were image's
[RUSTSEC-2019-0014](https://rustsec.org/advisories/RUSTSEC-2019-0014.html) and
[RUSTSEC-2020-0073](https://rustsec.org/advisories/RUSTSEC-2020-0073.html), patched before
pinned 0.25.8, and zlib-rs's
[RUSTSEC-2024-0401](https://rustsec.org/advisories/RUSTSEC-2024-0401.html), patched before
locked 0.6.8. No other published record appears in that pinned decoder closure. The upstream
[image advisory page](https://github.com/image-rs/image/security/advisories) also lists no
published advisory. This is dated metadata inspection, not a whole-lock cargo-audit, complete
vulnerability-absence guarantee, sanitization claim or hard decoder-resource proof. Closure,
advisory-file hashes and versions are retained in `image-advisory-review.json`.

## Historical checkpoint — 13 October 2026 UTC

The following ledger, commands and source identities are retained from the original session.
Their raw ignored artifacts became unavailable after recovery; they are not reasserted as the
current integrated tree's execution. Current activation/dependency state is the section above.

Implementation started 2026-10-13 UTC on `feat/issue104-private-work-artwork`, from refreshed
`develop` `007ae8f0a2d82ead84c5c0b98eecc3b5dece36b3` (PR92). The separately completed
[bounded plan](../work-plan/012-private-work-artwork.md) is retained as design input, rather than
feature evidence. Planning commit `e11184dbad41d1ce4151bbba65d8c439bb6e3bfc` was brought in as
`a37de3f`, resolving only the work-queue append while preserving the identity integration section.

## L1 ownership and invariant ledger

| Claim | Owner / failure prevented | Initial oracle / current scope |
| --- | --- | --- |
| Private cover never grants distribution | Media sidecar, downstream of Catalog; no Theatre/release DTO changes | Review new imports, routers and immutable receipts |
| Caller declarations cannot forge accepted image bytes | Sealed outer `VerifiedImage`; domain metadata is explicitly only validated facts | Constructor/doctest, real full decoder corpus, storage readback |
| One immutable accepted asset per known intent | Media intent/asset identity and later SQL uniqueness | Pure replay partitions then real concurrent SQL |
| Rejected or stale selection changes nothing | Catalog revision authority, one atomic metadata touch and Media ledger/receipt | Pure decisions/CAS laws then two real connections |
| Explicit clear remains a historical tombstone | Media append-only effective change history | Independent history model at exact Work revision |
| Rights sets have representation-independent identity | New validated request preimages only, sorted distinct evidence refs | Independent literal digest and JSON/Proto fixtures |
| Historical access requires current active owner | Catalog owner-work facade; no member/producer authority | Negative application tests, then real sessions/HTTP |
| Decode cancellation cannot reopen a running slot | Owned permit and encoded buffer held through blocking completion | Controlled cancellation barrier plus real decoder inputs |
| Old snapshots, digests and public DTOs stay unchanged | Existing Catalog/audio/publication contract owners | Unchanged old fixture corpus and additive migration oracle |

No new service, crate, provider adapter, public media path, UI, thumbnail or P1 metadata is
introduced. The existing Catalog `Revision` checked factory/successor is exported for its concrete
Media consumer instead of adding a competing sequence authority. The current Media→Catalog
edge already exists; Catalog does not import Media.

## Read inventory and fit decision

The new reads are exact intent/asset PK lookups, original operation PK lookup, exact Catalog
revision and one effective-artwork range lookup (`work_id`, highest revision <= requested).
All are bounded single-resource reads; no listing/search/discovery joins are added. The existing
Catalog global clock remains the simple write fence and list epoch. DBSP is unsuitable for this
small point/range workload absent a measured hot multi-consumer requirement. This is a design
fit decision, not a performance result. Actual application-role timings and `EXPLAIN (ANALYZE,
BUFFERS)` remain NOT_RUN until a permitted disposable PostgreSQL17 environment is available.

## Execution ledger

Results will be recorded per actually executed scope. No planning check, fixture-generator
unit test, compile-only ignored test or mocked result establishes feature integration acceptance.
At original publication, full logs were retained in ignored `artifacts/agent-runs/issue104*/`
records; those October13 artifacts are now unavailable after snapshot recovery.

| Layer / scope | Status | Evidence / gap |
| --- | --- | --- |
| L1 owner/dependency review | PASS | Concrete ownership above, actual repository/import scan and independent read-only source review |
| Clean refreshed-base API library | PASS | 223 passed, 53 ignored on untouched 007ae8f; no new feature code |
| Clean refreshed-base workspace tests | FAIL | Untouched 007ae8f reproduces 17 existing xtask service-fixture failures, 132 xtask tests passed; all failure reasons are the private-input checkout guard |
| L2/L3 domain | PASS | 74 Media domain tests, including 28 new cases and five fixed-seed properties; ten Media compile-fail doctests |
| L2/L3 metadata wire | PASS | 87 contract tests; independent Python/prost/Kotlin exact fixtures and hostile corpus; shared-scanner ordering regression killed the isolated mutant and passes on production source |
| L3 real PNG/JPEG inspection/resource cases | PASS | 17 actual decoder, metadata, hostile framing, limits and cancellation tests; measured subprocess resource cases below |
| L3 numeric Kani proof | PASS | One exact production image-bounds harness, nine checks and 13 satisfied covers; full symbolic input widths, unwind 2, no assumptions/stubs or unreachable allowances |
| L4 actual service with deterministic port fakes | PASS | 21 application tests exercise actual ArtworkService and sealed inspector; fakes do not establish PostgreSQL or actual storage semantics |
| HTTP adapter probes | PASS | Eight actual Axum Router/extractor tests: codec pairs, explicit null, exact receipts, hostile refusal-before-handler, limits/headers, PUT 200, anonymous no-body-poll admission, default/alias refusal and explicit private-host static fallback refusal |
| Ownership/static boundary | PASS | 69 architecture-checker tests plus actual repository scan |
| Canonical contracts/repository | PASS | Actual reference/contract/WAV/audio gates and 334 repository Python tests; final source/evidence rerun passes |
| Final Rust aggregate | FAIL | Format/all-target lint PASS; API 297 passed/63 ignored plus two composition tests PASS; xtask 132 passed/17 reproduced baseline failures |
| Available broad preflight gates | PASS | DBSP, both WASM clients/preview and offline CMP gates execute; JVM XML is 87 + 182 tests, zero failures/errors/skips |
| Selected Script IR mutation gate | FAIL | Before any mutant, isolated cold baseline exceeds existing 60s budget; same failure reproduced from clean base with freshly built base-owned xtask |
| PostgreSQL test-source compilation | PASS | All API test targets compile; nine primitive/constraint cases plus one actual PgArtworkRepository prepare/finalize/set/race case are discovered and ignored, not executed |
| L5 PostgreSQL/authorized HTTP/storage/restart | NOT_RUN | PR90 final reviewed storage dependency and permitted disposable database required |

## Dependencies, safe activation and remaining acceptance

Migration 0023 is reserved for this MVP; refreshed registry contains 18 migrations including 0018,
with 0023 unclaimed. It is additive; published migration bytes/checksums are not changed.

Storage runtime waits for the parent's separately reviewed/integrated PR90 SHA. Reading its
proposed source for interface planning does not adopt or accept its branch. The implementation
must extend that final sealed write capability with `VerifiedImage` and actual PNG/JPEG MIME,
retain audio bounds/receipts and recorded targets, and reuse its filesystem/S3 adapters. No second
S3 implementation or metadata-only storage-success claim is permitted.

Prior disposable PostgreSQL Unix-socket creation was denied. This task does not bypass that
restriction, substitute an inherited/production database or claim SQL semantics from mocks.
A permitted disposable PostgreSQL17 executor is required for the real constraints, upgrade,
two-connection races, actual loopback HTTP/storage and restart/fault checks named by the plan.
The responsible next owner is the parent-coordinated integration/acceptance task.

Routes remain unactivated until the reviewed storage and decoder safety boundary is ready.
Rollback removes/disables only the new route composition or uses the old binary while retaining
additive tables, immutable private bytes and historical targets. No deletion/cleanup is requested.

## Baseline attribution and bounded cache maintenance

`cargo test --workspace --locked` ran on an untouched detached007ae8f checkout before feature
compilation. All 17 existing `xtask::services::tests` failures reproduced the private-input
checkout guard (`private input must stay outside any Git checkout`), including one expected-error
assertion mismatch with that same actual reason. The full gate is FAIL, not waived. New-feature
failures must be compared against this baseline; no ownership/security guard is relaxed.

Before further builds, authorized cache maintenance removed only stopped regenerable Rust
incremental-cache directories from completed Protobuf C2 and C3 targets, reclaiming
2,540,189,758 bytes. The active shared target, source worktrees, Git refs, databases/WAL, evidence
logs/hash manifests and planning artifacts were retained. An exact path/size/file inventory
remains in ignored run records; observed free space after cleanup was 4,740,038,656 bytes.

## Inspector and dependency decision

`image = 0.25.8` enables only `png,jpeg` with default features disabled. Rust 1.87 compiled
the actual code and tests; declared direct/transitive MSRVs in the checked 20-package closure
are at most 1.85. A dated 2026-10-13 review against RustSec advisory-db tree
`6aed84668c23d7f535077a9fe918bf1fa0f6a5bc` found no applicable known advisory in that closure.
The old [image 2019 advisory](https://rustsec.org/advisories/RUSTSEC-2019-0014.html),
[image 2020 advisory](https://rustsec.org/advisories/RUSTSEC-2020-0073.html) and
[zlib-rs advisory](https://rustsec.org/advisories/RUSTSEC-2024-0401.html) patched ranges exclude
the actual locked versions. `cargo-audit` was NOT_RUN. This is a dated known-advisory check,
not a future-vulnerability or memory-safety guarantee.

The [pinned PNG source](https://github.com/image-rs/image/blob/v0.25.8/src/codecs/png.rs)
receives limits before parsing ancillary metadata. JPEG dimensions/output are checked before
pixel decode. Every accepted file fully decodes and retains exact original encoded bytes.
APNG, truncated headers/pixels, appended payloads/second images, unsupported format, MIME
spoofing, nonidentity JPEG EXIF, malformed/duplicate/late PNG ICC, hostile dimensions and
16-bit actual sample-layout overrun are refused. `png 0.18.1` can ignore an ICC decompression
error, so presence of iCCP also requires a successfully parsed bounded profile.

`max_alloc` is best-effort and is not a total-process RSS/CPU cap. Limits guarantee the checked
encoded body, dimensions, pixel/output layout and two admitted decode/body lanes. Decoders may
allocate codec/metadata buffers separately; accepted ICC is briefly cloned. Subprocess results
include fixture construction and harness overhead, with no latency/SLO claim:

| Actual case | Wall seconds | Peak RSS KiB |
| --- | ---: | ---: |
| Exact 64 MiB RGBA8 pixel output | 2.1989 | 77,712 |
| Hostile ICC metadata overload | 3.9016 | 78,564 |
| Huge dimensions / 16-bit rejection | 0.0105 | 12,036 |
| Exact 8 MiB encoded input | 0.7672 | 36,636 |
| Controlled blocking cancellation | 0.0172 | 15,496 |

## Independent application/source review

The actual service was tested against deterministic metadata/storage ports with exact clocks,
ordered effect traces, real synthetic PNG/JPEG decoding and controlled I/O barriers. The 21
cases cover prepare/admit/read/preview/select/replace/clear/history; original historical receipt
replay; active-owner/member/session/rights failures; digest/type/count refusal; private orphan
and unknown-outcome reconciliation; stale CAS; reauthorization after object verification; equal
admission on a changed active target; preview integrity/stream cleanup and cancellation.

A concrete finding was fixed: encoded hash/count were originally checked only after expensive
decode. The ticket now captures the reauthorized immutable expected facts and rejects empty,
oversize, hash/count mismatches before inspection, retaining independent inspector derivation
and fresh authoritative intent checks afterwards. Its negative regression passes.

Independent read-only review found no further functional defect in the inspected core, Catalog
facade, Media persistence and application transaction boundaries. This is source review, not
SQL or object-storage execution. The ignored primitive artwork-vs-Catalog race tests composed
facade/adapter primitives; they must not be described as production repository-path acceptance.
Final target/version resolution and actual shared storage readback remain IO_ACCEPTANCE_PENDING.

Further read-only review found the PUT admission status differed from the pre-change plan.
The production response helper now returns 200 for admission and equal replay; prepare remains
201. An exact JSON/Protobuf status regression passes. The explicit artwork-host constructor
also disables filesystem static fallback, with a real synthetic-file route regression; ordinary
hosting remains unchanged. Anonymous requests to all eight trusted mount methods are refused
before a panic-on-poll body is read. These probes use lazy pools, no database or object adapter.

The reviewed seven-file HTTP/contract/test snapshot was independently pinned as SHA-256
`3ace785eb7d7ee4ab6746899d29390721c5f3d9d39aafb8d59905f72a00d162e`
(sorted UTF-8 path + NUL + bytes + NUL). The artwork HTTP adapter is
`405f5e2bcef78cf0b546ddee863a0fe98a13c2a17517f13d0ba953fd3a6cfec7`;
the host composition is `fb81195ce013eb62db9404b9de2459e2044fb4932f8e93760a1ebafeccce29d3`.
This pins a read-only review, not execution of SQL, storage or a symbolic proof.
An editorial contract run-count wording refresh followed that snapshot without a behavior
change. The actual repository scan caught two test-only imports of PostgreSQL configuration
inside the HTTP adapter. Both composition probes were moved to the auto-discovered integration
test, preserving their behavior and the unchanged ownership checker. Its SHA-256 is
`6b2a0a30c647d427576e3948482a9c15b2c45e6cd493b595a3b90e10412e0277`.

The additional ignored repository-path case calls the actual `PgArtworkRepository` prepare,
equal receipt replay, full tiny PNG inspection, finalize/replay and select/replay, then races
its clear transaction against a real Catalog edit. Its synthetic SQL location is explicitly
not storage evidence. It closes source coverage beyond primitive composition; real execution
and its resulting semantics remain NOT_RUN.

## Aggregate local gates and resource accounting

The canonical `cargo xtask verify contracts` rerun is PASS for Script IR reference, independent
Protobuf reference/prost generation, contract tests, WAV reference and actual audio inspection.
The first WAV reference attempt failed because the primitive expects the worktree's default
`target/debug/inspect-wav` while Cargo used the existing shared warm target. A file-specific
ignored symlink now points to the actual Cargo-built binary; no source/guard was changed.

The complete Rust gate reports PASS format and all-target workspace lint with `-D warnings`,
and FAIL workspace tests. On the source before the two-probe test relocation, API library tests
were 299 passed/63 ignored; xtask was 132 passed/17 failed. The same 17 failures were reproduced
on untouched develop; a new stale six-kernel status assertion was separately found, corrected
to derive from `Kernel::ALL`, and the 13 focused Kani workflow/CLI/status tests pass. The guard
is unchanged. Read-only empty `.git` markers under `/tmp`, `/workspace/shared` and `/workspace`
make all allowed writable temporary roots fail its outside-any-checkout rule. No private test
fixture was relocated outside the approved filesystem roots.

The first changed-only preflight selected every gate because Cargo.lock is shared and reported
two CMP steps UNSUPPORTED with Gradle absent from PATH. A supported installed C4 Gradle8.14.3
and JDK21 were then selected; no install occurred. That rerun exposed the two inline test import
violations above. The repository gate after relocation is PASS, including 334 Python tests;
later preflight results are recorded separately and do not erase either fail-before log.
PostgreSQL remains explicitly NOT_RUN, skipped to respect the earlier socket-startup denial.

After all Cargo consumers stopped, a second authorized bounded cleanup removed only the
completed shared target's regenerable `debug/incremental` cache: 16,525 files,
3,534,484,715 file bytes, with 3,572,076,544 filesystem bytes reclaimed. Observed free space
rose from 2,677,788,672 to 6,249,865,216 bytes. Source, refs, logs, manifests and DB/WAL were
preserved. Exact process/path/size inventories remain in ignored run records.

## Actual numeric proof

`cargo xtask verify kani --harness image-bounds` executed on clean source
`98f81365c6c52a9fc743d0ed846abd3b88b49798` (tree
`bc6191f638c00f1058fa4ee8a938b1feb4ead29e`), using Kani 0.68.0, CBMC 6.11.0,
the pinned nightly-2026-08-21 x86_64 compiler, CaDiCaL and unwind 2. It PASSed one exact harness,
nine checks and all 13 covers in 96.4 seconds, with unchanged source/model, lock, approved
configuration and clean Git identity. No proof assumption, stub, excluded safety check or
unreachable allowance is used. Details and retained-output identities are in the
[Kani assurance record](../assurance/kani.md#private-work-artwork-numeric-admission).

The proposition calls the production `check_image_bounds` kernel with arbitrary full-width
u64 encoded length, u32 dimensions and u8 actual sample-layout size. A separate widened u128
specification pins the declared envelope and exact resulting pixel/output counts, including
valid minimum/maxima and each refusal partition. This is numeric admission evidence only.
It does not prove decoder implementation/memory behavior, byte authenticity, rights,
authorization, SQL isolation, historical storage resolution, HTTP or provider execution.

An independent read-only proof audit matched all 224 model-file hashes, current and committed
kernel source, lock/config/absence identities and both inner/outer verifier results. Nine
checks include four admission/count refinement assertions plus overflow/compiler-unreachable
safety properties; they are not nine application assertions. The thirteen covers are all
satisfied, with no UNREACHABLE result. The claim does not include exact error-variant precedence.
Raw compiler-generated `assume(false)` paths are checked-safe unreachable Rust match paths,
not authored restrictions on symbolic inputs. Two pinned-nightly macro-semicolon warnings
remain in the raw log. Metadata SHA-256 is
`c4a70659a07ce6442e2f536915daab76b5afd37512e5d9d3a4dc89a67f7d35b4`.

## Verified Git-data publication checkpoint

Ordinary Git push failed because the CLI lacked GitHub credentials (terminal prompts disabled).
The authorized Git-data connector then uploaded code/docs/fixtures with exact blob SHA checks,
created byte-identical trees and mapped the ordered parent chain without force pushing.
Every remote commit's tree and parent list was read back; the new branch HEAD was checked and
successfully fetched by ordinary Git. The initial published checkpoint is
`a0520ed96a1f3c45c428ac78e6b05b2b9e2d582a`, tree
`bc6191f638c00f1058fa4ee8a938b1feb4ead29e`, identical to the clean local proof-execution tree.

| Local commit | Published mapped commit | Exact unchanged tree |
| --- | --- | --- |
| `a37de3f4598e2ab1e64215cb03a681d19f3e68a7` | `53bceca1383837b2bbbbe889d8ac0e018bda01ec` | `7152c42356eab992801f182f7b36fd51e5ed3864` |
| `002c14067cd0f227f0ac9003cd23c85367cd4bfe` | `87f3cf5882f91f1d00d5adc879ef57503b3e3605` | `e8630a865ffe38b461a7ab640876f8ee1fd2c714` |
| `2ca8f266dd38f0b2deb87dd40c3a99e0d3bd67f8` | `bda10e7d4dd3414fd1c4bf86b376405904cdb2cd` | `904148f6768f5bde0c271139a1ad468b384e4295` |
| `e05711b6c7511282199a4ca4b283f89e876bc471` | `1fb1203972fb597af8ba214bbe192eac645f8450` | `a0ead7a642c72ef3ab60b9f8d0af0be35995487b` |
| `98f81365c6c52a9fc743d0ed846abd3b88b49798` | `a0520ed96a1f3c45c428ac78e6b05b2b9e2d582a` | `bc6191f638c00f1058fa4ee8a938b1feb4ead29e` |

The first mapped parent is refreshed develop `007ae8f0a2d82ead84c5c0b98eecc3b5dece36b3`;
each subsequent parent is the preceding mapped commit. Committer/timestamps differ, so local
and published commit SHAs are intentionally distinct. Later evidence-only fast-forward updates
use the exact expected-head guard and retain this source/proof provenance. Draft publication is scoped to one PR targeting develop, referencing rather than closing #104,
and keeping I/O acceptance/merge gates open.
Remote CI is not checked or awaited.

## Final broad-gate results

After relocating the probes, `cargo xtask verify rust` was rerun on the final product source:
format/lint PASS; ordinary API library 297 passed/63 ignored and both production-router probes
PASS; xtask 132 passed/17 failed, the reproduced baseline guard failures. Final repository
verification is PASS with 334 Python tests. These are actual statuses, not a waived Rust gate.

`cargo xtask preflight --changed-only --skip rust-tests --skip postgres` kept the original Rust
FAIL and the denied PostgreSQL gate explicit while running independent available scopes:
repository, format, Script IR/protobuf reference, all-target lint, DBSP, WAV/audio inspection,
both WASM clients, Leptos preview, CMP UI-system and Theatre desktop tests all PASS. DBSP's
actual API suite is 302 passed/63 ignored, including the real relocated router tests and
compile-fail type barriers. Current JVM XML is 87 UI-system and 182 Theatre tests, with no
failures/errors/skips; the latter includes the three new commonTest artwork-fixture cases.
WASM/JVM build/test evidence is not browser/native runtime or accessibility acceptance.

The installed C4 Gradle 8.14.3/JDK 21 cache was reused through the completed integration's
execution-only offline wrapper, content-restricted verified Maven cache and process-local
bundled Noto font configuration. Cached dependency hashes match provenance; two workers,
in-process Kotlin and no configuration cache were used. No software install, dependency
verification bypass, system font/security change or download was performed.

The preflight aggregate remains **FAIL** because the selected Script IR mutation primitive
exceeded its fixed 60s isolated cold-build baseline budget before executing any of its nine
mutants. The four Script IR input files are unchanged. An initial shared-target attribution
attempt was discarded because xtask embeds its root at compile time. A freshly compiled
base-owned xtask then pinned the actual untouched 007ae8f path; the same canonical mutation
gate reproduced the 60s baseline timeout there, with clean source before/after. Neither the
timeout nor assertions were weakened, and no mutation PASS is claimed. Independent wire
mutant kills and the actual numeric Kani proof remain separate evidence.
