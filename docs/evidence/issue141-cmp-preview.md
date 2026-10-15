# Issue 141 — CMP token and existing-component review

This fresh review preserves the existing bounded admission of `button`, `text-field`, `surface`,
`badge`, `back-button` and `logo` after the [v3 token reconciliation](../design/ui-token-v3-reconciliation.md).
It does not qualify the 70 proposed specimens or grant Android/iOS, screen-reader, product
Membership, native playback or release acceptance. Historical PR137 records remain unchanged.

## Inputs and executed evidence

Base revision: `2d816d9592075f7c3bdeb0a53b8896b2e7178043`. Canonical v0.6.0 increases UI body
to 16 logical units and the regular compact minimum target to 48; editor, auth roles, colors,
spacing, motion and independent Web/CMP shape mappings are preserved. The generated CMP lookup
retains logical numbers and its existing theme converts type to sp and geometry to dp. No new
component API, mutable state owner, platform adapter or product import was introduced.

Command, using already installed tools, without installation or provisioning:

```sh
RUSTUP_TOOLCHAIN=1.98.1 PATH=/Users/manh.pd1/.gradle/wrapper/dists/gradle-8.14.3-bin/cv11ve7ro1n3o1j4so8xd9n66/gradle-8.14.3/bin:$PATH cargo xtask ui-preview cmp verify
```

`ui-preview summary`: `PASS cmp-ui-preview`; Gradle `BUILD SUCCESSFUL in 16s`. Actual JUnit
output contains 115 tests across 18 suites, with zero failures, errors or skips. The run used
Gradle 8.14.3, installed Microsoft JDK 17.0.19 and the existing Kotlin/CMP/Material3 pins
2.2.20/1.9.0/1.8.2 on macOS arm64. Production composables ran on JVM Desktop Compose, not an
OS window, emulator or phone. The shared preview uses its existing generic platform font
families; this run does not claim the new Web Noto bundle loaded in CMP. Font-injection and
platform-density tests remain separate from glyph inspection.

Full run output: `artifacts/agent-runs/issue141/tokens-cmp-accepted.log`. Fresh PNGs and raw
JUnit XML were copied into `artifacts/agent-runs/issue141/cmp/`; its `manifest.json` records
their hashes and source/token digests. These ignored local artifacts are retained for review;
this document is the committed observation record, not a replacement test runner.

| Component ID | Actual runtime / semantics oracle | Opened pixels and observation |
| --- | --- | --- |
| `button` | `ComponentSemanticsTest`: enabled action dispatches once and disabled reason remains; `AuthAccountComponentTest`: auth native role, blocked callbacks, one intent before host loading; `AuthActionMotionTest`: pressed/cancel/reversal/reduced-motion dimensions | Light focus ring and dark pressed capture retain legible labels and fixed hit bounds; normal light/dark auth actions and loading/disabled/error reasons opened |
| `text-field` | `AuthAccountComponentTest`: external label above native edit node, exact retained Vietnamese value/error, rich selection/composition and readonly contract; `FoundationComponentTest` and actual selection capture | 390-wide/2× focused error field has separate label/help/error and visible focus boundary; native single-line value scrolls horizontally rather than shrinking the font |
| `surface` | `AuthAccountComponentTest`: primary-container paired foreground in both themes; `ThemeFoundationTest`: nested preferences/colors/minimum targets restore without compounding platform font scale | Auth/profile/membership/account groups opened in both themes; 390-wide/2× intrinsic dark scene wraps long Vietnamese content and retains the last account footer; reading surface opened at 1140 |
| `badge` | Fresh `every_badge_tone_retains_readable_vietnamese_status_at_large_text_in_both_themes`: all five status strings exist, have no click action, retain the final character, no ellipsis/height overflow, and drawn line bounds stay inside text width | Both 390-wide/2× captures opened: Neutral/Info/Success/Warning/Error remain textual; the long neutral status wraps with its Vietnamese marks visible |
| `back-button` | `back_only_dispatches_its_host_intent_and_has_one_named_focusable_target`: localized native target, actual requested focus, minimum bounds and callback; disabled fixture retains reason | Light/dark auth scenes show one contextual circle; blocked capture retains the visible reason and recognizable arrow |
| `logo` | `LogoRendererTest`: one localized standalone image name, decorative no-name mark, exact official inks, transparent knockout on two backgrounds and unstretched square-slot fit | Official light/dark symbols and 160-square Cantos Studio lockup opened; shapes/letterforms remain intact and the test background remains visible through knockouts |

Opened scenarios include `cmp-auth-account-{light,dark}-390-text1`,
`cmp-auth-account-dark-390-text2-error`, `cmp-auth-field-light-390-focus-error`,
`cmp-auth-primary-light-390-focus`, `cmp-auth-primary-dark-390-pressed`,
`cmp-auth-states-light-390`, `cmp-auth-badges-{light,dark}-390-text2`,
`cmp-foundation-text-field-selection-light-compact-text2.0-372`,
`cmp-interaction-surface-reading-light-compact-text1.0-1140`,
`cmp-jvm-dark-wide-components`, and `cmp-logo-{symbol-light,symbol-dark,studio-aspect-fit-square160}`.
Capture density is 1×; width/height and text scale are supplied to the actual renderer.
The account scene is 390×1600 at default text and 390×4800 at 2×; focus/error and badge scenes
are 390×1000. Images were opened before calling them screenshot-inspected.

## Reference and visual boundary

Codex opened the retained Mobile v2 request, received and plans references from the authenticated
[issue 128 handoff](https://drive.google.com/drive/folders/16prSfqzeFv2rIVluO6sPU9iHt1nCnEKS), at
`/private/tmp/cantos-pr137-design/theatre-mobile-membership-{request,received,plans}.png`.
They supply the unchanged scoped auth/account intent: single primary action, external field
labels, quiet tonal groups, official logo and visible contextual Back. Current shared specimens
preserve that intent while showing honest mock/disabled reasons. The v3 HTML source was read at
`artifacts/ui-evidence/issue-141/design-v3.html`, SHA-256
`6b5c42359b8530f443863ead2ac31af86c9f1735e903a5bac4ef117574ff8cfb`; body 16 and minimum target
48 are the explicitly adopted values. Proposed replacement shape/action roles remain deferred.
Static PNG design exports are conceptual references, not JVM pixel oracles. No similarity
percentage or device fidelity is claimed.

## Gates, failures and residual risk

Invariant: the canonical value change improves body readability and minimum targets without
erasing state, changing host intents or coupling native geometry to Web shapes. Owner/boundary:
shared tokens → generated logical CMP lookup → existing theme/components; preview/test shell
owns preferences, state, capture and input. L1/L2 are source-reviewed; no new durable domain
decision or I/O rule exists. L3 is example-tested and statically-checked through generation;
L4 production composables are semantics-tested, interaction-tested and screenshot-inspected
on JVM Desktop Compose. The six new JSON records bind this bounded review to the current
token/source digests. Shared registration status remains separate.

The initial 114-test CMP run passed. A new badge assertion using
`TextLayoutResult.hasVisualOverflow` failed: the intrinsic text node measured 176×28 while
its cached paragraph retained the 318-wide parent constraint, setting width overflow despite
the complete drawn short label. The failure image and diagnostic metrics were opened/read.
The regression now checks drawn line bounds, final-character retention, ellipsis and height
directly, including a deliberately wrapped long label. The final 115-test run passed; no
production implementation or callback guard was changed to satisfy this test.

The first repository attempt failed on stale PR137 qualifications and concurrently changing
catalog outputs, before Python tests ran. Fresh issue141 qualifications and final combined
repository/preflight results are owned by the PR handoff; a CMP PASS does not imply those gates.
No Kotlin formatter/linter is configured; touched imports/syntax were reviewed and the pinned
Kotlin compiler executed. Android/iOS rendering, OS IME/Back/insets, screen-reader navigation,
native motion recording, live services and product adoption beyond the six existing APIs are
NOT_RUN. Their owners are unassigned; target-specific evidence is required before claiming
those acceptance scopes. JVM layout/semantics and opened pixels cannot establish them.
