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
| Dependencies, phase exit and GitHub workflow | [Project planning](project-planning.md) |
| Brief/private audio completion | [020](020-cast-and-generate-dialogue.md), [live client matrix](live-client-api-matrix.md) |
| Review/storage/publication acceptance | [030](030-mix-review-and-publish.md) |
| Theatre Web and native listening | [040](040-theatre-web-listening.md), [050](050-cmp-native-listening.md), [Library/Progress handoff](listener-library-progress-handoff.md) |
| Public catalog, cover and metadata | [Catalog states](theatre-catalog-state-handoff.md), [cover handoff](public-cover-implementation-handoff.md), [P1 decisions](public-metadata-decision-handoff.md) |
| Identity activation and onboarding | [Identity handoff](theatre-identity-handoff.md); #253 API → Web → CMP Bearer migration, [contract and cutover](../../packages/contracts/bearer-v1.md). Preserve API → Web → CMP order and independently verify each layer. |
| Direct Hosting and VPS API (ADR 0041) | [Current topology/runbook](../deployment/vps-production.md#selected-custom-domains); owner selected `cantos.cc`, `studio.cantos.cc` and `api.cantos.cc`; #253 removes same-site domains as an auth prerequisite. Preserve legacy ingress while qualifying DNS/TLS, apply exact origins/callbacks through the configuration-only reload lane, then qualify the default-deny identity gateway and permitted real account/session journeys |
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
