# Current work queue

Scope: Web-only Studio, Theatre Web/CMP and producer-side Forge outside the Cantos API runtime.
Read the [documentation map](../README.md) and [current priorities](current-priorities.md) first.
GitHub issues/Projects own execution status; this folder owns recommended order and rationale.

## How to use the queue

Choose one independently reviewable outcome from [current priorities](current-priorities.md),
re-read its issue and current source, then update the owning contract/architecture document.
A merged core is not live acceptance. Completed Catalog work (#66) is a foundation to reuse,
not the next implementation task.

Numeric prefixes on work items are ordering markers, not permanent IDs; they may be renumbered.
The retained 020–050 files describe unfinished capability acceptance, not an instruction to
rebuild their implemented portions. Keep speculative expansion in [backlog](backlog/deferred-expansion.md).

## Active review inputs

| Boundary | Owning input |
| --- | --- |
| Editorial application/document separation (#258) | Review the [facts and projection boundary](../architecture/api-contexts.md#editorial-application-and-document-boundaries) against existing revision/adaptation and immutable Brief oracles; retain byte, permission, replay/CAS and transaction acceptance independently of the #97 transport pilot. |
| Android release/distribution (#288) | Review the [same-release audit and APK policy](../../apps/cantos-theatre/mobile/docs/firebase-distribution.md) on integrated #285/#286/#287 source; require separate optimized-device, callback, signing/registration and explicit tester acceptance. |
| Release artifact promotion (#277) | [CircleCI release boundary](../deployment/circleci-builds.md); review manifest v2/source/config provenance, same-workflow transfer/approval, artifact consumers and per-component recovery. Account restrictions, actual API/Web/unsigned Android builds and live activation remain separate acceptance before #278/#280. |
| Cloudflare release transition (#277–#280) | Review exact archive-byte binding through extraction and initial-response security headers, alongside artifact promotion and provider/publisher prerequisites before live cutover; [rollout/recovery runbook](../deployment/cloudflare-rollout.md), the #278 Pages publisher ([source-bound preview observations](../deployment/cloudflare-pages.md#acceptance-status)), standalone [read-only HTTP comparison](../../xtask/README.md#read-only-hosting-http-observation) and offline declared-evidence reporting can ship independently. Operator scope/budget, real build/publish/browser/rollback evidence and Hosting-only retirement remain separate acceptance. |
| Dependencies, phase exit and GitHub workflow | [Project planning](project-planning.md) |
| Brief/private audio completion | [020](020-cast-and-generate-dialogue.md), [live client matrix](live-client-api-matrix.md) |
| Review/storage/publication acceptance | [030](030-mix-review-and-publish.md) |
| Theatre Web auth and Library adoption (#271, #272) | Review independent login/registration readiness, [presented Web lists and authenticated Save/progress/history/resume sync](../../apps/cantos-theatre/web/README.md#scope-and-honest-behavior); qualify real issuer/emulator/PostgreSQL multi-device acceptance separately from unit/loopback evidence. |
| Theatre Web and native listening | [040](040-theatre-web-listening.md), [050](050-cmp-native-listening.md), [Library/Progress handoff](listener-library-progress-handoff.md) |
| Public catalog, cover and metadata | [Catalog states](theatre-catalog-state-handoff.md), [cover handoff](public-cover-implementation-handoff.md), [P1 decisions](public-metadata-decision-handoff.md) |
| Identity activation and onboarding | [Identity handoff](theatre-identity-handoff.md); #295 selects Web HttpOnly/Secure cookies on the custom domains, superseding only #253's Web Bearer target. Review the coordinated API/session/resource migration and exact-origin CORS/CSRF through [ADR 0041](../decisions/0041-direct-hosting-api-boundary.md) and the [shared Web owner](../../packages/web-auth/README.md); current Web source still uses Bearer. Native keeps the [Bearer contract](../../packages/contracts/bearer-v1.md), [secure journal/lifecycle](../../apps/cantos-theatre/mobile/README.md#opt-in-native-identity) and #285 [closed callback profiles](../deployment/native-callback-profiles.md). Qualify issuer/browser/device and co-install acceptance separately; source completion does not accept live rollout. |
| Direct Hosting and VPS API (ADR 0041) | [Current topology/runbook](../deployment/vps-production.md#selected-custom-domains); owner selected `cantos.cc`, `studio.cantos.cc` and `api.cantos.cc`; [legacy Firebase alias redirects](../deployment/firebase-hosting.md#canonical-browser-domains) are scoped to the Web entrypoints. #295 selects Web cookies on this topology, with API/client migration and browser acceptance pending; native retains Bearer. Preserve legacy identity ingress while qualifying the implemented default-deny gateway source with actual peer CIDRs/issuer/browser traces and permitted real account/session journeys |
| Mechanical infrastructure ownership (#212) | [Repository layout](../architecture/repository-layout.md), [infra assets](../../infra/README.md); separate from #203 Forge/Functions runtime work and live deployment acceptance |
| Forge paid-attempt safety (#203) | [Attempt core evidence](../evidence/forge-attempt-verification.md), [race/readback follow-up](../evidence/forge-attempt-reconciliation.md), [worker boundary](../deployment/forge-worker.md); establish invocation authority and cross-runtime fencing before enabling a real provider |
| Optional DBSP platform | [Current review, capture retirement and bounded core hardening](../evidence/dbsp-platform-review.md); review migration compatibility, terminal engine errors and snapshot/freshness evidence before proposing a measured non-synthesis consumer |
| Shared previews and localization | [Leptos](015-leptos-ui-system.md), [CMP](016-cmp-ui-system.md) |
| Optional craft and membership | [Audio craft](audio-drama-quality.md), [interest receipt](membership-interest-handoff.md), [plan publication proposal](../product/membership-plan-publication.md) |

The [archive index](../archive/README.md) preserves the previous 743-line queue and completed
handoffs. Historical commands, worktree paths, session permissions, SHAs and PASS/NOT_RUN results
belong there or in their original evidence, not in this queue.

At handoff, follow the repository's verification cadence and report final-diff evidence in the
PR. Do not append test transcripts or session logs here.
