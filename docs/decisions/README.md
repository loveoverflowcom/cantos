# Architecture decision index

Reviewed 2026-10-16. This index routes to decisions; each ADR owns its exact status and scope.
**Accepted** is policy authority only within that scope. **Proposed** needs its own decision,
even when a bounded core is implemented. **Historical/superseded** is traceability, not an
instruction to restore runtime behavior. Evidence proves only its named inputs/environment.

## Decisions

| ADR | Lifecycle | Scope / remaining boundary |
| --- | --- | --- |
| [0050 — Semantic localization](0050-localization-semantic-catalog.md) | Proposed; reversible fixture implemented | Resource-derived UI locale registry, typed Rust/Kotlin projection and API/FE audit under #167/#505; production runtime/formatters and #506/#507 adoption remain undecided |
| [0049 — Synthetic local mail](0049-synthetic-local-mail.md) | Proposed runner design; bounded fixture implemented | Explicit synthetic Resend HTTP capture; Identity proof, provider and native acceptance remain separate |
| [0044 — Native enrollment preview](0044-native-enrollment-preview.md) | Proposed development boundary | Shared CMP registration and separately enabled native admission; isolated full journey and native/production acceptance remain separate |
| [0001 — Modular monolith](0001-modular-monolith.md) | Accepted | Stack and inward module ownership; implemented baseline |
| [0002 — Script IR](0002-script-ir-contract.md) | Proposed policy; implemented contract | Production adoption remains separate |
| [0003 — Script revisions](0003-script-revision-persistence.md) | Proposed policy; implemented slice | Immutable local authenticated revision store |
| [0004 — Editorial handoff](0004-editorial-handoff.md) | Proposed adoption; implemented | Source/history/review compatibility |
| [0005 — Manuscript import](0005-manuscript-import.md) | Proposed adoption; implemented | Bounded preserved import/extraction |
| [0006 — Host-owned adaptation](0006-ai-script-adaptation.md) | Proposed adoption; implemented | Host inference, API proposal/review only |
| [0007 — Casting inputs](0007-casting-production-inputs.md) | Proposed adoption; implemented planning | Rights/guidance/approval metadata, no synthesis |
| [0008 — Local TTS inspection](0008-local-tts-inspection.md) | Historical; superseded by 0018 for backend | Original local experiment only |
| [0009 — Monorepo layout](0009-monorepo-layout.md) | Accepted | Current platform matrix overrides old Studio Mobile reservation; contract placement superseded by 0046 |
| [0010 — Neural TTS research](0010-local-neural-tts-research.md) | Historical; superseded by 0018 for backend | Retained research only |
| [0011 — Google TTS adapter](0011-google-cloud-tts-adapter.md) | Historical; superseded by 0018 for backend | No live backend provider capability |
| [0012 — xtask](0012-xtask-command-surface.md) | Accepted | One repository command surface |
| [0013 — MessagePack](0013-optional-messagepack-wire.md) | Accepted legacy foundation; binary preference superseded by 0023 | Existing route compatibility remains |
| [0014 — UI foundation](0014-ui-system-foundation.md) | Accepted bounded foundation | Renderer/runtime admission remains separate |
| [0015 — API surfaces](0015-api-surface-boundaries.md) | Proposed policy record; implemented naming/routes | Studio, Theatre and Admin stay separate |
| [0016 — Speech reuse](0016-speech-fingerprint-and-clip-reuse.md) | Historical; superseded by 0018 for backend | No synthesis-job/clip-reuse runtime |
| [0017 — DBSP job summary](0017-experimental-dbsp-job-summary.md) | Historical; superseded by 0018 for backend | Implementation/feature/gate retired; #384 defers incremental engines |
| [0018 — External production](0018-external-audio-production.md) | Accepted | No inference/mixing/encoding in Cantos API; Forge clarification in 0022 |
| [0019 — Content identity](0019-content-identity.md) | Accepted bounded implementation | Separate metadata/content authority and exact immutable bindings |
| [0020 — Private audio intake](0020-studio-private-audio-intake.md) | Accepted development scope | Explicit Opus envelopes; production/listening acceptance separate |
| [0021 — Podman services](0021-podman-service-runtime.md) | Accepted legacy; native dev/test dependency superseded | #348 selects OCI/Quadlet production packaging for two native executables; publish/host/native acceptance remain separate |
| [0022 — Cantos Forge](0022-cantos-forge-producer-cli.md) | Accepted owner direction | Relocated local CLI, attempt core/store and finite worker boundary exist; Studio-controlled local/Functions + private Drive app-data execution pending |
| [0023 — Protobuf wire](0023-versioned-protobuf-wire.md) | Implemented opt-in pilot | JSON default; activation/full #97 acceptance deferred; prost pin and binding ownership amended by 0046 |
| [0024 — Studio OIDC](0024-studio-oidc-bff.md) | Proposed production adoption; implemented slice | Live issuer/admission acceptance separate |
| [0025 — Actor identity root](0025-actor-identity-root.md) | Proposed; listener slice implemented | Studio migration slices pending |
| [0026 — Mobile storage sandbox](0026-theatre-mobile-local-storage-sandbox.md) | Proposed; bounded sandbox implemented | No production SQLDelight/Okio/durable sync adoption implied |
| [0027 — Quint](0027-quint-executable-behavior.md) | Proposed architecture/model review | Model lifecycle and implementation observations stay separate |
| [0028 — Work delegation](0028-work-scoped-delegation.md) | Accepted first slice; implemented | Catalog opaque grants, not producer/upload/publication permission |
| [0029 — Visual comparison](0029-local-visual-comparison.md) | Proposed; local MVP implemented | Advisory pixel metric, no baseline/UX acceptance |
| [0030 — Public metadata](0030-release-safe-public-metadata.md) | Accepted in part (owner, #157): option B principle and capability status matrix | Genre open; contributors, credits, seasons, markers, transcript deferred; no P1 field accepted |
| [0031 — Interest receipts](0031-membership-interest-receipts.md) | Proposed | Privacy/submitter/persistence decisions pending |
| [0032 — Origin delivery](0032-theatre-origin-playback-delivery.md) | Accepted for step-1 origin integration; configured filesystem adapter/server | Deployed TLS/proxy/performance and native playback acceptance pending |
| [0033 — Public cover](0033-release-pinned-public-cover.md) | Accepted (owner, #157); kernel, v1.2 DTOs, disclosure persistence/Studio route, `presentation=2` and the composition-gated origin cover route implemented (#171) | Web/CMP adoption (#158/#159), deployed-host acceptance, CORP/CORS and Studio disclosure UX |
| [0034 — Account policy](0034-theatre-signup-and-provisioned-studio.md) | Accepted product/onboarding direction | Theatre signup, provisioned-only Studio; live issuer activation separate |
| [0035 — Unit availability](0035-theatre-catalog-unit-availability.md) | Accepted owner decision | Version-aware metadata policy; implementation/consumer acceptance remains separate |
| [0036 — Firebase enrollment proof](0036-firebase-enrollment-proof.md) | Accepted bounded direction; default-disabled runtime | Dedicated enrollment UID and one-shot setup continuation; real provider/issuer/activation gates remain open |
| [0037 — Private storage](0037-provider-independent-private-storage.md) | Proposed adoption; implemented bounded slice | Former duplicate storage 0022; real-provider/publication acceptance separate |
| [0038 — Delivery evidence gates](0038-publication-streaming-evidence-gates.md) | Proposed | Former duplicate publication 0028; real delivery/native evidence required |
| [0039 — VPS production runtime](0039-vps-production-runtime.md) | Accepted runtime; Web/public identity topology superseded by 0041 | Production host, Podman Quadlet, local PostgreSQL and durability foundation; original proxy rationale retained historically |
| [0040 — Firebase email-proof alternatives](0040-firebase-email-proof-alternatives.md) | Proposed | Default Firebase handler cannot prove mailbox possession; owner probes requested; no behavior change |

| [0041 — Direct Hosting/API boundary](0041-direct-hosting-api-boundary.md) | Accepted owner direction; #295 Web-cookie migration pending | Direct static Web → explicit API origin and exact surface CORS; Web cookies on custom domains selected, native Bearer retained; gateway/browser/device acceptance separate |
| [0043 — Cloudflare Pages Direct Upload](0043-cloudflare-pages-direct-upload.md) | Proposed (owner requested Pages); publisher implemented | Pages Direct Upload only, artifact-consuming `deploy cloudflare`; no Worker/Functions; real account/browser acceptance and cutover pending |
| [0046 — Service-owned contracts](0046-service-owned-contracts.md) | Accepted owner request; source migration implemented | Canonical schemas/protos/fixtures per service and module, consumer-owned Rust/Kotlin representations and per-consumer Protobuf codegen; supersedes 0009's `packages/contracts` and 0023's prost 0.13.5 pin; no wire/runtime acceptance changed |
| [0048 — Signup link returns to CMP](0048-signup-email-link-return-to-app.md) | Accepted direction (owner, #422); stage 0 implemented | Native-first completion through verified App/Universal Links over the existing native completion route; stage 0 copy fix shipped; stage 1 choices fixed (`/verify-email`, completion allowed while signed in, no debug association), while its association, handler move and CMP completion still need their own reviewed PR |

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

- [ADR0042: Native streaming adapters and account runtime](0042-native-streaming-runtime.md) — proposed platform acceptance; requested source implementation, native/codec gates remain open.

- [ADR0045: v1 email/password sign-in for Theatre and Studio](0045-v1-email-password-sign-in.md) — accepted #335 v1 scope; Studio passkey/recovery deferred.

- [ADR0047: Studio roster provisioning](0047-studio-roster-provisioning.md) — proposed, not implemented; Identity-owned offline subject lookup, Server-owned roster and an explicit `xtask run --studio-account`.

ADR 0041/#253's Web Bearer direction is superseded by #295 and [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md): Web cookie with exact-origin/CSRF; CMP/native retains Bearer. Provider-only ADRs 0016/0034/0036/0040/0044 are historical within that scope; current mailbox/password behavior follows ADR 0045. Provisioned-only Studio and no registration-issued role remain required.
