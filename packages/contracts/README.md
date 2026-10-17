# Contracts

> Current [ADR 0018](../../docs/decisions/0018-external-audio-production.md):
> [external-audio-handoff-v1](external-audio-handoff-v1.md) is a **proposed**, not implemented,
> API contract with its [Production Brief v1](production-brief-v1.md) subset implemented. Durable TTS/receipt/speech-reuse contracts below document historical experiments;
> they are **not live server capabilities** after the 2026-10-12 refactor.

The separate [Studio owner audio intake v1](studio-audio-intake-v1.md) is implemented for
private development script-revision-bound intents, explicit four-profile Opus inspection and
immutable receipts. It is not the general external-producer proposal or a publication API;
required Rust/PostgreSQL execution is **NOT_RUN** in its implementation environment.

This directory owns versioned interchange descriptions and representative fixtures. Script IR
`0.1.0` has a hand-authored [JSON Schema](schema/script-ir/0.1.0.schema.json), a
[Rust reader and semantic validator](../../services/cantos-api/src/script_ir/mod.rs), and
[Vietnamese fixtures](fixtures/script-ir/0.1.0). This implements the contract part of
[#1](https://github.com/loveoverflowcom/cantos/issues/1). The [Studio v1 contract](studio-v1.md)
now serves the Axum/PostgreSQL store and Leptos consumer through shared raw Rust DTOs,
JSON Schema and serialized fixtures. API surfaces are separate URL spaces
(`/api/v1/studio`, reserved `/api/v1/theatre` and `/api/v1/admin`, named in `api/src/routes.rs`);
the crate root holds the Studio DTOs plus shared error types and `theatre`/`admin` hold future
surface DTOs. No generated Kotlin bindings are claimed.

The [Studio Catalog v1 contract](catalog-v1.md) defines the new JSON-only Work and ContentUnit
hierarchy, revisioned metadata, explicit PATCH omission/null semantics, soft archive, bounded
pages, atomic reorder and retry receipts. Its [schema](schema/catalog/v1.schema.json) and
[synthetic Vietnamese fixtures](fixtures/catalog/v1/README.md) are shared with the HTTP tests.
Catalog visibility grants no public delivery; optional immutable source/Script IR links grant
neither rights clearance nor publication. The deprecated alias and reserved surface groups gain
no Catalog routes, and Studio Web Catalog UX remains a separate acceptance slice.

The [private Work artwork v1 contract](artwork-v1.md) adds raw metadata DTOs, JSON Schema,
strict opt-in Protobuf presence and independent fixtures for one private PNG/JPEG Work cover.
Its metadata/byte/application acceptance stays separate from public delivery and client UI;
existing Catalog and pilot contracts retain their exact representation.

The [Content Identity v1 contract](content-v1.md) adds separate immutable content revisions,
normalized text or exact source/Script IR bindings, recording-only units, full revision digests,
owner-bound evidence, atomic retries and bounded history. Metadata/content counters are distinct;
legacy Catalog links do not move managed content. It adds no brief, accepted audio or publication.

The [Production Brief v1 contract](production-brief-v1.md) exports immutable Catalog content,
rights/guidance/profile pins and independent expiring read permissions. It produces no audio
and grants no upload/review/publication authority.

The [Work-scoped delegation v1 contract](delegation-v1.md) implements the
[ADR 0028](../../docs/decisions/0028-work-scoped-delegation.md) first slice: owner-issued,
show-once `cantos_dlg_` bearer grants for `work:read`, `content:read` and `content:write` on one
Work, recorded under a registered delegate actor and revocable behind the Catalog clock. Its
[schema](schema/delegation/v1.schema.json) and [fixtures](fixtures/delegation/v1/README.md) are
shared with the wire and PostgreSQL HTTP tests. It grants no Script, Brief, audio, review or
publication authority and replaces no producer grant.

The [Publication v1 contract](publication-v1.md) adds current-owner distribution clearance,
exact approval, complete readiness blockers, immutable releases, atomic active pointers and
withdrawal receipts over private candidates. Its JSON schema/fixtures and Studio routes are
implemented; [evidence](../../docs/evidence/publication-core.md) records verified core and pending
PostgreSQL/public delivery acceptance. Default delivery is unavailable. Audit manifests stay
Studio-only; no public Theatre projection or staging delivery is provided.

The [manuscript import v1 contract](manuscript-v1.md) adds a private, immutable source intake
receipt for TXT, Markdown, DOCX and validated Script IR. It uses the existing source registry,
separates exact original bytes from normalized extraction, and never saves a script revision or
grants publication permission automatically. Its hand-authored Schema and independent literal
wire examples are checked by the shared consumer and backend contract tests.

The [adaptation v1 contract](adaptation-v1.md) adds private revision-pinned caller contexts, a closed
untrusted proposal format and explicit reviewed acceptance. Server-created IDs/evidence and the
existing Script IR reader form the admission boundary. Proposal citation coverage and findings
stay outside Script IR and require source comparison. The [tool schemas](adaptation-tools-v1.json) expose source-bound context/read/submit/review
operations over the existing authenticated HTTP service and CLI. Caller provenance is declared,
unverified evidence; no inference integration or live model result is implied.

The [production input v1 contract](production-v1.md) shares versioned casting, script-scoped
rights/budget declarations, immutable frozen candidates and exact owner approval between Axum
and Studio. Its [schema](schema/production/v1.schema.json) and
[synthetic fixtures](fixtures/production/v1/README.md) describe a reference-only capability
catalog. Current eligibility is computed from authoritative facts; freezing is preservation,
not permission to dispatch. Those reference fixtures establish no real voice availability,
legal verification, provider price, audio, reservation or actual billing receipt. Historical
reference snapshots keep their exact identities and output contracts. The catalog now also
declares the separately implemented local eSpeak inspection entry and the Google Cloud TTS
vi-VN entries; their availability evidence belongs to the bounded local TTS and Google adapter
slices.

The [offline TTS receipt inspection contract](tts-receipt-v1.md) is a local server CLI format
over those exact frozen documents. It checks synthetic and actual local WAV bytes plus
caller-recorded attempt/cost facts without changing authority. A user-invoked eSpeak NG 1.52.0
probe saves private frozen-input/intent/receipt/output records with an explicit native 22,050 Hz
PCM profile. Three Vietnamese voices have live synthesis/byte-validation evidence; listening
quality is **NOT_RUN**. This offline format supplies no shared synthesis API, current production
authorization, durable job or audio-asset acceptance. Commercial selection and its live sample remain deferred;
the local slice does not close #6.

The [durable jobs v1 development contract](durable-jobs-v1.md) describes the same-package
authenticated CLI, PostgreSQL run/job/step/attempt records and separate local/fake worker.
It reuses production-p1 frozen documents and receipt-v1 admission; it adds no shared HTTP DTO,
new wire schema or provider abstraction. `durable-input-v1` is an exact resolved-input linkage
fingerprint, not the future reusable speech-cache key. Google commercial dispatch is disabled
and VieNeu remains research-only. Full #7 acceptance, clip reuse and delivery assets are not
claimed by this bounded foundation.

The [clip reuse v1 contract](clip-reuse-v1.md) defines the position-free `speech-v1` fingerprint, the
verified-clip registry, the attempt-less reuse selection and the read-only regeneration plan that
extend the durable foundation. Its independent Python oracle writes
[`speech-vectors.json`](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/fixtures/clip-reuse/v1/speech-vectors.json), Rust must reproduce them, and the
plan report has a [JSON Schema](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/schema/clip-reuse/v1.schema.json) and a literal
[fixture](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/fixtures/clip-reuse/v1/plan-edit-one-line.json). It adds no shared HTTP DTO, Studio route,
object storage, live provider or publication path.

The requester-selected [Theatre mock playback v1 corpus](fixtures/theatre/playback/v1/README.md)
shares exact state/event/effect vocabulary and hand-authored regression vectors between independent
Rust and Kotlin presentation cores. It is mock-only test data, not a listener HTTP DTO, publication
manifest, production authorization or audio acceptance. Each platform reports its own test gaps.

The historical [draft example](examples/episode-draft.json) stays byte-for-byte unchanged. Its
`0.1.0-draft` version is illustrative and explicitly unsupported by the reader. The new fixtures
reuse that original sample and add synthetic content. Their external record IDs are placeholders;
passing validation does not establish production or distribution rights.

## Read and write policy

Read exactly `{0.1.0}`; write `0.1.0`. Missing and unsupported versions fail explicitly. An
unsupported-version error reports the found and supported versions; use a compatible producer
or a reviewed converter. Unknown fields fail at every object level. New fields, enum values or
units require a new exact schema version, a compatibility corpus and a deliberate converter or
rejection policy. Retiring a supported read version requires an owner decision and migration
plan; retain its compatibility fixtures. No converter, persisted older revision or retired
supported version exists.

The Schema owns shape (types, required fields, spellings, bounds). Rust is authoritative for
cross-reference and text semantics. Rust independently checks shape; differential tests compare
it with the Schema over fixtures, missing/unknown/null fields and boundary values. Consumers must
run semantic validation even when their JSON Schema tool accepts a document.

The [contract ADR](../../docs/decisions/0002-script-ir-contract.md) is **proposed for production
adoption**, with these choices implemented and locally tested. This is the first experimental
contract, not approval of an application or publication gate.

## Identity, order and evidence

- Work, adaptation, episode, act, scene, dialogue, cue, character and provenance entry IDs are
  globally unique within one document: 1–64 ASCII characters from `[a-z0-9_-]`. The shell mints
  opaque IDs and preserves them across edits. The validator cannot infer whether a producer
  regenerated IDs between documents without a base revision.
- Acts, scenes and dialogues use array order. Each level is non-empty; there are no ordinal
  fields to drift out of sync. Cue array order breaks ties at an anchor. A cue anchor resolves
  to a dialogue in that same scene. Moving scenes/lines retains IDs.
- Every dialogue resolves its `speaker_id` to a character; exactly one character has the
  `narrator` role. Narration is ordinary spoken dialogue. Casting is external to the IR.
- Work `source_ref` and adaptation `provenance_refs` resolve to provenance entries; duplicate
  adaptation references fail. Every entry carries an external preserved `source_record_id` and
  `rights_record_id`. Generated provenance also requires `generation_record_id` for the recorded
  attempt. The adaptation's declared provenance applies to the whole episode. Adaptation runs
  separately record source-block citation coverage and review findings; those model claims do
  not prove faithful meaning or reliable omission detection.
- Work, adaptation and a selected cue asset require rights record references. Cue `asset.id`
  identifies an immutable asset record; an absent asset is an editorial intent. No URLs, media
  bytes, casting, rights status, revision numbers or workflow status belong in the IR. Backend
  adapters must resolve records, check tenant/actor access and evaluate rights at each gate.

Language tags are the bounded initial set `vi`, `vi-VN`, `en`, `en-US`; this does not promise
provider support. Delivery uses a closed emotion vocabulary and integer `intensity_permille`
from 0 to 1000. JSON Schema also admits integer-valued spellings such as `300.0`/`3e2`; Rust
normalizes them to an integer before canonical encoding using the exact decimal token, without
binary-float rounding. Fractional values (including tiny nonzero exponents) fail. Schema tools
that use binary floats may accept a rounded value; the byte reader remains authoritative for
that numeric boundary. Pronunciation
overrides are provider-neutral `{surface, replacement}` respellings of every left-to-right,
non-overlapping occurrence. Targets must occur in normalized text, and targets from different
overrides must not overlap. Phoneme alphabets, offsets and provider markup are not interpreted.

The reader limits input to 2 MiB before decoding and complete normalized exports to the same
bound before returning content. NFC expansion cannot create an unreadable or unstorable export.
Arrays are bounded at 10,000 entries; text at
10,000 Unicode scalar values both before and after normalization. These are parser/storage
bounds, not user-facing grapheme or provider request limits. JSON Schema cannot enforce the
byte limit or reject duplicate JSON object keys after another parser has discarded them; the
Rust byte reader rejects both. Serde's default recursion limit remains enabled.

## Text and diagnostics

Draft admission trims and collapses the frozen Unicode White_Space set to ASCII space (C1 NEL
is rejected), removes U+200B/U+FEFF/U+00AD, then applies NFC using Unicode 16.0.0 tables. Tabs
and line breaks become spaces. Other C0/C1 controls and the explicit invisible/format ranges in
[the validator](../../services/cantos-api/src/script_ir/validation.rs) are rejected. This bounded admission
profile is not an exhaustive Unicode security filter. NFC preserves punctuation and tone
placement: `hòa` and `hoà` remain distinct. Source bytes are kept outside Git and never normalized.

Semantic errors accumulate in fixed traversal order: work, adaptation references, narrator count,
characters, episode/acts/scenes/dialogues/cues, provenance IDs. Paths use stable IDs where known;
lists without identities use indexes. Shape errors use JSON Pointers; deserialization failures
include a line, column and cause. Shape failure stops before semantic validation. Studio HTTP
maps errors to stable codes/paths without exposing manuscript diagnostic payloads. Bracketed spoken text is retained; detecting cue-like markup for human
review remains editor/importer work.

## Canonical bytes and digest c1

The explicit Rust encoder uses compact UTF-8 JSON, fixed ASCII object keys in sorted order,
minimal string escaping, NFC text and integer values. It emits no BOM or trailing newline.
Characters/provenance sets sort by ID; provenance reference sets sort by ID; non-overlapping
pronunciation overrides sort by surface. Semantic sequence arrays retain their order. Empty
optional arrays and absent assets are omitted; explicit `null` is rejected on admission.

| Field family | Content bytes/digest | Per-line speech bytes |
| --- | --- | --- |
| All editorial IDs, titles, character name/role/personality | included | excluded |
| Act, scene, dialogue and cue order | included | excluded |
| Adaptation language; line text, delivery, pronunciation | included | included |
| Speaker reference | included | beside speech bytes for casting resolution |
| Cue kind, description, anchor, selected asset ID | included | excluded |
| Schema version; provenance/source/generation and rights references | excluded | excluded |

`canonical_bytes()` returns the content projection, which is **not a complete interchange file**.
`export_bytes()` returns a complete canonical `0.1.0` document including provenance and rights
references. `read_canonical_script()` admits only those exact export bytes; a stored-row adapter
must reject corruption instead of silently normalizing it. JSONB reformatting is not suitable for
that byte-exact read path.

```text
ContentDigest = "sir-c1:sha256:" + lowercase_hex(
    SHA256(b"cantos/script-content/c1\n" + canonical_content_bytes)
)
```

Digest and schema versions evolve independently. A field classification or encoding change
requires a new digest scheme and new goldens; stored digests/bytes must never be rewritten.
Only c1 is constructible today, so typed comparisons are always within c1. A future multi-scheme
reader must refuse comparisons across schemes. Speech bytes exclude IDs, position and neighbors;
the pipeline must add voice/casting revisions, provider/model, settings and output profiles before
using them as a cache key.

Rights/provenance changes can leave the content digest unchanged. Persist the **complete** export
and bind approvals/production to an immutable revision ID plus its digest and resolved evidence.
Never deduplicate revisions or verify evidence integrity by content digest alone. Authorization,
rights expiry, casting completeness and publication readiness are not proved by this validator.

## Run and verify

From the repository root with Rust 1.98.1 and Python 3:

```sh
cargo run --locked --bin validate-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json
cargo xtask verify rust        # fmt, clippy -D warnings, workspace tests
cargo xtask verify contracts   # reference_script_ir.py, contract test targets, WAV oracle
cargo xtask verify mutation    # mutate_script_ir.py
```

The CLI returns 0 plus the digest on success, 1 for rejected content and 2 for usage/I/O failure.
`--export` writes the complete canonical file; `--content` writes only the digest projection.
`cases.json` records literal rejection oracles. Accepted `.expected.json` files hold content
bytes, digest and speech bytes produced by the independent Python JSON/SHA-256 oracle. Deliberate
regeneration uses `python3 scripts/reference_script_ir.py --write-goldens`; tests never update them.
The mutation runner checks nine named substitutions in a disposable offline copy.

## Byte representations

[`api/src/wire.rs`](api/src/wire.rs) (`cantos_contracts::wire`) is the one place a DTO becomes JSON
or MessagePack bytes: MessagePack is a named map, decoded from a buffered slice with a nesting
bound, a top-level map and no trailing bytes. Which routes negotiate it is owned by the
[wire codec](../../docs/architecture/wire-codec.md) and the [Studio contract](studio-v1.md#representations).
[`fixtures/studio/v1/msgpack-goldens.json`](fixtures/studio/v1/msgpack-goldens.json) holds the
MessagePack bytes of each Studio fixture, written by the independent standard-library oracle
`scripts/reference_msgpack.py`; Rust tests require `rmp-serde` to produce the same bytes and decode
them to the JSON fixture values. After a deliberate fixture change, regenerate with
`python3 scripts/reference_msgpack.py --write-goldens`, review the diff and never edit hex by hand.

See [contract evidence](../../docs/evidence/script-ir-contract.md) and
[revision evidence](../../docs/evidence/script-revision-persistence.md). Studio v1 uses hand-authored
DTOs checked against the same Schema/fixtures in both native and browser-consumer tests; it is
not generated OpenAPI. Future generated bindings must name their source and generation command.

[Private Audio Lifecycle v1](audio-lifecycle-v1.md) implements Content/Brief-bound admission, independently scoped upload, append-only review and private candidate readiness. Public publication remains unavailable.

## Theatre Discovery/Playback

[Discovery/Playback v1](theatre-discovery-v1.md) adds real anonymous public-release queries,
opaque keysets and a fail-closed playback authorization port. Shared DTOs and strict fixtures
are implemented; real PostgreSQL/public delivery and drift-checked Web/CMP adoption remain
acceptance dependencies. No library/progress or Studio-cookie reuse is introduced.

[Admin v1](admin-v1.md) defines the dedicated backend capability/revocation responses and
append-only-audit policy boundary. It has shared Rust DTOs, schema and synthetic fixtures; no
Admin UI/client or live Kanidm acceptance is implied.

## Studio Protobuf wire pilot

[Versioned policy and schema](protobuf/README.md) own the four actual Studio
revision/validation wire roots. Existing Serde DTOs, exact Script IR text and JSON errors remain
unchanged. `cargo xtask verify contracts` checks independent Python goldens, exact generated
prost drift and production Rust codec tests; schema/fixture changes also select common/JVM
Kotlin tests in preflight. [The migration ledger](../../docs/evidence/protobuf-wire-migration.md)
records deferred browser/SQL/Native acceptance, measured costs and JSON rollback.

## Theatre Identity

[Theatre Identity v1](theatre-identity-v1.md) keeps Web cookie and native opaque bearer purposes
separate. Native public-client flow/exchange/session DTOs share the strict
[native schema](schemas/theatre-native-identity-v1.schema.json) and [nonsecret session fixture](fixtures/theatre-identity/v1/native-session.json).
The public Web [configuration schema](schemas/theatre-auth-configuration-v1.schema.json)
separately reports login configuration and registration readiness. Accepted Theatre self-registration
remains unavailable with the current pinned Kanidm adapter; OIDC login is not registration capability.
The provider bearer never reaches either client. Session UUID headers reject late account
intents and correlate private responses; they are fences, never authentication evidence.
Combined Theatre verification is pending; the Android host activation exception and actual
Web/device/provider/secure-store acceptance remain separate from these implemented contracts.
