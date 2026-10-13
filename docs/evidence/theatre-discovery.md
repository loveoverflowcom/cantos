# Theatre Discovery/Playback core evidence

**CORE_VERIFIED / IO_ACCEPTANCE_PENDING**, draft delivery, not production readiness. This is
PR2 only, part of #11/#12/#13. No UI redesign, native player, mock catalog server, listener
registry, Library/Progress, public activation or real provider/credential work occurred.

## Provenance and dependency boundary

The isolated branch began at freshly fetched develop `bc1dbd655598b2df2526144cbb4fc735bc2f3695`.
Publication PR91 remains an open draft at exact head
`8b7bbcddeaa8ee68cc1353862d798ec1bea96ef3` and is a prerequisite, incorporated without changing
its branch or merging into develop. Private Storage PR90 remains separate at
`9306d22954b44aa9be390495aa0b2581c0738d22`; its private port cannot authorize public delivery.
The final normal merge incorporates freshly published develop
`df7d62fecf0023cb557a3051ae75918cedcd2610` (Forge PR84), preserving its files and lock.
The actual final SHA/tree are reported by the PR after guarded remote publication verification.

Four own logical commits: contract/domain → application/read model → HTTP/integration → evidence.
Published implementation checkpoints are `effe09e9467e9d36b1d4093567f122ac6a55b267`,
`4c004d36edcd2ed5767c66e5cec934f5f2009d9f` and
`23b87e997484590803f79ab5e6d8cc5ca539a770`; the fourth includes evidence, confirmed final
regressions and the fresh-develop parent. PR91's four commits and Forge's three commits are
inherited prerequisites/develop work, not this session's implementation. Local original history
is preserved. Shell push could not authenticate; Git-data publication uses exact blobs/modes/
trees, expected-head fast-forward updates and explicit Git fetch/tree checks. No force push,
issue mutation, CI wait, merge into develop or unrelated cleanup occurs.

## Implemented capability matrix

| Mock capability | Domain / actual backend data | API / evidence | Disposition |
| --- | --- | --- | --- |
| Khám phá / neutral recent shelf | admitted Work plus active immutable Media release; fixed first actual-publication anchor | GET works; pure/property + reader/Router tests | implemented core; PG acceptance pending |
| Search | exact released Work title/synopsis; NFC/case normalization; genuine kind/audio language | GET search/works query; Unicode/property/fixture tests | implemented; no genre or ranking engine |
| WorkDetail description | representative newest eligible release, exact pinned Work revision | GET works/{id}; same representative as list | implemented core |
| Chapter/EpisodeList | stable ContentUnit identity; exact pinned title/description; first public order anchor | GET works/{id}/units; ordered keyset | implemented core; public reorder policy deferred |
| Player information | exact release/rendition/manifest/SHA/duration/codec/profile/Content language | GET publications/{id}; strict shared fixture and sealed projection | implemented core |
| Play / seek / refresh | current public witness, two reads around external grant, bounded expiry | POST playback-authorizations; named double tests | fail-closed implemented; real adapter unavailable |
| Loading/empty/unavailable/retracted | application result and private/absent equivalence | explicit page/error DTOs; Router and application tests | implemented; anonymous retraction is not_found |
| Genre, cover, author, featured/popular/rating | no real source in current Catalog/Media | no fabricated fields or endpoints | missing/deferred |
| Continue/Saved/History/bookmarks/queue | client memory-only mocks; no listener identity or persistence | later scoped Library/Progress | deferred |
| Offline/native controls | no actual public grants or native engine | owning client/storage acceptance | deferred |

Web PR83 and CMP PR85 source, plus later mocks/live probes, were mapped read-only. Web's three
WorkId variants/static strings and CMP DemoMapping/PlaybackSource do not admit arbitrary UUIDs.
Both live probes target the reserved root and reject successful data rather than decode these
resources. No live-client compatibility is claimed from the backend fixture alone.

## L1–L5 ledger

| Level / claim / exact command | Status | Held evidence and remaining gap |
| --- | --- | --- |
| L1 owner/dependency/source review | PASS | Independent read-only review; confirmed findings fixed; Media eligibility, Catalog-owned batch, Platform owner facts; no cross-context table SQL or Studio reuse |
| L2 typed construction, `cargo test --locked -p cantos-api --doc` | PASS | 14 compile-fail/type docs, including public witness barriers |
| L3 `cargo test --locked -p cantos-api --lib` | PASS | 186 API tests; 9 pure listener tests include visibility/privacy/archival/owner/withdrawal/expiry, real manifest identity, mixed languages and mutation paging |
| L3 `cargo xtask verify kani --harness listener-access` | PASS | Actual production order/access kernels; 27 checks, 5 covers, unwind2, full i64/u128, no assumptions/stubs/unreachable exemptions, unchanged final lock |
| L3 `cargo test --locked -p cantos-contracts --test theatre_discovery_wire` | PASS | 2 schema/shared-DTO fixture and disclosure tests; current broader contract gate also runs them |
| L4 production application with SequencedRead/NamedDelivery doubles | PASS | 5 tests: typed default unavailable, post-signing withdrawal re-read, exclusive/out-of-window expiry, range refusal, empty/private/absent, opaque cursor query/expiry/tampering |
| L4 exhaustive HTTP mapping | PASS | 1 exact status/code/retry/disclosure table test; no DB claim |
| Actual Router, `cargo test --locked -p cantos-api --test theatre_contract --test publication_contract --test surface_routes` | PASS | 4 Theatre + 2 publication + 8 isolation tests; native missing/mismatched Origin, bounded raw body, invalid filters/cursors/IDs, no Studio-cookie authority, unavailable PG is not mock data |
| `cargo clippy --locked -p cantos-api -p cantos-contracts -p xtask --all-targets -- -D warnings` | PASS | Current source/all targets; real PG tests compile, not execute |
| `cargo test --locked -p cantos-api --test catalog_http_postgres --no-run` | PASS | Includes three actual Theatre PG/Router regression tests; SQL runtime/type preparation still NOT_RUN |
| `cargo xtask verify repository` | PASS | Repository architecture, links/JSON, skills, Python tests and whitespace; no runtime claim |
| `cargo xtask verify contracts` | PASS | Shared fixtures/reference/wire plus actual WAV/Opus inspection; no subjective listening claim |
| `cargo xtask verify rust` | FAIL | Format/lint PASS; 17 pre-existing managed private-input fixture failures in xtask remain; no assertion suppression |
| `cargo xtask verify postgres` | FAIL | Disposable initdb succeeds; pg_ctl fails at /tmp Unix-socket EPERM; migrations/real SQL/concurrency tests NOT_RUN |
| `cargo xtask preflight --changed-only --base df7d62fecf0023cb557a3051ae75918cedcd2610` | UNSUPPORTED | Missing Gradle for CMP gates; all other selected gates NOT_RUN at prerequisite admission; no skips |
| Public origin/file/CDN Range/seek/CORS/per-fetch revocation | NOT_RUN | No authorized actual public adapter; default has no URL |
| Web/browser/CMP/native/generated-wire adoption | NOT_RUN | Current mocks intentionally preserved; transport fixture does not establish UI or device behavior |

The complete machine-readable [source/artifact ledger](theatre-discovery-results.json) records
hashes, exact proof inputs and command outcomes. Raw output remains in ignored engineering
artifacts; no private source/audio or signed grants are committed.

### Confirmed defects and corrected evidence

Independent review found and verified fixes for four normal/boundary defects: incorrect
Work-language equality rejecting mixed Content; Studio Origin middleware blocking native public
POST (including nested-path matching); copied older approval snapshots in newer scalar rows
bypassing current withdrawal; inconsistent list/detail representative metadata. Catalogue
restoration now checks complete requested/current identity/revision/owner/kind/content digest;
Media checks release/submission/rendition/candidate/clearance/approval scalar bindings and
sequences. An actual forged approval-row PG regression is compiled and remains NOT_RUN.

The initial full Rust gate had one additional PR2 failure: the Kani pipeline test expected four
registered outcomes after adding the fifth kernel. Its expected five-result invariant was fixed,
its focused test passed and full Rust returned to the known 17 baseline failures. Initial failures
and corrected results are separate hashed artifacts; no failure is relabeled as environment.
The managed private-input fixtures fail because their inputs fall under an inherited /tmp Git
marker; do not weaken that protection. This is separate from the Mac Darwin killpg history.

Kani proves strict tuple-order/local half-open expiry only. It does not prove complete public
visibility, cursor randomness, snapshot consistency, database grants, actual bytes, Range, crashes
or delivery revocation. Each of those has its own source/property/double/integration obligation.

## Read inventory, targets and DBSP decision

Targets chosen for a synthetic acceptance workload, **not measured results or production SLOs**:
10,000 works /100,000 units, skew up to 1,000 units/work, page 20/50, 8 concurrent reads, 95% reads,
up to 5 publication/withdrawal writes/sec. Target warm end-to-end p50 ≤100 ms, p95 ≤500 ms,
p99 ≤1,000 ms, process memory ≤512 MiB and publication/index maintenance p95 ≤100 ms. Freshness is
an authoritative current PostgreSQL snapshot, no allowed asynchronous rights lag.

| Read | Serving work / writes feeding it | Assessment |
| --- | --- | --- |
| works/search | bounded representative releases + immutable first anchors; pointer/rights/approval/withdrawal/revocation/owner/visibility changes | search substring and DISTINCT joins need real scale/plans; no rank/cache |
| work detail | newest eligible active release; same representative rule | indexed work/publication lookup, no per-item SQL |
| ordered units | immutable public unit order, current active eligibility | work/order index; ≤51 candidate rows |
| manifest | exact active publication join and admitted immutable chain | point lookup, current facts reapplied |
| playback | manifest/current facts before and after external grant | fresh separate snapshots, no persistent session authority |

Each read uses a repeatable-read, read-only transaction: clock, one bounded Media query, one
Catalog batch, one Platform activity batch and a final clock. There is no round-trip N+1.
Eligibility SQL prefilters use narrow Catalog/Platform SECURITY INVOKER functions; complete pure
witness and scalar admission follow. SQL uses explicit C-collated UUID tie-breakers. Current
expiry is reapplied at the final clock; candidate continuation prevents short-page skips.
Payload/cardinality and private audit reconstruction costs remain performance risks to measure.

Migration 0017 indexes work time/identity, work/unit order and release work/ID joins; existing
candidate/clearance/approval unique indexes support latest-record reads. There is no trigram
extension, materialized serving cache, CDC consumer or plan/performance claim. No EXPLAIN,
p95, throughput, buffer/spill, index maintenance or measured speedup was obtained because PG
startup failed. DBSP is **deferred/rejected for current adoption**: no measured hot aggregate,
current eligibility/clock complexity, and publication audit events are not a safe global CDC
frontier. Reconsider only after the authorized PG baseline earns an actual experiment.

## Deferred acceptance and next owner

- Persistence owner: run 0017+0016 migration/replay with exact app SELECT/INSERT and function
  EXECUTE grants in an authorized disposable PG environment; preserve old checksums and actual
  reserved 0015 storage migration. Existing releases intentionally stay unindexed/fail-closed;
  any backfill requires separately authorized full restoration and public-byte verification.
- Media/Catalog: execute three new real Theatre PG tests, plus many-work equal-time pages,
  actual mixed-language list/detail, changes while paging, missing/current scalar corruption,
  owner deactivation, publication rollback/concurrency, clock expiry and process reconnect/crash.
  Full candidate mutation/per-fetch fault acceptance is not inferred from a pure property.
- Performance owner: capture exact current app-role queries and EXPLAIN ANALYZE BUFFERS,
  representative skew/load, p50/p95/p99 and index/write maintenance under the targets above.
  No alternate startup route bypass or production benchmark is authorized here.
- Publication/storage: implement reviewed actual publication-class immutable retention and
  delivery, exact SHA/length, safe bounded signed Range, content type/CORS/cache and current
  withdrawal/revocation on every fetch; authorized live provider acceptance remains separate.
- Identity/client: consume the coordinated typed listener identity adapter for account-owned
  state. Discovery reserves 0017, Identity must use 0018 or higher. No raw dev-token registry or
  Studio-cookie reuse. Establish drift-checked schema/OpenAPI/Kotlin DTO adoption.
- Rights/QC: real distribution approval/legal terms and named subjective listening still belong
  to Publication; envelope/profile admission does not settle them.

## Theatre Web/CMP acceptance checklist

- [x] Shared strict public schemas/fixtures, exact milliseconds and stable UUID identity
- [x] Example/property public-only rules, actual manifest projection and opaque mutation keysets
- [x] Named-fake playback composition and real host protocol isolation/native Origin handling
- [ ] Real PG visibility/forged-row/concurrency/mutation paging and query-plan/load evidence
- [ ] Actual public object SHA/bytes, Range/seek/expiry/CORS/cache/per-fetch revocation
- [ ] Dynamic Web/CMP identity and generated/drift-checked resource decoders
- [ ] Real single Web player; native Android/iOS interruption/lock-screen/lifecycle acknowledgements
- [ ] Keyboard/narrow-screen/large-text/accessibility and stale/duplicate result acceptance
- [ ] Account-scoped retained access, durable saves/history/progress/conflicts and offline policy

[Next Library/Progress handoff](../work-plan/discovery-library-handoff.md). No PR3 work was started
by this session, and none of these checkboxes authorizes public activation or merging.
