# External Audio Handoff v1 — Proposed Contract

> **Design only / NOT IMPLEMENTED.** No route or DTO in this document is a promise about an
> existing server. [ADR 0018](../../docs/decisions/0018-external-audio-production.md) owns the
> product boundary. Freeze schemas and update this document with code/evidence in a future PR.

## Objective

Cantos supplies immutable, reviewable production instructions to independently operated AI
agents/plugins and accepts finished audio bytes and provenance. Cantos never dispatches the
producer, pays a model, does TTS/mixing or performs audio format conversion.

## Proposed resources

- `Work`: immutable work ID; type `novel | story | podcast`; title, synopsis, author and rights.
- `ContentUnit`: chapter/episode ID; work ID, stable order, optional text/script revision.
- `ProductionBrief`: immutable brief ID/version/digest tied to a content unit and permissions;
  contains scoped text/references, language, performance/lexicon/cue guidance if applicable,
  target audio profile and QA instructions.
- `AudioSubmission`: owner/content/brief scope, client operation ID, pending upload intent,
  object identity, expected/observed digest, declared source/provenance, status.
- `AudioRendition`: immutable admitted media object with verified size/digest/format/duration
  metadata, language, profile and private storage reference.
- `AudioReview`: named reviewer and findings tied to the exact rendition/checksum/profile.
- `Release`: atomically published immutable rendition manifest; replaced by a new release,
  never overwritten in place.

The Rust domain model should use distinct validated IDs and explicit state transitions.
A recording can be accepted without Script IR, casting or a model ID. This is essential for
human-narrated podcasts and simple chapter audiobooks.

## Proposed authenticated API surface (not live)

| Operation | Tentative route | Ownership |
| --- | --- | --- |
| Read frozen instructions | `GET /api/v1/studio/content-units/{id}/production-briefs/{version}` | Studio owner or scoped producer |
| Create upload intent | `POST /api/v1/studio/content-units/{id}/audio-submissions` | Scoped producer |
| Upload binary bytes | Short-lived **private object-store URL** returned by prepare | External producer |
| Finalize one upload | `POST /api/v1/studio/audio-submissions/{id}/finalize` | Same scoped producer |
| Read status/receipt | `GET /api/v1/studio/audio-submissions/{id}` | Authorized creator/producer |
| Record QC/editorial decision | `POST /api/v1/studio/audio-submissions/{id}/reviews` | Authorized reviewer |
| Publish an approved rendition | `POST /api/v1/studio/content-units/{id}/releases` | Publisher |

Finalize needs a stable idempotency key; it must not succeed with a different expected digest
under the same operation ID. The API must read actual object facts, not trust an arbitrary URL.
Publisher permissions and content-owner permissions are distinct from producer upload grants.

## Producer instruction bundle

An external agent should receive: canonical work/episode identity, frozen content references
and digest, declared rights constraints and permitted use, reading style/genre/scene notes,
voice and pronunciation guidance if known, optional Script IR and asset cue sheet, expected
audio profile/version, expected naming/metadata, privacy limitations, QA procedure and upload
endpoint/credential acquisition. Keep secrets and unlicensed full texts out of public exports.

## Verification and publication invariants

- Scope matches the producer credential, work ID, content-unit ID and frozen brief digest.
- Object digest, byte count and playable metadata are checked by Cantos against stored bytes.
- Media files are private until an explicit QC/editorial/rights approval and release transaction.
- Double finalize and network retry replay the original receipt; contradictory replay conflicts.
- Partial or orphaned transfers never become releases. Repair/cleanup is bounded and auditable.
- Claimed model, vendor cost, voice rights and subjective quality remain evidence **claims**
  unless independently verified or listened to.
- Revoked/expired permission, stale brief or missing object blocks publication.
- Theatre never sees private upload credentials, unapproved media or TTS endpoints.

## Codec / audio profile

Default suggestions remain a proposal: Opus mono 20/24/32/40 kbps for bandwidth-sensitive
speech/mixed programs, AAC-LC mono 40 kbps as a compatibility fallback. Do not hardcode a
mandatory profile or advertise codec compatibility until [#51](https://github.com/loveoverflowcom/cantos/issues/51)
and [#64](https://github.com/loveoverflowcom/cantos/issues/64) resolve.
