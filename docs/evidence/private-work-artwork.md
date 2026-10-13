# Private Work artwork implementation evidence

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
Full logs are retained in ignored `artifacts/agent-runs/issue104*/` records.

| Layer / scope | Status | Evidence / gap |
| --- | --- | --- |
| L1 owner/dependency review | PASS | Concrete ownership above, actual repository/import scan and independent read-only source review |
| Clean refreshed-base API library | PASS | 223 passed, 53 ignored on untouched 007ae8f; no new feature code |
| Clean refreshed-base workspace tests | FAIL | Untouched 007ae8f reproduces 17 existing xtask service-fixture failures, 132 xtask tests passed; all failure reasons are the private-input checkout guard |
| L2/L3 domain | PASS | 74 Media domain tests, including 28 new cases and five fixed-seed properties; ten Media compile-fail doctests |
| L2/L3 metadata wire | PASS | 87 contract tests; independent Python/prost/Kotlin exact fixtures and hostile corpus; shared-scanner ordering regression killed the isolated mutant and passes on production source |
| L3 real PNG/JPEG inspection/resource cases | PASS | 17 actual decoder, metadata, hostile framing, limits and cancellation tests; measured subprocess resource cases below |
| L4 actual service with deterministic port fakes | PASS | 21 application tests exercise actual ArtworkService and sealed inspector; fakes do not establish PostgreSQL or actual storage semantics |
| HTTP adapter probes | PASS | Eight actual Axum Router/extractor tests: codec pairs, explicit null, exact receipts, hostile refusal-before-handler, limits/headers, PUT 200, anonymous no-body-poll admission, default/alias refusal and explicit private-host static fallback refusal |
| Ownership/static boundary | PASS | 69 architecture-checker tests plus actual repository scan |
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
