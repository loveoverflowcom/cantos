# Localhost Turso dataset profiles

Implemented test tooling for [#568](https://github.com/loveoverflowcom/cantos/issues/568).
These are synthetic private Server datasets, not production seeds, Studio accounts, rights
clearance or publication evidence. The fixed [six-Work sample](theatre-seeded-catalog.md) and
its non-login principals remain unchanged. No migration or production search default changes.

## Select and operate a dataset

Use Rust 1.98.1 from the repository root. The parent directory must already exist outside
**every** Git checkout. Use a dedicated absolute root; never point at an Identity, development,
user or production directory. Keep outputs and test credentials private.

```sh
cargo xtask seed dataset --action list
cargo xtask seed dataset --action plan --profile mixed-kinds --seed 568
mkdir -m 700 /tmp/cantos-local-profiles
cargo xtask seed dataset --action create --profile mixed-kinds --seed 568 --dataset-root /tmp/cantos-local-profiles/mixed
cargo xtask seed dataset --action verify --profile mixed-kinds --seed 568 --dataset-root /tmp/cantos-local-profiles/mixed
cargo xtask seed dataset --action serve --profile mixed-kinds --seed 568 --dataset-root /tmp/cantos-local-profiles/mixed
# After stopping the fixture with Ctrl-C:
cargo xtask seed dataset --action archive --profile mixed-kinds --seed 568 --dataset-root /tmp/cantos-local-profiles/mixed
```

`list` and `plan` are read-only selections; they do not build, open a database or create a root.
`create` runs the exact ignored Server fixture with current service migrations, a controlled
clock and production Catalog handlers over real loopback HTTP. `verify` requires the completed
receipt, reopens both files and rechecks declared rows, HTTP pages and integrity. It does not
reseed business records. `serve` first verifies, then binds only `127.0.0.1:8080`; an occupied
port is refused. No Identity process, browser login or production storage is composed.

The private `test-callers.json` contains random development credentials for the five named
synthetic actors. They are **not** passwords or an Identity signup/provisioning procedure. They
are only for this test host's existing `cantos_session` cookie and exact loopback Origin. Do
not log, commit or attach that file. Serve uses the pinned fixture clock and must remain local.
Exploratory writes use new operation IDs and current revisions; they deliberately diverge from
the seed oracle. Archive and create a new root when a pristine oracle is needed. The immutable
`ready.json` attests the completed seed, not the live state after exploratory writes.

Commands use a sibling operation lock across create/verify/serve/archive, plus the actual
Server/snapshot owner locks. Foreign markers, changed profile/seed/size/generator/schema,
symlink components and an open database refuse. `archive` atomically renames the whole owned
root to a sibling `.retired-*` directory, retaining DB/WAL, credentials and evidence. It never
removes files. Use the same profile/seed/size for all lifecycle operations. A changed generator
requires the original checkout to archive its root, or a new root; it is never silently adopted.

A partial import has no READY receipt. Resume `create` with the **same selection/root/source**:
its durable actor/operation identities replay the original Catalog receipts, even if the
accepted response was lost. Unknown staging files remain untouched. Successful responses,
complete snapshots and READY are published in that order; a failure never manufactures success.
A 30-minute timeout bounds each create/verify campaign and preserves partial evidence. Do not
blindly create fresh operation IDs, delete DB/WAL or rewrite a marker to recover.

## Registry and artifacts

The [versioned registry](../../services/cantos-server/tests/fixtures/local-datasets/registry.json)
is consumed by xtask and the Server fixture. [Pure plans](../../services/cantos-server/tests/support/local_dataset/plan.rs)
derive IDs from dataset version/profile/seed/purpose. Reproducibility means canonical business
records and expected identities, not DB file bytes, session IDs or random credentials.

| Profile | Shape and selection | Executable observation / remaining boundary |
| --- | --- | --- |
| `empty` | Zero business rows, five separate fixture actors | Empty Studio/Theatre lists; 401 without a session is distinct. First-create is an exploratory request. |
| `minimal` | One Work, `--size 0` (default) or `1` child | No cover/audio/optional links; reads/history/reopen. |
| `mixed-kinds` | Nine Works; novel/story/podcast, duplicate titles, 0/1/3 children, one archived Work | UUID identity, matching chapter/episode kind, archive filter, preserved history. |
| `pagination-boundaries` | `--size 0,1,19,20,21,49,50,51,101` (choose one; default 21) | Studio limits 1/20/50, exact IDs/order/end, 0/51 rejection, actor/filter scope. |
| `relationships-capacity` | Three Works: 0/1/N children; N is `0,1,19,999,1000` (default 19) | Nonempty dense Work retains one archived tombstone. At 1000, boundary+1 returns 422 without another receipt/history row. |
| `unicode-and-patch` | Original VI/EN/NFC/NFD/emoji/multiline corpus, duplicate titles | Literal 512-byte title and 16,384-byte synopsis; preserve/replace/empty, invalid null/size/blank request oracles. |
| `substring-search` | Seven documents and handwritten query membership | Native `instr`/normalization/filter oracle; separate release-backed adapter test; private profile itself remains invisible. |
| `fts-corpus` | Seven private Works plus separate engine-only documents/query membership candidates | Real native index-method probe; FTS runtime remains **UNSUPPORTED** on default features. |
| `vector-corpus` | Empty Server DB plus separate engine-only fixed 3D vectors | Real vector32/L2 full scan, tolerance 1e-6, top-k/filter/ties, invalid dimensions/nonfinite, insert/update/delete/reopen; no ANN/index/product claim. |
| `studio-crud` | One Work/two children, patch/reorder/archive | Parent/child CAS, tombstones, exact replay, immutable history; no hard delete or restore. |
| `permissions-conflicts` | Two owners plus editor/reader/stranger | Existing SetMember application seam; HTTP 403/404, owner-only visibility/archive and operation/CAS negatives. |
| `visibility-recovery` | Private/unlisted/public **metadata**, no releases | All remain absent from anonymous discovery/detail. Recovery is original-operation receipt replay. No generic sample disclosure is added. |
| `scale-local` | Default 20; explicit 1000 or 10000 Works; 10 children per hundredth Work, one per other tenth | Counts/skew, measured seed+verification elapsed time and DB/snapshot bytes; #384 workload input, no SLA. |

The explicit `publication-access` profile accepts 8/20/200/1000 Works. It adds synthetic
metadata publication, actor/group/Unit grants, duplicate candidate paths, HIDE, unpublished
drafts, equal anchors and Vietnamese/literal search to this existing lane. Create/verify
write new immutable publication-measurements UUID reports (100 serial samples per case),
never during ordinary gates/startup. Fixed experimental indexes exist only during the
explicit campaign and are dropped afterward. A failed campaign retains files and never
publishes a new READY; preserve/archive with matching source rather than editing markers.
The [measurement boundary](../evidence/publication-access-performance-657.md) distinguishes
synthetic rights/session facts, pending owner budgets and runtime adoption. No Identity,
audio, production budget, cache or production index is accepted by this profile.

Sizes above are allowed values, not comma-separated CLI syntax. Large sizes are opt-in; no
startup/build/preflight/CI path invokes a persistent profile, FTS/vector campaign or scale run.
Ordinary bounded fixture regressions run in the existing Rust gate; no additional gate exists.
For scale, every Work is compared through SQL and bounded lists; detail probes sample the first
ten Works. Dense child lists use limit 50; small pagination profiles exercise all three limits.

Each root contains:

- `sample-owner.json`: distinct dataset marker, canonical root, plan/generator/engine/schema binding.
- `plan.json`: exact positive HTTP requests (and the existing membership application commands),
  actor indices, operation IDs and independently planned final metadata/revisions.
- `negative-requests.json`, `oracles.json`, `substring-corpus.json`, `checks.sql`: separate invalid
  requests, expected IDs/counts by owner/kind/lifecycle, original query goldens and native SQL.
- `server.db`, `sample.db`: actual migrated Server database and owner-supported verified snapshot.
- `manifest.json`, `ready.json`: source SHA, generator content fingerprint, exact migration
  checksums/engine features, clock, logical digest, measurements and separate capability states.
- `engine-corpus.json` for FTS/vector: synthetic documents/vectors, dimensions/metric and independent
  expected membership/distances; engine probe files themselves are disposable and never Server tables.
- Private `test-callers.json`: random fixture credentials, excluded from logical equivalence/reporting.

`OwnedDatabase::check_integrity()` checks integrity and actual FK metadata with anti-joins;
`PRAGMA foreign_key_check` returning no rows is insufficient on the pin. Snapshot uses the
owner's `backup_to`, not a copy of a possibly uncheckpointed DB. No Python sqlite3 is involved.
The supplied SQL is read-only reference material for a stopped owner; `verify` executes the
actual SQL/HTTP checks through the pinned engine and refuses mismatching snapshots.

## Search qualification and surface boundaries

The [substring corpus](../../services/cantos-server/tests/fixtures/local-datasets/substring.json)
has handwritten expected document indices for NFC/NFD/case, accented versus unaccented input,
literal `%`/`_`, emoji, title/description, duplicate titles, kind/language and no-match queries.
Expected membership is not computed by the query being tested. Native SQL and a separate
Theatre release fixture consume these goldens; the latter uses the existing typed synthetic
publication seam and does not certify rights, QC, delivery bytes or a real publish journey.

Studio uses UUID / `(position,id)` / revision order and global Catalog epoch. Theatre uses its
own opaque cursor, query/limit/representation binding, current eligibility and keyset anchors.
An empty page with a next cursor must continue. Neither gains total counts or page numbers.

The [engine probe](../../packages/store-turso/src/search_probe.rs) inspects the actual
`turso =0.8.3`, `default-features=false` dependency. Native FTS uses
`CREATE INDEX ... USING fts`, not SQLite FTS5; the
[upstream engine manual](https://github.com/tursodatabase/turso/blob/main/docs/manual.md)
is a discovery reference, while the pinned crate source and probe own local evidence.
Enabling experimental index methods in a disposable DB still reaches the absent FTS module.
No Tantivy feature, engine upgrade or dependency was added. AC-10 remains open pending an
explicit test-feature/dependency review followed by real token/phrase/prefix/rank and
insert/update/delete-index qualification. The corpus is preparation, not FTS PASS.

Vector qualification compares the real engine with handwritten Euclidean distances and an
explicit ID tie-break, including duplicates, empty corpus, k above corpus size, metadata
filter/no-hit, update/delete/reinsert and reopen. It is exact full-scan SQL, not ANN recall or
index qualification. No embedding provider is called. Public substring behavior is unchanged.

## Coverage and handoff

| Issue criteria / boundary | New evidence owner | Existing tests reused / remaining acceptance |
| --- | --- | --- |
| AC-01–03 contract, reproducibility, lifecycle | `local_dataset_turso`, `xtask::seed::dataset`, shared sample lock/archive/atomic-file regressions | Existing #427 sample tests remain unchanged in meaning. Check exact command outcomes in the PR, not this runbook. |
| AC-04–06 shapes, pagination, Unicode/search | Profile SQL/HTTP oracles, cardinality matrix, separate Theatre corpus/cursor test | `catalog_http_turso::http_optional_links_pin_exact_owned_historical_bytes_and_patch_omission_differs_from_null`; native current/parent/filter cursor tests. |
| AC-07–08 CRUD, conflict, permission/recovery | Interrupted seed/reopen/replay/snapshot test, profile negative requests | `catalog_studio_turso` concurrency, lock-wait reauthentication, membership revocation and immutable barriers; `catalog_http_turso` concurrent reorders/append rollback. |
| Theatre public visibility and empty continuation | Separate typed release corpus test and existing owner suites | `support/theatre_catalog_walk_http_turso.rs` withdrawal/republish, representative filters and expiry; `listener_cursor.rs` deterministic expiry; wire empty-continuation fixture. Persisted normal profiles contain no public releases/disclosures. |
| AC-09–11 engine capabilities | Explicit `fts-corpus` / `vector-corpus` reports | FTS runtime pending; vector index/ANN/product integration not claimed. |
| AC-12 real browser | [#565](https://github.com/loveoverflowcom/cantos/issues/565) | **NOT_RUN** until legitimate Studio account provisioning/integration exists. ADR 0047 is still proposed; no roster bypass is provided. |
| AC-13 scale and handoff | Explicit 20/1k/10k selections, actual manifest measurements | Unexecuted sizes remain NOT_RUN; #384 owns optimization. GitHub acceptance/comments require the user's corresponding authorization. |

Browser continuation: after #565 supplies an approved local Identity/Studio provisioning lane,
coordinate the same owned Server DB/actor with that lane; do not copy a test cookie into a login
journey or assume the native run command accepts these fixture files. Capture real Studio
empty → create → edit → archive → reload and unsaved-draft/CAS behavior separately from these
DB/HTTP results. No new browser connection flags or account workflow is invented here.

At handoff record base/source/content fingerprints, exact commands/cwd/profile/root, status,
logs and measured size/time. Retain all FAIL/NOT_RUN/UNSUPPORTED results and any unexecuted
public-disclosure, real browser or scale acceptance. `Part of #568` is appropriate while these
boundaries remain open; a fixture PASS or merged PR does not complete the tracking issue.
