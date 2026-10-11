# Casting and production-input evidence — issue #5

This record concerns [#5](https://github.com/loveoverflowcom/cantos/issues/5): versioned casting,
rights/budget declarations, immutable frozen candidates, exact owner approval and current
eligibility on the existing local Axum/PostgreSQL authority. [ADR 0007](../decisions/0007-casting-production-inputs.md)
is proposed for production adoption. The [production v1 contract](../../packages/contracts/production-v1.md)
separates wire shape from domain evaluation and from the reference-only capability catalog.

Generation and payment are zero in this slice. There is no TTS/LLM request, Ollama/model install,
provider credential, voice clone, generated audio, reservation, provider receipt or actual charge.
The original Vietnamese/synthetic fixtures demonstrate logic, not a real provider voice, legal
clearance or the user's authority to spend. Product approval grants no agent payment authority.

## Claim ledger and evidence boundary

| Invariant | Failure prevented | Cheapest independent evidence | Execution |
| --- | --- | --- | --- |
| Settings version/pin exact voice, model, performance and pronunciation; unsupported controls fail | Silent provider/voice substitution | Exact domain error and DTO/Schema shape examples | PASS: `example-tested`, `statically-checked` schema/closed DTO |
| Frozen input digest binds full export/c1/e1, settings, rights, policy and metadata | Rights-only/text/cost edit inherits approval | Python canonical JSON/SHA-256 oracle and scoped mutations | PASS: scoped examples, `differentially-tested` Python p1 fixture |
| Immutable candidates remain byte-identical after draft/settings edits | Active draft changes historical production inputs | Real HTTP reads plus SQL immutable records | PASS: `integration-tested` PostgreSQL and HTTP reads |
| Current claims fail closed at missing/pending/revoked/mismatched/expiry boundaries | Stale permission permits work | Domain truth table, current HTTP eligibility and PostgreSQL claim history | PASS: domain examples, `integration-tested` persisted status/time checks |
| Unknown estimate stays unavailable; integer estimate/limit checks are exact | Unknown-as-zero, rounding or overflow authorizes spend | Independent Unicode scalar count and rational ceiling/boundary oracle | PASS: integer boundary examples, `differentially-tested` Python line rounding |
| Approval binds exact snapshot/digest and current authority | Cross-scope or revoked owner approval remains eligible | Direct HTTP denial, input-scope changes and actor revocation | PASS: `integration-tested` private routes/current owner/transaction revocation |
| CAS/idempotent operation receipts serialize duplicates/concurrent freezes | Duplicate records, conflicting snapshots or changed retry payload | Concurrent HTTP requests and immutable SQL row counts | PASS: `integration-tested` concurrency/exact replay, duplicate injection |
| Rollback and restart preserve exact records and replay receipts | Partial durable fact or lost accepted authorization | Named transaction failure and real HTTP/PostgreSQL restart | PASS: `fault-injected`, `integration-tested` rollback, host kill and database restart |
| Catalog is reference-only and dispatch availability false | Fixture misrepresented as a paid provider integration | Catalog/eligibility HTTP assertions and no attempt/charge rows | PASS: domain examples and real HTTP/SQL zero-attempt observations |
| Studio renders typed blockers and preserves uncertain mutation snapshots | UI bypass or changed input retried under original operation | Pure reducer suite then browser interaction, keyboard/focus and narrow/zoom | PASS: 16 production reducer examples and actual Safari interaction, keyboard/focus and narrow/zoom observations |

These labels name only the executed boundary below. The finite scoped-change/domain tables are
`example-tested`; no production-domain fuzzing, mutation run or symbolic proof is claimed.
Compilation does not establish interaction, rights clearance, provider availability or audio quality.

## Reproducible verification

| Command or observation | Result | Evidence boundary |
| --- | --- | --- |
| `python3 scripts/check_repository.py` | PASS (245 text files, 15 skills after bounded evidence update) | Links, JSON/text and skill hygiene |
| `cargo test -p cantos-server --test production --test production_contract --locked` | PASS (13 domain + 2 schema tests in the final workspace run) | Pure domain, Rust JSON Schema meta validation/literal fixtures and actual prepared-response admission |
| `cargo test -p cantos-api --test production_wire --locked` | PASS (3 wire tests) | Independently authored settings/pending claim literals, closed tags and integer transport |
| Actual prepared-preview response-schema regression | FAIL before correction; PASS after correction in final workspace run | Separate resolved response pronunciation admits inherited Script IR values; actual 600-character preview and frozen-document regression |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS (80 tests; corrected undefined UI CSS token) | Python regression suites |
| `cargo fmt --all -- --check` | PASS on final source | Formatting |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS on final source | Native static checks |
| `cargo test --workspace --locked` | PASS (257 passed, 50 ignored) | Includes 116 Studio tests, 16 production reducer tests and 2 schema tests; ignored database suites ran separately |
| `python3 scripts/reference_script_ir.py` | PASS | Existing independent Script IR digest/shape oracle |
| `TMPDIR=/private/tmp python3 scripts/mutate_script_ir.py` | PASS (9 killed, 0 survived/timeouts/unviable; 15-test baseline) | Bounded existing Script IR validator/encoder mutations; no production-domain mutation claim |
| `PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" TMPDIR=/private/tmp python3 scripts/test_postgres.py` | PASS on final source (50 PostgreSQL tests + actual HTTP/CLI/oracle/restart journeys) | New disposable cluster; 12 revisions, 9 imports, 21 adaptation and 8 production tests |
| `cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings` | PASS on final source | Browser target static checks |
| `NO_COLOR=true trunk build --locked` from `apps/web` | PASS on final source | Studio WASM build; runtime evidence recorded separately |
| `git diff --check` | PASS after bounded evidence update | Patch whitespace |
| Live Studio interaction, narrow window/zoom, desktop dark and keyboard/focus | PASS | Actual Safari inputs and transcript captures on the final Studio build; scope detailed below |
| OS Vietnamese IME, screen reader or native mobile interaction | NOT_RUN | Keyboard/focus/text scaling observations do not establish these runtime paths |
| Real provider generation, billing or audio listening/QC | NOT_RUN | Outside #5; no real call or audio produced |
| Native Android/iOS runtime or production identity deployment | NOT_RUN | Outside #5; no runtime gate exists |

### Executed domain, wire and PostgreSQL boundary

Execution used macOS 15.6 arm64, Python 3.13.5, Rust/Cargo 1.87.0, Trunk 0.21.14 and PostgreSQL
17.4 (Homebrew). The feature working tree was based on `4c8038d710fcc14ce5970432e09a7c4c682ca553`
and dirty on `feat/casting-production-inputs`. This is final local feature evidence, not a
merged deployment observation. Logs remain in ignored `target/production-evidence/`:
`production-unit-initial.log`, `production-wire.log`, `postgres-final.log`,
`python-tests-final.log`, `mutation.log`, `fmt-final.log`, `clippy-native-final.log`,
`workspace-tests-final.log`, `clippy-wasm-final.log` and `trunk-final.log`. Historical prepared
response runs failed before the correction; the final workspace log records both schema tests
passing. The final disposable cluster was
`target/revision-evidence/cluster-1791647987850456000/pgdata`, newly initialized without an
inherited database URL. The runner stopped it; the root agent verified the PID file was absent
and removed only the stopped disposable data directory. Logs were retained.

The 29-file tested-source manifest has SHA-256
`736d89714d0345a94e36f2b00d8268a5c10f2f8fb18d8363ce5a88e2c2ff5bf1` (the hash of
`target/production-evidence/source-fingerprint.txt`). The [safe extract](casting-production-read-results.json)
preserves its file hashes and the final raw-record checksum
`73b3020ad2a746a0b22386cce019d80ce507ebea14a9143711671c41a04d7156`.
It describes the final source after the response-schema and Studio corrections. A staged
whitespace check then found one trailing blank line in the oracle; removing it changed no
executable source or fixture. The Python suite and staged whitespace check were rerun, and the
safe extract retains the PostgreSQL-execution manifest alongside the final manifest. The final
native/WASM checks, workspace suites, disposable PostgreSQL run and browser observations apply
to this boundary; no later edit inherits these results without a proportionate rerun.

The 13 domain tests cover unsupported controls/registry IDs, missing/unknown casting, exact
language, simultaneous longest-first NFC pronunciation, conflicting replacements, amplification
bounds, canonical identity/order, unrelated-claim exclusion, every bound-input mutation,
rights status/term/subject/language/scope/restriction, exact integer line rounding/overflow,
blocked candidate inspection, frozen immutability and approval/input eligibility separation.
The two schema tests use the existing pinned Rust `jsonschema` crate; no Python dependency or model
was installed. Three shared DTO tests independently assert unknown authority fields, closed
rights/estimate variants and rejection of noninteger money.

The eight production PostgreSQL tests specifically check:

- actual pinned-receipt verification rejects a self-consistent forged document;
- concurrent settings and failed freeze cannot accept a stale preview;
- expired claims, unsupported settings and injected transaction failure leave no partial fact;
- immutable candidates and exact concurrent retries survive reopened store;
- missing helper EXECUTE rollout fails without authority edits;
- current permission revocation waits for the real transaction and blocks its next write;
- private HTTP/owner actions deny access while a blocked candidate remains inspectable to its owner;
- settings/rights/full-export changes invalidate authorization without mutating candidate bytes.

The production oracle is `scripts/production_oracle.py`, invoked by
`production_before_restart` / `production_after_restart` hooks in the disposable runner. It
crosses the actual loopback HTTP service and application-role PostgreSQL database. It compares
the tagged input digest and integer planning estimate with independent Python calculations,
checks immutable row counts and exact receipts, then compares after host/database restart and
migration replay. Its final log reports PASS across settings → declarations → review → freeze →
approval → exact snapshot/eligibility on actual HTTP and application-role PostgreSQL. It injects
missing/pending/revoked/expired/not-yet-valid claims, unknown/exceeded cost, stale settings/text,
concurrent duplicate freeze, host process kill after committed records, database restart and
migration replay; exact document/receipt replay and current stale eligibility both survive.

The original Vietnamese fixture includes an NFD input and an astral theatre emoji. Four resolved
lines have 41/36/29/31 Unicode scalars (137 total). A synthetic VND planning rate of 3 minor units
per 7 scalars produces line ceilings 18/16/13/14 = **61**, while aggregate rounding would be 59.
The independent Python oracle therefore detects the per-dialogue policy. SHA-256 is independently
computed over compact recursively sorted scoped JSON with the `cantos/production-inputs/p1\n`
tag. This is a finite fixture comparison, not provider pricing or model/audio quality evidence.
SQL counts are 10 settings, 11 rights versions, 3 snapshots, 2 approvals, 2 accepted revisions and
2 owner reviews; the fixture database has zero adaptation generation attempts and none of the
six explicitly checked production attempt/reservation/charge/usage/settlement table names. The full safe
synthetic record remains at `target/production-input-evidence/http-restart.json`; a bounded
[read-results extract](casting-production-read-results.json) records timings/plans and provenance.

## Affected read inventory and DBSP assessment

All production reads run on the existing PostgreSQL authority. Current actor/session and script
membership checks precede private data reads; immutable bytes alone never grant current access.
There is no cache, CDC, incremental engine or asynchronous fallback on this path.

| Consumer / read | Shape and expected cardinality | Feeding writes and consistency |
| --- | --- | --- |
| `GET /api/v1/production/catalog` | Static bounded reference capability contract; no SQL | Catalog version is pinned in frozen documents |
| `GET /api/v1/scripts/{script}/production` | Latest settings, current script-scoped rights and newest twenty metadata summaries; no full documents or per-history approval reads | Settings/rights/snapshot inserts; actor/session/membership revocation; authorized consistent read |
| `GET .../production/review` | One accepted head, one settings version and latest claims; resolves bounded dialogue | Revision/settings/rights/editorial review writes; evaluation clock changes validity even without writes |
| `GET .../production/snapshots/{snapshot}` | Indexed `(script, snapshot)` immutable document plus current eligibility/approval | Snapshot immutability; current revision/settings/claims/access determine eligibility |
| `GET .../production/snapshots/{snapshot}/eligibility` | Same candidate/current-fact evaluation without replacing the document | Scope/rights/approval changes and expiry are checked on request |
| Internal reads for settings/rights/freeze/approval POSTs | Exact operation replay, current CAS versions/head and bounded facts inside one transaction | Same script/actor/operation unique identity; script and actor locks serialize decisions |

Migration `0006_production_inputs.sql` adds `production_settings`, `production_rights_claims`,
`production_snapshots` and `production_approvals`. Their primary/unique keys serve exact point
and operation lookups; `production_snapshot_history(script_id, recorded_at DESC, id DESC)` serves the
bounded newest-first history. The names and bounded inspected plans below describe this initial
implementation; measured frequency and representative production cardinality
are unknown, not inferred from this synthetic suite.

Actual route handlers are `catalog`, `state`, `save_settings`, `save_rights`, `preview`, `freeze`,
`snapshot`, `eligibility` and `approve` in `apps/server/src/http/production.rs`. They delegate to
`Store::{production_state,save_production_settings,save_production_rights,production_preview,
freeze_production,production_snapshot,production_eligibility,approve_production}` in
`apps/server/src/postgres/production.rs`. Shared `facts`, `settings`, `rights`, `reviewed` and
`revision` reads apply the existing authorization boundary. Exact document loads compare
historical revision/settings/rights record versions and actor/time receipts, re-admit canonical
bytes and recompute the input digest. Required rights are selected from Script IR evidence and
casting bindings; unrelated current declarations do not invalidate a candidate.

The migration-owner's `production_lock_actor(text)` and `production_lock_member(text,text)`
SECURITY DEFINER helpers use fixed search path/qualified SQL and revoked PUBLIC execution.
Runtime execution grants obtain share locks without runtime UPDATE authority on actors/members.
Database denial/revocation evidence is separate from this documented grant boundary.

| Read-performance item | Result / decision |
| --- | --- |
| Workload | PASS: one synthetic script, nine sequential HTTP samples per path before/after real restart; no representative throughput/production workload |
| Synthetic PostgreSQL timings/plans | PASS: actual app-role `EXPLAIN (ANALYZE, BUFFERS)` related projections plus complete authenticated HTTP timings; [raw extract](casting-production-read-results.json) |
| Simpler PostgreSQL comparison | PASS: normal plans and transaction-local `enable_seqscan=off` use the same existing primary/history-index topology; no new index or speedup claim |
| DBSP | **DEFER**: no measured hot aggregate/join or large shared view consumer; current authorization and time validity need synchronous authoritative facts |
| Engine integration / freshness / fallback / rebuild | NOT_RUN / not implemented; no engine path is served |
| Mutation/maintenance overhead | PASS: descriptive settings/rights/approval HTTP samples below; CPU/RAM/storage/write amplification/CDC maintenance NOT_RUN; no maintained view added |
| Reopen criteria | A concrete consumer exceeds its measured latency/resource budget at representative scale after the simpler PostgreSQL comparison; propose scoped checkpoint/freshness/access/recovery semantics first |

### Bounded local observations

Each read path had nine sequential loopback samples in each phase against the same committed
synthetic state: 10 settings versions, 11 claim versions (4 current selected claims), 3 snapshots
and 2 approvals. The local descriptive target was p95 < 100 ms. With nine samples, the
nearest-rank p95 is simply the sample maximum; no representative tail distribution or throughput
claim follows. The runner executed prior setup/recovery requests; these observations are not a
controlled cold-cache test. Post-restart sampling followed correctness/replay reads.

| Actual HTTP read | Before restart median / p95 (ms) | After restart median / p95 (ms) |
| --- | --- | --- |
| State/summary history | 2.205 / 3.022 | 2.113 / 2.614 |
| Review/resolve | 3.489 / 3.577 | 3.436 / 3.691 |
| Exact snapshot/current eligibility | 7.965 / 8.360 | 8.160 / 8.773 |
| Eligibility route | 7.637 / 7.915 | 7.641 / 7.706 |

All observed maxima met that bounded local target. Complete HTTP timings include authentication,
facts, historical receipt checks, resolution and response materialization. SQL probes retained
the actual script/owner filters and ordering but omitted some response metadata columns; they
are related query-shape observations rather than an exhaustive handler plan trace. EXPLAIN
profiling time is distinct from HTTP latency.

| Related SQL probe | Ordinary plan / actual rows | Normal / existing-index probe execution time (ms), before restart |
| --- | --- | --- |
| Latest settings | `production_settings_pkey`, backwards index scan + limit, 1 row | 0.015 / 0.012 |
| Latest rights | `production_rights_claims_pkey`, incremental sort + unique, 11 versions → 4 current rows | 0.035 / 0.034 |
| Metadata history | `production_snapshot_history`, index scan + limit, 3 rows | 0.013 / 0.013 |

The ordinary planner already selected those indexes. Both phases/probes recorded zero shared
read blocks and zero temporary read/write blocks; rights sort reported 31 KiB. Plan topology
remained the same with `SET LOCAL enable_seqscan=off`; this demonstrates index availability,
not a faster alternative. No schema/query/index optimization or incremental read model is
admitted from these small measurements.

Descriptive write samples were 10 settings operations (maximum/p95 3.401 ms), 11 rights
operations (3.401 ms) and 2 approvals (15.362 ms). Approval's two samples cannot establish a
tail budget. CPU/RAM, storage/WAL amplification, throughput, concurrent read/write performance,
larger cardinality/skew, actual production frequency, p99, cache preparation and engine resource/
freshness/rebuild/outage metrics remain NOT_RUN. DBSP stays DEFER with those limits explicit.

## Remaining limits and follow-up ownership

Reference capabilities and rates cannot prove real synthesis support, voice identity, provider
terms, cost, consent, legal ownership or acoustic quality. Rights assertions are not legal
verification. Production identity/deployment and public distribution rights remain separate.
Frozen candidate history is bounded; its current eligibility can change after expiry, revocation,
edits or loss of actor scope without changing its historical bytes.

No external provider attempt exists, so provider timeout/rebilling, worker lease/cancellation,
reservation settlement, media integrity, QC, mixing, publication and listener/mobile behavior
are not exercised. The [queue](../work-plan/README.md) recommends #6 only after #5 acceptance
and explicit authorization for the concrete provider adapter. No CI workflow is added or widened.

The bounded #5 implementation and local verification are complete at the recorded source
boundary. GitHub issues/PRs own acceptance, review and execution status; this record does not
itself approve a merge or establish production readiness. The next dependency is #6's concrete
provider adapter, with explicit provider, applicable rights and spend inputs before real calls.
No adapter or generation work starts as part of this slice.

## Observed Studio runtime

The root agent exercised its own Safari window against the disposable local fixture; an
independent reviewer later read accessibility state and a screenshot without changing data.
Actual browser input superseded the earlier locked-Mac failure. Captures are retained in the
workchat computer-use transcript only; no PNG artifact or automated pixel/viewport measurement
is claimed here.

- Initial stored selections rendered `synthetic-narrator`, `vi-VN` and `VND` correctly after the
  select synchronization correction. Settings version 10 froze snapshot
  `1e94e581-af18-49ff-9071-f372414ba270`; owner `alice` explicitly approved the exact candidate
  at `2026-10-10T15:43:25.676756Z`.
- Typing an unsaved Vietnamese rights declaration cleared the acknowledgement and blocked the
  action. Explicit discard recovered the saved state. Saving settings version 11 with rate 1001
  and unknown cost made the old approval stale at `settings.version`; the frozen version-10
  document/receipt remained identical. Pending voice claim version 7 and rights/`budget.rate`
  blockers were visible.
- An initial 175% zoom observation found horizontal overflow. After scoped fieldset/select
  minimum-width corrections and the final Trunk build, a 720 × 863 physical Safari window at
  exactly 200% zoom showed wrapped Vietnamese text and no horizontal scrollbar. Approximately
  360 CSS pixels is an inference from window width/zoom, not an instrumented viewport value.
  Tab from rate to pitch showed a strong purple focus ring in an actual capture.
- The independent reviewer observed a 1324 × 966 desktop Safari window at 100% zoom in dark
  mode, settings version 11, all three bindings showing `synthetic-narrator`/`vi-VN`, `VND`,
  named controls, readable Vietnamese in two columns and no horizontal scrollbar in the visible
  layout. The no-provider/payment notice was visible.

These observations establish the exercised interaction, native control labels, keyboard focus
and narrow/zoom behavior only. OS Telex/VNI composition, VoiceOver/screen-reader semantics,
mobile/native devices, motion and exhaustive contrast/assistive-technology coverage remain
NOT_RUN. The root agent restored the system theme and 100% zoom, closed only its own window,
and stopped its disposable host/cluster; no persistent user environment was intentionally changed.
