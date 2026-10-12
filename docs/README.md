# Documentation map

| Document | Owns |
| --- | --- |
| [Product brief](product/brief.md) | Audience, surfaces, scope and MVP outcome |
| [Business rules](product/business-rules.md) | Domain invariants, rights, revisions and approvals |
| [Production pipeline](product/production-pipeline.md) | Rendering flow, recovery, costs and publication gates |
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
| [Decision 0017](decisions/0017-experimental-dbsp-job-summary.md) | Accepted as an opt-in local experiment: DBSP job-summary read model, committed outbox feed and fenced serving; production adoption gated |
| [Work plan](work-plan/README.md) | Recommended sequence, dependencies and review boundaries |
| [GitHub planning](work-plan/project-planning.md) | Issue/dependency map, Kanban workflow and links to source documents |
| [Audio drama quality plan](work-plan/audio-drama-quality.md) | Research reconciliation and ordered issues improving the adaptation process and audio output |

These documents specify intended behavior, unless a section explicitly provides implementation evidence. Update the owning document instead of copying the same rule into multiple competing specifications. Use [templates](../templates/README.md) for subsequent decisions and production records.
