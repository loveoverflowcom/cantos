# Cantos API service guide

The `cantos-api` package (the one backend service, formerly `cantos-server`) now implements the Script IR contract, a local development Axum
host, PostgreSQL revision persistence and a migration CLI. It remains one modular package.
Versioned casting and immutable production-input authorization use that same authority.
Casting can bind Google Cloud TTS voices, and an opt-in probe can synthesize one frozen line;
production identity deployment, workers, attempt persistence and provider dispatch are not
implemented.
The only HTTP surface today is the Studio v1 API; the listener (Theatre) API is planned as a separate
route group and permission context in this same service. The `cantos-server` binary keeps its name.
[`services/kanidm`](../kanidm/README.md) holds identity-provider configuration; the service does not use it yet.

## Boundaries

- `script_ir/`: private validated values, version admission, canonical export/content/speech.
  No HTTP, SQL, UI or provider dependency.
- `revisions.rs`: pure access table and append/replay/stale/key-reuse decisions.
- `imports.rs`: bounded deterministic TXT, Markdown, DOCX and Script IR extraction.
  Unknown speakers remain unresolved; prose labels and scene cues require review.
- `adaptation.rs`: closed proposal admission, bounded source context, trusted evidence bindings,
  source coverage findings and complete Script IR semantic validation. Domain decisions do
  not import provider transport; the development host has no enabled generation provider.
- `production.rs`: pure supported-control validation, per-dialogue pronunciation/performance
  resolution, integer planning estimates, tagged input digest and current rights/approval gate.
  Its catalog preserves historical reference entries and adds the explicit eSpeak local
  inspection capability plus the explicitly research-only VieNeu Nano profile; no billable dispatch exists.
- `postgres/production.rs` and `http/production.rs`: script-scoped append-only settings,
  declarations, candidates/approvals and thin authenticated route mapping. Current permissions,
  accepted head and evidence are rechecked inside the same PostgreSQL transactions.
- `postgres.rs`: current actor/session/access facts, transactions, locked heads, complete
  immutable bytes, evidence ownership links and integrity checks on every load.
- `http.rs`: thin Axum mapping over the [Studio v1 contract](../../packages/contracts/studio-v1.md).
- `http/wire.rs`: JSON/MessagePack negotiation for the head, pinned revision, save and validation
  routes; extractors settle the representation before the store is called and errors stay JSON
  ([wire codec](../../docs/architecture/wire-codec.md)).
- `script_ir/diff/`: pure stable-ID comparison of two admitted scripts of one episode; `diff-script`
  is its only shell. No HTTP, SQL, UI, provider or file-system dependency.
- `wav_inspection/`: pure, bounded RIFF/WAVE integer PCM inspection over bytes. No HTTP, SQL,
  Script IR, provider or file-system dependency; consumed by `inspect-wav` and `tts/audio`.
- `tts/`: receipt inspection over existing frozen documents, conservative recovery advice,
  bounded WAV bytes and concrete eSpeak NG 1.52.0 and research-only VieNeu Nano local adapters. `inspect-tts-receipt` is
  read-only; `probe-local-tts` explicitly invokes the installed local runtime and retains private
  sample intent/input/receipt files. Neither establishes current production permission, durable
  jobs, reservations or artifact acceptance. See the
  [local receipt contract](../../packages/contracts/tts-receipt-v1.md).
- `tts/google_contract.rs`, `tts/google.rs` and `tts/google_http.rs`: the Google Cloud TTS
  identity and dated vi-VN capabilities used by casting and the receipt audit; the pure planner,
  receipt-v1 classification and audio admission; and the one-request HTTPS shell.
  `probe-google-tts` is the only caller. No SQL, route or Studio code change.
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

## Local TTS sample probe and receipt inspection

The additional [neural research adapter](../../docs/evidence/local-neural-tts.md) uses a
separately installed pinned VieNeu Nano runtime/model. Production is refused before runtime
or model I/O. Its exact two presets and default controls are research-only; current live and
verification results are recorded in that evidence rather than inferred from source code.

```sh
cargo run --locked --bin probe-neural-tts -- --research-only /absolute/frozen-document.json dialogue-id /absolute/private/runtime/bin/python /absolute/private/pinned-model /absolute/new-private-output-directory
cargo test -p cantos-api --test tts_neural --test tts_neural_receipt --locked
```

The neural probe saves create-only intent, frozen document, research purpose, receipt, inspected
PCM/hash and resource observations. Existing destinations are refused and errors retain
uncertainty without retries. It installs/downloads nothing, sends no input to a cloud service
and creates no accepted asset. The fixed helper verifies local model/SDK/preset identity,
rejects unsupported text/phonemes and captures mono 24 kHz PCM WAV on stdout under resource
bounds. Consult [the research comparison](../../docs/evidence/local-neural-tts-research.md) and
[ADR 0010](../../docs/decisions/0010-local-neural-tts-research.md) before installation. Heavy runs
need a coordinated execution window; preserve the prior eSpeak evidence described below.

The user-authorized local selection is eSpeak NG 1.52.0, invoked as a separate installed
executable. Its exact adapter identity is `espeak-ng-local` / `espeak-ng-1.52.0` /
`espeak-ng-local-v1`; output contract `wav-pcm-s16le-mono-22050-v1` preserves native 22,050 Hz
mono signed 16-bit PCM. Supported voices are `vi`, `vi-vn-x-central` and `vi-vn-x-south`, and
languages are `vi` and `vi-VN`. The current adapter admits rate 1000 permille, pitch zero,
emotion `neutral` and intensity zero; other controls are rejected before invocation.
Pronunciation uses the existing resolved spoken text. Installed version and exact voice
availability are checked for each run, with bounded process time, I/O and private regular-file
output. The adapter installs nothing and silently substitutes no voice or output format.

```sh
cargo run --locked --bin probe-local-tts -- /absolute/frozen-document.json dialogue-id /absolute/path/to/espeak-ng /absolute/new-private-output-directory
cargo run --locked --bin inspect-tts-receipt -- /absolute/new-private-output-directory/frozen-document.json /absolute/new-private-output-directory/receipt.json /absolute/new-private-output-directory/output.wav
```

Supply an exact frozen document with the supported local provider/voice/controls/output profile;
do not relabel an existing reference snapshot. The sample probe checks frozen input identity,
creates a new private directory and writes `intent.json` plus `frozen-document.json` before
synthesis. Success saves `output.wav`, `receipt.json` and `audit.json`; failure retains its
intent and a failure receipt when possible. Existing destinations are refused, with no
overwrite or automatic retry. A retained ambiguous intent requires inspection and reconciliation;
these local files are not a PostgreSQL job queue or production crash-recovery guarantee.
The offline path establishes no current owner/rights/budget approval and accepts no production
asset. Local charge remains unknown; resource use is not a provider billing receipt.

All three Vietnamese voices produced actual inspected samples in the local run; listening
quality is **NOT_RUN**. [Execution evidence](../../docs/evidence/tts-receipt-inspection.md)
separates live synthesis from synthetic failure tests. [ADR 0008](../../docs/decisions/0008-local-tts-inspection.md)
records this local inspection choice, with production adoption proposed. Commercial selection,
terms, rights, authentication boundary, spending cap and live sample remain deferred in
[provider prerequisites](../../docs/evidence/tts-provider-prerequisites.md).
This is part of #6; it grants no publication clearance and does not complete that issue.

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

## Google Cloud TTS adapter

[ADR 0011](../../docs/decisions/0011-google-cloud-tts-adapter.md) (proposed) records the provider
facts, mapping and failure policy. The catalog offers `google-cloud-tts` with models
`google-chirp3-hd`, `google-neural2`, `google-wavenet` and `google-standard` and their documented
vi-VN voices. Google has no emotion or intensity control, so a binding freezes only when every
line's effective delivery is explicitly `neutral` at intensity `0` (set the binding's performance
override); inherited Script IR emotion is `unsupported_control`. Chirp3-HD has no rate or pitch
control. Resolved text must be nonblank NFC plain text of at most 5,000 bytes. Frozen lines pin
`google-cloud-tts-synthesize-2` and `wav-pcm-s16le-mono-24000-v1`.

`tts::google::plan` refuses any line frozen for another adapter or version and re-checks every
capability; a plan holds the exact LINEAR16/24 kHz body, its `google-tts-request-1` digest and a
list-price estimate. `classify` maps each observation to a receipt-v1 class and delivery state:
a connection that never opened is `timeout`/`not_sent`; everything after it is
`may_have_executed`, so the shared recovery planner advises reconciliation, never a resend. The
exact HTTP status, canonical `error.status` name or audio defect stays in a private observation.
Audio must pass the `cantos-google-tts-pcm-1` profile. Provider messages are discarded; records
never carry dialogue text or credentials.

`probe-google-tts` re-admits a frozen document through the production authority and plans one
dialogue. Without `--live` it reads no credential, creates nothing and sends nothing:

```sh
cargo run --locked --bin probe-google-tts -- frozen-document.json dialogue-01
```

A live run is an explicit, operator-paid action on original sample text, never a private
manuscript. The new private directory must be outside Git:

```sh
CANTOS_GOOGLE_TTS_ACCESS_TOKEN="$(gcloud auth print-access-token)" \
CANTOS_GOOGLE_TTS_QUOTA_PROJECT=your-project-id \
cargo run --locked --bin probe-google-tts -- frozen-document.json dialogue-01 \
  --live /absolute/new-private-directory --max-estimate-cents 1
```

It refuses an estimate above the cap before reading the token, writes create-only `intent.json`
and `frozen-document.json` before sending, sends exactly one request and never retries. It then
writes `observation.json` and `receipt.json`, plus `output.wav` and `audit.json` on success, for
`inspect-tts-receipt`. Exit `0` means a plan or admitted audio; `1` a blocked plan, rejected
scope, exceeded cap or failed attempt (the report includes the recovery advice); `2` usage,
input, credential or file errors. The cap compares the list-price estimate, not a bill. This is
not production dispatch: no reservation, durable attempt or current approval check exists yet.
The [adapter evidence](../../docs/evidence/google-tts-adapter.md) lists what ran and what did not.

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
