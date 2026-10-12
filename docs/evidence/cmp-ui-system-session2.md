# CMP UI-system session 2

Status: bounded shared-component implementation and local handoff, development previews only.
Session 2 of the authorized Mac mini Ultra → Extra High → Ultra sequence continues
`feat/cmp-foundation-parity` from `b1e3c39a1039976eefcd6cd3ea9431d8138ff4c2`.
The tree was clean before edits. A fresh read-only fetch found develop unchanged at
`7f416a1e1bac5bbf338d461463770bffd983b5c1`. The session-1 commit is retained.
No push, PR, merge or next workchat is authorized. The [queue](../work-plan/016-cmp-ui-system.md)
owns session 3's recommended order.

## Invariant and ownership

A component renders the caller's current value and requests one change per activation; label,
keyboard, interrupted availability and modal focus behavior must not invent or discard state.
The concrete consumers are both package-local diagnostic previews. There are no product app,
backend, persistence, audio, provider, font-resource or dependency changes.

| Claim / failure mode | Owner / boundary | Cheapest oracle and evidence held |
| --- | --- | --- |
| A choice label activates one named native target, without a second focus/callback path | Choices; full row toggleable/selectable, visual Material control has null handler | Failing-first pointer-label regression; JVM Space, role, focus, repeat, controlled rejection, error/reason and both density bounds |
| Reordered options retain identity; empty/missing values do not dispatch; unsupported choices explain their reason | SelectOption and stable-value Select; caller owns value, local menu owns open interaction | JVM stable identity, unsupported option, empty/missing, interruption and Escape; SSR fixtures and live Safari order/value/count observations |
| Numeric state never silently rounds an observable fraction or contradicts a stepped thumb | Range validation/native tick projection/key adapter; caller owns value/valueText | Failing-first fractional label regression; finite/range/tick examples, Float edge regressions, native progress/key/completion callbacks |
| Tabs expose named selection and complete labels, keeping focus separate from callback acceptance | Navigation; native Tab and adaptive group, no product router | JVM Arrow/Home/End, selected/focus/repeat/disabled; opened narrow 2× same-scenario capture |
| Dismissal is a request, input survives close, and focus returns to a mounted trigger | Dialog; caller owns open/input/FocusRequesters and properties; native modal owns containment | JVM initial/default focus, Tab/Shift+Tab, Escape properties, rejected dismissal, close/reopen/return; bounded Safari input/Escape/reopen |
| Horizontal groups reflow; spacing/reading surfaces and recovery announcements remain caller-owned | Layout/Feedback within the existing package | JVM wrapping geometry, explicit live-region mode and slot callback; both preview specimens and opened pixels |
| Registration cannot become production admission | Shared registry/generator, unchanged compatibility baseline | Repository generator/inventory/adoption gate; 29 APIs, 151 scenarios, 73 canonical tokens, zero qualifications |

No durable domain rule or new abstraction is introduced. The small deterministic core validates
range/link inputs and projects bounded native ticks/key targets. The shell keeps Compose focus,
menu and dialog lifetimes local. The public package remains `com.cantos.ui`; Choices, Navigation,
Layout and Feedback now own the wrappers formerly in Components. Components retains root,
field messages and official decorative icons. String field/action APIs and Leptos library APIs
remain compatible. No new component enters an app.

## Implemented API and platform differences

- Checkbox/Radio/Switch keep `modifier` on their whole field; `controlModifier` attaches to the
  full-label row. Help and normalized error semantics sit on that same interactive node; blocking
  reasons remain visible. Native choices retain enabled/disabled/loading, without a fake readonly mode.
- `SelectOption(value, label, state)` separates stable identity from localized labels. The new
  `Select(options: List<SelectOption>, value: String?, onValueChange, ...)` requires localized
  placeholder/empty/expanded/collapsed descriptions. Values are unique; missing selections show
  the placeholder and stay caller-owned. Empty lists disable the trigger with a reason. Option
  availability is visible. Changed options, selection or availability close an open menu.
  The existing nonempty index overload delegates and remains available for fixed lists.
- RangeField exposes `steps`, `valueText`, completion, help/error and native control modifiers.
  Values/endpoints/span must be finite and ordered, with the controlled value inside the range.
  Zero remains a real value. `steps` counts interior native ticks and is bounded to 0–1000 because
  Material allocates/renders ticks. Stepped configurations with collapsed/nonfinite Float ticks
  are rejected before dispatch. A value must match a native tick within one ULP for ordinary
  decimal representation; this does not parse or normalize NumberField's raw text.
  The projection uses the pinned native Float `lerp`, not Double interpolation. Native Material3
  1.8.2 has progress semantics but lacks a key handler; the wrapper supplies arrows/Home/End/Page
  keys, immediate controlled change requests and completion on key release. Continuous arrow
  increments are one percent of the span; page increments are ten percent. Focus loss or lost
  availability cancels unfinished keyboard completion.
- Link keeps URI opening/routing caller-owned and rejects network-path references, blank HTTPS
  authorities, whitespace/control characters and backslashes. Its optional availability and
  control modifier are native action semantics. NavItem has the same availability boundary.
  Named NavTabs uses native in-place Tab selection, Arrow/Home/End focus and adaptive wrapping;
  long labels are constrained to the host rather than clipped by a fixed tab row. Leptos NavTabs
  remains navigation links; its anchors remain available while actions are blocked. No ARIA-tabs
  behavior or disabled-anchor equivalence is claimed.
- Dialog exposes optional initial/return FocusRequesters and native DialogProperties. Keep it
  mounted across close; requesters must point to mounted focusable targets, with initial focus
  inside the modal. The safe default focus is the close action; outside-click dismissal defaults
  off. Callers accept dismissal by changing `open`, and own retained input. Removed/disabled
  triggers require caller-owned fallback focus; no recovery guarantee is claimed for stale targets.
- Space supplies existing semantic gap/inset names; Surface adds Reading shape/type intent.
  Horizontal Stack reflows; Wrap remains a compatible alias. Grid and Container retain host-width
  ownership. Announcement defaults to Off for loading/error and explicitly supports Polite or
  Assertive; Status retains its boolean compatibility argument. ErrorState adds a recovery slot
  while retaining the retry-label overload. Hosts own announcement deduplication/throttling.

The [pinned Material3 Slider source](https://github.com/JetBrains/compose-multiplatform-core/blob/v1.8.2/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/Slider.kt)
defines interior steps, completion and native tick interpolation. The independent reviewer also
inspected the locally cached Material3 1.8.2/ui-util 1.9.0 bytecode. Current Android
[checkbox anatomy](https://developer.android.com/develop/ui/compose/components/checkbox) and
[slider documentation](https://developer.android.com/develop/ui/compose/components/slider)
were read as reference guidance; actual compatibility was compiled/tested against the unchanged
pinned artifacts. Neither documentation nor those libraries' names establish mobile acceptance.

Both previews declare 22 additional scenarios: full-label activation; empty/missing/reordered/
unsupported/interrupted selection; stepped/completed range; overflow/keyboard/disabled native
navigation; retained dialog input; gap/insets; and recovery/announcement policy. The Leptos
counterparts use actual native HTML controls and existing shared components. Unsupported native
versus HTML behavior is explained explicitly, not filled with passing placeholders.

## Actual local evidence

Commands run from the repository root with Rust 1.87.0, Cargo 2 jobs and `TMPDIR=/private/tmp`.
Task-local PATH selects existing official Gradle 8.14.3 and full Microsoft JDK 21.0.7+6;
Kotlin/compiler 2.2.20, Compose 1.9.0 and stable Material3 1.8.2 remain pinned. Initial resources:
40 GiB free disk and memory_pressure 86% system-wide free. No downloads, cache deletion, TLS,
OS/security setting or remote CI change was made. Gradle9.7 consumers and private assets remain intact.

Ignored evidence lives under `target/cmp-session2-evidence/`. The final handoff and per-state
matrix carry current renderer/token digests; the matrix is an evidence inventory, not qualification.

| Check | Actual scope / result | Evidence artifact |
| --- | --- | --- |
| `cargo xtask ui-preview cmp verify` | PASS, 64 executed, 0 failures/errors/skips: 23 common, 31 desktop semantics/composition and 10 capture methods | `cmp-verify-float-final.log`, final XML/aggregate; final-source preflight below |
| `cargo xtask ui-preview leptos verify` | PASS, 48 package tests including 15 gallery tests, WASM warnings-denied lint, actual Trunk build | `leptos-verify-second.log` |
| `cargo xtask ui-preview cmp run` | Owned foreground PID guard/task launched; MainKt observed; OS input BLOCKED | `cmp-runtime.log`, native-tool conversation and `runtime-observations.json` |
| `cargo xtask ui-preview leptos run` | Real Safari pointer, keyboard, menu, range, Vietnamese paste, Dialog and recovery observations | `leptos-runtime.log`, native-tool conversation and `runtime-observations.json` |
| Repository/Rust/contracts/Web/final changed-only preflight | Final gate results recorded in handoff completion below | Named final logs |

The first CMP regression run executed 43 tests with two failures on the old implementation:
label-pointer callback absence and fractional range label truncation. These were fixed before
broader tests. The first extended key tests failed compilation until the pinned test API's
experimental opt-in/member imports were corrected; those attempts are not passing tests.
The next run's only failure was absent Slider key handling. Leptos's first run failed an SSR
assertion expecting a Select value attribute: that component intentionally sets a browser
property. The corrected assertion checks the requested-value output, while the actual property
was subsequently observed in Safari. Failed logs are preserved. No checker was weakened.
The range decimal/quantization examples were added during review without a separate failing-first
renderer build; the reviewer reproduced their Float failures from actual dependency arithmetic.
Only executed final XML counts are reported; UP-TO-DATE is not another test execution.

## Captures, runtime observations and independent review

JVM roots are explicitly 1140×840 or 372×840, density 1, platform fontScale 1 with the named
preview text multiplier. Primary and independent reviewers opened all 13 new `cmp-interaction-*`
PNGs. These include dark comfortable narrow 2× checkbox, light radio/switch error, empty/missing/
reordered Select, dark stepped Range, narrow light 2× navigation, horizontal Stack, reading
Surface, three-column Grid, inset-free Container and dark recovery actions. Existing foundation
and full-root captures were also freshly rendered; new-state inspection is distinct from complete
registry/reference acceptance. No static mobile capture ran because both apps remain guides.

| Finding | Responsible fix | Same-scenario recheck |
| --- | --- | --- |
| P2 narrow 2× Tab label cut at both edges, then height clipped under native row measurement | Host-bounded intrinsic native Tabs reflow in FlowRow | Same long labels, 372×840/light/2× reopened; all four labels complete |
| P2 stepped specimen said45 while native thumb snapped50 | Stepped specimen50; bounded native tick/value contract | Same Range scenario reopened with matching label/center tick |
| P2 Switch error used title validation copy | Choice-specific preview error text | Same narrow light/error reopened; reason now describes the choice |
| P2 validator rejected its own Float keyboard/native outputs | Pinned native Float interpolation, bounded finite distinct ticks, one-ULP decimal tolerance and nearest native keyboard index | 0.1–0.9 decimal, power-of-two/large-offset collapse and forward/reverse regressions PASS |

Independent `cantos-code-review` returned `no-actionable-findings` after these fixes, with
`complete-for-declared-scope` coverage of core, preview, tests and catalogue/admission boundaries.
Its source snapshot fingerprint is `a315ae1d7a9167f89138386e5a054c24b3dee1c0b991f5ff70b9bb91f363826c`.
It read actual64-test output and reopened the three corrected captures, without editing or
running another heavy build. Final documentation/gate aggregation is primary-reviewed separately.

Bounded live Safari observations (opened tool screenshots were 1621×862; CSS viewport/zoom was
not instrumented, so no current narrow/200% browser claim):

- Stable→reordered retained the native `Người dẫn chuyện` label and narrator value at callback0.
  Choosing `Nguyễn Thị Minh Phương` set `minh-phuong` at callback1. Empty/missing then restore
  retained callback1 and caller state. The native menu exposed the unsupported option as disabled.
- Clicking Checkbox, Switch and Radio text labels produced three selection events and the expected
  checked/selected states. No source-only callback result is substituted for these UI observations.
- Pointer range selection produced50/completion1. Ordinary Tab skipped the range in this Safari
  configuration; Option+Shift+Tab reached it. Focused Home then Right produced25/completion3,
  with localized `25 trên 100`. No preference was changed and no full keyboard pass is claimed.
- Dialog safely focused Close. Native Vietnamese paste retained `Nguyễn Thị Minh Phương · Ỗ, Ữ`.
  Escape from the field requested dismissal and returned focus to Open. Reopening retained text.
  Verified theme selection and opened pixels showed both light/dark modal treatments. Paste does
  not establish Telex/VNI or IME composition. Two recovery buttons produced count2.
- Only the task-created Safari tab was closed, returning to the existing Cantos Theatre tab.
  Ctrl-C stopped the owned Leptos task. CMP Ctrl-C reported its owned launcher23776 stopped;
  later app inventory contained no MainKt/Java app. The initial filtered PID snapshot was empty,
  so a complete descendant-PID shutdown proof is not claimed.

CMP app inventory observed `MainKt` / `net.java.openjdk.java`, but supported bindings by name
and ID both returned `Invalid app`. No binding workaround, synthesized OS input, truststore
change or security bypass was attempted. JVM injected input is `interaction-tested` on that
renderer, not OS-window input or Android/iOS evidence.

## Completion and remaining matrix

Surface intensity: Low diagnostic controls and reading specimens with spacing/quiet tonal
containment. Localization: synthetic vi-VN developer-only labels, no product key changes.
Tokens: canonical v0.2.0/73 roles unchanged; generated catalogue updated through its primitive.
Evidence levels: `example-tested`, `compiled`, `semantics-tested` and bounded `interaction-tested`
on JVM; `screenshot-inspected`, `cross-theme-inspected`, listed `cross-viewport-inspected` specimens;
bounded Safari `interaction-tested`. No `screen-reader-walked`, `device-tested` or full reference
fidelity claim. Kotlin formatting/lint has no configured gate; handwritten Kotlin was manually
reviewed, then compiled/tested. Rust formatting and warnings-denied gates apply normally.

Final implementation handoff runs `cargo xtask verify repository`, `verify rust`, `verify contracts`,
`verify web` and `preflight --changed-only` without skips. Their tool-printed final outcomes and the
local commit/tree are in ignored `handoff.json`; this document never promotes a planned run to PASS.
The tool-printed completion outcomes are:

| Final suite | Outcome | Log under ignored evidence root |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS; generator/admission/link/whitespace checks and192 Python tests | `repository-final.log` and final preflight |
| `cargo xtask verify rust` | PASS; workspace format/lint/tests | `rust-final.log` |
| `cargo xtask verify contracts` | PASS; Script IR, contract tests and WAV reference | `contracts-final.log` |
| `cargo xtask verify web` | PASS; Web lint, both consumer builds and preview build | `web-final.log` |
| `cargo xtask preflight --changed-only` | PASS; all9 applicable gates, no skip/FAIL/UNSUPPORTED; final CMP source desktopTest executed64 tests | `preflight-final.log` and final XML |

PostgreSQL/mutation are not selected by this UI-only diff and were not run in this session.
Remote CI is not checked. Publication remains local only.

Remaining: CMP OS pointer/keyboard/focus/resize **BLOCKED**; real Vietnamese IME/active-composition
locking, screen reader, motion/reduced-motion interaction, Noto delivery/metrics, complete actual
reference and every registry/state/theme/density/viewport cell **NOT_RUN**. Dialog outside-click
property and stale/removed focus targets lack runtime acceptance. Android/iOS app surfaces remain
**UNSUPPORTED**, mobile runtime **NOT_RUN**. Qualification stays **0/29**. Session 3 must audit the
current matrix and fresh digests, resume supported missing evidence, and leave every incomplete
component unqualified; it must not use desktop/browser evidence as mobile acceptance.
