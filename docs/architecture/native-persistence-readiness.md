# Native persistence ownership and runtime prerequisites

This is the source ownership prerequisite for [#332.2](https://github.com/loveoverflowcom/cantos/issues/332),
under the [selected native architecture](native-turso-refactor.md). It describes the current
PostgreSQL source and the selected destination of its data. This inventory supplies no importer,
service schema, credential verifier or runtime switch.

## Per-table disposition

The exhaustive [native table ownership manifest](../../services/cantos-server/schema/native-table-ownership.json)
lists each of the 82 current SQL tables and the two metadata tables declared by the PostgreSQL
migrator. The existing repository gate checks the manifest against all current schema SQL files
and the migrator's literal declarations. New, missing, duplicate or unrecognized table declarations
require review; a newly added table does not inherit permission to migrate from its prefix.

| Disposition | Source tables | Destination and restriction |
| --- | --- | --- |
| `server` | 67 tables, listed individually in the manifest | Server owns the corresponding business data in `server.db`. Preserve stable ActorId, exact external identity bindings, immutable revisions/digests, ownership, rights, receipts and listener sync semantics. A table name alone defines neither its Turso DDL nor a lossless conversion. |
| `replace-provider-auth` | 14 tables, listed individually | Do not port their provider sessions, access tokens, PKCE verifiers or enrollment state to either native DB. #334 owns fresh Identity state; #335 owns new Server sessions and resumable binding. Retain legacy source/data for authorized migration and recovery until replacement acceptance. |
| `development-only` | `sessions` | This source table mixes `development` and `studio_oidc` rows. Only development-session behavior may exist in dev/test composition; **no existing `studio_oidc` row/token is eligible**. The manifest does not authorize copying even development rows or enable them in production. #335 supplies the new production session schema. |
| `replace-registry` | `cantos_schema`, `cantos_migrations` | Keep the PostgreSQL epoch/checksums as legacy recognition evidence. Each native owner uses the shared foundation's own checked registry; never replay the PG registry as a Turso migration. |

None of these source tables moves wholesale to `identity.db`. Identity subjects, credentials,
email/proof/recovery/passkey state, security events and `instance_epoch` need the new #334
schema and its own migration registry. Server retains ActorId and exact subject bindings;
an email match grants neither identity binding nor Studio/Admin/resource authority.

The manifest supplements the existing context-level SQL dependency guard in
[`check_repository.py`](../../scripts/check_repository.py); it does not change those context
owners or replace their contracts. Its source recognizer handles the repository's literal
`CREATE TABLE public.name (...)` form, including the migrator's Rust SQL string. It is not a
general SQL/Rust parser, production data inventory, export allowlist or secret detector.
Dynamic/generated declarations and future source locations need explicit review. These source
checks conservatively refuse backslash-escaped quote forms rather than infer a SQL string
escape mode. They do not establish database constraints, migration parity or runtime conformance.
Supported unquoted source identifiers use PostgreSQL lowercase folding; manifest names must
be canonical ASCII lowercase, and duplicate detection uses the folded identity. The migrator
is scanned as literal Rust source, so SQL comment markers (`/*` or `--`) anywhere in that
source are conservatively refused. This avoids silently missing comment-separated DDL without
claiming to parse or unescape Rust SQL literals; even a harmless future marker needs review.

## Actual serving path

Inspect these owners before attempting the native connection:

| Boundary | Implemented source | Consequence for the next slice |
| --- | --- | --- |
| Shared embedded store | [`cantos-store-turso`](../../packages/store-turso/README.md) supplies `OwnedDatabase`, checked startup/migrations, serialized writes, typed commit outcomes and verified backup | Identity's opt-in security-state shell consumes this owner. Server does not. Reuse the owner API; engine tests alone do not establish serving/auth runtime acceptance. |
| Process entrypoint | [`cantos-server`](../../services/cantos-server/src/bin/cantos-server.rs) reads `DATABASE_URL`, calls PostgreSQL `verify_schema`, constructs `Store::new`, then composes the [one-listener, per-surface HTTP host](server-http-composition.md) | Selecting `CANTOS_THEATRE_NATIVE_ISSUER` still configures the existing provider adapter; it does not start or call Cantos Identity. |
| Application storage | [`Store`](../../services/cantos-server/src/application.rs) contains `deadpool_postgres::Pool`; [`platform::postgres`](../../services/cantos-server/src/platform/postgres/mod.rs) owns connections and concrete transactions | A new file flag or disconnected actor DB would not replace this durable path. All context adapters and cross-context transaction fences must be ready before #333.9's atomic flip. |
| HTTP/session admission | [`Studio admission`](../../services/cantos-server/src/adapters/inbound/http/studio/admission.rs), [`bearer`](../../services/cantos-server/src/platform/bearer/mod.rs) and its [`PostgreSQL adapter`](../../services/cantos-server/src/platform/bearer/postgres.rs) | Preserve provider checks, final transaction admission, purpose, roster, ActorId, expiry and resource fences until the accepted native verifier/session path replaces them. SessionPolicy reuse is not credential verification. |
| Durable business writes | Context-owned adapters under [`contexts`](../../services/cantos-server/src/contexts), e.g. editorial revision/receipt/head updates and listening state/device-sequence/receipt updates | Preserve each atomic unit, late authority recheck and unknown-outcome reconciliation; do not translate PostgreSQL locks/functions by text substitution. |
| Current identity | [`Studio OIDC`](../../services/cantos-server/src/platform/studio_oidc), [`Theatre identity`](../../services/cantos-server/src/platform/theatre_identity), [`registration configuration`](../../services/cantos-server/src/platform/theatre_registration/config.rs), [`Admin`](../../services/cantos-server/src/platform/admin) | Kanidm/OIDC introspection, SMTP `KanidmIssuer`, PostgreSQL registration/audit/revocation and optional Firebase enrollment still have genuine consumers. |
| Native Identity | [`cantos-identity`](../../services/cantos-identity/README.md) has a default pure core and opt-in file-backed shell | Instance/subject state, disable events, mailbox-proof lifecycles and bound receipts use the shared owner. Proof-secret verification grants no setup/login authority. With the opt-in `service` feature it also persists Argon2id credentials and verified emails and ships the `cantos-identity` executable with Theatre signup, password authentication and an OS-peer-authenticated private Unix-socket RPC. It yields authentication facts only: no Studio passkey, recovery, Server session or production composition. |

## Ordered prerequisites for a real connection

1. Reuse the shared [`cantos-store-turso`](../../packages/store-turso/README.md)
   owner/migration/transaction API supplied by [#341](https://github.com/loveoverflowcom/cantos/pull/341)
   on the integration branch. Its Identity security-state consumer is implemented; Server and
   Identity runtime composition below are still missing. Do not copy it or create a competing owner. Engine tests alone do not
   establish a service consumer.
2. Complete #332's draft `cantos.identity.v1` RPC contract, compatible Tonic/Prost and credential
   library qualification. A generated DTO is not a sealed authentication fact.
3. #333 owns single-Server composition, the complete `server.db` schema/invariant mapping,
   context adapters and one atomic runtime flip. The existing
   [`IdentityRegistry`](../../services/cantos-server/src/platform/identity/ports.rs) and
   [`IdentityService`](../../services/cantos-server/src/platform/identity/application.rs) provide
   an eligible actor/binding adapter seam **inside that owner**, not a reason to open a second
   partial `server.db`. Its real tests must cover exact-key replay, no losing actor, inactive
   targets, immutable binding, concurrent writes, reopen and unknown-commit reconciliation.
4. #334 supplies the real Identity DB/credential lifecycle, `AuthenticationFact`, security events,
   `instance_epoch`, executable and authenticated UDS RPC (#334.7). #335 can then verify bound
   responses, reuse SessionPolicy, persist fresh sessions/bindings/checkpoints and restore
   first-party app journeys. Timeout/cancel remains an unknown effect until receipts reconcile it.
5. #336 accepts two production binaries against two run-owned file-backed databases, real
   transaction/reopen/fault and revocation/restore tests, authenticated RPC, browser/device
   journeys and actual D1 Kani runner evidence. Model/fake and file-backed evidence remain separate.

The source inventory is checked by `cargo xtask verify repository`, already selected by
`cargo xtask preflight --changed-only` and the existing repository CI job. It adds no gate,
database connection, service startup or independent orchestration command.

## Retirement and local data

Keep PostgreSQL/Kanidm source, dependencies, configuration and recovery/export evidence while
the default serving path above consumes them. Remove active legacy paths only alongside the
accepted replacement and its real tests. No broad cleanup request authorizes deleting databases,
WAL, volumes, backups, accounts, media or installed services. Preserve Firebase Functions/App
Distribution, Drive, Forge and producer tools; Firebase Auth retirement does not retire them.

Production count/size inventory is separate from this source inventory. Without the complete
authorized inventory and the owner's confirmation, migration remains Tier B. No live mail,
provisioning/reset, export/import, cutover or deployment follows from this document.
