# Production Brief evidence

Boundary: [immutable Production Brief v1](../../services/cantos-server/contracts/studio/briefs/v1/README.md),
under [ADR0018](../decisions/0018-external-audio-production.md). Source base is fresh
`develop` at `c28a3d0f4f1c1d1e262cfd56f02120e6a62966f0`; checkpoints are in
[PR71](https://github.com/loveoverflowcom/cantos/pull/71). This record separates implementation
handoff from production identity, rights, audio quality and publication acceptance.

The [independent assurance review](production-brief-review.md) records resolved findings,
coverage and the final no-actionable-findings assessment by a separate read-only agent.

## Invariants and evidence

| Claim / boundary | Cheapest oracle and evidence level |
| --- | --- |
| Brief cannot forge embedded Content, IDs, version, profile or controls | `brief_domain`: exact constructor errors, capability table, distinct private IDs — type-enforced/example-tested |
| Every guidance/delivery input affects identity; NFC is stable | per-field examples; independent Python-authored literal `narration-document.json` — example/differentially-tested |
| Plain text and recording need no Script IR, cast or provider | pure and real HTTP create/read across all four profiles — example/integration-tested |
| Rights/content/guidance drift never retargets historical instructions | actual owner read, producer denial, original retry equality — integration-tested |
| Producer cannot reuse cookie, wrong scope, expired/revoked grant or inactive owner | actual Router/owner/producer/DB flow, expiry while waiting on clock — integration-tested |
| Source extraction and exact Script export preserve their owning authority | copied receipt/extractor/full export, exact stored bytes/readback and revalidation — integration-tested |
| Brief/head writes roll back atomically and competing versions have one winner | SQL trigger injection before head, two competing real requests, independent row count — fault-injected/integration-tested |
| Settled Brief rows refuse mutation; malformed rows never export | admin UPDATE/DELETE/TRUNCATE attempts; disabled-trigger corrupt-row probe →500 — integration-tested |
| Additive0013 preserves old migrations and explicit0010 gap | old upgrade regressions; versions1–9,11,12,13/count12; checksum replay — integration-tested |
| Committed Brief/grant/export survives process and PostgreSQL restart | exact HTTP read/create/grant replay before/after owned server kill, cluster restart and migration replay — integration-tested |

The code sends no external provider request and introduces no storage/provider doubles.
Real integration crosses local PostgreSQL; no object store, TTS, mixer, audio or listening run
is claimed. The producer capability list and rights remain caller declarations.

## Read inventory and fit decision

New owner read and producer export fetch one immutable Brief by `(unit_id,version)` through
the unique `(unit_id,version)` constraint, with current Catalog owner/head/lifecycle checks.
The artifact records the actual planner choice among eligible indexes, rather than asserting
a fixed index name.
Creation writes Brief+head; grant issuance/revocation writes immutable rows under the same
clock fence. There is no paginated aggregate or independent derived projection to maintain.
Indexed PostgreSQL is the smaller correct implementation; DBSP is rejected for this read shape.
No incremental read model or engine dependency is added.

`brief_read_baseline_uses_actual_authorized_export_and_indexed_pins` records30 immutable
versions,20 warm full producer exports, write elapsed time and an actual EXPLAIN plan under
`target/catalog-evidence/brief-read-baseline.json`. This is synthetic initial-scale evidence,
not a production SLO, cold-cache or concurrency benchmark. No performance gain is claimed.
Current grants/revocation and owner changes remain authoritative; no cache bypasses them.

## Verification provenance

Rust1.87.0 (`17067e9ac`), PostgreSQL17.4 (Homebrew), disposable xtask-created cluster,
synthetic Vietnamese inputs. Final implementation/review-fix HEAD
`0ec39a12496f1178c0a4e9606585d73012ee6047`, tree
`d7a6ef27fb8b23dd00862ffdfee043529b0c9ac2`. Later evidence-only edits do not change executable
inputs. All commands ran from the isolated `/tmp/cantos-production-brief` worktree with
`RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp RUST_TEST_THREADS=1`.

| Command | Printed gate outcome / evidence |
| --- | --- |
| `cargo xtask verify repository` | PASS repository;218 Python tests plus boundaries/links/whitespace |
| `cargo xtask verify rust` | PASS rust-format, workspace rust-lint and workspace rust-tests; final source/test deltas also covered by preflights |
| `cargo xtask verify contracts` | PASS script-ir-reference, contract-tests, wav-reference |
| `cargo xtask verify postgres` | PASS postgres; subsequent final preflight reran the complete real cluster gate |
| `cargo xtask preflight --changed-only` | PASS repository, rust-format, rust-lint/tests (API/contracts/Studio), dbsp, wav-reference, Studio web-lint/build, postgres; no skips |
| `cargo xtask preflight --changed-only --base accba5ee5345edd91b7f4fa91d0ccdf804b6cbf5` | PASS repository, rust-format, API rust-lint/tests, dbsp, wav-reference, postgres for the final two test-only review fixes; unchanged executable/Web inputs retain the first preflight evidence |

Final domain target:8 tests PASS, including independent Python literal digest, forged embedded
Content, exact scope errors, CRLF/lone-CR normalization and control rejection. Final real HTTP
PostgreSQL target:23 tests PASS plus its separate restart test. Its source negative cases reject
legacy, failed and oversized receipts without a Brief/head. App-role baseline explicitly asserts
and records `current_user=cantos_app`;20 warm exports across30 versions measured a12.8ms
median in the captured run. This is a local synthetic observation, not a latency guarantee.

Session logs: `/tmp/cantos-brief-{repository,rust,contracts,postgres,preflight,review-fixes}.log`.
Ignored runtime artifacts: `target/revision-evidence/`, `target/catalog-evidence/`, including
`brief-read-baseline.json`, `http-restart.json` and `http-server-restart.json`.
GitHub CI is not inspected, polled, dispatched or awaited. Runtime UI/mobile, provider, object
storage, audio measurement/listening and publication are NOT_RUN and outside this boundary.

Early failed runs exposed and corrected: missing PostgreSQL parameter after composite FK,
partial Content constructor comparison, old registry count11, semantic400/422 mapping,
FK-aware TRUNCATE assertion, EXPLAIN text decoding and a restart fixture changed during a run.
The macOS Python temp alias was resolved with `TMPDIR=/private/tmp`; no assertion or tool
prerequisite was relaxed.

## Residual risk

Local development identity/HTTP only; production TLS and safe producer credential delivery are
unaccepted. The global Catalog fence limits concurrency. Rights references are not clearance;
source labels/cues require external editorial interpretation.256KiB export bounds deliberately
reject large receipts, and legacy text-only sources need import or explicit Text admission.
PR69 stays separate/unmerged, with its own incomplete Rust/runtime evidence. AudioLifecycle,
upload scopes/storage, QC/review/publication and live Studio/Theatre/CMP API UX remain separate.
No broad GitHub issue is closed and no phase-exit/audio/publication acceptance is claimed.
