# Cantos API service guide

The `cantos-api` package (the one backend service, formerly `cantos-server`) now implements the Script IR contract, a local development Axum
host, PostgreSQL revision persistence and a migration CLI. It remains one modular package.
Versioned casting and immutable production-input authorization use that same authority.
Cantos does not synthesize audio: it manages works, episodes, script revisions and production
guidance, and inspects audio that external AI producers will submit
([ADR 0018](../../docs/decisions/0018-external-audio-production.md)). The audio-submission API is
proposed and not implemented. Production identity deployment remains unimplemented.
The HTTP host is one modular monolith with three surfaces, each with its own router module
(`src/http/{studio,theatre,admin}.rs`): `/api/v1/studio/*` is implemented (the pre-split `/api/v1/*`
paths remain a deprecated alias of the same handlers); `/api/v1/theatre/*` and `/api/v1/admin/*`
are reserved empty routers that answer `404 not_found` and serve no data. Shared code is limited to
middleware, error mapping, the domain modules and the PostgreSQL store. See
[decision 0015](../../docs/decisions/0015-api-surface-boundaries.md). The `cantos-server` binary keeps its name.
[`services/kanidm`](../kanidm/README.md) holds identity-provider configuration; the service does not use it yet.

## Boundaries

The [context architecture](../../docs/architecture/api-contexts.md) owns dependency direction,
transaction boundaries, compatibility aliases and the repository architecture guard.

- `script_ir/`: the pure versioned shared kernel, private validated values, canonical
  export/content/speech identity and stable-ID diff; no HTTP, SQL, UI or provider dependency.
- `contexts/editorial/`: access, evidence, accepted revisions and review. Pure append/replay/
  stale/key-reuse decisions remain separate from application transactions and SQL.
- `contexts/ingestion/`: bounded TXT/Markdown/DOCX/Script IR extraction, immutable source bytes
  and import receipts. Unknown speakers remain unresolved; prose labels and scene cues need review.
- `contexts/adaptation/`: frozen caller context, closed proposal admission, trusted evidence,
  acceptance and cancellation. Domain performs no inference; the host enables no generation provider.
- `contexts/production/`: casting/settings/rights/estimate/frozen/eligibility decisions and the
  transaction use cases that append settings, declarations, candidates and approvals. Pure
  `domain/catalog/` holds dated voice capabilities as casting vocabulary. It dispatches nothing;
  an approved snapshot is guidance that an external producer reads.
- `contexts/media/`: admission and read-only inspection of externally produced audio. The pure
  `domain/wav_inspection` reports RIFF structure, checksums and sample peak; nothing here
  synthesizes, converts or mixes. The submission use cases will live here when they exist.
- `platform/`: IDs, connection/session/migration plumbing, trusted operator tools and the
  unused optional incremental engine support. Business tables/queries stay with their context owner.
- `interfaces/http/`: thin Axum Studio mapping plus separate reserved Theatre/Admin routers.
  `wire.rs` owns JSON/MessagePack negotiation; errors stay JSON
  ([wire codec](../../docs/architecture/wire-codec.md)).

Hidden library aliases remain only for retained contexts (`adaptation`, `imports`, `production`,
`revisions`, `postgres`, `wav_inspection`, `operator`, `incremental`). The removed `tts`, `worker`, `jobs`, `postgres::durable`,
`speech_fingerprint`, `clip_reuse`, `regeneration` and `clip_probe` paths no longer exist.

The existing migrations remain the persistent compatibility boundary:

- `migrations/0001_script_revisions.sql`: atomic schema, deferred head FK, operation uniqueness
  and immutable revision/link triggers. Migration replay verifies its SHA-256 checksum.
- `migrations/0002_editorial_handoff.sql`: preserved sources, immutable editorial reviews and
  their operation receipts, added without rewriting migration 0001 or existing revisions.
- `migrations/0003_manuscript_import.sql`: exact original binary bytes, import metadata and
  immutable extraction outcomes on the existing source records.
- `migrations/0004_ai_adaptation.sql`: immutable run inputs, dispatch intent, observations,
  proposals, cancellation receipts and atomic acceptance links.
- `migrations/0005_caller_adaptation.sql`: host-owned contexts and append-only submission
  receipts; historical input decoding remains compatible.
- `migrations/0006_production_inputs.sql`: versioned settings/rights assertions and immutable
  candidates/approvals; no reservation, generation attempt or charge table.
- `migrations/0007_durable_jobs.sql`, `0008_speech_clips.sql`, `0009_durable_step_changes.sql`:
  tables of the removed in-process synthesis runtime. They are still applied (history is
  append-only), nothing reads or writes them and they are not a supported interface; retiring
  them needs a new migration after a data-retention decision
  ([ADR 0018](../../docs/decisions/0018-external-audio-production.md)).

All validated saves are immutable accepted storage revisions. They are not production approvals.
Metadata-only edits can retain c1 while changing e1 and revision identity; no digest deduplication
is performed. SQL rows are re-admitted through the canonical reader, not deserialized directly
into trusted domain values.

## Local verification and serving

Run from the repository root with Rust 1.87 and PostgreSQL binaries on `PATH`:

```sh
cargo run --locked --bin validate-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json
cargo xtask verify rust
cargo xtask verify contracts
PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" cargo xtask verify postgres
```

`verify postgres` runs `scripts/test_postgres.py`, the primitive described here. The runner removes inherited `DATABASE_URL`, `PG*` and `CANTOS_*` configuration, disables psql
startup files, and refuses optimized Python (`-O`) before starting processes. It initializes a
fresh loopback-only cluster in ignored `target/revision-evidence/`, creates a non-superuser app
role, runs the real revision and manuscript PostgreSQL suites, and then kills/restarts HTTP
and PostgreSQL processes. Recovery compares exact TXT/DOCX/failed-input bytes and import receipts
as well as the accepted script. It stops the cluster by default.
The Rust integration tests are deliberately ignored by ordinary `cargo test`; that result
alone is not database evidence. The runner needs permission to allocate PostgreSQL shared memory.

For repeatable read measurements, run `scripts/benchmark_revision_reads.py` from the root
with the same PostgreSQL binaries on `PATH`. Its [protocol](../../docs/evidence/script-revision-benchmark-protocol.md)
defines safe synthetic scale/skew, actual HTTP serving, full PostgreSQL oracle comparisons,
permission isolation, EXPLAIN plans and write/storage accounting. Candidate indexes exist only
inside disposable comparison databases, not in the production migration. Resource sampling
also needs access to `ps`/`sysctl`; a denied/missing tool is a blocker, not a zero-cost result.

For a synthetic live Studio walkthrough, build Web using [its guide](../../apps/cantos-studio/web/README.md), then:

```sh
PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" cargo xtask verify postgres --keep
cargo xtask run cantos-api --test-cluster
```

`run --test-cluster` builds `cantos-server` and starts `scripts/serve_studio.py`, which refuses any
database other than the retained disposable one. Use the printed UUID and synthetic alice token.
The runner prints the exact `pg_ctl ... stop` command for its retained cluster. Stop the HTTP host
with Ctrl-C; xtask stops only the process group it started. No production data, real
credential, provider call or object-storage write is needed.

For another isolated development database, run `cargo run --locked --bin cantos-migrate`
with an explicitly exported migration-owner `DATABASE_URL`. Provision actors, high-entropy
64-hex-character session tokens (store only SHA-256), expiry and immutable owner evidence records
through that trusted operator connection. The runtime app role must be a separate non-owner,
non-superuser with only:

```sql
GRANT USAGE ON SCHEMA public TO cantos_app;
GRANT SELECT ON actors,sessions,scripts,script_members,script_evidence,
  script_revisions,revision_evidence TO cantos_app;
GRANT INSERT ON scripts,script_revisions,revision_evidence TO cantos_app;
GRANT UPDATE(head_revision) ON scripts TO cantos_app;
GRANT UPDATE(revoked) ON sessions TO cantos_app;
GRANT SELECT ON source_records,script_reviews,script_review_operations TO cantos_app;
GRANT INSERT ON script_reviews,script_review_operations TO cantos_app;
GRANT INSERT ON source_records,script_evidence TO cantos_app;
```

Migrations do not create deployment roles or mint credentials. Never give the DDL owner
to the HTTP host. PostgreSQL superusers can bypass constraints/triggers; they are outside the
application threat boundary. The local runner's trust authentication and predictable tokens
are disposable test fixtures, not deployment guidance.

Run `cargo xtask run cantos-api` with the app-role local `DATABASE_URL` exported; it builds
`cantos-server` and sets `CANTOS_ENV=development` and `CANTOS_BIND_ADDRESS=127.0.0.1:8080`
(`--web-origin` sets `CANTOS_WEB_ORIGIN`). Default origin is the bind URL
and default static directory is `apps/cantos-studio/web/dist`; overrides are `CANTOS_WEB_ORIGIN` and
`CANTOS_WEB_DIST`. No dotenv loader exists. Remote PostgreSQL/TLS/production hosting are
explicitly unsupported by this host.

See [evidence and residual risks](../../docs/evidence/script-revision-persistence.md) and the
[affected read inventory](../../docs/evidence/script-revision-reads.md). Production-input planning
eligibility remains distinct from the sample-only local adapter below; legal/publication eligibility,
stable entity identity across revisions and the full #1/010 journey remain
separate work. Domain rules never import Narrative Forge or AI provider internals.

## Editorial handoff and operator tools

Use the migration/operator database role for these commands, only against a local development
instance. The HTTP app role above cannot provision actors or credentials. Its evidence/source
INSERT privileges serve authenticated owner-scoped manuscript imports; general evidence
registration and operator source capture remain trusted operator commands.

```sh
cargo run --locked --bin cantos-migrate
cargo run --locked --bin cantos-operator -- create-creator
cargo run --locked --bin cantos-operator -- issue-token ACTOR_ID 1
cargo run --locked --bin cantos-operator -- revoke-tokens ACTOR_ID
cargo run --locked --bin cantos-operator -- register-evidence OWNER_ID rights RIGHTS_ID 'pending creator assertion'
cargo run --locked --bin cantos-operator -- record-source OWNER_ID SOURCE_ID 'preserved source reference' /path/to/source.txt
```

Issuance accepts 1–720 hours, prints the new 64-hex credential once, and stores only its SHA-256.
Keep the credential in a local secret store; use the existing Studio session exchange to sign
in. Rotation means issue a replacement and revoke existing credentials explicitly. Revocation
of all tokens for an actor includes the replacement if issued before `revoke-tokens`.
The source command preserves UTF-8 text bytes (including NFD/CRLF), bounded to 1 MiB, and records
an immutable source registry link plus SHA-256 in one transaction. Register the remaining
rights/generation/asset references explicitly; registration is not rights clearance. Source
correction requires a new ID. Existing registry descriptions are not overwritten.

[Studio API extensions](../../packages/contracts/studio-v1.md#editorial-handoff-extensions) supply
currently authorized history summaries, linked private sources and owner editorial review.
Reviewing an already reviewed revision returns its first review, even after the head changes.
Reviewing an older unreviewed revision conflicts. Reviews bind immutable revision identity and
c1/e1; they do not approve production or publication. Existing Studio controls consume the
original save/read contract; the new handoff operations have no browser controls yet.

The [handoff evidence](../../docs/evidence/editorial-handoff.md) records the migration upgrade,
operator CLI, preservation, permission, corruption and rollback checks. Run the existing
PostgreSQL suite to include these cases; its ignored tests also cover the additive import migration.

## Manuscript intake

The [manuscript v1 contract](../../packages/contracts/manuscript-v1.md) supplies authenticated import,
reopen and exact-original download routes. The source registry, bytes, metadata, checksum and
parse outcome commit together. An identical actor/operation retry returns its first receipt;
changing bytes or metadata under that operation conflicts. A parse failure preserves the source
and its actionable error. No import creates a ready script, changes a revision head or grants
publication permission. Structured scripts pass the existing Script IR validator and retain
their canonical representation for a later explicit revision save.

The bounded local source documents use existing PostgreSQL `source_records`, as recorded in
[ADR 0005](../../docs/decisions/0005-manuscript-import.md). Media/audio remain object storage.
Larger sources, a storage adapter and production identity require their own reviewed work.
DOCX formatting, embedded content and omitted annotations are visible conversion limitations;
tracked changes and suspected legacy Vietnamese encodings require an explicit future choice.
See [import evidence](../../docs/evidence/manuscript-import.md) for executed checks and limitations.

## Caller-owned adaptation tools

The [adaptation contract](../../packages/contracts/adaptation-v1.md) exports immutable context and accepts
caller-created proposals on this same authenticated development host. The user-controlled
Gemini / ChatGPT / Codex host owns generation. Cantos performs no inference, has no provider
transport or credentials, and creates no provider attempt for a c1 context.

Use the [four HTTP/CLI tool schemas](../../packages/contracts/adaptation-tools-v1.json): create context,
reopen exact context, submit proposal and read editorial review. The CLI requires an explicit
numeric loopback host/port and an existing process-local `CANTOS_SESSION_TOKEN`; it neither
provisions credentials nor connects a cloud account. All commands use the backend's current
session, source ownership and script permissions. The tool surface has no acceptance mutation.

A context pins source ID/SHA-256/extractor version, ordered blocks/warnings, optional exact base
export, rights assertion, prompt/contract versions and digest. It requires a parsed owned import
with nonblank rights holder, permission evidence and usage scope. These are recorded claims,
not legal adjudication. Publication permission remains unknown. Source/model strings have no
host or tool privileges. No context operation advances a revision head.

A submission pins the exact context digest and a separate operation ID. Host/provider/model/
config/prompt, supplied usage and cost are caller-declared, unverified evidence; absent values
stay unknown. Closed output admission constructs trusted IDs/provenance and uses the actual
Script IR validator. An invalid domain result stores an immutable receipt while awaiting a new
explicit correction. The first valid proposal is immutable; later distinct submission conflicts.
Exact context/submission retries reconcile their first result, including after acceptance.

Studio opens the saved context/proposal and supports source comparison, findings, bounded JSON
correction and explicit review/acceptance. Acceptance uses the same atomic revision-save/CAS/
idempotency path. Source and prior exports survive invalid submissions, lost replies and failed
commits. Full scene/character editing belongs to #4.

Application bounds are 24 KiB/256 source blocks, 96 KiB prepared text, 256 KiB proposal output,
1000 proposed nodes and 16 KiB/depth8/256nodes caller configuration. Context/submit HTTP bodies
are bounded to 64 KiB/1 MiB. The CLI bounds stdin/request to 1 MiB, response to 8 MiB and wait to
10 seconds, with no proxy, redirect, automatic retry or automatic operation ID. An uncertain
write must be retried with the exact operation and payload. These are pipeline bounds, not an
external model tokenizer/context guarantee. There is no automatic splitting or truncation.

The runtime role needs the existing adaptation permissions plus the append-only submission table:

```sql
GRANT SELECT, INSERT ON adaptation_runs,adaptation_attempts,adaptation_observations,
  adaptation_proposals,adaptation_cancellations,adaptation_acceptances,
  adaptation_submissions TO cantos_app;
GRANT UPDATE(status,problem,dispatch_deadline,updated_at) ON adaptation_runs TO cantos_app;
```

Migrations 0004/0005 retain the existing source/revision authority and add no UPDATE/DELETE rights
for its immutable facts. a1 frozen history keeps its exact decoder/digest; c1 has a separate type
and fingerprint. Retired start/provider endpoints fail unavailable/null, and historical queued/
expired attempts are never redispatched. See [ADR 0006](../../docs/decisions/0006-ai-script-adaptation.md)
and [current evidence](../../docs/evidence/ai-script-adaptation.md). Historical local-adapter
checks are explicitly superseded. Actual CLI/socket/PostgreSQL proof is required; real external
model generation and output quality remain separately NOT_RUN for this data-pipeline slice.

## Casting and production-input authorization

The [production v1 contract](../../packages/contracts/production-v1.md) adds versioned casting/settings,
script-scoped rights/budget declarations, reviewable immutable candidates and separate exact
owner approval. `prepare_inputs` resolves one complete accepted export, required claims and
the exact catalog entry's supported controls; `evaluate_snapshot` computes staleness and rights/time/budget findings
from current facts. A content digest alone cannot authorize inputs because c1 excludes rights
and provenance. The separate `production-p1` scope pins complete revision identity/e1, catalog/
adapter/output versions, settings, required rights versions, actor/time metadata and planning rate.

Every read applies current access. Every mutation rechecks the actor/session/script scope and
CAS/digest inside the existing locked transaction. Exact same-operation retries return the
immutable receipt; changed payloads conflict. State reads return newest twenty summaries;
an authorized known snapshot read checks its full frozen document against persisted historical
revision, settings and rights receipts before reevaluating current eligibility. Frozen candidates
survive later edits while eligibility fails closed; a blocked candidate can be inspected but
cannot acquire an input/cost approval.

The runtime app role additionally needs only:

```sql
GRANT SELECT, INSERT ON production_settings,production_rights_claims,
  production_snapshots,production_approvals TO cantos_app;
GRANT EXECUTE ON FUNCTION production_lock_actor(text),
  production_lock_member(text,text) TO cantos_app;
```

No UPDATE/DELETE permission is added for these settled records or authority tables. PostgreSQL
row locks require UPDATE privilege; the migration-owner's two narrowly scoped SECURITY DEFINER
helpers obtain actor/member share locks without granting the runtime authority-edit permission.
They use fixed `pg_catalog` search path, fully qualified tables, no dynamic SQL and revoked
PUBLIC execution. Their only granted outputs are active-actor presence and current member role.
Migration replay verifies the same recorded checksums. PostgreSQL remains authoritative;
no additional service/read engine, accounting ledger or worker is introduced by #5.
The catalog retains synthetic reference controls and now adds an explicit local inspection
entry, always with billable dispatch unavailable. Rates and rights are
private-planning actor assertions, not provider/legal verification. Estimates are neither
reservations nor charges. No generation/payment/audio occurs in #5.

Run the existing disposable PostgreSQL runner for production oracle/restart hooks along with
the current suites. [Casting evidence and affected reads](../../docs/evidence/casting-production-inputs.md)
names actual outputs, limits and DBSP deferral; [ADR 0007](../../docs/decisions/0007-casting-production-inputs.md)
remains proposed for production adoption. Real adapter/dispatch and settlement work follows only
in its explicitly authorized owning slice.

## Experimental incremental core

The `dbsp-experimental` Cargo feature (off by default) compiles only the engine-independent
`platform/incremental` core and its DBSP circuit. After [ADR 0018](../../docs/decisions/0018-external-audio-production.md)
no read model uses it: the durable job-summary view was removed with the in-process synthesis
runtime ([ADR 0017](../../docs/decisions/0017-experimental-dbsp-job-summary.md) is superseded for
that consumer). `cargo xtask verify dbsp` still lints and tests the feature. Removing the feature
is a separate decision.

## WAV PCM inspection

`inspect-wav` reads one local file and prints a JSON report of its RIFF/WAVE structure: the chunk
table, declared `fmt ` fields, SHA-256 of the file and of the `data` payload, every structural
error and, for structurally valid integer PCM only, frame count, duration and per-channel sample
peak. It never converts, uploads or calls a provider, and it writes nothing.

```sh
cargo run --locked --bin inspect-wav -- path/to/file.wav
cargo run --locked --bin inspect-wav -- path/to/long-master.wav --max-bytes 2147483648
cargo build --locked --bin inspect-wav
python3 scripts/reference_wav_inspection.py
```

Exit `0` means the structure is valid and `1` that it is invalid; both print the report. Exit `2`
covers usage errors, unreadable or non-regular input and inputs above `--max-bytes` (default
1 GiB); they print only a message on stderr. "Valid" is a structural statement. The report lists
loudness, true peak, clipping, listening quality and publication readiness as `not_assessed`;
it records no QC result and approves nothing. The oracle script compares the CLI with Python's
`wave`/hashlib (and ffprobe when installed) on seeded synthetic files in a temporary directory.
The [inspection evidence](../../docs/evidence/wav-pcm-inspection.md) defines every field, error
code, limit and residual risk.

## Script IR diff

`diff-script` compares two Script IR files of one work, adaptation and episode by stable ID and
prints a versioned JSON report (`cantos-script-diff-1`). It lists added, removed, modified and
moved acts, scenes, dialogue, cues, characters and provenance sources, each field change with its
aspect (text, speaker, delivery, pronunciation, order, cue, rights, provenance and so on), and
keeps reporting provenance and rights changes when the content digests are equal. A moved line is
one `moved`, never a deletion plus an addition. It only reads: both inputs go through the same
admission as `validate-script`, each is refused above 2 MiB, nothing is written, and no provider
or revision store is involved.

```sh
cargo run --locked --bin diff-script -- before.json after.json
cargo run --locked --bin diff-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json packages/contracts/fixtures/script-ir/0.1.0/accept/provenance.json
```

Exit `0` means a report was printed, whether or not the versions differ (`identical`,
`summary` and `changes` say). Exit `1` means an input was rejected by Script IR admission or the
two are not versions of the same work, adaptation and episode. Exit `2` covers usage errors and
unreadable, non-regular or oversized input. Failures print only a message on stderr. The report is
a review aid and decides nothing about audio reuse; the
[diff evidence](../../docs/evidence/script-diff.md) defines every field, move rule, limit and
residual risk.
