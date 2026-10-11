# Private production inputs — v1 contract

Issue [#5](https://github.com/loveoverflowcom/cantos/issues/5) adds casting and authorization
records to the existing local Studio backend. [Shared Rust DTOs](api/src/production.rs) and the
[JSON Schema](schema/production/v1.schema.json) define wire shape; the pure server production
module validates controls, resolves speech inputs, estimates planning cost, computes the digest
and evaluates current eligibility. [ADR 0007](../../docs/decisions/0007-casting-production-inputs.md)
is proposed for production adoption. [Execution evidence](../../docs/evidence/casting-production-inputs.md)
owns which checks actually ran.

HTTP remains a planning data pipeline: requests do not call TTS/LLM, install a runtime, reserve
money or create accepted audio. Two reference entries keep `reference_only: true`. The local
eSpeak NG 1.52.0 descriptor exposes adapter-supported Vietnamese identities with
`reference_only: false`; runtime presence is checked by the explicit local probe. Four Google
Cloud TTS entries (`google-chirp3-hd`, `google-neural2`, `google-wavenet`, `google-standard`)
declare the documented vi-VN voices and controls of [ADR 0011](../../docs/decisions/0011-google-cloud-tts-adapter.md),
not account availability. Like eSpeak, a Google binding freezes only with an explicit `neutral`
delivery at intensity `0` and text admissible to that adapter (`invalid_google_text`,
`google_text_too_large`); resolved lines pin `google-cloud-tts-synthesize-2` and
`wav-pcm-s16le-mono-24000-v1`. Every entry keeps `billable_dispatch_available: false`.
Approval grants no agent permission to spend. The [receipt contract](tts-receipt-v1.md) owns the
separate user-invoked local and Google sample probes. The JSON Schema enumerates every
implemented provider, model, voice, adapter version and output contract; the server remains
authoritative for which combinations a capability allows.

The optional `vieneu-nano-local` descriptor appends a separate research-only entry with its
exact pinned model, `duc-tri`/`my-duyen` presets and neutral/default controls. It has
`reference_only: false` but `billable_dispatch_available: false`; this denotes a concrete research
adapter, not production rights. Its invocation boundary refuses production before runtime/model
I/O independently of approval. Model/preset provenance and live evidence are recorded in
[the neural slice](../../docs/evidence/local-neural-tts.md). Existing entries, catalog version,
production-p1 encoding, frozen snapshots, shared DTOs and JSON Schema are unchanged.

## Routes and current permissions

Paths below are relative to `/api/v1`. Script IDs, snapshot IDs and operation IDs are canonical
lowercase UUIDs; actor/evidence/editorial IDs use the existing bounded identifier contract.

| Method and path | Shared request → response | Permission and result |
| --- | --- | --- |
| `GET /production/catalog` | — → `ProductionCatalogResponse` | Static non-sensitive reference, local and Google descriptors; no private SQL |
| `GET /scripts/{script}/production` | — → `ProductionStateResponse` | Current reader; latest settings/claims and newest twenty snapshot summaries |
| `POST /scripts/{script}/production/settings` | `SaveProductionSettingsRequest` → `ProductionSettingsResponse` | Current editor/owner; 200, append version with script/settings CAS |
| `POST /scripts/{script}/production/rights` | `SaveProductionRightsRequest` → `ProductionRightsClaimResponse` | Current owner; 200, append one script-scoped declaration version |
| `GET /scripts/{script}/production/review` | — → `ProductionPreviewResponse` | Current reader; resolve head/settings, explain findings and digest |
| `POST /scripts/{script}/production/snapshots` | `FreezeProductionRequest` → `ProductionSnapshotResponse` | Current owner; 201, preserve an immutable candidate |
| `GET /scripts/{script}/production/snapshots/{snapshot}` | — → `ProductionSnapshotResponse` | Current reader; exact document with current approval/eligibility |
| `GET /scripts/{script}/production/snapshots/{snapshot}/eligibility` | — → `ProductionEligibility` | Current reader; current reevaluation, no persisted readiness flag |
| `POST /scripts/{script}/production/snapshots/{snapshot}/approvals` | `ApproveProductionRequest` → `ProductionApprovalResponse` | Current owner; 200, exact snapshot/digest, valid inputs required |

Use the existing same-origin `cantos_session` cookie and mutation Origin policy. The browser
never authorizes a record. A private record's existence/contents remain unavailable without
current script access; known historical IDs do not bypass that check. This is local loopback
development authentication, not production identity/TLS or a configured public AI connector.
The explicit HTTP contract is the tool surface for this slice; no additional production CLI or
MCP server is claimed.

## Settings and resolved inputs

`SaveProductionSettingsRequest` carries `operation_id`, `expected_revision`,
`expected_settings_version` and complete `settings`. Zero is the no-prior-settings base; stored
versions start at one. A settings response records script/revision/version, actor, operation and
UTC microsecond timestamp. Saving against a changed script head or settings version conflicts.

`ProductionSettings` contains character `bindings` and `budget`. Each binding supplies exact
`character_id`, `provider_id`, `model_id`, `voice_id`, language and `voice_rights_record_id`,
plus performance and pronunciation. Provider/model/voice/language/control combinations must
match the explicit capability contract. Performance has integer `rate_permille`, integer
`pitch_semitones`, nullable emotion and nullable `intensity_permille`. Null overrides preserve
the accepted Script IR value; they never substitute neutral delivery. Pronunciation uses
provider-neutral `{surface, replacement}` respelling, not phoneme markup or byte offsets.
Global presets may be unused by current dialogue. Resolution merges line overrides with global
presets, rejects conflicting replacements for the same surface, matches the longest surface
at each position and never scans replacements again. Normalize the resulting effective text
to NFC and record the text and override set.

Every speaking character needs one binding. Unknown/missing/duplicate character bindings,
unsupported voices/models/controls, language mismatch and invalid pronunciation are field
issues rather than substituted defaults. The complete accepted export remains unchanged.
`ResolvedSynthesisInput` records dialogue/character IDs, effective text/language, exact voice
identity/rights reference, resolved performance/pronunciation, `adapter_version` and
`output_contract`. The reference versions explicitly identify no dispatch/no audio, not a
fictional codec or model revision. Resolve in accepted dialogue order.

## Rights declarations

`SaveProductionRightsRequest` carries `operation_id`, `expected_version` and `claim`; version
zero declares a new record. The claim's immutable identity is its `record_id` and exact tagged
subject: `{"kind":"evidence","record_id":"..."}` or
`{"kind":"voice","provider_id":"...","model_id":"...","voice_id":"..."}`.
An existing record cannot be rebound to a different subject. Evidence subjects must resolve to
owned rights evidence linked to this script; voice claims register a script-scoped assertion
through the same evidence registry. Nothing infers a shared creator-wide voice grant.

The declaration requires `scope: "production_synthesis"`, status `pending`, `granted` or
`revoked`, claim reference, rights holder, explicit languages, territory, attribution,
restrictions, permitted scope and integer Unix validity terms. Languages cover the accepted
script's exact tag or explicit `all`. This slice uses `private-planning` territory and matches
`permitted_scope` to the frozen budget's exact scope. Required attribution/restrictions must
record explicit values, including an asserted non-applicability/none-declared value when
appropriate; blank omission is not permission. `valid_until_unix: null` explicitly records an
open-ended term. A finite interval is `[valid_from_unix, valid_until_unix)`.

Claims are recorded actor assertions, **not legal verification**. Missing, pending, revoked,
not-yet-valid, expired or mismatched subject/language/territory/scope blocks inputs. Append
another version to correct/revoke. Candidates retain the pinned earlier claim while current
eligibility compares current selected claims and their terms. Time reevaluation does not require
a new write. No public distribution, publication, cross-creator reuse or cloning consent is
established by this planning contract.

## Budget, estimate and billable boundary

Budget records `currency` (`USD` cents or `VND` dong), integer `limit_minor`, exact nonempty
`scope`, `territory` and nullable `rate`. A rate records its caller-asserted `reference`/`version`,
positive `units_per_charge` and nonnegative `amount_minor`. Currency is part of the scope;
no conversion or exchange rate is inferred. Unknown rate returns
`{"status":"unavailable","reason":"..."}`. A known estimate returns status, currency,
integer amount/units and rate identity/version. It is a planning assertion, never a provider
price attestation, reservation or settled charge.

For each dialogue, count Unicode scalar values of resolved effective text and compute
`ceil(line_units × amount_minor / units_per_charge)` with checked integer arithmetic. Sum the
independently rounded line estimates and their units with checked bounds; dialogue is the
future send unit. No binary floating point or provider billing assumption is used.
Equal-to-budget is permitted; greater-than-budget
blocks. Versions, unit/rate/money values remain within the PostgreSQL signed 64-bit contract;
overflow is rejected explicitly. Input integer fields require integer JSON tokens in Rust;
JSON Schema implementations may admit integer-valued decimal spellings, so server admission is
authoritative at that boundary. Unknown is neither zero nor unlimited.

`inputs_eligible` describes current reviewed inputs/rights/budget. `approval_current` additionally
requires an exact current owner approval. `billable_dispatch_available` remains false for this
catalog even when planning inputs and approval are eligible. There is no dispatch endpoint.
A later adapter must independently establish real capabilities/terms, repeat the gate and
reserve budget before an explicitly authorized call. Actual charges require real provider
receipt/reconciliation evidence; this slice produces neither attempts nor charges.

## Frozen candidates, digest and approval

Review returns exact `script_revision`, `settings_version`, resolved lines, estimate, findings
and input digest. A freeze request binds `operation_id`, `expected_revision`,
`settings_version` and that digest. The backend re-resolves the current facts inside its locked
transaction; revision/settings/rights drift cannot freeze a different reviewed input under the
same request. Freeze may preserve a blocked/unreviewed candidate for inspection. It creates no
approval and cannot start work.

`FrozenProductionDocument` pins owner, `catalog_version`, complete `RevisionResponse` (including
canonical Script IR `script_json`, revision, c1/e1, accepted actor/time), versioned settings,
required rights claim responses, ordered resolved inputs, estimate and input digest. Required
claims cover Script IR rights evidence and each binding's exact voice subject; unrelated claims
are excluded. Canonicalize bindings by character ID, pronunciation by surface and selected
claims by record ID before freezing. Record
metadata is included; expiry findings and evaluation clock are not. Compact UTF-8 JSON uses
recursively lexicographic object keys, preserving sequence-array order, without a trailing
newline. Hash the whole document except `input_digest`:

```text
production-p1:sha256:<lowercase hex SHA256(
  b"cantos/production-inputs/p1\n" + canonical_document_without_input_digest
)>
```

This digest complements immutable revision identity and e1; it does not replace either and is
not the per-speech artifact cache fingerprint. Rights/provenance changes can leave c1 unchanged
and still change this scope. New encoding/classification semantics require a new digest label.
Stored complete documents and metadata are re-admitted and digest-checked on every load.

Approval carries a distinct `operation_id` and exact `input_digest`; the route names the
snapshot. It appends an immutable owner/UTC time/snapshot/digest receipt only if current inputs
remain eligible and the owner has reviewed the accepted revision. Accepted script/settings or
selected rights changes invalidate eligibility/approval without changing candidate bytes.
Unsaved browser typing has no backend effect. Owner or actor scope loss fails closed. An exact
old approval retry returns its original receipt even when eligibility is now stale; it never
regrants authority. A distinct new approval operation on an already-approved candidate conflicts.
Use a replacement candidate after input corrections.

Finding codes are stable snake-case values: `script_unreviewed`, rights `missing`, `pending`,
`revoked`, `not_yet_valid`, `expired`, `subject_mismatch`, `language_mismatch`,
`territory_mismatch`, `scope_mismatch`, `restrictions_require_review`, plus `estimate_unavailable`, `budget_exceeded`,
`revision_changed`, `settings_changed`, `rights_changed`, `approval_missing` and
`approval_scope_mismatch` (rights codes carry the `rights_` prefix). Each finding includes a
path and inert detail. Studio copy localizes typed findings; a disabled control is not enforcement.

## Persistence, retry and bounds

Migration 0006 appends `production_settings`, `production_rights_claims`,
`production_snapshots` and `production_approvals` on the same PostgreSQL authority. Immutable
triggers reject update/delete/truncate. Runtime roles get SELECT/INSERT on production facts and
EXECUTE on the migration-owner's narrow `production_lock_actor(text)` /
`production_lock_member(text,text)` helpers. No authority-table UPDATE privilege is granted.
Fixed search path/qualified SQL and revoked PUBLIC execution bound these share-lock helpers;
[server rollout grants](../../services/cantos-api/README.md#casting-and-production-input-authorization)
name the exact signatures. Current actor,
session, owner/membership and script locks make permission checks and decisions transactional.
The operation identity is `(script, actor, operation_id)` within each operation kind; exact
serialized request values reconcile one original record. Changed requests under an existing
key return `operation_reused`; retries do not bypass current backend access. The client keeps
its actor, operation and immutable payload through response loss. There is no automatic retry.

Settings request/storage is capped at 256 KiB, bindings at 1,000 and global pronunciation at 100
per binding; rights/freeze/approval at 64 KiB. Effective text is capped at 64 KiB per dialogue,
the materialized resolved vector at 1 MiB and a frozen document at 4 MiB. Current rights records
are capped at 128. State history returns newest twenty snapshot summaries ordered by
`recorded_at DESC, id DESC`; open a known ID through the exact snapshot route for full inspection
under current access. Schema scalar/array bounds complement byte/NFC/control checks; Script IR
retains its existing 2 MiB admission cap. Rights holder/reference/scope/rate reference and
character preset pronunciation strings are bounded to 512 UTF-8 bytes, rate version to 128
bytes, attribution and restrictions to 2,048 bytes. `ResolvedPronunciation` in prepared previews
and snapshots retains admitted Script IR pronunciation surfaces and replacements up to 10,000
normalized Unicode scalar values; the smaller preset admission limit does not narrow inherited
accepted overrides. The effective text and resolved vector limits above still apply. JSON Schema
cannot alone enforce NFC, bytes or semantic relations.

The existing `ApiError` returns typed codes/field issues without private diagnostic payloads.
New codes are `stale_production_inputs` (409) and `production_blocked` (409 with finding
paths/rules). Invalid shape/control/request is 400, unavailable linked evidence is 422 and
oversized mutation bodies are 413; existing
unauthenticated, forbidden/not-found, stale revision, operation reuse and corruption responses
retain their existing meanings. A failed or rolled-back mutation writes no partial record.
Restart reopens the same document/approval and reconciles exact operations; no worker/provider
lease or billing recovery claim follows from database recovery.
