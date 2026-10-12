# Studio owner audio intake v1 — Development implementation contract

This is a narrow private **development-only** intake slice on existing accepted Script IR
revisions. It is separate from the general Work/ContentUnit/scoped-producer handoff proposed in
[external-audio-handoff-v1](external-audio-handoff-v1.md). [ADR 0018](../../docs/decisions/0018-external-audio-production.md)
still owns the product boundary: production, recording, mixing and encoding happen outside
Cantos. No TTS SDK, provider worker, backend mixer or automatic regeneration is introduced.

The shared raw DTOs, JSON Schema and safe transport fixtures are added with this change.
They are an implementation contract, **not evidence that a running deployment accepts files**.
Rust compilation, Rust tests and real PostgreSQL/runtime verification are `NOT_RUN` in the
implementation environment because Cargo/Rust are unavailable. Separate service tests own
authorization, actual-byte inspection, object integrity, transactions and crash/retry evidence.

## Boundary and flow

Only the current script owner using an active Studio session may create an intent, upload
bytes or read the intent/receipt. A third-party producer must not receive or reuse that session.
There are no scoped external-producer credentials or grants in this slice. Theatre, Admin and
the deprecated unprefixed Studio aliases gain no audio intake routes.

The HTTP boundary admits at most four raw upload requests before buffering their bounded
bodies. Two blocking inspection/storage lanes bound expensive work and remain held through
private object persistence even when a client request is cancelled. A busy lane fails closed
with HTTP 503; cancellation cannot release a lane while its blocking work still runs.

1. The owner explicitly selects a supported **versioned** profile and the exact current
   accepted script revision and content digest.
2. A separate JSON intent-creation request records those immutable pins and an operation ID.
3. The owner sends finished bytes to the intent's authenticated same-origin binary endpoint.
4. The server inspects the actual bounded bytes, rechecks current session/owner/revision
   authority before acceptance, stores a private immutable object and appends one receipt.
5. Status returns the original intent and either its original receipt or `null`. There is no
   acceptance based solely on a producer's hash, extension, URL or claimed metadata.

## Routes and transport

| Method | Route | Request | Successful body |
| --- | --- | --- | --- |
| POST | `/api/v1/studio/scripts/{script}/audio-intents` | JSON `CreateAudioIntentRequest` | `AudioIntentResponse` |
| GET | `/api/v1/studio/scripts/{script}/audio-intents/{intent}` | None | `AudioIntentStatusResponse` |
| PUT | `/api/v1/studio/scripts/{script}/audio-intents/{intent}/bytes` | `application/octet-stream` bytes | `AudioReceiptResponse` |

Metadata transport is JSON. The binary body has no JSON wrapper, base64 field, client media
metadata, upload filename, arbitrary source URL or format-conversion instruction. Returned
`upload_path` and `status_path` are same-origin authenticated API paths for the exact script
and intent; they are not object-storage keys, public delivery links or bearer grants.
Successful create, status and byte-acceptance responses use HTTP 200.

The [shared DTOs](api/src/audio_intake.rs) are plain raw values with closed object fields.
The [schema](schema/audio-intake/v1.schema.json) names matching `$defs` and provides scalar,
identity, codec/profile and envelope constraints. All independent request pins are required:

- `operation_id`: canonical lowercase UUID, stable across response-loss retries
- `expected_revision`: exact current positive revision, within PostgreSQL signed 64-bit range
- `expected_content_digest`: exact current `sir-c1:sha256:<lowercase hex>` content digest
- `profile_id`: one explicit profile below; omission never chooses a default

The intent returns its ID, script/revision/content/profile/operation pins, creator/UTC time
and two authenticated paths. The receipt returns a new receipt ID, the same immutable pins,
accepted actor/UTC time and `AudioMediaMetadata`. Status serializes absent acceptance as
`receipt: null`; raw serde also allows omission and converts it to `None`. Neither means a
review passed, a file is published or an upload is permanently rejected.

## Selected intake envelope

The owner explicitly chooses one of four Opus profiles. **There is no default or automatic
profile selection.** AAC, MP4, WAV, stereo and arbitrary profile versions are outside this
intake contract. The earlier AAC fallback proposal does not authorize AAC acceptance here.

| Canonical profile | Producer nominal guidance | Maximum observed average coded audio bitrate |
| --- | --- | --- |
| `opus-mono-20-v1` | 20,000 bits/s | 24,000 bits/s |
| `opus-mono-24-v1` | 24,000 bits/s | 28,800 bits/s |
| `opus-mono-32-v1` | 32,000 bits/s | 38,400 bits/s |
| `opus-mono-40-v1` | 40,000 bits/s | 48,000 bits/s |

All four require Ogg/Opus, mono, a 48,000 Hz Opus decoding timeline, positive duration no
greater than 3,600 seconds, and a complete body no greater than 33,554,432 bytes (32 MiB).
The versioned engineering ceiling is 120% of the nominal target. Nominal target is external
encoding guidance, not a minimum bitrate: silence and VBR may produce lower average rates.
The observed bitrate concerns coded audio payload rather than Ogg/container/tag overhead.
Duration is derived from the Ogg terminal granule minus Opus pre-skip on the decoding timeline.

`AudioMediaMetadata` carries server-observed `sha256`, `byte_length`, `container: "ogg"`,
`codec: "opus"`, `channels: 1`, `sample_rate_hz: 48000`, `duration_ms`,
`audio_bitrate_bps` and the exact `profile_id`. Intake combines bounded structure/profile
inspection with a bounded decode-to-discard check. It saves no decoded waveform and performs
no conversion. This remains distinct from listening, loudness/true-peak QC or certifying useful
content. This envelope
does not settle [#51](https://github.com/loveoverflowcom/cantos/issues/51) playback compatibility
or loudness/listening adoption, or the wider [#64](https://github.com/loveoverflowcom/cantos/issues/64)
production profile roadmap.

JSON Schema cannot verify actual bytes or SHA-256, derive coded-packet duration/bitrate,
prove path/receipt identities equal their containing pins, authorize an actor, or enforce a
transaction. Raw String fields likewise do not create refined domain witnesses. The service
must independently admit the request and inspect media, then validate persisted metadata on load.
Raw Rust integer fields also require integer JSON tokens; a JSON Schema validator may admit an
integer-valued decimal spelling. Server transport/admission remains authoritative at that edge.

## Immutable operations and retries

Intent creation uses an actor-scoped operation ID and pins one exact script. An exact retry
returns its original immutable intent; changing the script, revision, digest or profile under
that operation conflicts with `operation_reused`. Editing content requires a newly pinned
intent and operation.
Intents and receipts are never rewritten or rebound to another script, revision, digest or
profile. Current access is required even when reconciling a previously successful operation.
An exact intent-creation retry can recover its historical intent after revision drift, provided
the session and owner remain current; it cannot authorize a stale upload.

An intent accepts at most one byte sequence. Repeating identical already accepted bytes returns
the exact original receipt, including its ID, accepted actor/time and observed metadata. Sending
different bytes to an accepted intent conflicts with `operation_reused`; no replacement receipt
is written. Rejected, malformed, truncated, oversized or profile-mismatched bytes create no
accepted receipt. An accepted receipt never automatically reviews or publishes its object.
Accepted byte-upload retries also require the exact current revision/content pins. Historical
accepted receipts remain recoverable through the authorized status GET after revision drift.

Current active session, owner and exact pinned revision/content authority are rechecked after
media inspection and before committing a new receipt. Loss of permission or revision drift
cannot be bypassed by a previously created intent, status read or repeated upload. No automatic
producer retries, rendering jobs, billing reconciliation or media conversion are promised.

## Private object persistence

The development adapter requires an explicit private `CANTOS_AUDIO_ROOT` outside public/static
serving. When private audio storage is enabled, the API host disables **all** same-process
static serving, including otherwise disjoint public roots with nested symlinks. Serve Studio
separately through its existing Trunk proxy and matching Web origin. That separate public tree
must not reference private objects. Actual media bytes live in content-addressed immutable filesystem objects **outside
PostgreSQL**. PostgreSQL holds intent/receipt metadata and the private object reference.
Neither storage keys nor filesystem paths appear in wire responses.

Private object persistence uses an atomic no-overwrite step; existing objects are checked rather than
silently replaced. A receipt may commit only after the object has passed integrity checks.
Filesystem and PostgreSQL do not share a transaction. A crash or database rollback may leave
safe private orphan objects, never an accepted receipt that knowingly refers to an incomplete
object. Rollback must not delete a content-addressed object that another receipt may share.
Automatic orphan cleanup, storage lifecycle operations, S3 grants and production object storage
are deferred; there is no S3/live/public-delivery readiness claim.

## Errors and deferred gates

Failures use the existing JSON `ApiError`. HTTP 422 `audio_rejected` identifies rejected observed
media with `issues` containing the safe path `audio` and the inspector's stable rule code.
An unavailable inspector or inspection timeout returns HTTP 503 `unavailable` with a safe
inspection rule; it must not be treated as invalid media or create an accepted receipt.
Field issues omit private bytes, SQL errors, parser traces and storage details.
Existing `invalid_request`, `unauthenticated`, `forbidden`/`not_found`, `stale_revision`,
`operation_reused`, `unavailable` and corruption meanings remain distinct. Oversized bytes are
rejected with HTTP 413 `invalid_request` before receipt persistence. A rejection or rolled-back
request must not yield success.

This contract creates no provenance/rights attestation, human-listened QC, editorial approval,
catalog/content-unit model, external-producer token, public streaming/download route, release
manifest or publication authority. Private intake is a prerequisite for those later gates.

The [fixtures](fixtures/audio-intake/v1/README.md) contain synthetic JSON only. The raw wire and
schema tests protect closed object boundaries, exact pins, explicit profile versions, bitrate/
size/duration bounds, nullable no-receipt state and AAC rejection. They do not prove actual media
inspection or storage/authorization behavior. Runtime and database tests must exercise those
real boundaries separately before deployment.
