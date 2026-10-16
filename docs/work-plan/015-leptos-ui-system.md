# Leptos UI preview and consumer follow-ups

Active review inputs for [#141](https://github.com/loveoverflowcom/cantos/issues/141) and
[#142](https://github.com/loveoverflowcom/cantos/issues/142). The original four-session plan
and results are [archived](../archive/superseded-plans/015-leptos-ui-system.md); those session
permissions, branches and command sequences are not current instructions.

## Why and boundary

Shared tokens, components, preview workbench/scenarios and Studio/Theatre consumers already
exist. Inspect the [renderer API](../../packages/ui-system/leptos/README.md), [shared admission
contract](../../packages/ui-system/shared/README.md#preview-registration-and-adoption) and
[UI requirements](../design/ui-system.md) before extending them. Keep one canonical token
source, Material 3 Expressive roles, explicit theme/motion injection and component-first adoption.

## Recommended next boundaries

1. Review the current registry/qualification freshness and finish one missing component/state
   matrix from #141; update its preview before app adoption.
2. Finish #142's actual browser scenario/deep-link/matrix and animation/gesture acceptance.
   A deterministic clock, screenshot or string-render test does not prove real motion/focus.
3. Review a bounded Studio/Theatre consumer at representative widths, both themes and 200% text,
   with long Vietnamese copy, IME/unsaved/conflict state, keyboard/dialog focus and reduced motion.
4. Keep [#146](https://github.com/loveoverflowcom/cantos/issues/146)'s performance measurements and
   [#167](https://github.com/loveoverflowcom/cantos/issues/167)'s neutral localization decision
   separate. #168's typed locale/library comparison slice is completed; do not pick another
   library merely because the old plan predates it.

## Review and evidence

Use the existing preview/consumer runbooks and repository verification cadence. Keep source
qualification, browser interaction, accessibility and production acceptance separate. Findings
must point to current code and named rendered scenarios; pixel metrics are advisory. Put run
transcripts in their evidence/PR, not this plan.
