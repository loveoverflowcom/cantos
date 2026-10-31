# Publication/access query observations (#657)

Scope: [#657](https://github.com/loveoverflowcom/cantos/issues/657), part of
[#634](https://github.com/loveoverflowcom/cantos/issues/634). This is an opt-in,
source-bound synthetic local measurement. It does not accept #384/#568 or production capacity.
[ADR 0054 D08a](../decisions/0054-publication-generation-and-access.md#d08--workload-revocation-and-restricted-transport)
still requires owner acceptance of budgets before runtime optimization. **No production SQL,
index, migration, cache, engine feature or authority changed in this slice.**

## Inventory before the experiment

| Consumer / production seam | Authority and consistency | Cost observed or remaining measurement |
| --- | --- | --- |
| Anonymous Discovery/Search: listener_metadata::metadata_works → Catalog public_works | Active generation, current owner/archive and PUBLIC/HIDE; immutable anchor; one snapshot | Global scope + public query, with an additional one-row continuation probe for a full service page. The measured Catalog seam excludes that probe, covers and wire encoding. Correlated visible Unit count precedes LIMIT. |
| Authenticated Shared list/search: listener_shared::shared_works → shared_candidates + shared_work | Four principal lanes only reduce candidates; current Work AND Unit is final authority. The real service also rechecks Identity/session and restricted activation. | UNION deduplicates Work actor/group and Unit actor/group before keyset/LIMIT 21. Each admitted fixture Work then takes 14 statements (one synthetic rights ref). Measurement excludes RPC/session admission. |
| Shared Work/Unit: shared_work → shared_units | Generation membership, current owner/rights/Work grant, current Unit grant and archive; one snapshot | Two bounded Unit policy/grant statements per Work, then one pinned Unit batch; HIDE precedes count/ordinal/page. Work 0 has 20 entries, one hidden from the measured reader. |
| Public Library: Media library_work_presentations | Correlated active generation/audio association and public Work/Unit gates before representative selection; Media rechecks live release rights | Actual SQL/plan measured with zero audio: an empty negative control only. Populated release/association cost remains NOT_RUN; no Library capacity claim. |
| Public/shared cursor scope: Catalog viewer_reads::scope | Publication/policy/group versions, three expiry counts and lifecycle clock; fence grants no authority | One statement, seven subqueries and table/index scans. EXPLAIN details are retained; no scanned-row counter was observed. |
| Metadata publish: MetadataPublicationService::settle | Owner auth/rights, exact pins, coherent generation/CAS/receipt on the serialized writer | Initial publish latency per Work through the owner command; index build separately. Mixed publish/revoke wait, indexed write amplification and 1,000-Unit full-copy cost remain NOT_RUN. |
| Studio chapter matrix/preview, manifest/grant/fetch | Existing exact-revision and current rights/session/Work AND Unit gates | Owners unchanged. These complete paths are not timed; correctness evidence remains independent. |

## Reproduction and fixture

Base: develop@027f9b72d1e70b46886b6c0fa7366c0fbcb4a600, after #656.
Engine: exact turso =0.8.3, default-features=false; MVCC/FTS off.
Host: macOS 15.6 / Darwin 24.6.0 arm64; Rust 1.98.1.
Build: unoptimized test profile, debug information off, incremental off, two build jobs.
The marker binds selection, generator inputs and the complete checksummed Server schema.
The manifest records HEAD, plan/generator/schema digests and logical data hash. A dirty run is
identified by these input hashes; HEAD alone is not its content identity.

    export RUSTUP_TOOLCHAIN=1.98.1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
    export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 TMPDIR=/private/tmp/cantos-657-tests
    mkdir -p "$TMPDIR" /private/tmp/cantos-657-datasets
    cargo xtask seed dataset --action plan --profile publication-access --size 20 --seed 657
    cargo xtask seed dataset --action create --profile publication-access --size 20 --seed 657 --dataset-root /private/tmp/cantos-657-datasets/twenty
    cargo xtask seed dataset --action verify --profile publication-access --size 20 --seed 657 --dataset-root /private/tmp/cantos-657-datasets/twenty
    cargo xtask seed dataset --action create --profile publication-access --size 200 --seed 657 --dataset-root /private/tmp/cantos-657-datasets/two-hundred

Use fresh, dedicated roots outside every Git checkout. Marker/generator mismatches refuse;
retain/archive with their original source, never rewrite a marker. Do not attach private
test-callers.json or databases. The lane creates no Identity accounts and sends no mail.

Explicit sizes are 8/20/200/1000, seed 657; 1000 remains NOT_RUN. Work 0 has 20 Units
(1,000 only at explicit size 1000); other Works cycle 0/1/3/5 children. Each eight-Work block
contains public, restricted/ungranted, actor, group, duplicate actor+group, public/Unit-actor,
public/Unit-group, and unpublished draft cases. Four owner-scoped groups contain the reader.
Membership has a future expiry; elapsed expiry and 1,000 grants per Work are not simulated.
Equal publication anchors test the ID tie-break and a deep keyset at 75% of IDs. Original text
contains NFC/NFD Vietnamese, duplicate titles, literal %_ and hidden Unit text that never becomes
Work search text. Handwritten membership/shape oracles precede each baseline/candidate campaign.

Each case has five warmups and 100 serial samples; p50/p95/p99 use nearest rank. Five reopened
owner samples have their own report. Reopening does not evict OS caches: **not OS-cold evidence**.
No CPU-intensive verification ran concurrently. EXPLAIN QUERY PLAN uses exact captured SQL and
parameters in the same snapshot. Query text/hash, plan, successful statement count, returned
rows and decoded value bytes are retained; parameters/credentials are not emitted.

Statement time includes engine execution, drain/decoding and observer overhead, not engine-only
CPU. Seam wall time also includes snapshot creation and benchmark result construction, excluding
EXPLAIN and final close. Decoded bytes count Text/Blob length, eight per Integer/Real, zero NULL;
they exclude allocation, names and HTTP bytes. Whole-process RSS includes reports/caches and
cannot isolate index memory. DB+WAL observations are not per-index size.

## Experiment and adoption

Only this synthetic profile creates four fixed s657_* indexes: Unit policy
(work_id,mode,unit_id,owner_id) and expiry indexes on Work grants, group members and Unit grants.
The same engine/data/seed/cases run before/after, requiring identical membership/count/order.
The fixed experimental indexes are dropped afterward. There is no registry migration or runtime
adoption; the ordinary Server binary includes neither observer nor test-support fixture.

The final reports share generator SHA-256
7ae20ea5eb757b7114e16aa0cdd282c93ca1ebfb4f76798a2b5120f7a6fe06cc.
The [sanitized summary](publication-access-657/summary.json) retains all 20 cases, nearest-rank
p50/p95/p99, source/schema/plan bindings and report hashes. The [19 actual SQL texts/plans](publication-access-657/plans.json)
retain before/after observations on both sizes. Raw samples/reports and private files stay local.

| Works / seam | Statements | Baseline p50 / p95 / p99 (ms) | Candidate p95 (ms) |
| --- | --- | --- | --- |
| 20 / public | 2 | 5.416 / 5.584 / 5.603 | 5.518 |
| 20 / shared | 170 | 26.075 / 26.888 / 26.982 | 28.076 |
| 20 / units | 15 | 3.162 / 3.252 / 3.276 | 3.285 |
| 20 / scope | 1 | 0.464 / 0.489 / 0.525 | 0.486 |
| 200 / public | 2 | 20.662 / 21.119 / 23.074 | 20.852 |
| 200 / shared | 296 | 53.091 / 54.185 / 54.539 | 53.539 |
| 200 / units | 15 | 3.450 / 3.569 / 3.580 | 3.572 |
| 200 / scope | 1 | 0.647 / 0.677 / 0.684 | 0.627 |

At 200 Works the empty-audio Library control was 27.341 → 27.278 ms p95, not a populated
Library result. Whole-process RSS was 78368 / 88784 KiB before/after, including retained
reports/caches. Index build took 13.393 ms; DB stayed 3502080 bytes while WAL grew
786952 → 819912 bytes. These are not isolated per-index costs. Initial new publishes
(n=175, 0/1/3/5/20 Units) had p50/p95/p99 5.525/10.780/17.968 ms; the 20-Unit sample is
not a 1,000-Unit copy result. No consistent cross-size/case benefit justifies adoption.

All four candidate lanes already use existing principal-first indexes. Plans also expose global
scope scans, UNION temporary storage and ordering. No O(1) or scanned-row claim follows.

**Cache decision: none.** No TTL authority, cache-on/off lane or service is introduced.
No accepted latency threshold exists. Re-evaluate adoption after accepted workload/budgets,
mixed-write parity and populated-Library evidence.

## Proposed decision, pending owner

Propose a local regression workload, separate from a production SLA: this exact 200-Work,
single-reader fixture, page 20/lookahead 21, one rights ref, no media, five warmups/100 samples.
Candidate limits for review: p95 ≤100 ms for public/Unit seams, ≤250 ms for Shared, ≤10 ms
for scope, whole-process RSS ≤256 MiB. Publish, concurrency, 1,000-Unit full-copy, HTTP/RPC and
populated Library need their own baseline before proposing numbers. **These values are proposals,
not accepted targets, gates or PASS/FAIL thresholds.** ADR 0054 remains unchanged.

## Limits and continuation

The bounded regression exercises real file-backed Turso and public HTTP HIDE. Existing Server
tests retain concurrent publisher/CAS, revoke/member removal, expiry, restart, lost-ack and
Library rejection oracles. These do not establish mixed-load performance parity or actual
Identity/media/browser/native acceptance.

NOT_RUN: OS-cold eviction, CPU/scanned-row counters, engine-only time, per-index bytes, indexed
publish/revoke write amplification/wait, populated Library, preview/chapter-matrix and per-fetch
timing, large histories, 1,000 Work/Unit/grant sizes, concurrent performance distributions,
deployed capacity and devices. Budget acceptance is pending. The single review ledger is
[#676](https://github.com/loveoverflowcom/cantos/issues/676); #658 owns integration/recovery.
Restricted serving stays default OFF and inherited Web/native private playback locks remain.
