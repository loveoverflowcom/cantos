# Actor identity registry

PR1 implements provider-neutral internal identity management in the existing modular backend.
This registry never creates an `ActorContext` or verifies claims. The separate opt-in
[Studio OIDC boundary](studio-oidc.md) now owns credential/session verification; development
sessions remain available when it is disabled. Theatre and Admin credentials remain separate work.

## Ownership and consumers

`services/cantos-server/src/platform/identity` owns pure values/decisions, `IdentityRegistry`,
`IdentityService` and the PostgreSQL adapter. Platform already owns `actors` and `sessions`.
Explicit operator registration/linking uses the production service/adapter; application tests
supply an atomic in-memory registry and an unavailable registry. Existing actor creation and
session row decoding also validate `ActorId`. The architecture checker enforces domain purity
and prevents business contexts reading the registry table.

No new service, crate, gateway, directory or provider SDK is introduced. A verifier port without
a concrete consumer would add an unearned abstraction; The separate [Studio OIDC slice](studio-oidc.md) introduces verification with its real
BFF consumer and signed HTTP mock, while the general registry stays provider neutral.

## Identity contract

| Value / operation | Implemented rule |
| --- | --- |
| `ActorId` | Existing opaque actors.id: 1–64 ASCII lowercase letters/digits/`_`/`-`. Preserve spelling, keys, ownership and history. New operator IDs remain UUID v4 text. |
| `ExternalIdentityKey` | Exact case-sensitive `(issuer, subject)`; no normalization/email/username. Nonblank UTF-8 without controls; issuer ≤1024 bytes, subject ≤512 bytes. This is metadata, not provider proof. |
| Registration | Atomically create actor+binding for an unbound key. Replay returns the bound active actor even with a different candidate; inactive binding and occupied candidate for an unbound key are typed refusals. |
| Linking | Explicitly attach a key to an existing active actor. Same pair is idempotent; missing/inactive target or key owned by another actor is rejected. No transfer, merge, unlink or email matching. |
| Lookup | Optional actor snapshot and active status; grants no session, role or authorization. |
| Errors | Typed input/rejection errors, `Unavailable` (including ambiguous commit acknowledgement), `CorruptRecord`; no raw SQL/private keys in error text. |

Constructors and serde share validation. SQL uses explicit Unicode whitespace/control ranges
rather than locale-dependent POSIX classes. Composite bounds keep B-tree keys small. Protocol
issuer/client validation belongs at PR2's credential boundary.

## Persistence, isolation and recovery

Additive `0018_actor_external_identities.sql` creates a C-collated exact primary key, actor FK,
timestamp and actor index; triggers reject update/delete/truncate. Original migrations 0001–0014
and settled records are unchanged. Versions 0015–0017 belong to concurrent Storage/Publication/
Discovery work. Reconcile the ordered checksum registry when merging; independent lower versions
can be applied after 0018 without changing original checksums or renumbering deployed migrations.

Mutations take a transaction advisory lock from a length-framed SHA-256 key digest, read facts,
call pure decisions and commit actor+binding together. Lock collisions merely serialize unrelated
keys; the SQL primary key establishes identity. Actor share locks serialize disable against the
decision; primary keys reject competing candidate IDs. Rejection rolls back any speculative actor.
The existing local pool/statement/lock deadlines apply; transactions make no provider call.
A lost commit response returns `Unavailable`; retry the same exact key to recover the winner.
No exactly-once request or external account-creation claim is made.

Read inventory: `lookup` joins one exact binding to its actor; registration reads one binding
and candidate ID; linking reads one binding and target ID. Primary keys cover all predicates;
the actor index supports future reverse lookup, which is not exposed here. Reads are currently
operator-only, frequency unknown; no cache or aggregate exists. Writes insert bindings/actors,
and active-state changes remain authoritative in PostgreSQL. DBSP decision: reject for this
bounded point-lookup workload; no maintained aggregate justifies engine state/lag/rebuild cost.
The synthetic baseline uses 2000 bindings, 1 actor, 20 warm sequential service lookups and
EXPLAIN(ANALYZE,BUFFERS). Target: exact case/issuer results under existing 5s statement/3s lock
timeouts, not a production latency SLO. Load, cold-cache, p95/p99 and runtime-role acceptance
remain successor work before OIDC registration activation; no speedup is claimed.

The uncomposed [Turso candidate](server-turso-schema.md) (`TursoIdentityRegistry`, #333.4) keeps
this contract inside the one `server.db` registry: one `BEGIN IMMEDIATE` transaction on the
owner's serialized writer replaces the advisory and share locks, a lost acknowledgement is
`Unavailable` and is reconciled by replaying the exact key, and its file-backed tests and
invariant mapping are recorded there. PostgreSQL remains the serving adapter.

PUBLIC and `cantos_app` receive no registry grant. Only explicit operator DB privilege permits
writes today. PR2 must design least-privilege credential-driven grants before activating runtime
registration/linking. No identity HTTP route is exposed in PR1.

## Reference audit and successor boundaries

Fresh Cantos develop: `df7d62f`; fresh VOT Workspace develop: `f3fea641`. VOT's identity/registry
core separates exact external keys from canonical IDs; its Kanidm mapping separates DTO
interpretation from transport; Console proof separates verified credential from admin authority.
Cantos keeps these principles while retaining existing actor IDs. No VOT code, resource-ID
system, directory/group framework, gateway headers or live bootstrap is copied.

PR2 must verify OIDC evidence before creating Studio authority and authorize account linking
with old-account control plus fresh external-account proof, or explicit privileged operator intent.
PR3 needs separate typed Listener credentials; a shared ActorId never shares Studio cookies/roles
or producer grants. PR4 owns platform roles/admin audit. Account merge/unlink/recovery and provider
deletion/issuer migration remain deferred. See [evidence](../evidence/identity-registry.md) and
[handoff](../archive/handoffs/identity-authentication-handoff.md).
