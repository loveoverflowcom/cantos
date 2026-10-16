# Architecture decision index

Reviewed 2026-10-16. This index routes to decisions; each ADR owns its exact status and scope.
**Accepted** is policy authority only within that scope. **Proposed** needs its own decision,
even when a bounded core is implemented. **Historical/superseded** is traceability, not an
instruction to restore runtime behavior. Evidence proves only its named inputs/environment.

## Decisions

| ADR | Lifecycle | Scope / remaining boundary |
| --- | --- | --- |
| [0001 — Modular monolith](0001-modular-monolith.md) | Accepted | Stack and inward module ownership; implemented baseline |
| [0002 — Script IR](0002-script-ir-contract.md) | Proposed policy; implemented contract | Production adoption remains separate |
| [0003 — Script revisions](0003-script-revision-persistence.md) | Proposed policy; implemented slice | Immutable local authenticated revision store |
| [0004 — Editorial handoff](0004-editorial-handoff.md) | Proposed adoption; implemented | Source/history/review compatibility |
| [0005 — Manuscript import](0005-manuscript-import.md) | Proposed adoption; implemented | Bounded preserved import/extraction |
| [0006 — Host-owned adaptation](0006-ai-script-adaptation.md) | Proposed adoption; implemented | Host inference, API proposal/review only |
| [0007 — Casting inputs](0007-casting-production-inputs.md) | Proposed adoption; implemented planning | Rights/guidance/approval metadata, no synthesis |
| [0008 — Local TTS inspection](0008-local-tts-inspection.md) | Historical; superseded by 0018 for backend | Original local experiment only |
| [0009 — Monorepo layout](0009-monorepo-layout.md) | Accepted | Current platform matrix overrides old Studio Mobile reservation |
| [0010 — Neural TTS research](0010-local-neural-tts-research.md) | Historical; superseded by 0018 for backend | Retained research only |
| [0011 — Google TTS adapter](0011-google-cloud-tts-adapter.md) | Historical; superseded by 0018 for backend | No live backend provider capability |
| [0012 — xtask](0012-xtask-command-surface.md) | Accepted | One repository command surface |
| [0013 — MessagePack](0013-optional-messagepack-wire.md) | Accepted legacy foundation; binary preference superseded by 0023 | Existing route compatibility remains |
| [0014 — UI foundation](0014-ui-system-foundation.md) | Accepted bounded foundation | Renderer/runtime admission remains separate |
| [0015 — API surfaces](0015-api-surface-boundaries.md) | Proposed policy record; implemented naming/routes | Studio, Theatre and Admin stay separate |
| [0016 — Speech reuse](0016-speech-fingerprint-and-clip-reuse.md) | Historical; superseded by 0018 for backend | No synthesis-job/clip-reuse runtime |
| [0017 — DBSP job summary](0017-experimental-dbsp-job-summary.md) | Historical; superseded by 0018 for backend | Generic DBSP library remains optional; job adapter retired |
| [0018 — External production](0018-external-audio-production.md) | Accepted | No inference/mixing/encoding in Cantos API; Forge clarification in 0022 |
| [0019 — Content identity](0019-content-identity.md) | Accepted bounded implementation | Separate metadata/content authority and exact immutable bindings |
| [0020 — Private audio intake](0020-studio-private-audio-intake.md) | Accepted development scope | Explicit Opus envelopes; production/listening acceptance separate |
| [0021 — Podman services](0021-podman-service-runtime.md) | Accepted; tooling/config implemented | Live infrastructure and provider acceptance separate |
| [0022 — Cantos Forge](0022-cantos-forge-producer-cli.md) | Accepted owner direction | Local CLI and library attempt/recovery core exist; Studio-controlled local/Functions + private Drive draft execution pending |
| [0023 — Protobuf wire](0023-versioned-protobuf-wire.md) | Implemented opt-in pilot | JSON default; activation/full #97 acceptance deferred |
| [0024 — Studio OIDC](0024-studio-oidc-bff.md) | Proposed production adoption; implemented slice | Live issuer/admission acceptance separate |
| [0025 — Actor identity root](0025-actor-identity-root.md) | Proposed; listener slice implemented | Studio migration slices pending |
| [0026 — Mobile storage sandbox](0026-theatre-mobile-local-storage-sandbox.md) | Proposed; bounded sandbox implemented | No production SQLDelight/Okio/durable sync adoption implied |
| [0027 — Quint](0027-quint-executable-behavior.md) | Proposed architecture/model review | Model lifecycle and implementation observations stay separate |
| [0028 — Work delegation](0028-work-scoped-delegation.md) | Accepted first slice; implemented | Catalog opaque grants, not producer/upload/publication permission |
| [0029 — Visual comparison](0029-local-visual-comparison.md) | Proposed; local MVP implemented | Advisory pixel metric, no baseline/UX acceptance |
| [0030 — Public metadata](0030-release-safe-public-metadata.md) | Proposed | No P1 field/source policy accepted |
| [0031 — Interest receipts](0031-membership-interest-receipts.md) | Proposed | Privacy/submitter/persistence decisions pending |
| [0032 — Origin delivery](0032-theatre-origin-playback-delivery.md) | Proposed; composition-gated core implemented | No accepted live public delivery |
| [0033 — Public cover](0033-release-pinned-public-cover.md) | Proposed; pure kernel only | No public cover wire/storage/delivery activation |
| [0034 — Account policy](0034-theatre-signup-and-provisioned-studio.md) | Accepted product/onboarding direction | Theatre signup, provisioned-only Studio; live issuer activation separate |
| [0035 — Unit availability](0035-theatre-catalog-unit-availability.md) | Accepted owner decision | Version-aware metadata policy; implementation/consumer acceptance remains separate |
| [0036 — Firebase enrollment proof](0036-firebase-enrollment-proof.md) | Accepted bounded direction; default-disabled runtime | Dedicated enrollment UID and one-shot setup continuation; real provider/issuer/activation gates remain open |
| [0037 — Private storage](0037-provider-independent-private-storage.md) | Proposed adoption; implemented bounded slice | Former duplicate storage 0022; real-provider/publication acceptance separate |
| [0038 — Delivery evidence gates](0038-publication-streaming-evidence-gates.md) | Proposed | Former duplicate publication 0028; real delivery/native evidence required |

## ID allocation and historical references

ADR **0036** belongs to the Firebase enrollment decision, initially proposed in merged
[PR #208](https://github.com/loveoverflowcom/cantos/pull/208), then accepted within its stated
bounded scope; runtime implementation is distinct from live acceptance. It was
already allocated when the duplicate 0022/0028 records were reassigned to 0037/0038.
Check current develop and open PRs before allocating the next ID; gaps are valid.

The 2026-10-16 cleanup resolves collisions without changing the two renamed decisions:

| Previous filename / ambiguous ID | Current canonical decision |
| --- | --- |
| `0022-provider-independent-private-storage.md` | [0037 — Private storage](0037-provider-independent-private-storage.md) |
| `0028-publication-streaming-evidence-gates.md` | [0038 — Delivery evidence gates](0038-publication-streaming-evidence-gates.md) |

0022 uniquely means Forge; 0028 uniquely means Work delegation. Old prose saying only “ADR
0022/0028” needs its title/context to disambiguate. Repository links/labels are updated;
commit-pinned external history and original quoted command/path lists remain historical.
Do not recycle old numbers or renumber unrelated accepted decisions.
