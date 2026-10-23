# ADR 0003: Local authenticated immutable Script IR revisions

- Status: proposed
- Date: 2026-10-10
- Decision owner: repository maintainer
- Related issue or PR: [#1](https://github.com/loveoverflowcom/cantos/issues/1)
- Supersedes / superseded by: N/A

## Context

Prompt 1 supplies private validated Script IR values and independent c1 goldens. A concrete
Studio consumer now needs complete durable exports, access checks, stale-write rejection and
recovery after ambiguous responses. c1 excludes rights/provenance, so it cannot identify an
accepted storage revision alone. This record describes implemented local choices; it does not
accept ADR 0002 or a production deployment.

## Decision

Keep one server package with inward pure Script IR/access/save rules, PostgreSQL transactions
and an Axum HTTP shell. Add one shared serde-only wire crate for the concrete Axum and Leptos
consumers. Store exact canonical complete exports as BYTEA with unchanged c1 and domain-separated
e1, actor, UTC acceptance time, operation key/base and owned evidence links. Append every
validated acceptance, including metadata-only changes; do not deduplicate content digests.

A row lock serializes writes to each head; expected revision rejects competing new writes.
Actor-scoped operation uniqueness plus full-export/base comparison reconciles retries before
checking a moved head. Inserted revisions, evidence links and the head change commit together.
Accepted rows/links have immutable triggers, and the HTTP app role lacks update/delete/DDL
rights. Atomic migrations use a checksum ledger and transaction advisory lock.

Use operator-issued expiring/revocable development sessions hashed in PostgreSQL, exchanged
for same-origin HttpOnly/SameSite=Strict cookies. Enforce all read/write access in PostgreSQL
shell transactions. Local-only host/DB configuration and separate app/migration roles bound
this experiment. Identity-provider adoption, HTTPS/Secure cookies and production provisioning
remain undecided.

Pinned adapters are Axum 0.8.4, Tokio 1.47.1, tokio-postgres 0.7.15, deadpool-postgres 0.14.1,
tower-http 0.6.6 and uuid 1.18.1. The browser is Leptos 0.7.8 CSR, gloo-net 0.6.0 and
web-sys 0.3.77, built by Trunk 0.21.14/wasm-bindgen 0.2.100 on Rust 1.87.0. Official
[Axum](https://docs.rs/axum/0.8.4/axum/),
[tokio-postgres](https://docs.rs/tokio-postgres/0.7.15/tokio_postgres/),
[Leptos](https://docs.rs/leptos/0.7.8/leptos/) and
[PostgreSQL 17 locks](https://www.postgresql.org/docs/17/explicit-locking.html) informed
the boundaries; downloaded pinned crate source was inspected where doc retrieval failed.
Compilation/integration results support these versions, not a vulnerability audit.

Use a small complete-JSON Studio editor to consume the actual contract now. Its pure reducer
owns request tickets, immutable snapshot retries and draft preservation; a concrete HTTP
adapter is provided through context. No speculative fake-port trait or generated binding is
introduced. A typed vi-VN/en catalog suffices for fixed messages in this slice; plural/select
formatting should adopt appropriate resources when needed. Token JSON generates Web CSS with
source version/hash; native mapping waits for a real CMP consumer. Page-local theme controls,
system fonts, native HTML, visible focus and no animation keep the first surface reviewable.
Some Web-only layout geometry remains in CSS; logical dimensions and colors use semantic roles.
No extra view/CSS formatter is installed; Rustfmt, WASM Clippy, token drift checks and visual
inspection are the current gates. Development debug symbols are disabled to limit local disk
usage, without changing release behavior.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Exact BYTEA export + metadata | Preserves full admitted bytes and oracle compatibility | Decode/validate on each read | Selected for integrity; measure read cost separately |
| JSONB only / c1 deduplication | Convenient queries / fewer rows | Reformatting, metadata collision, lost accepted identity | Rejected |
| Lock head + expected base + durable operation key | Clear serialization, replay and rollback | Per-script contention | Selected; workload unknown |
| In-memory/mock persistence | Fast local scaffolding | No restart/transaction evidence | Rejected for this task |
| External auth service / separate domain service | Deployment capabilities | Unselected infrastructure and new service boundaries | Deferred pending production consumer |
| DBSP derived reads | Candidate maintained joins/aggregations | No measured workload or current aggregate consumer | Deferred; PostgreSQL remains authoritative |

## Consequences

- Product and user impact: live validated save/read and private immutable history; no importer,
  production approval, public delivery or audio.
- Compatibility and migrations: initial atomic migration 0001; checksum changes fail closed.
  Full-export e1 is new, Script IR/c1 unchanged. No down migration deletes accepted facts.
- Operations, storage and production cost: local PG17.4 tests with synthetic records, bounded
  pool/timeouts, no production/paid services. Retained test clusters are explicitly stoppable.
- Implementation and recovery requirements: current actor/access checks, row lock, exact replay,
  integrity admission, verified pool recycling after killed sockets and preserved ambiguous UI
  retry identity. Session locks serialize revocation with an in-flight transaction; role/actor
  changes affect subsequent checks, not a strict commit barrier for already authorized writes.
- Deferred questions: production identity/TLS/operator APIs, legal rights eligibility, stable
  entity continuity across documents, durable browser recovery, full editor accessibility and
  measured read/write sizing.

## Validation and revisit conditions

[Evidence](../evidence/script-revision-persistence.md) records real PG migration/rollback,
concurrency, killed connections and HTTP/PG restart plus native Safari interaction observations.
[Read inventory](../evidence/script-revision-reads.md) maps the implemented consumers;
[prompt-3 measurements](../evidence/script-revision-performance.md) retain the existing PG path.
The [conditional DBSP decision](https://github.com/loveoverflowcom/cantos/blob/c3acd81bfa94fe29bf4944dcafd5520dd0d50cdd/docs/evidence/script-revision-dbsp-decision.md) records DEFER and
admission criteria for a future experiment, without changing this ADR's proposed status. Revisit the
lock strategy at measured contention, auth at production deployment, wire versions at a new
consumer and e1 at any canonical-export policy change. Neither this proposed ADR nor a passing
local run establishes the complete issue #1 or 010 acceptance gate.
