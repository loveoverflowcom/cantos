# Story or stage index

Copy into a story or stage directory as `index.md`; fill from observed artifacts. Omit fields
that do not apply to the scope rather than inventing evidence.

This is the agent-operated ordinary Drive/local-mirror workflow, not a Forge app-data schema.
[ADR 0022](../docs/decisions/0022-cantos-forge-producer-cli.md) separately selects `appDataFolder`
for Forge remote drafts, including audio; no migration or implemented Drive mapping follows.
App-data has no share links. Keep this manual index outside Git and pin repository references
when copying it; use the [workspace contract](../docs/production/story-workspace.md) for its fields.

- Workflow: Cantos story workspace v1
- Scope: project | story | raw | score | audio
- Story ID / original title / author:
- Canonical work URL and confirmed source aliases:
- Ordinary Drive folder ID and verified URL, or unresolved/local-only with reason:
- Updated at (UTC) / actor:
- Content stage / sync state:
- Requested coverage / acquired or adapted coverage / gaps:
- Current raw revision / score revision / audio run:
- Name map revision / SHA-256 / inventory coverage and unresolved names:
- Rights evidence reference and limits:
- Pending operation IDs / recovery issue URL or `pending_issue`:
- Next action and responsible skill:

## Artifacts

| Relative path | SHA-256 | Input path/revision + SHA-256 | State | Drive file ID | Operation ID |
| --- | --- | --- | --- | --- | --- |

## Checks and blockers

| Artifact checksum | Check / evidence level | Outcome | Gap / recovery action |
| --- | --- | --- | --- |

## Update log

| UTC | Actor / skill | Changed artifact or pointer | Evidence / operation |
| --- | --- | --- | --- |

Only an observed remote readback can establish synchronization. Completed external production,
an audit pass, API admission and publication approval/release are different facts. Follow the
[workspace contract](../docs/production/story-workspace.md).
