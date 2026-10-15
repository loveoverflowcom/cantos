# CMP UI-system sequence

The requester authorized three separate, sequential Mac mini sessions after PR39/40/41 merged:
Ultra → Extra High → Ultra. The parent starts the next session after the current handoff. This
queue owns order and rationale; GitHub owns later publication status. No push, PR or merge is
authorized by this local sequence.

## Boundary

Session 1 started from freshly fetched clean develop
`7f416a1e1bac5bbf338d461463770bffd983b5c1`. Read the
[preview evidence](../evidence/ui-system-previews.md), actual APIs and
[admission contract](../../packages/ui-system/shared/README.md#preview-registration-and-adoption).
Keep one canonical token source/generator, the 29-component registry, evidence freshness and
existing Leptos import compatibility. The concrete consumer is the package-local desktop preview;
mobile apps remain guides. App journeys, backend, TTS and playback are outside this sequence.

## Recommended order

| Session | Cohesive outcome | Acceptance boundary |
| --- | --- | --- |
| 1 — Ultra | Scoped theme/density/native text foundation, reusable state contract separated from diagnostics, composable actions and both preview specimens | [Session 1 evidence](../evidence/cmp-ui-system-session1.md): examples/JVM semantics/opened captures and bounded Safari observations; unresolved native gates explicit |
| 2 — Extra High | Interaction, navigation, layout, feedback and complex-input contracts below | Both previews for every changed API/state; callbacks/semantics, interrupted/repeated interactions, inspected captures and independent review |
| 3 — Ultra | Independent final review and acceptance matrix for current registry scenarios/tokens/targets | [Final readiness handoff](../evidence/cmp-ui-system-session3.md): reproduced/fixed defects and fresh matrix; incomplete runtime/reference/assistive-technology gates leave 0/29 qualified |

Session 1 implements theme scaling/validation, action control modifiers, a named Button content
slot, inherited icon foreground, normalized blank messages and rich editing/keyboard hooks while
retaining String overloads. Its scenarios bring the registry from 118 to 129; derive later matrix
counts from the current source. There are still 29 candidates and zero production qualifications.

## Session 2 contract slice

Continue local `feat/cmp-foundation-parity` from the session-1 commit. Inspect status and freshly
fetched develop; read its evidence before edits. Keep public package `com.cantos.ui` and add no
module/dependency without a concrete consumer.

1. Finish Checkbox/Radio/Switch label interaction: one reachable named native target, full label
   activation, controlled callbacks and toggle/radio semantics. Check reasons and interactive bounds
   at both densities; desktop compact targets do not establish mobile sizes.
2. Review Select stable identity, empty options and interruption; RangeField steps, finite values,
   localized range/value descriptions and completion. Preserve raw numeric text/caller validation.
3. Make navigation differences explicit: Leptos NavTabs uses links; CMP uses in-place Material tabs.
   Review group name, selected/current meaning, keyboard/overflow, Link URI dispatch and Dialog
   Escape/containment/return focus/retained input.
4. Complete layout/feedback parity for real preview consumers: wrapping horizontal Stack,
   caller-owned gap/insets/reading surfaces, grid/reflow, announcement policy and recovery slots.
   Keep quiet tonal grouping and spacing.
5. Improve state/status discovery after component contracts. Defer product navigation, native
   adapters, font-resource delivery and speculative widgets; keep the session reviewable.

The [session 2 evidence](../evidence/cmp-ui-system-session2.md) records full-label targets,
stable-value options and interruption, bounded native range ticks/keys/completion, named adaptive
native tabs, controlled Dialog focus/dismissal, semantic layout spacing and explicit announcements/
recovery slots. Both previews now declare 151 scenarios across the same 29 APIs; qualification
remains zero. Its ignored handoff/matrix under `target/cmp-session2-evidence/` distinguishes
component tests, opened captures and bounded Safari observations from missing acceptance.

## Session 3 review scope

Continue the same local branch from the session-2 handoff commit. Verify status and retained
session-1 ancestry before editing; no reset, push, PR, merge or automatic next workchat is authorized.
Read both session evidence records and `target/cmp-session2-evidence/handoff.json` plus
`remaining-matrix.json`; refresh source/token digests before using a record. Prioritize:

1. Independent review of current public contracts/callers, especially bounded Float steps and
   native callbacks, dynamic option changes, blocked interaction and Dialog focus lifetime.
2. Finish per-component/state evidence inventory for both previews, then named tokens/aliases/
   theme/density and actual reference comparison. Registration is not an acceptance result.
3. Resume supported native OS-window evidence if binding works; retain `Invalid app` as a blocker
   otherwise. Audit real IME, assistive technology, motion, Noto metrics and responsive text without
   altering OS/security settings or inventing a mobile target.
4. Repair actionable defects at their owner, rerun the same scenarios and applicable full gates,
   update honest records, and return a clean local commit. Keep unqualified APIs outside apps.

## Session 3 matrix

One row per current component/state/renderer: exact source/token digests; API/example;
structural/state/callback result; named token/alias/density result; runtime input/focus;
opened capture/actual reference; reviewer; blocker/owner. Registration is inventory, not a pass.

| Cross-cut | Required evidence and remaining boundary |
| --- | --- |
| Light/dark, compact/comfortable, narrow/wide, 1×/2× Vietnamese | Opened captures for each renderer; labels, diacritics, target geometry and reflow |
| Actions/input | Pointer, keyboard traversal/activation, focus, repeat/disabled/loading; raw `0`/empty/incomplete decimals, controlled updates, selection/copy/composition/IME |
| Navigation/menus/dialogs | Availability loss, changing/empty options, Escape/Back where supported, focus containment/restoration, retained input and resize |
| Tokens/reference | Current semantic roles and effective theme/density/alias/motion values in both previews; actual reference fidelity distinguished from token agreement |
| OS/native | CMP window binding, Vietnamese IME, font fallback/Noto metrics, motion/reduced motion and assistive technology; JVM semantics/captures do not substitute |
| Android/iOS | Real configured targets/device or simulator evidence before any mobile claim; product apps currently remain guides |

Missing tooling is an evidence blocker, not permission to bypass binding, TLS or settings. Do not
install large mobile SDKs to disguise unsupported apps. Leave qualification unapproved if runtime,
accessibility or reference evidence is missing.

The [final session](../evidence/cmp-ui-system-session3.md) now closes negative/nonfinite range
callbacks, Float keyboard precision and same-tick alias completion, preview fraction truncation,
two inherited admission inventory bypasses and LoadingState announcement discovery. The
[readiness inventory](../evidence/cmp-ui-system-readiness.json) records 29 APIs / 152 scenarios /
304 renderer rows and all 73 named roles (72 canonical plus one alias). Local structural and
behavioral evidence is held separately from incomplete production qualification. No automatic
additional session or product-app expansion is recommended; manual acceptance and publication
remain with the parent/owner under the existing contract.

At final handoff, external [PR42](https://github.com/loveoverflowcom/cantos/pull/42) advanced
develop to `342d65fdbd3764f76b772ee0a398a9e209115b65`, renaming Theatre to `apps/cantos-theatre`
and migrating its exact baseline import path. The local three-session sequence remains tested
against merge-base `7f416a1e1bac5bbf338d461463770bffd983b5c1`; it is not rebased/merged here.
Before any later authorized publication, reconcile paths/catalogue baseline/generators and
refresh integrated pinned-toolchain evidence. The external merge does not authorize publishing.

## PR43 integrated acceptance

The requester separately authorized review, local execution, fixes and merge of
[PR43](https://github.com/loveoverflowcom/cantos/pull/43) on 2026-10-12. The
[integrated acceptance record](../evidence/cmp-pr43-acceptance.md) reconciles PR42 paths and generated
catalogues, closes equivalent-option Select menu interruption with a failing-first JVM regression,
and records fresh pinned local evidence. The three original commits remain intact. This accepts
the desktop preview implementation; 0/29 components remain qualified for new app use, and native,
reference and accessibility qualification remains the next prerequisite before adoption.
The requester then selected Android Studio host preview instead of a connected-device run. The
[Android host follow-up](../evidence/cmp-pr43-android-preview.md) records a real isolated library
consumer, four static PNG/semantics captures and adapter/freshness fixes. This does not implement
either product mobile app or change the 0/29 qualification boundary.
The subsequent requester-authorized [choice focus-spacing fix](../evidence/cmp-choice-focus-spacing.md)
adds clearance between the shared choice ring and supporting text, with a failing-first JVM
regression and an inspected repeat of the narrow dark/2× Android host preview. The next adoption
prerequisite remains native, reference and accessibility acceptance; GitHub owns publication status.

## Design v3 CMP slice (issue 143)

[Issue 143](https://github.com/loveoverflowcom/cantos/issues/143) is the CMP counterpart of the
merged [Leptos v3 workbench](../evidence/leptos-preview-v3.md) and shares its
[inventory](../evidence/ui-design-inventory-v3.md) and
[token reconciliation](../design/ui-token-v3-reconciliation.md). Its first slice leaves the token
source, the registry and the admission contract unchanged and gives the CMP gallery the discovery,
injection and evidence plumbing later batches need: a Design v3 tab over the shared inventory,
`Light / Dark / System` resolved at a composition root, named dp evidence constraints and
JSON capture records. See the [evidence record](../evidence/issue143-cmp-preview.md); registration,
qualification and Android/iOS acceptance are unchanged by it.

Recommended order for the rest of the issue; each batch needs a concrete consumer and follows
lookup → reuse/extend → declare and preview → state/theme/motion evidence → admission → product
adoption:

1. Theatre first batch over registered primitives, starting with the shared `list-row` anatomy and
   then work/continue cards, episode rows, progress, mini/full player controls, queue/bookmark and
   offline/download/recovery states. The Theatre mock's `features/shell/presentation` screens compose
   raw Material controls today (Membership already uses the shared Button, TextField, Surface, Badge,
   BackButton and Logo), so these are app-owned compositions over registered primitives with a
   preview first, never copied controls.
2. CMP font delivery. Done for the desktop preview host (shared Noto bundle, nested-theme
   inheritance, opened type-specimen comparison against the design's fonts in the
   [evidence](../evidence/issue143-cmp-preview.md#type-specimen-against-the-design-fonts)). Still open:
   the Android Studio wrappers and each product host, and the line-box difference that drifts text
   up to 8 px against the CSS reference (a shared theme decision with product-wide layout impact, so
   its own reviewed change).
3. Android Studio captures of the Design v3 wrappers once a ready Quail 2+ project exists; JVM
   captures stay a different renderer.
4. Per-component two-renderer admission for any new shared primitive, with the Leptos side owned
   by the Web issues.

## Motion and gesture slice (issue 145)

[Issue 145](https://github.com/loveoverflowcom/cantos/issues/145) adds one injected motion policy
to the CMP package: a pure intent × property × preference mapping fed by the canonical
`motion.*` tokens, a test-owned virtual-clock replay, and the two consumers that exist today (the
Auth action press and the Theatre launch entrance). Its [evidence record](../evidence/issue145-cmp-motion-policy.md)
labels what the JVM replay does and does not show. It changes no token, registry or product
default, and it adds no mini ↔ full player, sheet, drag or Android/iOS adapter.

Recommended order for the rest of the issue; each step needs a concrete consumer or a target:

1. Android and iOS reduce-motion adapters at the composition roots, with `android-runtime` and
   `ios-runtime` observations of Full and Reduced and a reversal recording. A `UiPreferences`
   toggle never stands in for the OS signal.
2. The first transition or drag consumer (play/pause morph, mini ↔ full, sheet) declares its
   preview scenario and named gesture values with units before product use; the Theatre mock has
   none today.
3. Motion timeline controls (replay, pause, seek-to-frame, restart) join the scenario workbench
   of #144 rather than a second gallery; the replay recipe model is the seam.
4. Generate the Leptos mapping from the same token source when the Web has a real motion consumer.

## Commands and risks

Use pinned toolchains and `cargo xtask verify repository`, applicable Rust/contracts/Web checks,
`cargo xtask ui-preview cmp verify`, both `ui-preview ... run` commands and final
`cargo xtask preflight --changed-only`. One heavy lane at a time; retain Gradle9.7, other consumers'
caches and existing evidence/DB/audio/models. No remote CI polling.

Residual: OS Java-app binding, real IME, screen reader, motion, full reference and mobile acceptance
remain incomplete. Editable→ReadOnly during active native IME composition needs platform evidence.
