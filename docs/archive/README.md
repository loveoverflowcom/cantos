# Historical plans and handoffs

Archived on 2026-10-16 from develop `178def7c`. These records preserve original proposals,
handoff reasoning, commands, SHAs, session permissions and verification outcomes. They are
**not active implementation instructions**. Read the [documentation map](../README.md),
[current priorities](../work-plan/current-priorities.md), owning contract/ADR and current issue.

Completed core work may still have live acceptance gaps. Archiving a handoff neither accepts
those gates nor closes an issue. The original bodies are retained with lifecycle banners and
updated repository links. Evidence, JSON fixtures, images and SQL remain at their original paths.

## Handoffs

| Historical record | Current owner / successor |
| --- | --- |
| [Catalog API](handoffs/011-studio-catalog-api.md) | [Catalog contract](../../services/cantos-server/contracts/studio/catalog/v1/README.md); #66 completed, remaining QA in #131/#172/#179/#180 |
| [Private-cover planning session](handoffs/catalog-media-mvp-ultra-handoff.md) | [Artwork contract](../../services/cantos-server/contracts/studio/artwork/v1/README.md), [implementation evidence](../evidence/private-work-artwork.md), [Studio UI](../evidence/studio-private-artwork-ui.md); #156 completed |
| [Content → Brief](handoffs/content-identity-production-brief-handoff.md) | [Content](../../services/cantos-server/contracts/studio/content/v1/README.md) and [Brief](../../services/cantos-server/contracts/studio/briefs/v1/README.md) contracts |
| [Brief implementation](handoffs/production-brief-implementation.md) | [Brief contract](../../services/cantos-server/contracts/studio/briefs/v1/README.md) |
| [Brief → Audio Lifecycle](handoffs/production-brief-audio-lifecycle-handoff.md) | [Audio Lifecycle contract](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md) |
| [Audio Lifecycle → frontend](handoffs/audio-lifecycle-frontend-handoff.md) | [Live client matrix](../work-plan/live-client-api-matrix.md), #8/#179/#180 |
| [Publication → Discovery](handoffs/publication-discovery-handoff.md) | [Publication](../../services/cantos-server/contracts/studio/publication/v1/README.md), [Discovery](../../services/cantos-server/contracts/theatre/discovery/v1/README.md), #170 real delivery |
| [Discovery → Library](handoffs/discovery-library-handoff.md) | [Library/Progress contract](../../services/cantos-server/contracts/theatre/library/v1/README.md) and [current handoff](../work-plan/listener-library-progress-handoff.md) |
| [Identity registry → OIDC](handoffs/identity-authentication-handoff.md) | [Identity architecture](../architecture/identity-registry.md), [current identity handoff](../work-plan/theatre-identity-handoff.md) |
| [Admin integration](handoffs/admin-authorization-handoff.md) | [Admin authority](../architecture/admin-authorization.md), #179 real acceptance |
| [Theatre identity session log](handoffs/theatre-identity-2026-10-16.md) | [Current identity handoff](../work-plan/theatre-identity-handoff.md), #174/#191/#206 |
| [Quint conformance sessions](handoffs/quint-conformance-handoff.md) | [Behavior catalog](../../specs/behavior/README.md), #106 wider scope |
| [Delegation research/implementation](handoffs/work-scoped-delegation-handoff.md) | [Accepted ADR 0028](../decisions/0028-work-scoped-delegation.md), [contract](../../services/cantos-server/contracts/studio/delegation/v1/README.md); #113 completed |

## Superseded plans

| Historical record | Current owner / successor |
| --- | --- |
| [Original 010 script/editor plan](superseded-plans/010-import-and-edit-script.md) | [Current acceptance reference](../assurance/script-editor-acceptance.md) and [Script IR](../architecture/script-ir.md) |
| [Private artwork API/DB plan](superseded-plans/012-private-work-artwork.md) | [Artwork contract](../../services/cantos-server/contracts/studio/artwork/v1/README.md); retained field/fixture rationale, not a new implementation task |
| [Catalog/media standards research](superseded-plans/catalog-media-standards-reference.md) | Dated #104 research; no RSS runtime or P1 metadata acceptance |
| [Previous 743-line queue](superseded-plans/work-queue-2026-10-16.md) | [Current queue](../work-plan/README.md) and [priorities](../work-plan/current-priorities.md) |
| [12 October dependency plan](superseded-plans/project-planning-2026-10-12.md) | [Current dependency/phase-exit guide](../work-plan/project-planning.md) |
| [Leptos multi-session plan](superseded-plans/015-leptos-ui-system.md) | [Current preview follow-ups](../work-plan/015-leptos-ui-system.md) |
| [CMP multi-session plan](superseded-plans/016-cmp-ui-system.md) | [Current preview follow-ups](../work-plan/016-cmp-ui-system.md) |

## References after the cleanup

The [ADR index](../decisions/README.md#id-allocation-and-historical-references) maps the old
duplicate storage/publication filenames to 0037/0038. Source-pinned historical links and quoted
path lists retain their original meaning; follow this map when an unqualified old ID is ambiguous.

When archiving another record, preserve useful source/evidence, add a historical banner and
current pointer, update incoming links and keep session instructions out of the active queue.
Do not bulk-move evidence or rewrite its outcomes to match today's status.
