# Documentation map

Read the [product brief](product/brief.md), [architecture overview](architecture/overview.md)
and the relevant owning contract before implementing. The [current queue](work-plan/README.md)
contains next steps; GitHub issues and PRs own live execution status.

## Authority and lifecycle

| Material | How to use it |
| --- | --- |
| Product, architecture, design and production guidance | Current requirements within the stated scope; a proposed section still needs its own decision |
| [ADR index](decisions/README.md) | Decision status, scope and supersession; accepted policy, implemented code and verified behavior are separate facts |
| [Versioned contracts](../packages/contracts/README.md) and actual source | Exact API/schema compatibility and implementation; do not infer routes from a proposal |
| [Executable behavior](../specs/behavior/README.md) | Catalog lifecycle and accepted model expectations; draft/model-only PASS does not prove application conformance |
| [Current priorities](work-plan/current-priorities.md) | Recommended order, dependencies and unresolved decision gates; never a second issue tracker |
| [Evidence](evidence/README.md) | Results tied to named source/tool/environment; old PASS or NOT_RUN is not the status of today's code |
| [Archive](archive/README.md) | Historical plans, session instructions and handoffs; traceability only, not implementation authority |
| [Agent skills](../.agents/skills/README.md) | Working method and verification cadence; product rules stay in their owning document |

If an active document conflicts with code or a newer accepted decision, identify and repair the
stale statement. Do not silently accept a proposal, revive an archived task, or infer production
acceptance from a merged PR.

## Current reading paths

For #348's native production packaging, read the [OCI release contract and next slices](deployment/native-oci-release.md),
then the [native architecture](architecture/native-turso-refactor.md) and [retained VPS runbook](deployment/vps-production.md).
The offline contract verifier is separate from publish, native qualification and live activation.

| Goal | Start here |
| --- | --- |
| Product scope and invariants | [Brief](product/brief.md), [business rules](product/business-rules.md), [production pipeline](product/production-pipeline.md) |
| Runtime boundaries and repository ownership | [Overview](architecture/overview.md), [API contexts](architecture/api-contexts.md), [layout and platform matrix](architecture/repository-layout.md), [services](../services/README.md), [infra assets](../infra/README.md) |
| PostgreSQL fresh schema and controlled cutover | [API guide](../services/cantos-server/README.md#current-schema-and-explicit-cutover), [cutover runbook](deployment/postgres-schema-cutover.md) |
| Catalog, content, briefs and private audio | [Contract index](../packages/contracts/README.md), [API service guide](../services/cantos-server/README.md), [live client matrix](work-plan/live-client-api-matrix.md) |
| Private storage and owner artwork | [Object storage](architecture/object-storage.md), [artwork contract](../packages/contracts/artwork-v1.md); public cover remains gated by [ADR 0033](decisions/0033-release-pinned-public-cover.md) |
| Publication and listener delivery | [Publication contract](../packages/contracts/publication-v1.md), [delivery evidence gates](decisions/0038-publication-streaming-evidence-gates.md), [origin-mode proposal](decisions/0032-theatre-origin-playback-delivery.md), [catalog state handoff](work-plan/theatre-catalog-state-handoff.md) |
| Identity and account activation | [Registry](architecture/identity-registry.md), [Studio OIDC](architecture/studio-oidc.md), [Theatre identity](architecture/theatre-identity.md), [Admin](architecture/admin-authorization.md), [current identity handoff](work-plan/theatre-identity-handoff.md), [Firebase proof decision](decisions/0036-firebase-enrollment-proof.md) and [operator runbook](deployment/firebase-enrollment-proof.md) and [service-account design/runbook](deployment/firebase-enrollment-service-account.md) |
| Producer tools and private drafts | [Forge ADR](decisions/0022-cantos-forge-producer-cli.md), [Forge implemented commands](../services/cantos-forge-functions/README.md), [worker acceptance](deployment/forge-worker.md), [story workspace](production/story-workspace.md) |
| Script/editor and production craft | [Script IR](architecture/script-ir.md), [editor acceptance](assurance/script-editor-acceptance.md), [radio score](production/radio-score.md), [craft](production/audio-drama-craft.md), [listening protocol](production/listening-review-protocol.md) |
| Web/CMP UI | [UI system](design/ui-system.md), [shared preview catalog](../packages/ui-system/README.md), [mobile architecture](architecture/mobile.md), [UI migration](architecture/ui-system-migration.md) |
| Wire transport | [Wire codec](architecture/wire-codec.md), [Protobuf policy](../packages/contracts/protobuf/README.md); JSON default and existing MessagePack compatibility remain scoped |
| Local checks and deployment | [xtask](../xtask/README.md), [seeded Theatre catalog runbook](deployment/theatre-seeded-catalog.md), [current production topology/runbook](deployment/vps-production.md), [Hosting tooling](deployment/firebase-hosting.md), [Cloudflare Pages publisher](deployment/cloudflare-pages.md), [CircleCI artifact builds](deployment/circleci-builds.md), [Kanidm operator guide](../services/kanidm/README.md), [native callback profiles](deployment/native-callback-profiles.md) |
| Cloudflare Web transition | [Rollout/recovery and offline report](deployment/cloudflare-rollout.md); #277/#278 own promotion/publishing prerequisites, #280 owns live acceptance and Hosting-only retirement. Source preparation is independent of operator execution. |

## Keeping documents current

- Update the owner rather than appending a parallel summary to the queue. Keep the queue short;
  retain only unfinished review boundaries, linked to their issue and source document.
- Label a proposal, implementation record, acceptance gap or historical snapshot at the top.
  Dates alone do not define status; completion does not automatically accept production policy.
- On a completed or superseded handoff, preserve useful reasoning in the owner and move the
  session record to the archive with a current pointer. Keep evidence and fixtures at their
  existing paths; correct links without rewriting their historical outcomes.
- Record supersession in both ADRs and the index. Allocate a free ID after checking open PRs;
  never reuse an ID or reinterpret an unqualified old reference without its title/context.
- Fix repository links when moving a document. Keep commit-pinned historical links intact.
  Record checks in the PR; avoid creating another per-session evidence file for wording alone.

Use [templates](../templates/README.md) for decisions and production records.
