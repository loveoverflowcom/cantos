# ADR 0029: Local PNG comparison for advisory visual QA

- Status: proposed (implemented local MVP for review; no production or baseline acceptance)
- Date: 2026-10-15
- Decision owner: unassigned — design/tooling owner review required
- Related issue or PR: [#140](https://github.com/loveoverflowcom/cantos/issues/140)
- Supersedes / superseded by: N/A

## Context

Existing Web and CMP captures need reproducible pairing and region coordinates for reviewers.
Neither a screenshot nor pixel similarity establishes UX, accessibility, interaction or native
acceptance. The [calibration experiment](../evidence/visual-compare.md) compared the issue's four
options on original HTML captures and a real CMP JVM capture with a controlled displacement.

## Decision

Use optional, pinned Pillow 11.3.0 for the **local MVP's PNG adapter**, with standard-library
pixel accounting and occupied-tile grouping. Expose it only through `cargo xtask visual-compare`.
Preserve exact input bytes and provenance, default to exact premultiplied RGBA deltas, require
review records for masks, reject size/scenario mismatches and never change design references.
This is a proposed tooling choice, not approval of any product baseline or merge policy.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| looks-same 10.0.1 | Existing perceptual comparison and clusters | Node tool lock; implicit AA/caret policies need explicit review; counts vary with perception rules | Tried first; retained for optional calibration |
| pixelmatch 8.0.0 + pngjs 7.0.0 | Small JS comparison and mask | Needs region/report code; exploratory PNG path slower on this host | Retained in spike, no MVP dependency |
| odiff-bin 4.5.0 | Fast native backend | Platform-specific binary; line/column bounds do not identify clusters; threshold markedly changes low-contrast displacement counts | Retained in spike; other OS binaries untested |
| Pillow 11.3.0 | Fast measured mask backend; fits Python primitives; explicit alpha/max-channel semantics | Optional QA install; own grouping; threshold is not perceptual equivalence | Selected for the bounded local prototype |

## Consequences

- Product and user impact: review coordinates and local images only; production app behavior unchanged.
- Compatibility and migrations: additive local command/report v1; no database, public API or app dependency.
- Operations, storage, and production cost: offline comparison, ignored artifacts, no upload/provider/CI calls.
- Implementation and recovery requirements: fresh output directories, explicit optional setup, input/configuration hashes, focused core/PNG tests; a failure cannot reuse a prior successful report.
- Deferred questions: approved per-platform baselines, representative cross-host font calibration, optional SSIM, capture automation and any component gate.

## Validation and revisit conditions

Retain exact rectangle/count, alpha, masks, empty diff, dimension/scenario mismatch, metadata pin
and recheck-policy tests. Open generated overlay/mask/crops and trace advisory findings through
existing skills. Revisit for larger images, a dependency upgrade, unacceptable clustering, real
cross-platform false positives, or an owner-proposed component-specific gate. Do not select a
90/95% merge threshold from this short experiment.
