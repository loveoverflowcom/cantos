# ADR 0018: Cantos is the audio content authority, not a TTS engine

- **Status:** Accepted (product direction)
- **Date:** 2026-10-12
- **Decision owner:** Cantos project
- **Supersedes for active runtime:** ADR 0008 (local TTS), ADR 0010 (local neural TTS),
  ADR 0011 (Google TTS adapter), ADR 0016 (server speech clip reuse), and the TTS-job
  implementation assumptions of ADR 0017. Their historical evidence is not deleted.

## Context

The previous design combined source/script editorial tooling with a server that would call
local/cloud TTS providers, manage paid attempts, maintain audio generation workers and mix
rendered clips. The actual product requirement is lighter: Studio manages published and
unpublished novels/stories/podcasts, their content units and associated audio; independently
operated agents or plugins produce the media by following Cantos instructions.

Provider coupling and job infrastructure increase dependency surface, resource cost, credential
risk and deployment complexity without making Cantos better at content management.

## Decision

1. **Studio is a Web-only CMS** for novel/story/podcast catalogs, chapters/episodes, optional
   Script IR, versioned production briefs, audio assets, rights, review and publication.
2. **No in-process generation.** Cantos API and runtime must not integrate TTS SDK/HTTP
   transports, local neural model runtimes, direct synthesis CLIs, synthesis workers or a
   backend audio mixer. No auto generation upon editing or listening.
3. **External production.** A caller-controlled AI tool/plugin/human producer fetches scoped,
   immutable instructions and any authorized content, produces/mixes/encodes locally or with
   chosen external services, and uploads a completed artifact with an immutable receipt.
4. **Verified intake.** The API authorizes a narrow upload, verifies actual stored bytes and
   provenance linkage, preserves ownership/rights/review states and publishes only approved
   immutable releases. Publishing and listener routes never dispatch production.
5. **Pure tools are permitted.** Script IR validation/diff, metadata checks, a bounded WAV
   structure inspector and declarative audio profile schemas remain useful. Their presence
   does not imply inference/mixing.
6. **Historical compatibility.** Keep applied SQL migrations 0007–0009 and audit/evidence docs
   without destructive schema rewrite. Deprecate obsolete tables/write paths. Plan eventual
   data retention/cleanup separately, after surveying deployed installations.
7. **DBSP stays technology-optional.** Engine-independent incremental computation may remain as
   an isolated library for future metadata projections; the old production-job summary adapter
   is not a current use case. Do not add DBSP solely to preserve orphaned TTS jobs.
8. **No implicit implementation.** Proposed `audio-handoff-v1` routes and DTOs are design
   only until a separately reviewed API PR and tests implement them.

## Producer clarification — 2026-10-16

The owner selected [Cantos Forge](0022-cantos-forge-producer-cli.md) as a producer outside the
API runtime: Studio may request producer operations through Firebase Functions, sharing the
same Rust core as the local CLI. Private personal Drive holds drafts; API admission/publication
remains authoritative on the VPS. This clarifies “external” without reviving backend TTS.
Only the local artifact CLI foundation exists today; Functions/Drive/provider execution and
automatic handoff remain #203 implementation/acceptance work.

## Security and invariants

Producer tokens must be scoped per work/content unit, time-limited and revocable; they must
not reuse a Studio session cookie. Upload bytes remain private until human/editorial approval.
Use object-store grants, server-side digest/size/format verification, exact revision binding,
idempotency keys, owner isolation, bounded QC and atomic release pointer updates. Preserve the
old release if upload/review/publication fails. Caller-declared provider/cost fields are not
platform-verified usage or permissions.

## Consequences and migration plan

- Remove the TTS execution/synthesis context, providers, worker/CLI probes, generation tests and
  obsolete development scripts; preserve pure WAV validation under `contexts/media`.
- Keep existing editorial/Script IR/adaptation/planning APIs while a simpler Work/Content/Audio
  model replaces the former generation-oriented Studio UI.
- Retarget issues #6, #7, #8, #9, #10, #11 and relevant craft tasks to external producer
  handoff, accepted audio assets, metadata and publication.
- Deliver separate reviewable PRs for (a) work catalog/content units and (b) external upload,
  provenance, QC/review/publication. Do not claim either exists merely from this ADR.
