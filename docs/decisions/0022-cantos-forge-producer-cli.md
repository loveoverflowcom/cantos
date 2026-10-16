# ADR 0022: Cantos Forge is a producer outside the API runtime

- Status: **accepted product direction**, clarified by the owner on 2026-10-16 in #82/#203.
  Local artifact CLI, attempt/recovery core, local filesystem attempt adapter and bounded worker
  boundary are implemented at `services/cantos-forge-functions`. Real provider/Functions/Drive
  execution and API submission are not. Deployed packaging and live acceptance remain gates.
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
server or submit to Cantos API; [the application README](../../services/cantos-forge-functions/README.md)
owns exact available commands.

On 2026-10-16 the owner chose Studio-controlled Gemini TTS, sharing one Forge implementation
between local CLI and Firebase Functions. The owner also selected private personal Google Drive
for remote **drafts**, reusing the existing story workspace rather than adding draft storage.
This refines the original CLI-only scope and the former wording that Studio must never start
a producer operation. It does not move TTS into the API or accept a deployed runtime.

## Decision

1. **One producer core.** CLI and Functions adapters invoke the same Rust domain/application
   use cases. Forge uses packages/contracts, never another service's code, another app's internals or
   Narrative Forge internals. Extract another shared crate only for a concrete consumer.
2. **Studio owns the workflow.** Web buttons, generation settings, status, preview and explicit
   acceptance stay in Studio; no Forge GUI. Studio calls two separately configured backends:
   VPS Cantos API for authoritative content/permissions/admission/publication, and Forge via
   local/deployed Firebase Functions for producer operations.
3. **API isolation.** Cantos API never calls Gemini, spawns Forge, holds provider credentials or
   depends on Forge for catalog/playback. Theatre never starts generation. The browser requests
   an authorized operation; only the producer executes it.
4. **Draft ownership.** The owner's private Drive `appDataFolder`, accessed with `drive.appdata`
   by the intended Cantos OAuth application, is the primary remote home for applicable
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

## Owner amendment: workspace and service placement

The owner's consolidated [#203 direction](https://github.com/loveoverflowcom/cantos/issues/203)
and #82 clarification select a Forge workspace section inside Studio Web, private Drive
`appDataFolder` and `services/cantos-forge-functions` as the producer source location. They
supersede the earlier `apps/cantos-forge` placement and ordinary-Drive-folder default. This
amendment records that accepted product direction; it does not accept an authorization grant,
coordination service, cloud packaging or a completed production workflow.

One crate retains the `cantos-forge` package and CLI binary name, local workspace schemas,
artifact IDs, revisions, evidence and handoff-bundle formats. The additional finite worker
binary consumes that same library. Studio never imports it. Function runtime source belongs
to the service; future deploy configuration belongs to `infra/firebase/functions` under #212.

App-data is hidden from the normal Drive UI, cannot be shared or moved across spaces, and has
no Trash recovery. Existing ordinary Drive sources remain untouched. An explicit import copies
authorized bytes into new verified artifacts with lineage; export/backup is a separate action.
Application tombstones precede any separately authorized purge with retention/reference checks.
The existing `forge-workspace.json` artifact/evidence/blob format is an offline representation;
`raw/score/audio`, `story.json` and `index.md` are story import/export conventions, not a second
editable authority or an implemented app-data mapping.

A draft has its own stable identity without fabricated API Work/Content/Brief IDs. A future
workspace must keep execution, storage, technical/AI/human review, selection, explicit handoff,
API admission and publication independent: Generated ≠ Saved ≠ Selected ≠ Synced ≠ Admitted
≠ Published. Only acknowledged API results create authoritative mappings. No draft change
silently regenerates audio or publishes anything.

## Runtime packaging feasibility and remaining decision

| Candidate | Required next evidence |
| --- | --- |
| Gen 2 Function with thin Node 22 wrapper and packaged Rust worker | Selected for the local feasibility boundary in #203; finite protocol and process adapter only. Prove deployed ABI/dependencies, encoder, cold start, deadlines, bounded memory and child cleanup before deployment support; no copied producer logic |
| Function gateway to authenticated private Cloud Run Rust container | Fallback only if the first approach is unsuitable; explain added service/cost and service authentication |

Firebase Functions does not offer a native Rust runtime. The implemented local candidate is
Node 22 plus a prepackaged Rust executable, not Rust-to-WASM, a native addon or a TypeScript
producer rewrite. See the [worker boundary and acceptance runbook](../deployment/forge-worker.md).
The HTTP adapter fails closed while trusted owner invocation/access grants are unavailable;
it cannot generate, read private drafts or pass a Studio cookie to a worker. No deployment,
queue, cloud credential setup or billing change follows from this candidate.

Local process success does not prove a deployed Function. A real Linux worker/encoder bundle,
supported OS/CPU/ABI, permissions, TLS/CA/native libraries, runtime resource/deadline policy
and child termination must be observed in an explicitly authorized deployed environment.

## Consequences and compatibility

Existing local artifact/workspace/bundle formats and offline commands remain available.
The API's Audio Lifecycle still uses owner preparation and a scoped binary PUT that admits/
finalizes; no direct S3 write, automatic submission or publication is implied.

Local provider recovery, authorized Functions/Drive I/O and API handoff require separately
reviewed implementation. The Ogg/Opus inspector and prospective shared Script IR validator
should be consolidated only through a justified shared pure boundary, never imports from
service internals. Provider-generated claims remain producer-declared.

The first slice of [#203](https://github.com/loveoverflowcom/cantos/issues/203) implemented only
the attempt and recovery rules (item 6) as a pure core with a mock provider
([record](../evidence/forge-attempt-verification.md)). The local filesystem attempt adapter now
persists write-once, bounded histories and requires durability before dispatch can be allowed.
It is a local adapter, not a Drive queue or a distributed fence. The finite worker permits
capability discovery and deterministic text inspection only; no production generation path is
enabled by these foundations. The [runbook](../deployment/forge-worker.md) owns the remaining
grant/coordinator, app-data, Gemini/encoder, Studio and explicit handoff boundaries. A closed
issue does not establish requested runtime or acceptance.

## Validation and revisit conditions

Historical local CLI/domain/filesystem/command evidence stays in the
[Forge README](../../services/cantos-forge-functions/README.md). This amendment records owner direction;
it adds no provider-live, Functions, Drive, SQL, device or listening evidence.

Deliver one bounded requested Gemini segment before chapter-scale orchestration. Prove local/
cloud use-case parity, private draft integrity/recovery, scoped authorization and uncertain
billable-outcome handling; then verify explicit API admission. No production/deployment or
paid-call authority follows from writing these docs.
