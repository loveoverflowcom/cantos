# Documentation map

> **Active product direction:** [ADR 0018 — external audio production](decisions/0018-external-audio-production.md).
> [Proposed external producer API](../packages/contracts/external-audio-handoff-v1.md) is not
> implemented yet. Prior TTS worker/provider/DBSP job-summary evidence below is **historical**,
> not current deployment or roadmap scope.

| Document | Owns |
| --- | --- |
| [Product brief](product/brief.md) | Audience, surfaces, scope and MVP outcome |
| [Content Identity](decisions/0019-content-identity.md) | Work/unit/content/script authority split and immutable content; [evidence](evidence/content-identity.md), [Production Brief handoff](work-plan/content-identity-production-brief-handoff.md) |
| [Publication core](evidence/publication-core.md) | Studio publication contracts, immutable release/rights gates, source-bound proof evidence and pending SQL/public delivery acceptance; [successor handoff](work-plan/publication-discovery-handoff.md) |
| [Business rules](product/business-rules.md) | Domain invariants, rights, revisions and approvals |
| [Quint foundation evidence](evidence/quint-behavior-foundation.md) | Executed draft-model scenarios, witnesses, negative controls, locked provenance and separate implementation-conformance/runtime gaps |
| [External audio handoff pipeline](product/production-pipeline.md) | Versioned producer briefs, private upload/finalize, QC and immutable publication; no in-backend synthesis |
| [Story workspace](production/story-workspace.md) | Agent-operated Drive/local story folders, indexes, synchronization and offline recovery |
| [Radio score](production/radio-score.md) | Dialogue adaptation, proper-name mapping and performance cues using Script IR |
| [Audio drama craft](production/audio-drama-craft.md) | Ear-first adaptation, narration purpose, silence, voices, motifs and mix intent |
| [Original Vietnamese reference episode](production/examples/vietnamese-reference/episode_0001.r0001.review.md) | Shared two-scene audition text, exact craft/cue coverage, unresolved 0.1.0 controls and structural validation evidence; no audio |
| [Listening-review protocol](production/listening-review-protocol.md) / [template](../templates/listening-review.md) | Named-reviewer, checksum-bound line/scene/episode evidence, two passes and required devices; proposed thresholds |
| [Listening protocol handoff](evidence/listening-review-protocol.md) | Documentation review, applicable local gates and unresolved listening/threshold/rights decisions; no scored audio |
| [Story workspace evidence](evidence/story-workspace-skills.md) | Skill/initializer checks, editorial rehearsal, verified Drive setup and residual limits |
| [PostgreSQL/DBSP skill evidence](evidence/postgresql-dbsp-skills.md) | Read-performance workflow validation, routing walkthrough and explicitly unrun database/engine gates |
| [DBSP job-summary evidence](evidence/dbsp-job-summary.md) | Implemented experimental DBSP read model: differential/failure/authorization tests, mutants, full-suite feature run and bounded benchmark with limits |
| [Architecture overview](architecture/overview.md) | Planned boundaries, deployment and data ownership |
| [API context architecture](architecture/api-contexts.md) | Context authorities, concrete composition, private intake/read facade and dependency boundaries; no worker |
| [Work-scoped delegation RFC](architecture/work-scoped-delegation-rfc.md) / [ADR 0028](decisions/0028-work-scoped-delegation.md) | A/B/C/hybrid comparison and call-site inventory ([synthetic research](evidence/work-scoped-delegation-research.md)); first Catalog slice with central opaque grants ([contract](../packages/contracts/delegation-v1.md), [evidence](evidence/work-scoped-delegation-implementation.md)); further actions remain decision-pending |
| [Private object storage](architecture/object-storage.md) / [ADR 0022](decisions/0022-provider-independent-private-storage.md) | Immutable target resolution, filesystem/S3 adapters, checked topology, owner delivery and separate provider/publication acceptance |
| [API refactor evidence](evidence/api-bounded-contexts.md) | Architecture guard, compatibility and invariant regressions, final local gates and residual limits |
| [Firebase Hosting runbook](deployment/firebase-hosting.md) / [evidence](evidence/issue199-firebase-hosting.md) | Manual static Web deployment of Theatre or Studio to Firebase Hosting: setup, routine, verification, recovery, the unmet API-origin gate and the unrun live acceptance |
| [Repository layout](architecture/repository-layout.md) | Apps, services and packages: ownership, dependency direction, status and moved paths |
| [Restructure evidence](evidence/monorepo-restructure.md) | Checks run for the monorepo reorganization and what remains unverified |
| [xtask evidence](evidence/unified-xtask.md) | Commands run for the `cargo xtask` surface, with PASS, NOT_RUN and UNSUPPORTED results |
| [Wire codec](architecture/wire-codec.md) | JSON and optional MessagePack representations: route scope, negotiation, limits, errors, client switch and rollback |
| [MessagePack wire evidence](evidence/messagepack-wire.md) | Goldens, parity and hostile-input tests, native/WASM codec timings, bundle sizes, browser runs and unrun PostgreSQL checks |
| [Script IR](architecture/script-ir.md) | Versioned interchange and reuse without project coupling |
| [Script IR evidence](evidence/script-ir-contract.md) | Executed contract checks and unrun application gates |
| [Final revision sequence review](evidence/script-revision-final-review.md) | Independent findings, fixes, final local checks and remaining runtime limits |
| [Editorial handoff evidence](evidence/editorial-handoff.md) | Compatible source/history/review port, upgrade/recovery checks and read baseline |
| [WAV PCM inspection evidence](evidence/wav-pcm-inspection.md) | Standalone structural/checksum/sample-peak report contract, fixtures, oracles and non-claims |
| [Script IR diff evidence](evidence/script-diff.md) | Read-only stable-ID comparison of two script versions: report contract, move semantics, oracles and non-claims |
| [Casting/input evidence](evidence/casting-production-inputs.md) | Frozen planning scope, rights/budget/approval checks, actual tests and affected read/DBSP assessment |
| [Local TTS evidence](evidence/tts-receipt-inspection.md) | Bounded local samples, snapshot-bound receipts, actual checks and commercial/production limits |
| [TTS prerequisites](evidence/tts-provider-prerequisites.md) | Official provider declarations, local selection and deferred commercial prerequisites |
| [Google TTS adapter evidence](evidence/google-tts-adapter.md) | Google casting/adapter claims ledger, mocked checks, NOT_RUN live synthesis and residual provider risk |
| [Studio provider casting evidence](evidence/studio-provider-casting.md) | Explicit neutral/0, capability-driven controls, retained unsupported values and real backend/browser checks |
| [Local neural research](evidence/local-neural-tts-research.md) | Piper/VieNeu engine, model, preset and training-rights comparison; pinned local resource plan |
| [Local neural evidence](evidence/local-neural-tts.md) | Research-only adapter boundary, exact identities, checks, actual versus mocked results and limits |
| [Mobile architecture](architecture/mobile.md) | CMP shell and native listening behavior |
| [UI system](design/ui-system.md) | Shared semantic design requirements and validation |
| [UI foundation evidence](evidence/ui-system-foundation.md) | Token/icon provenance, focused verification and unrun runtime gates |
| [UI consumer integration](evidence/ui-system-integration.md) | Studio, bounded Theatre, PR29 reconciliation and runtime gaps |
| [UI publication verification](evidence/ui-system-publication.md) | PR31/PR32 reconciliation, combined local gates and remaining acceptance |
| [Leptos localization spike](evidence/leptos-localization-spike.md) | Three-candidate Leptos 0.7.8 measurements, single-pass interpolation, typed locale roots and `<html lang>`; no library selected (#167) |
| [UI migration](architecture/ui-system-migration.md) | Token package rename, public role compatibility and renderer ownership |
| [UI sequence](work-plan/015-leptos-ui-system.md) | Component, app integration and independent review handoff |
| [Decision 0001](decisions/0001-modular-monolith.md) | Initial stack and modular-backend decision |
| [Decision 0002](decisions/0002-script-ir-contract.md) | Proposed adoption of implemented Script IR and digest policy |
| [Decision 0003](decisions/0003-script-revision-persistence.md) | Local revision persistence and the minimal Studio consumer |
| [Decision 0004](decisions/0004-editorial-handoff.md) | Compatible editorial handoff on the existing revision backend |
| [Decision 0007](decisions/0007-casting-production-inputs.md) | Proposed production adoption of versioned casting and scoped input authorization |
| [Decision 0008](decisions/0008-local-tts-inspection.md) | Accepted local eSpeak inspection selection; production adoption proposed |
| [Decision 0009](decisions/0009-monorepo-layout.md) | Product-grouped apps, one backend service and shared packages |
| [Decision 0010](decisions/0010-local-neural-tts-research.md) | Proposed VieNeu Nano research selection; production dispatch disabled |
| [Decision 0011](decisions/0011-google-cloud-tts-adapter.md) | Proposed Google Cloud TTS adapter: documented vi-VN capabilities, mapping, failure and charge policy |
| [Decision 0012](decisions/0012-xtask-command-surface.md) | `cargo xtask` as the single build, run and verification surface; CI runs its jobs |
| [Decision 0013](decisions/0013-optional-messagepack-wire.md) | Optional MessagePack beside JSON on four Studio routes; the Studio ships JSON until revisit conditions hold |
| [Decision 0014](decisions/0014-ui-system-foundation.md) | Shared canonical UI tokens and separate Leptos/CMP renderer ownership |
| [Decision 0015](decisions/0015-api-surface-boundaries.md) | Proposed: `apps/cantos-theatre` naming and `studio`/`theatre`/`admin` API route groups |
| [Decision 0016](decisions/0016-speech-fingerprint-and-clip-reuse.md) | Proposed: position-free speech fingerprint, verified clip registry and attempt-less reuse, regeneration plan |
| [Decision 0017](decisions/0017-experimental-dbsp-job-summary.md) | Historical opt-in local DBSP TTS job-summary experiment (runtime adapter retired); production adoption gated |
| [Decision 0022](decisions/0022-cantos-forge-producer-cli.md) | Proposed: Cantos Forge, a producer-side CLI outside the API runtime; API/Studio/Theatre still never execute generation |
| [Decision 0025](decisions/0025-actor-identity-root.md) | Proposed: `ActorId` as the root of owner and listener identity; listener slice implemented, Studio slices pending |
| [Decision 0026](decisions/0026-theatre-mobile-local-storage-sandbox.md) | Proposed: Theatre Mobile per-layer storage test doubles, directory classes and adoption gates; `ProgressPort` contract suite implemented, no storage library adopted |
| [Executable behavior](../specs/behavior/README.md) / [ADR 0027](decisions/0027-quint-executable-behavior.md) | Quint catalog and private admission model with independent pure-core observations; [PR B observation handoff](work-plan/quint-conformance-handoff.md), DB/API/runtime acceptance separate |
| [Work plan](work-plan/README.md) | Recommended sequence, dependencies and review boundaries |
| [GitHub planning](work-plan/project-planning.md) | Issue/dependency map, Kanban workflow and links to source documents |
| [Audio drama quality plan](work-plan/audio-drama-quality.md) | Research reconciliation and ordered issues improving the adaptation process and audio output |

These documents specify intended behavior, unless a section explicitly provides implementation evidence. Update the owning document instead of copying the same rule into multiple competing specifications. Use [templates](../templates/README.md) for subsequent decisions and production records.

- [Theatre Discovery/Playback evidence](evidence/theatre-discovery.md) and
  [Library/Progress handoff](work-plan/discovery-library-handoff.md).
