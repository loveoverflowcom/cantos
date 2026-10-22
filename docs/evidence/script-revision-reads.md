# Script revision affected-read inventory

Initial inventory: prompt 2, local development persistence slice based on
`ea8e1d98a31301b0d63dfa75d8e56450397b581d`. Inventory follows the
[PostgreSQL read-performance skill](../../.agents/skills/cantos-engineering/references/turso-read-performance.md).
The table records implemented semantics. Prompt 3 adds the
[bounded measurement protocol](script-revision-benchmark-protocol.md) and
[completed measurements/DBSP decision](script-revision-performance.md); measured evidence is
kept separately from the prompt-2 historical NOT_RUN record below.

## Actual serving reads

All query owners are [postgres.rs](../../services/cantos-server/src/contexts/editorial/postgres.rs); the
[HTTP routes](../../services/cantos-server/src/adapters/inbound/http/router.rs), [migration](https://github.com/loveoverflowcom/cantos/blob/3e6c183d3d616e8ec4a782ba788b4b70d0c6e49a/services/cantos-api/migrations/0001_script_revisions.sql),
[wire contract](../../packages/contracts/studio-v1.md) and
[real PostgreSQL suite](../../services/cantos-server/legacy-pg-tests/tests/revisions_postgres.rs) are immediate consumers.

| Read / consumers | Query, parameters and permission scope | Cardinality / index / order | Feeding writes / time | Consistency and cost uncertainty |
| --- | --- | --- | --- | --- |
| Authentication middleware, POST session, load/save transactions | sessions JOIN actors by SHA-256 token; active, not revoked, expires_at > transaction timestamp; FOR SHARE OF sessions | 0..1; session hash and actor PK; no pagination/order | Session insert/revoke/expiry; actor activation | Middleware and operation transaction each recheck; expired/revoked denial tested. Duplicate authentication adds an extra DB trip; frequency/cost unknown |
| GET scripts/{id}/head: Studio Read | Locked scripts row by canonical UUID; owner equality or script_members(script,actor); current head revision lookup | 0..1 script/member/revision; scripts PK, member composite PK, revision composite PK; no pagination | First save, head append, membership/actor/session changes | READ COMMITTED, script FOR SHARE against concurrent writers; current access before export; complete decode + canonical/c1/e1 validation. API latency unknown |
| GET scripts/{id}/revisions/{n}: pinned history | Same authorization, exact (script,revision) lookup | 0..1; revision composite PK; no listing/pagination | Immutable acceptance; future rights/access changes still affect retrieval | Immutable bytes, current permission; inaccessible/nonexistent both 404. No shared cache |
| POST save: prior-operation lookup | Revision by (script,accepted_by,operation_id), only after current write authorization | 0..1; unique composite operation index; no pagination | New accepted operation; same-key retries | Script FOR UPDATE serializes; exact full export/base comparison before stale check; replay returns original actor/time. No c1-based shortcut |
| POST save: evidence resolution | One owner/kind/id existence query per distinct evidence ref | 0..1 per query; registry composite PK; bounded by admitted IR, fixture has 5 refs | Operator registry insert; immutable evidence versions | Same owner as script; all checks and link writes in save transaction. Sequential per-ref queries are a potential write-side N+1 cost; not measured |
| POST save: accepted result materialization | Exact inserted revision key, same columns as GET | 1; revision PK | The same transaction's inserted row | Read-your-write before commit; response only after commit. If acknowledgement is lost, replay is authoritative |
| Migration CLI: checksum ledger | version=1 lookup after advisory transaction lock | 0..1; ledger PK | Atomic migration insert | Owner connection only; checked replay, mismatch refusal and failed DDL rollback tested |

No catalog, list, aggregate, listener GET, pagination, cache or derived read model exists.
No unimplemented endpoint is included as an implemented optimization.

## Authority, freshness and failures

PostgreSQL is the only authority. No engine/change-log watermark is needed because there is no
derived serving adapter. Reads take a shared head-row lock; accepted writes take an exclusive
one, so each response refers to one committed revision and returns exact immutable bytes.
Membership/actor access facts are checked using READ COMMITTED statement snapshots; already
authorized transactions are not a strict role-revocation commit barrier. Session SHARE locks
serialize revoke updates with the current transaction. Subsequent operations recheck all facts.
There is no public delivery route, response cache or cross-actor reused response.

Pool size 8, connect/wait 5s, statement 5s, lock 3s, idle transaction 10s and HTTP 15s guard this
experiment. These constants are not workload latency targets. Corrupt bytes/hashes fail 500;
DB/pool/deadline failures fail typed 503. Killed transaction rollback and verified-pool reconnect
are fault-injected; exact response replay across real HTTP/PG restart is integration-tested.

## Measurement boundary and DBSP decision

The following is the **prompt-2** boundary. Prompt-3 measurements follow the linked protocol;
they do not retroactively change which checks ran in prompt 2.

Decision: **defer**. Current implemented reads are indexed point lookups, without a repeated
join/aggregate read model or measured traffic. Adding an engine/CDC/state/fallback service now
has no demonstrated benefit and would add consistency/recovery obligations. This is a fit
assessment, not a measured rejection of DBSP.

Execution: latency p50/p95/p99, throughput, CPU/RAM under workload, EXPLAIN ANALYZE/BUFFERS,
cold/warm preparation, skew, pagination workload, index write/storage cost, and DBSP comparison
are **NOT_RUN** in prompt 2, per the request reserving read measurements for the next prompt.
Read frequency, representative cardinalities and hot-path ranking remain unknown. No numeric
speedup, performance acceptance or engine equivalence is claimed.

The next authorized measurement slice should first agree workload-specific latency/throughput,
resource, freshness and write-overhead targets, then use a new disposable cluster with synthetic
size/skew and the real app-role/session/owner filters. Record server/query/index/schema hashes,
seed, row counts, samples, cache preparation and raw results. Compare simpler query/index changes
(including duplicate auth trips and evidence batching if measured hot) before a bounded DBSP
proposal. Preserve complete export admission, current permissions and exact retry responses.
Targets and the baseline remain missing prerequisites; this inventory does not execute them.

## Relevant correctness comparisons already run

| Skill edge class | Actual oracle / disposition |
| --- | --- |
| Transaction boundaries, rollback | Atomic head/export/evidence writes; failed first save and failed migration leave no partial rows |
| Duplicates/concurrency | Barrier-controlled two competing writes: one acceptance, one stale; duplicate same-key writes return identical accepted response |
| Reconnect/restart | Killed DB connection before commit rolls back; retry reconnects. HTTP kill, PostgreSQL fast restart, checksum replay preserve export/actor/time |
| Tenant/access | Wrong owner read/history/write concealed; reader write denied; editor accepted actor recorded; member revocation/expired/revoked session rechecked |
| Metadata integrity / SQL time | Same c1 but changed evidence creates a different full export/revision/e1; UTC microsecond timestamp replays byte-identically |
| Sort/pagination, maintained bag/aggregate, CDC snapshot/rebuild/lag/fallback | Not applicable to this point-read slice; no such adapter/query exists. Any future addition needs its own evidence |

Residual risk: baseline/targets and scalability are unknown; READ COMMITTED access checks do
not revoke a request retroactively. No production, provider, DBSP, CDN or listener behavior
was observed.
