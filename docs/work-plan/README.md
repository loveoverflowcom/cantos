# Work plan

Read [current priorities](current-priorities.md) for the short dependency queue. GitHub owns
assignment, execution and acceptance; these documents own scope, rationale and reading paths.
The [native checkpoint](../architecture/server-turso-porting.md#checkpoint-boundary) overrides
provider/PG-era implementation assumptions in historical product slices.

## How to use the queue

Resolve the user's integration base, fetch/pin it and inspect current source/PRs before selecting
one reviewable outcome. Reuse the implemented native owner/RPC/session/client foundation.
Do not reintroduce a retired provider, PG/DBSP gate or second framework to satisfy a stale task.
Keep original acceptance gaps: model/core/file evidence never proves full HTTP/browser/device,
restore, Kani, footprint or deployed behavior.

| Need | Owner / reading path |
| --- | --- |
| Native business parity / qualification | [#333](https://github.com/loveoverflowcom/cantos/issues/333), [porting guide](../architecture/server-turso-porting.md), [readiness](../architecture/native-persistence-readiness.md) |
| Identity and sessions | [Identity v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md), [RPC](../../services/cantos-identity/contracts/identity/v1/README.md), #334/#335; Web cookie, native Bearer |
| Native acceptance / restore / migration / footprint records | [#336 historical campaign](https://github.com/loveoverflowcom/cantos/issues/336) (closed; evidence stays source-bound), [isolation](../assurance/native-isolation.md), [Kani](../assurance/kani.md) |
| Read optimization | [#384](https://github.com/loveoverflowcom/cantos/issues/384), [Turso method](../../.agents/skills/cantos-engineering/references/turso-read-performance.md) |
| OCI packaging and activation | [#348](../deployment/native-oci-release.md); historical VPS assets establish no native activation |
| Phase boundaries and planning | [Project planning](project-planning.md) |
| Script/editor | [Script IR](../architecture/script-ir.md), [editor acceptance](../assurance/script-editor-acceptance.md) |
| Production briefs/private audio | [020](020-cast-and-generate-dialogue.md), [client matrix](live-client-api-matrix.md) |
| Review/publication | [030](030-mix-review-and-publish.md) |
| Theatre Web/CMP listening | [040](040-theatre-web-listening.md), [050](050-cmp-native-listening.md), [Library handoff](listener-library-progress-handoff.md) |
| Catalog/cover/metadata | [Catalog states](theatre-catalog-state-handoff.md), [cover](public-cover-implementation-handoff.md), [metadata](public-metadata-decision-handoff.md) |
| Runtime image transition | [#607](https://github.com/loveoverflowcom/cantos/issues/607), [image policy and dependency order](../design/runtime-images.md); AVIF-only source admission is separate from historical data conversion, repinned sample inputs and native/deployed qualification |
| Shared UI | [Leptos](015-leptos-ui-system.md), [CMP](016-cmp-ui-system.md) |
| Forge | [Worker boundary](../deployment/forge-worker.md), [attempt evidence](../evidence/forge-attempt-reconciliation.md); #203 |
| Delivery | [Firebase Hosting](../deployment/firebase-hosting.md), [Pages](../deployment/cloudflare-pages.md), [rollout](../deployment/cloudflare-rollout.md) |
| Deferred expansion | [Backlog](backlog/deferred-expansion.md) |

Completed/superseded session instructions are history, not current plans. Preserve unique
source-bound evidence through its pinned provenance; delete redundant guidance and repair
incoming links. Do not create a second issue tracker here or refresh historical PASS into a
native qualification. [Templates](../../templates/README.md) remain for concrete consumers.
