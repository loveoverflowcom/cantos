# External Audio Production Handoff

[ADR 0018](../decisions/0018-external-audio-production.md) keeps synthesis/mixing/encoding
outside Cantos API. [Forge ADR 0022](../decisions/0022-cantos-forge-producer-cli.md) records the
owner's planned local/Functions producer controlled by Studio. Manual/other external producers
remain valid. The API is the content/admission/review/publication authority.

## End-to-end flow

1. Studio creates a Work/chapter/episode, optional text/Script IR, rights and an immutable brief.
2. An authorized producer reads exact instructions and produces or records audio externally.
   The planned Forge route lets Studio request generation and listen to private Drive drafts.
3. The owner prepares an exact AudioSubmission and a separate scoped upload permission.
4. The producer sends one bounded binary PUT; the API inspects stored bytes and performs
   admission/finalization, persisting an immutable receipt.
5. Studio reviews the exact private artifact, rights/provenance and named QC/listening evidence.
6. An eligible immutable release is published through reviewed delivery policy; Theatre consumes
   published media without ever starting production.

[Production Brief v1](../../services/cantos-server/contracts/studio/briefs/v1/README.md),
[Audio Lifecycle v1](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md) and
[Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md) own the implemented contracts.
The [general handoff proposal](../../services/cantos-server/contracts/producer/handoff/v1/README.md) is broader
and remains a proposal; its object-store PUT plus separate finalize design is not a current API.

## Instructions and external production

Pin content/brief revision and digest, relevant guidance/rights, explicit audio profile and
producer provenance. Scene, speaker, pronunciation and ambience data apply only when needed;
plain narration and human-recorded podcasts require no forced drama/casting workflow.

The producer owns provider credentials, hardware, attempts/costs, ambiguous outcomes,
pronunciation, cue timing, conditioning, mixing and encoding. The API records submitted claims
as provenance, not proof that it executed a provider or verified commercial rights.

For #203, CLI and Firebase Functions share Forge Rust use cases; Studio owns controls/status/
preview/acceptance. Private personal Drive is the chosen remote **draft** workspace for
applicable raw/score/audio artifacts and indexes, with local filesystem for offline work.
Do not add another draft bucket/Catalog. Drive synchronization, technical checks, human
listening, API admission and publication are different facts. Functions/Drive/provider
execution and automatic API submission are not implemented by today's local Forge foundation.

## Private admission, review and publication

- Prepare binds one operation, unit, exact content/brief/profile and expected byte identity.
  A producer upload bearer is distinct from the Studio session and has bounded scope/lifetime.
- The current capped PUT admits/finalizes through the API; partial bytes, wrong hash/size/profile,
  stale binding, revoked/expired permissions and conflicting replays fail closed.
- Private storage has filesystem/S3 adapters and immutable recorded targets. Verify actual bytes;
  a SQL transaction cannot roll back an external object write. Preserve receipts and reconcile
  uncertain outcomes rather than creating a second operation blindly.
- Review distinguishes producer declarations, server-inspected technical facts and named human
  listening/rights approval tied to the exact revision and bytes.
- Publication core owns readiness, immutable releases and atomic active pointers. Real delivery,
  provider isolation, retention and client playback remain separate acceptance gates.

The WAV PCM inspector is a limited structural/measurement tool, not a mixer, encoder or
loudness/true-peak/listening approval. [#51](https://github.com/loveoverflowcom/cantos/issues/51)
owns profile/acceptance decisions; the broader preset proposal #64 was closed as not planned.
The implemented four-profile intake does not choose an AAC fallback.

## Implementation and remaining acceptance

Catalog/Content/Brief, bounded Audio Lifecycle, private storage/artwork, named review/candidates,
Publication core and Discovery/Library backend slices exist. Studio has bounded real consumers.
Ordinary unconfigured public delivery remains unavailable; the origin-delivery core is gated
pending its decision and real acceptance (#170). Live identity/provider/storage, stronger
rights/listening policy and Web/native end-to-end acceptance remain with #9/#10/#11/#179/#180.

See [current priorities](../work-plan/current-priorities.md) rather than treating old per-session
NOT_RUN results as today's status. No provider, deployment or runtime gate was exercised by
this documentation update.

## Historical compatibility

Former synthesis workers, provider probes, TTS receipts and the DBSP job-summary adapter were
retired from the API. Historical migrations 0007–0009 and their evidence remain intact.
Forge's external producer direction does not revive backend generation tables or workers.
