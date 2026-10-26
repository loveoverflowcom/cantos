# Scoped UI repair and delivery

Contents: [owner](#locate-the-smallest-owner), [verification](#verification-scope-and-progressive-delivery),
[escalation](#escalation), [handoff](#handoff-contract).

## Locate the smallest owner

Trace screen → mounted component → input state/presentation mapping → locale key/token source.
Name one invariant and failure mode. A correct API state with a wrong label/icon/tone belongs to
presentation; incorrect eligibility, error or transition belongs to the domain/application
owner. Do not hide a domain defect in the UI. Different symptoms may share one fix only with
evidence of their common cause.

Select references, not a copied checklist:

| Defect or seam | Method owner |
|---|---|
| renderer-local layout or CSS | [Web styling](../../cantos-leptos-web/references/styling.md) or [CMP design system](../../cantos-cmp-mobile/references/cmp-design-system.md) |
| canonical token or generated output drift | [shared tokens](../../cantos-ui-design/references/tokens.md), both renderer skills |
| incorrect visible/accessible label or Vietnamese overflow | [localization](../../cantos-ui-design/references/localization.md), [typography](../../cantos-ui-design/references/typography.md) |
| wrong mapping of a correct domain fact | [component states](../../cantos-ui-design/references/component-states.md), [functional core](../../cantos-engineering/references/functional-core.md), owning domain skill |
| focus, keyboard, semantics or motion | [accessibility](../../cantos-ui-design/references/accessibility.md), [motion](../../cantos-ui-design/references/motion.md) |
| verbose, redundant or hidden copy; tooltip, disclosure or long-press help | [contextual help](../../cantos-ui-design/references/contextual-help.md), then [localization](../../cantos-ui-design/references/localization.md) |
| stale response, lifecycle or duplicate state owner | [Leptos async](../../cantos-leptos-web/references/leptos-async.md) or CMP's required Kotlin/architecture/testing references |
| capture and same-scenario comparison | [visual review](../../cantos-ui-design/references/visual-review.md), [Web testing](../../cantos-leptos-web/references/web-testing.md) or [CMP testing](../../cantos-cmp-mobile/references/cmp-testing.md) |

Prefer the existing component and semantic role. Name all actual Web/CMP consumers and checks
for a shared change. Do not hand-edit generated CSS/Kotlin, invent visual literals or create a
second state owner. Renderer-local geometry exceptions remain the token owner's decision, with
a stated reason. Keep domain/business rules in their existing owner.

Write the plan as: invariant → cause evidence → owning path → minimal change → regression oracle
→ affected consumers. Avoid a new component, abstraction or dependency without demonstrated
need; propose wider work separately when this boundary cannot repair the defect.

## Verification scope and progressive delivery

Use L1–L5 as **independent claim scopes**, not five mandatory execution steps. Choose applicable
checks through the [foundation verification strategy](../../cantos-engineering/references/verification-strategy.md)
and renderer. Reuse its evidence vocabulary; layers add no proof level or relaxed gate.

| Layer | Scope to report |
|---|---|
| L1 — Architecture & Abstraction | correct owner, dependency boundary, existing consumer, minimal scope; cite source/review evidence |
| L2 — Domain, Types & Error Handling | domain facts preserved; exhaustive typed presentation mapping and exact states/errors when touched |
| L3 — Unit / Property / Regression Tests | smallest deterministic regression, preferably failing before the patch; applicable property/state-matrix tests |
| L4 — Application & Mocked Integration | real application/component flow, named doubles, DOM/Compose semantics and interactions when applicable |
| L5 — Real I/O, DB, API, Device / Runtime | real boundary or named renderer/device; opened captures, target/browser walkthroughs and required acceptance gaps |

For each layer list applicability, actual result, command/oracle, evidence level and limitation.
Report executed gates exactly as printed: `PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`. For irrelevant
scope, state why it is not applicable instead of inventing a passing command. Separate mocked
and real evidence; a desktop/static preview does not prove device behavior.

Run the cheapest failing behavior regression before the patch, then assert the exact mapping,
state or error. Apply [foundation exemptions](../../cantos-engineering/SKILL.md#5-every-behavior-change-gets-the-cheapest-deterministic-regression-evidence)
only with their stated evidence. Do not change expected outputs merely to bless the patch.
An actual regression failure is `FAIL`; fix it or disclose it. Missing runtime does not defer
a failing core or presentation test.

Pin the before/after scenario and source separately. Capture and open rendered pixels for each
cited result; record observations, not just a path or `inspected: true`. Keep fixture, viewport,
theme, locale, text scale and UI state the same. Disclose unavoidable scenario changes and their
limited comparability. A screenshot cannot prove interaction, motion, screen-reader operation,
authorization or durability.

Without a renderer/emulator, complete available source, mapping, regression, format/lint and
repository checks and deliver independently verified work when requested. Name visual/device
acceptance as pending with the exact recheck needed. Do not hold all work for an absent server
or device, nor call it a visual pass or production-ready. Use
`CORE_VERIFIED / IO_ACCEPTANCE_PENDING` only for applicable pure-core claims with actual passing
core invariants/tests; never apply it to database concurrency or an untested UI fix.

Schedule affected renderer/checkpoint checks and final handoff preflight through the foundation's
[cadence and reuse policy](../../cantos-engineering/references/verification-cadence.md).
Do not add a repository run before preflight or repeat the campaign for each edit/commit. Follow
[local handoff](../../cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion)
for unavailable required checks and the narrow docs-only fallback. Do not wait for remote CI.
The workflow grants no commit, push, upload, PR or other outward-action authority.

## Escalation

Stop at large architecture change, migration/breaking API, authorization/security model,
important publication/rights/durability semantics, unrelated cross-module defects, whole-screen
UX redesign or domain-model redesign. Report evidence, impact, proposed next boundary and owner
through [cantos-work-item](../../cantos-work-item/SKILL.md). Keep the verified small part and
unresolved risk explicit; do not refactor, redesign or upgrade dependencies unasked.

## Handoff contract

Emit one concise report. Populate every field; unknown or not observed is better than an
unsupported claim. Evidence includes the foundation's command, provenance and edge-coverage
fields; add relevant [UI fields](../../cantos-ui-design/SKILL.md#completion-report) there instead
of producing a second competing report.

```text
Issue / Input source:
Surface / Module:
Observed:
Expected:                  owning rule/contract
Root cause:                confirmed + evidence | hypothesis + confidence | unknown
Invariant:
Owner / boundary:
Scope:                     defect, affected consumers, exclusions/escalation
Files changed:
Verification:
  L1:                      applicability · result · evidence level · oracle/command
  L2:
  L3:
  L4:
  L5:
Evidence:                  exact commands/results; source/build/dirty state; fixtures
                           live/mocked dependencies; edge classes; opened image identity
                           screen/renderer; state/viewport/theme/locale/text scale
                           token/locale impact; accessibility/focus/motion observations
Deferred acceptance:       required target/scenario and blocker; no regression FAIL hidden here
Residual risks:            what remains unproven and next proportional action
Commit / PR:               requested publication result or not requested
```

Keep [foundation evidence](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
and [Web additions](../../cantos-leptos-web/SKILL.md#web-report-additions)
(or [CMP additions](../../cantos-cmp-mobile/SKILL.md#evidence-report-additions)) attributable.
An inspected historical screenshot is not HEAD verification; `compiled` is not visual evidence.
