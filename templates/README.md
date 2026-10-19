# Reusable project templates

Choose a template for its actual workflow and owning contract, then copy it to the appropriate
location below. Keep template sources generic. Replace placeholders with observed facts, not
assumed success; a blank template is not evidence or an application record.

| Template | Use |
| --- | --- |
| [Architecture decision](adr.md) | Record a consequential technical or product decision, its alternatives, and consequences. |
| [Feature brief](feature-brief.md) | Define a bounded creator/listener outcome, scope, dependencies, and acceptance criteria. |
| [Production run](production-run.md) | Record pinned inputs, external producer/run/attempt provenance, immutable output bytes, costs, QC and explicit API handoff for narration, recording or drama. |
| [Listening review](listening-review.md) | Record two-pass human listening on headphones and a phone speaker, exact render/input identity, dimension results and editorial findings under the shared protocol. |
| [Publication checklist](publication-checklist.md) | Verify permissions, QC, approval, delivery, and recovery before making an episode revision public. |
| [Hosting rollout record](hosting-rollout-record.json) | Private operator-declared evidence for `cargo xtask hosting-report`; start with every stage NOT_RUN. Use the [rollout runbook](../docs/deployment/cloudflare-rollout.md#offline-report-contract); a valid record never proves live acceptance. |
| [UI component qualification](ui-component-qualification.json) | Start an unqualified record for one registered component/renderer. The [adoption contract](../packages/ui-system/shared/README.md#preview-registration-and-adoption) requires fresh structural, runtime, accessibility and visual evidence for both renderers; NOT_RUN is not qualification. |
| [Story index](story-index.md) | Record artifact hashes, stage handoffs, name-map revisions and ordinary Drive/local-mirror synchronization outside Git. |
| [Story name map](story-name-map.yml) | Manual score-workflow inventory mapping original proper names and aliases to consistent new names; snapshot and checksum each reviewed revision. |
| [Story recovery issue](story-recovery-issue.md) | Track a blocked manual-workspace capture/sync operation using safe diagnostics, never manuscript content. |

## Where completed records belong

- ADRs and feature briefs may live in the relevant repository documentation after review for
  private content. Link safe completed records from the corresponding issue or PR.
- Story indexes, name maps, production/listening records and publication records containing
  private material stay outside Git under the [story workspace contract](../docs/production/story-workspace.md).
  Do not attach manuscripts, audio or private evidence to GitHub to complete a checklist.
- Hosting/operator records and underlying sanitized evidence belong in a private ignored run
  directory, such as `artifacts/agent-runs/ROLLOUT/`, as the
  [rollout runbook](../docs/deployment/cloudflare-rollout.md#offline-report-contract) requires.
- UI qualification JSON is referenced by `qualifications` in
  `packages/ui-system/shared/preview-catalog.json`. Follow the adoption contract for one record
  per renderer, current token/source digests and existing repository-local check artifacts.
  The skeleton's `artifacts/ui-evidence/` paths are placeholders, not supplied evidence; keep
  private captures/logs ignored and make required sanitized artifacts available to the gate's
  checkout without publishing private data. Record validation does not perform the named checks.

When copying outside the repository, replace relative instruction links with repository URLs
pinned to the source revision, and retain the template path/revision. Do not copy credentials,
tokens, cookies, signed URLs or other private material into Git/GitHub.

## Workflow and evidence boundaries

The agent-operated story workflow may use an ordinary Drive folder or local mirror.
[Forge ADR 0022](../docs/decisions/0022-cantos-forge-producer-cli.md) separately selects private
`appDataFolder` for remote drafts, including audio; it does not migrate those manual files or
implement Drive I/O. Its [implemented commands](../services/cantos-forge-functions/README.md)
and [worker acceptance boundaries](../docs/deployment/forge-worker.md) remain distinct from
product direction. App-data records need no share URL. Sync, technical QC, human listening,
explicit API admission and publication are separate facts.

Suggested document names are `NNNN-short-decision.md`, `feature-short-name.md`, `production-episode-revision.md`, and `publication-episode-revision.md`. Choose a stable naming scheme with the project team; these are suggestions, not identifiers assigned by the system.

Statuses, owners, approval requirements, cost budgets and audio profiles follow their owning
contracts, not a common template enum. Keep NOT_RUN/unknown placeholders until evidence exists;
use N/A with a reason where the workflow permits it, never to hide an unrun check. Preserve both
JSON input shapes: schema-valid hosting metadata is not rollout acceptance, and a parseable UI
skeleton is not qualified. Link actual measurements, reviewed bytes, decisions and approvals.
