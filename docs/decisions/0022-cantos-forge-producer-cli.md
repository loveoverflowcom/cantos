# ADR 0022: Cantos Forge is a producer outside the API runtime

- Status: **accepted product direction**, clarified by the owner on 2026-10-16 in #82/#203.
  Local artifact CLI foundation is implemented; provider/Functions/Drive execution and API
  submission are not. Runtime packaging and live acceptance remain decision gates.
- Date: 2026-10-13; owner clarification: 2026-10-16
- Decision owner: Cantos project
- Related issues: [#82 — Forge vision](https://github.com/loveoverflowcom/cantos/issues/82),
  [#203 — local/Functions implementation](https://github.com/loveoverflowcom/cantos/issues/203)
- Refines [ADR 0018](0018-external-audio-production.md)'s producer wording without changing
  its prohibition on synthesis inside Cantos API.

## Context

Forge manages story/script/audio production outside the API runtime. The first foundation
added a Rust library/CLI in `apps/cantos-forge`: local immutable artifact revisions, deterministic
checks and handoff-bundle export. It does not yet generate audio, use Drive, expose an HTTP
server or submit to Cantos API; [the application README](../../apps/cantos-forge/README.md)
owns exact available commands.

On 2026-10-16 the owner chose Studio-controlled Gemini TTS, sharing one Forge implementation
between local CLI and Firebase Functions. The owner also selected private personal Google Drive
for remote **drafts**, reusing the existing story workspace rather than adding draft storage.
This refines the original CLI-only scope and the former wording that Studio must never start
a producer operation. It does not move TTS into the API or accept a deployed runtime.

## Decision

1. **One producer core.** CLI and Functions adapters invoke the same Rust domain/application
   use cases. Forge uses packages/contracts, never service code, another app's internals or
   Narrative Forge internals. Extract another shared crate only for a concrete consumer.
2. **Studio owns the workflow.** Web buttons, generation settings, status, preview and explicit
   acceptance stay in Studio; no Forge GUI. Studio calls two separately configured backends:
   VPS Cantos API for authoritative content/permissions/admission/publication, and Forge via
   local/deployed Firebase Functions for producer operations.
3. **API isolation.** Cantos API never calls Gemini, spawns Forge, holds provider credentials or
   depends on Forge for catalog/playback. Theatre never starts generation. The browser requests
   an authorized operation; only the producer executes it.
4. **Draft ownership.** Private personal Drive is the remote home for applicable raw/score/audio
   drafts, immutable revisions/checksums, indexes and producer state. Preserve the existing
   [story workspace](../production/story-workspace.md); original authoring is not fabricated
   source acquisition, and narration/podcast may omit Script IR. Deliberate offline CLI work
   uses filesystem without mandatory Firebase credentials or server Work IDs.
5. **No second authority.** Forge/Drive owns producer drafts, not a Catalog or release store.
   No extra Firebase Storage/S3/Firestore merely for drafts. Reviewed canonical content and
   media enter Cantos only through explicit authorized API admission. Draft upload or technical
   PASS never grants review, rights or publication approval.
6. **Safe effects.** Record pinned inputs and an attempt before a billable effect. Same key with
   different input conflicts; retry is not a new generation decision. Unknown provider outcome
   requires reconciliation/operator decision before paid replay. Drive is not a transactional
   queue/lock: local/cloud serialization and cost/concurrency safety must be demonstrated.
7. **Trusted adapters.** Drive OAuth, Gemini, Firebase/HTTP and process/encoder details stay in
   adapters, not domain/Script IR. Use configured private destinations and trusted secret
   bindings; never embed private folder IDs/tokens in Git or browser bundles. A publicly
   reachable endpoint still authorizes every draft/status/preview/generation operation.

Issue #203 owns concrete OAuth/access, byte readback/index updates, attempt recovery,
authorization, limits and acceptance. Do not turn illustrative port/route names into contracts.

## Runtime packaging decision still open

| Candidate | Required next evidence |
| --- | --- |
| Gen 2 Function with thin Node/TypeScript wrapper and packaged Rust worker | Preferred feasibility spike in #203; prove deployed ABI/dependencies, encoder, cold start, deadlines, bounded memory and child cleanup; no copied TTS business logic |
| Function gateway to authenticated private Cloud Run Rust container | Fallback only if the first approach is unsuitable; explain added service/cost and service authentication |

Local process/emulator success does not prove a deployed Function. No runtime, worker protocol,
HTTP DTO, queue, new service or cloud deployment is selected merely by this ADR.

## Consequences and compatibility

Existing local artifact/workspace/bundle formats and offline commands remain available.
The API's Audio Lifecycle still uses owner preparation and a scoped binary PUT that admits/
finalizes; no direct S3 write, automatic submission or publication is implied.

Local provider recovery, authorized Functions/Drive I/O and API handoff require separately
reviewed implementation. The Ogg/Opus inspector and prospective shared Script IR validator
should be consolidated only through a justified shared pure boundary, never imports from
service internals. Provider-generated claims remain producer-declared.

## Validation and revisit conditions

Historical local CLI/domain/filesystem/command evidence stays in the
[Forge README](../../apps/cantos-forge/README.md). This amendment records owner direction;
it adds no provider-live, Functions, Drive, SQL, device or listening evidence.

Deliver one bounded requested Gemini segment before chapter-scale orchestration. Prove local/
cloud use-case parity, private draft integrity/recovery, scoped authorization and uncertain
billable-outcome handling; then verify explicit API admission. No production/deployment or
paid-call authority follows from writing these docs.
