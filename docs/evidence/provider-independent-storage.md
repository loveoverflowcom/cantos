# Provider-independent storage and verified private reads

This is the historical 13 October implementation record. The
[14 October integrated review](provider-independent-storage-review.md) records current
develop integration, repaired compatibility tests and fresh local results; its results
supersede the original failures and unavailable tools below for that reviewed tree.

## Scope and baseline

Implementation handoff for provider-independent private Media storage, exact Studio audio
preview and retained Catalog/Editorial/Ingestion reads. Baseline: `origin/develop`
`7e9a7ec934d027682ac866ac90d86fdf3efc82e4`, fetched on 2026-10-13 before safely fast-forwarding
an unoccupied local `develop` and creating the isolated `feat/provider-independent-storage`
worktree at `/private/tmp/cantos-provider-storage`. The original checkout and its local work,
existing databases, media and evidence were preserved. A later fetch observed upstream
`7f02e65`; its unrelated CMP changes are not part of this PR's merge-base diff.

This record distinguishes implemented behavior from executed verification. It does not accept
production deployment, provider performance, publication, listener playback or subjective audio
quality. No real provider credentials, Long Vân/R2/B2 calls, live upload, bucket policy change,
paid operation, Podman service or installation was used. All media fixtures are synthetic and
remain outside Git.

## Invariant and verification inventory

| Claim | Production owner | Evidence and limit |
| --- | --- | --- |
| Immutable private keys/digests/lengths, typed ranges and owner decisions | Media `domain/object_storage.rs` | Deterministic unit/property tests, independent range oracle; no network/database |
| Actual bytes precede accepted receipt, equal retries reconcile, conflict does not overwrite | Media application and `PrivateStorage` | Existing intake/retry/crash tests, filesystem atomic-link tests, controlled S3 HTTP peers |
| Private directory/inode safety survives range reads and cancellation | Filesystem adapter | Real owned Unix fixtures; same verified descriptor, bounded blocking permits retained by task |
| S3 conditional create, exact streamed readback, response pin and strict ranges | Generic S3 adapter | Actual adapter against loopback controlled responses; official independent SigV4 goldens; **not provider-live** |
| Historical locations survive active write-target changes | Target registry + migration 0015 | Real registry with deterministic doubles and real PostgreSQL/two filesystem targets; no migration engine |
| Authorization and exact revision remain authoritative | Catalog/Editorial/Ingestion and Media application | Existing exact revision/owner regressions plus current owner preview tests; no mutable-head substitution |
| Binary delivery has bounded I/O, correct Range/ETag and separate auth | Studio audio-read boundary | Real Router/PostgreSQL/filesystem requests and native TCP/restart probes; after-header failures terminate body |
| Theatre cannot expose unpublished media | Reserved Theatre router | Wrong-surface preview tests; approved publication/listener authorization remain dependencies on #11/#12 |
| No platform-to-context shortcut | Concrete composition + owning Media read facade | Independent review found new imports; corrected without allowing domain/adapter imports from HTTP/platform |

## L1–L5 ledger

Working directory for every command below: `/private/tmp/cantos-provider-storage`. Rust is
`RUSTUP_TOOLCHAIN=1.87.0`; Python/mutation runs use `TMPDIR=/private/tmp`. The available fixture
`ffmpeg`/`ffprobe` directory is added to PATH. Local socket/subprocess/PostgreSQL tests run outside
the workspace sandbox with explicit reviewed escalation; all peers/processes/data are owned
synthetic fixtures. Logs are retained under ignored `artifacts/agent-runs/provider-storage/`.
`commit5-source-snapshot.json` records the baseline, HEAD plus dirty state and 273 source hashes;
its code matches the subsequently committed read implementation plus final regression tests.

| Layer/check | Actual status | Scope and retained evidence |
| --- | --- | --- |
| L1 ownership, dependencies, authorities | PASS | Independent source review; static repository checker validates 480 text files/18 skills in `commit5-repository-fixed.log`; narrow facade checker regressions pass |
| L2 pure identities/integrity/access/ranges | PASS | 12 deterministic/property domain tests, `l3-domain-final.log`; private constructors and exhaustive typed outcomes |
| L3 Rust format/lint/workspace tests | PASS | `cargo xtask verify rust`, `commit5-rust-serial.log`: 199 API unit tests, including 46 storage tests and two HTTP transport helpers; full workspace tests run |
| L3 repository gate | FAIL | `cargo xtask verify repository`, `commit5-repository-fixed.log`: static checks PASS; 304 Python tests end with 37 errors/one fallout failure in baseline Darwin child-group cleanup |
| L3 contracts/reference/actual codec inspection | PASS | `cargo xtask verify contracts`, `commit5-contracts.log`: script-ir-reference, contract-tests, wav-reference, audio-inspection all PASS; synthetic fixtures and existing tools |
| L4 production adapter and orchestration | PASS | Controlled loopback S3 responses + real filesystem + production registry/deterministic doubles; full Rust/PG logs; this is mocked S3 acceptance, not real provider evidence |
| L5 additive migration/real DB/API/filesystem | PASS | `cargo xtask verify postgres`, `commit5-postgres.log`: fresh owned cluster; 21 audio-intake tests, target switching, migration 0015 compatibility, owner preview/Range/headers; actual TCP before/after API kill + PG restart |
| L5 real Long Vân/R2/B2 or disposable Podman S3 | NOT_RUN | Not authorized; no credentials or service startup used |
| Kani bounded proofs | NOT_RUN | Not required for changed storage slice; no proof claim |
| Production latency/throughput/CORS/presigned expiry | NOT_RUN | Requires operator/provider/browser acceptance |

Exact command prefixes used for commit-5 checks:

```sh
RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 TMPDIR=/private/tmp PATH=/private/tmp/cantos-audio-tools:$PATH cargo xtask verify rust
RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 TMPDIR=/private/tmp PATH=/private/tmp/cantos-audio-tools:$PATH cargo xtask verify contracts
RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 TMPDIR=/private/tmp PATH=/private/tmp/cantos-audio-tools:$PATH cargo xtask verify postgres
RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask verify repository
```

Mock scope is explicit: controlled S3 peers exercise the production S3 adapter/signing/transport,
not a whole Studio-to-S3 deployment. Registry tests exercise the production registry with named
`RecordingPrivateStorage` doubles; PostgreSQL/application/Router/TCP tests use the actual
filesystem adapter. No test claiming a fabricated successful fake as end-to-end provider evidence
is included. A real Studio-to-provider acceptance campaign remains `NOT_RUN`.

Initial failures remain evidence. `commit5-repository.log` exposed new forbidden imports and a
misresolved local alias; the composition/Media facade correction fixes these and static checks
pass on rerun. `commit5-rust.log` and `commit5-rust-rerun.log` failed only the unchanged xtask
`probe::tests::port_ownership_reflects_an_existing_listener` fixture at port release. Serial
fixture scheduling (`RUST_TEST_THREADS=1`) passes the whole gate without skipping or changing
an assertion; tests that explicitly exercise concurrency still do so. Earlier default sandbox
runs could not bind local sockets or execute the owned-process probes; the approved escalated
runs supersede those environment-restricted results for the named checks.

Independent final code review: BASE `7e9a7ec934d027682ac866ac90d86fdf3efc82e4`,
HEAD `4ae75170f6c751efa2968f4305408636a75fb775` plus the exact dirty read/regression snapshot;
review fingerprint `b09aedf54385fa9378953172631f91249db05b74b5e7039c4563e38ee8359f42`.
Assessment: `no-actionable-findings`. Corrected findings were case-insensitive/unknown HTTP
range units and platform-to-Media dependencies. Coverage is complete for implementation,
contracts, migrations, configuration and verification scripts; the final documentation
alignment is checked separately. No Kani proof or real provider/performance acceptance is claimed.

## Final changed-only preflight

`final-preflight.log` is the original unskipped
`cargo xtask preflight --changed-only` run. `Cargo.lock` broadens impact to every gate.
Its exact outcome is **UNSUPPORTED**: `cmp-ui-preview` and `theatre-cmp-demo` lack installed
Gradle 8.14.3. All other gates are **NOT_RUN**, because xtask checks prerequisites before
executing any step. This is separate from the independently executed repository gate **FAIL**.
No Gradle, service or toolchain is installed to manufacture passing evidence.

The available-gates follow-up retains that original outcome and the repository failure:

```sh
RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 CARGO_BUILD_JOBS=2 TMPDIR=/private/tmp PATH=/private/tmp/cantos-audio-tools:$PATH cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview --skip theatre-cmp-demo
```

The skipped gates remain **NOT_RUN** in that run. Source: commit 5 `79ab5a1` plus the final
registry/PostgreSQL regressions; code is unchanged from the independently reviewed/tested
snapshot. The following is the exact completed `final-preflight-available.log` summary:

```text
preflight summary
  NOT_RUN      repository                                skipped by --skip
  PASS         rust-format
  PASS         script-ir-reference
  PASS         rust-lint (workspace)
  PASS         rust-tests (workspace)
  PASS         dbsp
  PASS         wav-reference
  PASS         audio-inspection
  PASS         web-lint (cantos-studio, cantos-theatre)
  PASS         web-build (cantos-studio)
  PASS         web-build (cantos-theatre)
  PASS         ui-preview
  NOT_RUN      cmp-ui-preview                            skipped by --skip
  NOT_RUN      theatre-cmp-demo                          skipped by --skip
  PASS         mutation
  PASS         postgres
preflight: PASS with 3 explicitly skipped (NOT_RUN) step(s)
```

This establishes the available gates, including DBSP, Web lint/build, preview build, mutation
and a second fresh PostgreSQL run. It does **not** make the original preflight or the repository
gate pass. Web/preview builds do not establish browser interaction or native runtime evidence.
Independent final documentation review also reports `no-actionable-findings`; coverage is
complete for the 11 changed documentation/hand-off records. Final diff/whitespace checks pass.

Final `cargo xtask verify repository` output is retained in `final-repository-fixed.log`:
services/static architecture/links pass (480 text files/18 skills), then the same 304-test Python
suite fails with 37 baseline Darwin errors and one fallout failure. `final-repository.log`
retains the preceding ledger-summary trailing-whitespace failure; that formatting was fixed.
Tool versions observed: rustc 1.87.0, Python 3.13.5, PostgreSQL 17.4 (Homebrew), Trunk 0.21.14.

## Database read inventory and performance decision

New preview authorization reads an exact UUID-scoped accepted intent/receipt or a
Catalog-authorized exact submission/rendition and its persisted location. It performs a short
share-locked authority read, commits before object I/O, and rechecks the same immutable scope
before returning a stream or grant. Existing primary/unique indexes and relational bindings
remain the lookup path; no aggregate, listing or new scan/query family is introduced.

Expected consumer: Studio creator preview/seek and refreshable access. Request frequency,
production data volume and latency/throughput are **unmeasured**. DBSP is rejected for this slice:
indexed exact point reads require current authorization and immutable bindings, not incremental
aggregation. There is no new experimental outbox/read-model consumer. PostgreSQL correctness
and concurrency tests do not establish a production performance budget. A separate authorized
large-fixture EXPLAIN/load/connection-concurrency/latency test is `NOT_RUN`; assignment is
required before production acceptance.

Hash verification is bounded-memory but can read the whole object before a small range. Current
assets are limited to 32 MiB, operation lanes are bounded and transport/response deadlines are
finite. This cost, provider download billing, slow clients, backpressure and browser CORS require
real acceptance; mocks do not prove streaming performance.

## Compatibility and operational limits

Migration 0015 adds `storage_target_id` to Media receipt/rendition rows with
`legacy-filesystem` defaults. Existing serialized snapshots, original checksums, receipts and
migrations 0010/0014 are unchanged; old filesystem layout remains readable. Migration 0016 is
reserved for parallel Publication work. Bounded canonical text (65,536 UTF-8 bytes), import
originals (1 MiB) and Script IR exports (2 MiB) remain in PostgreSQL under their existing owners.

`CANTOS_AUDIO_ROOT` retains legacy development behavior; `CANTOS_STORAGE_CONFIG` selects
strict non-secret version-1 TOML and is mutually exclusive with it. Historical target IDs and
locations must remain configured and stable. Changing the active write target performs no copy.
See the [configuration example](../../services/cantos-server/config/storage.example.toml),
[read routes and wire behavior](../../services/cantos-server/contracts/studio/private-content-reads/v1/README.md) and
[architecture/provider security matrix](../architecture/object-storage.md).

The existing host still accepts only the reviewed local/development runtime with loopback
PostgreSQL; an S3 adapter does not create production TLS/identity/deployment support. Private
intake may use one private bucket. Future publication must review policy/physical isolation and
must never make private prefixes public to work around a provider limitation.

## Required follow-up acceptance

- Baseline Darwin verifier cleanup: `scripts/verify_kani.py::bounded_run` can receive `EPERM`
  when signalling an exited, unreaped child group. Repository Python tests and preflight fail
  on this Mac even outside the workspace sandbox. An EOF/waitid check alone is unsafe because
  a live child can close its pipe. Do not suppress the error or weaken cleanup. Unassigned —
  assignment required before clean repository acceptance; rerun `cargo xtask verify repository`
  and `cargo xtask preflight --changed-only` after a separately reviewed fix.
- Real S3-compatible acceptance: operator authorization, isolated target, least-privilege secret
  injection and cost limit. Apply the [provider checklist](../architecture/object-storage.md)
  for conditional PUT, actual bytes, HEAD/GET, ranges, version-pinned grants/expiry, CORS,
  timeout/concurrency/performance. Unassigned — assignment required before activation.
- Publication #11 and Theatre #12: immutable approved manifest and independent listener access
  decision before public routes. No fake publication/entitlement or Studio-cookie reuse.
- Migration/retention: copy → verify actual bytes/hash → register ready location → controlled
  read switch → retention/rollback. No replica registry or orphan collector is implemented.
- Browser playback/seeking, named listening, Android/iOS native audio, accessibility and
  production identity/TLS: `NOT_RUN`, outside this backend slice. No mobile/UI code changed.

GitHub execution status belongs to the requested draft PR; remote CI is not waited for and is
not claimed as verified.

## Changed files

The merge-base feature diff changes these 49 paths; generated media, fixtures and raw logs are excluded.

```text
.env.example
Cargo.lock
README.md
docs/README.md
docs/architecture/api-contexts.md
docs/architecture/object-storage.md
docs/decisions/0022-provider-independent-private-storage.md
docs/evidence/provider-independent-storage.md
docs/work-plan/030-mix-review-and-publish.md
docs/work-plan/040-theatre-web-listening.md
docs/work-plan/README.md
docs/work-plan/project-planning.md
packages/contracts/private-content-reads-v1.md
scripts/check_repository.py
scripts/test_check_repository.py
scripts/test_postgres.py
services/cantos-api/Cargo.toml
services/cantos-api/README.md
services/cantos-api/config/storage.example.toml
services/cantos-api/migrations/0015_media_storage_targets.sql
services/cantos-api/src/bin/cantos-server.rs
services/cantos-api/src/composition.rs
services/cantos-api/src/contexts/media/application.rs
services/cantos-api/src/contexts/media/audio_intake.rs
services/cantos-api/src/contexts/media/domain/mod.rs
services/cantos-api/src/contexts/media/domain/object_storage.rs
services/cantos-api/src/contexts/media/lifecycle_application.rs
services/cantos-api/src/contexts/media/mod.rs
services/cantos-api/src/contexts/media/postgres.rs
services/cantos-api/src/contexts/media/postgres/lifecycle.rs
services/cantos-api/src/contexts/media/read_application.rs
services/cantos-api/src/contexts/media/storage.rs
services/cantos-api/src/contexts/media/storage/config.rs
services/cantos-api/src/contexts/media/storage/port.rs
services/cantos-api/src/contexts/media/storage/registry.rs
services/cantos-api/src/contexts/media/storage/registry_tests.rs
services/cantos-api/src/contexts/media/storage/s3.rs
services/cantos-api/src/contexts/media/storage/sigv4.rs
services/cantos-api/src/interfaces/http/mod.rs
services/cantos-api/src/interfaces/http/studio/audio_reads.rs
services/cantos-api/src/interfaces/http/studio/mod.rs
services/cantos-api/src/lib.rs
services/cantos-api/src/platform/postgres/migrations.rs
services/cantos-api/src/platform/postgres/mod.rs
services/cantos-api/tests/audio_intake_postgres.rs
services/cantos-api/tests/catalog_postgres.rs
services/cantos-api/tests/revisions_postgres.rs
services/cantos-api/tests/support/audio_lifecycle_http_postgres.rs
services/manifest.toml
```
