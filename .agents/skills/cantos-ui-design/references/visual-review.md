# Visual review

> **Scope.** Judge the rendered result of a Cantos UI change: how to pin a scenario so a recheck
> is meaningful, where evidence lives, the top-down screenshot pass (orientation, intensity,
> color, shape, size, containment, harmony, text, states), the Studio and Theatre checks, a
> grayscale and color-vision pass for status, the findings ledger and the same-scenario recheck,
> and what an image can never establish. Use when you capture or open screenshots, review a UI
> pull request, write a finding, or recheck a fix.

The requirements are owned by [UI system](../../../../docs/design/ui-system.md) (the
[validation matrix](../../../../docs/design/ui-system.md#future-validation-matrix) lists the
conditions per surface) and the rules are `UI-INTENT`, `UI-STATE` and `UI-PIXELS` in
[`../SKILL.md`](../SKILL.md#rules). Studio Web, the isolated Theatre Web/CMP mocks and shared
Web/CMP preview hosts exist. JVM capture tests export PNGs and metadata/semantics; the optional
[Compose preview adapter](../../cantos-cmp-mobile/references/compose-preview.md) requires the
selected real Gradle project, official CLI and synced Android Studio. Neither path establishes
native device acceptance. Match fidelity only against inspected, attributed design and renderer
pixels ([UI system § Direction and reference](../../../../docs/design/ui-system.md#direction-and-reference)).

## 1. Pin the scenario before the first capture

A fix is only shown by repeating the *same* scenario. Record, before capturing:

| Field | Example |
|---|---|
| surface and route | Theatre Web full player; Studio script editor |
| source SHA and dirty state | `abc1234` + "2 modified files" |
| renderer and target | Chromium 130 on Linux; Pixel 8 emulator API 35; iPhone 15 simulator iOS 18 — a desktop or JVM host is named as such |
| viewport | `320×720 CSS px`, `360×800 dp`, `390×844 pt` |
| theme, locale, text scale | dark, `vi-VN`, 200 % zoom or the OS font scale |
| fixture and state | the example episode, `Một lời hẹn`, state `Buffering` |
| input | pointer, keyboard, touch, screen reader |

Unknown values are written `unknown`, never guessed. Reuse a reviewed scenario from
[`cantos-ui-inspector`](../../cantos-ui-inspector/references/scenarios.md) when it covers the
state; a scenario edited between runs proves nothing.

## 2. Evidence: location, capture and hygiene

- **Location:** `artifacts/ui-evidence/<run-id>/`,
  ignored by `/artifacts/` in the root [`.gitignore`](../../../../.gitignore). `target/` is also
  ignored. Confirm with `git check-ignore -v <path>` before writing; never write evidence to a
  tracked path.
- **Never commit** screenshots with private text, provider output, real voice samples, signed URLs
  or credentials. Sanitize a copy before an image or log reaches an external tool. Audio used in a
  flow is generated at test time, and audio extensions are already ignored.
- **Capture context and detail** for each finding: the whole screen and the region, in the same
  state. Name files `<state>-<viewport>-<theme>-<locale>-<scale>.png` so a recheck lines up.
- **Capture tooling:** `cargo xtask compose-preview` is the optional Android Studio host capture
  path; follow its [workflow](../../cantos-cmp-mobile/references/compose-preview.md). Other
  candidates are the browser pane or a headless browser driver for Web; configured Compose
  screenshot tests for CMP layout; `adb exec-out screencap`
  on Android and `xcrun simctl io <device> screenshot` on iOS for native captures. Verify each tool
  and version before relying on it and record the choice in an ADR.
- **Open the image.** An unopened capture is `screenshot-captured`. Open every cited image with an
  image-capable tool, record who or what opened it, when, and what was concretely visible.

### Native launcher icons

Use the canonical Logo Kit guide/provenance in `packages/ui-system/shared/assets/` rather than
defining another brand palette here. Open the actual source artwork before reviewing generated
native resources. OS launcher icon, native splash/LaunchScreen and CMP Logo are independent
surfaces: a splash screenshot or Compose preview cannot accept a launcher icon.

Record source-inspected, packaged and device-tested evidence separately. Inspect installed
icons in Home, drawer/App Library and Settings, including supported masks, theme/themed modes,
and fresh install/update/cache behavior. Pin OS/launcher, mask, density and APK/bundle hash.
Pair the reference at the same displayed scale; OS masking is an intentional platform
adaptation, not automatically a pixel defect. Missing device/runtime evidence remains pending.

## 3. The review pass: hierarchy first, then the levers

Work top-down and stop at the first broken level: spacing polish on a screen with no legible
primary action is wasted work.

### 3.0 Orientation (three seconds, no scrolling)

| Question | Failure signature |
|---|---|
| Where am I? | no visible identity for the work, episode or section; the title reads like body text |
| What is the primary content? | chrome (bars, cards, metadata) carries more weight than the script or the cover |
| What is the primary action? | two or more equally loud filled buttons, or the primary action is a quiet text link |
| What is secondary? | filters, metadata or navigation shouting as loudly as the content |

If any answer takes longer than three seconds, that is at least a P2 whatever follows.

### 3.1 Intensity against the surface row

Compare with the [surface intensity table](../SKILL.md#material-3-expressive-is-an-attention-system)
and [`material3-expressive.md`](material3-expressive.md):

- **Studio script editor (Low):** the text is the loudest thing; no card per utterance, no loud
  page header, no heavy toolbar.
- **Production status, QC, approval, publish (Low-Medium):** trust and clarity; revision identity
  visible; one primary action.
- **Casting and voice preview (Medium):** selected voice and the preview action stand out.
- **Theatre discovery (Medium):** cover art, title and availability lead; no oversized banner or
  gradient; peer episode rows have equal weight.
- **Theatre full player (Medium-High):** transport is expressive and playback state always
  legible; secondary actions group quietly.
- **Mini-player (Medium-Low):** compact, persistent, never obscuring content or focus.

### 3.2 Color — Lavender Stage

- Every color traces to a semantic role ([`tokens.md`](tokens.md)); a local hex literal is a
  finding even when it looks identical today.
- **Tonal hierarchy, not borders.** Group with surface steps and space; a 1 px outline on every
  group is the wrong mechanism.
- **Accent budget.** The primary hue marks the primary action and the active state; five accented
  things means there is no primary action.
- **Calm lavender.** Brand lavender is an accent on low-chroma reading surfaces, not a wash over
  every container; dynamic color must not replace the identity.
- **Dark is designed, not inverted.** Check dark separately: containers distinguishable in light
  can collapse into one flat field, and text that passes in light can fail in dark.
- Text over cover art, scrims or tinted containers needs a measured pass on the real pixels
  ([`accessibility.md`](accessibility.md#1-measuring-the-targets)).

### 3.3 Shape, size and containment

- **Shape differentiates classes.** Pills mean *action* (filters, speed, compact commands); a
  static pill label is a fake affordance. Scripts and data keep stable rectangular reading areas.
- **Size shows importance.** The primary action is visibly heavier than secondary ones; headline,
  title and body differ enough to be a hierarchy; density fits the context (an editor tolerates a
  denser toolbar than the player).
- **Targets.** Measure interactive bounds, not the drawn icon: 48 × 48 dp on Android, 44 × 44 pt
  on iOS, generous hit areas on touch Web.
- **Containment extremes are both defects:** a card in a card in a card is chrome without meaning;
  everything floating with no groups has no scannable structure. The wrong fixes are another card,
  border, shadow or gradient; the right ones are alignment, spacing tokens, tonal level, type scale
  or removing an element.

### 3.4 Text — Vietnamese first

| Look for | Typical cause |
|---|---|
| clipped diacritics (`Ấ`, `Ệ`, `Ữ` cut at top or bottom) | fixed line height or box height |
| tone marks colliding with the line above | line height too tight for stacked diacritics |
| a long title pushing the primary action off-screen | no wrapping or no max lines policy |
| `Người dẫn chuyện` or a long character name overflowing a chip | nowrap in a flex row |
| times and costs that jitter as they update | proportional figures instead of tabular |
| lines longer than about 90 characters in prose | no measure limit |
| the longer locale breaking a layout | sized for one language's labels |
| a paragraph explaining a familiar control; the same explanation in two regions | copy not classified by role ([`contextual-help.md`](contextual-help.md#2-classify-each-piece-of-copy-by-role)) |

Use realistic strings: `Người dẫn chuyện`, `Ngày mai, mình có diễn tiếp không?`,
`Ánh đèn cuối sân khấu`, one long title, one long name ([`typography.md`](typography.md),
[`localization.md`](localization.md#6-layout-under-text-expansion)).

### 3.5 Harmony

Scan the whole composition: misaligned baselines between adjacent columns; spacing rhythm that is
not from the spacing tokens; unbalanced whitespace; a dense island beside an empty region;
unexpected wrapping; inconsistent icon sizing; controls of incompatible heights; a dialog that
feels unrelated to its parent; a heading louder than the content it introduces.

### 3.6 Motion

A still image cannot show motion. Look only for fake affordances (hover lift on a static card) and
for state that exists *only* in motion. Motion, interruptibility and reduced motion need a
recording on the target ([`motion.md`](motion.md)).

## 4. States and checks per surface

Capture the states a change can reach, not only the happy one
([`component-states.md`](component-states.md)).

| State | What to check |
|---|---|
| empty | an explanation with an action, never a blank region |
| loading | a placeholder matching the final layout; no shift on arrival |
| error | names what failed, whether work is safe, retry; passes contrast in both themes |
| disabled | still readable, visibly inert, with its **reason** beside it |
| focused | a visible indicator on every reachable control, unobscured |
| selected / current | distinguishable from hover and focus without color alone |
| dialog / sheet | scrim contrast; the primary action reachable at the smallest size; focus contained |
| long text / longer locale | no truncation of actionable labels |

**Studio.** Script position and selection survive pane changes; speaker, dialogue, emotion,
pronunciation notes and cues are distinguishable without a card per line; the draft/saved state
and the **revision** are visible; stale clips are marked with their source revision; the scope of
a regeneration is visible before it starts; "Generate selected dialogue" and "Publish episode"
name their object; publish shows its blockers; cost estimates read "unavailable" when unknown.

**Theatre.** The mini-player persists across navigation without covering content or focus;
elapsed and total time are text; seek works without the waveform; played, in progress, downloaded,
downloading, unavailable and failed episodes differ by text or icon; a missing cover and an empty
library are designed states; resume and bookmark prompts state the real position; expired access
and offline are recoverable states, not blank screens.

## 5. Status without color

For any status that carries meaning (QC, run, approval, download, playback), also capture it in
grayscale and, where a tool is available, with a simulated color-vision deficiency. Each state must
remain distinguishable by icon and text alone. A red dot as the only blocking signal fails
(`UI-STATE`).

## 6. The findings ledger

Record one row per finding while reviewing, and keep the same rows across rechecks.

| # | Where (screen / state) | Evidence (image, viewport, theme, locale, scale) | Rule | Symptom (observed) | Severity | Confidence | Fix owner | Recheck |
|---|---|---|---|---|---|---|---|---|

- **Rule** cites the owner by ID and heading (`UI-INTENT`, `UI-STATE`, `UI-A11Y`, a
  [`tokens.md`](tokens.md) section), never a paraphrase.
- **Symptom is observed**, not inferred. Root cause is `unknown` unless the source was read and
  established it.
- **Severity (guide):** P0 data loss, a bypassed gate or an unusable core flow; P1 a blocked or
  misleading primary journey, inaccessible control, clipped Vietnamese that changes meaning; P2 a
  hierarchy, state or consistency defect with a workaround; P3 polish. Intensity mismatch and a
  slow orientation answer are at least P2.
- **Confidence** is stated with a reason: `high` when reproduced and measured, `medium` when
  reproduced by eye, `low` when a single capture suggests it.
- **Rule out known-good states first:** intentional scrims, hover and pressed treatments,
  platform-appropriate geometry.
- The report format of an inspection is in
  [`report.md`](../../cantos-ui-inspector/references/report.md); a review inside a change uses the
  same fields in its notes.

## 7. The same-scenario recheck

1. Fix the responsible owner — token, component contract, renderer code — with the smallest
   coherent change, not a patch at the symptom.
2. Rerun the **identical** scenario: same fixture, viewport, theme, locale, scale, state and tool;
   only the source differs.
3. Open the new captures and update each ledger row: `fixed`, `not fixed`, `regressed` or
   `not rechecked`, with the new image.
4. Recheck the neighbors the fix could affect (the other theme, the longer locale, the other
   renderer for a shared token).
5. A scenario that changed between runs, or a recheck with an unopened image, proves nothing.

## 8. What a screenshot review can never conclude

- that the screen is accessible, operable by keyboard or usable with a screen reader;
- that motion is smooth, interruptible or reduced correctly;
- that Firefox, Safari or another OS version renders the same;
- that Web evidence applies to Compose, or a desktop render to a phone;
- that it matches a design reference that was never captured;
- that the data shown is correct, or that a saved or published state is durable.

State the scope in the report; an honest narrow claim beats a broad one nobody can check.

## 9. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| the layout and theme look right at a viewport | capture and **open** it | `screenshot-inspected` |
| it holds at each listed viewport or both themes | one opened capture per cell | `cross-viewport-inspected`, `cross-theme-inspected` |
| status survives color-vision loss | opened grayscale or simulated captures | `screenshot-inspected` |
| the recheck fixed the finding | same scenario, opened new capture, ledger updated | `screenshot-inspected` with the recheck recorded |
| files exist but nobody looked | image files only | `screenshot-captured` |
| it works on a phone | a named device, emulator or simulator run | `device-tested` |

## 10. Reference pairing and local comparison

`cargo xtask visual-compare` consumes **two existing PNGs**; its optional dependency, report v1
and parameter semantics live in the [tool contract](../../../../tools/visual-compare/README.md).
The [spike/decision](../../../../docs/evidence/visual-compare.md) is local calibration, not
approval of the product design. Do not add a second capture runner merely to compare a pair.

1. Pin a versioned, owner-approved design reference: ID/revision, exact PNG hash, approval
   record and source HTML/MJS/design revision. Original controlled fixtures use `calibration`,
   never product approval. An arbitrary located PNG is `unreviewed`; missing attribution means
   raw comparison only, no design-fidelity finding.
2. Pair actual build/source SHA and dirty state, renderer/version, physical dimensions and
   logical CSS px/dp/pt context, pixel ratio/density, exact font, theme, locale, text scale,
   fixture/state and capture time. Keep source and capture SHA separate. A size or known scenario
   mismatch is a mismatch, never a reason to auto-resize, crop or silently normalize. Capture
   a newly reviewed variant when platform geometry legitimately differs.
3. Freeze comparison policy before looking at results. Default threshold is exact; a nonzero
   channel threshold needs a recorded calibration rationale. There is no implicit AA/caret
   filter or ICC/gamma conversion. Dynamic masks need explicit bounded rectangles, reason,
   named reviewer and review record. Never add a mask, raise a threshold, change tile grouping
   or replace a reference merely to make a fix look better.
4. Record `comparison/report.json` and its hash. `diff_ratio` uses the whole-image denominator;
   also read excluded/compared counts and the unmasked ratio. Region `local_ratio` concerns its
   bounding box; `severity_hint` is area only. None is a percentage of design quality, a UX
   PASS, a P0–P3 assignment, native acceptance or a merge criterion. No global 90/95% gate exists.
5. Open reference, actual, numbered overlay, diff/mask/heatmap and cited detail crops. Write
   concrete observations and normal image records after opening. **Compared** describes an
   operation, not a new foundation evidence level: a computed metric or `images_inspected:false`
   report is separate from `screenshot-inspected`. Empty diff alone accepts no UI behavior.
6. Advisory AI findings name region ID and physical bbox, observed difference, rule, impact,
   severity/confidence with a reason, context/detail images, suggested change and recheck.
   Add a token/component, DOM locator or Compose testTag only when current source/DOM/semantics
   establishes the link. Pixels alone do not identify the file or root cause. Suggestions are
   advisory; the normal inspector report remains the finding owner.
7. Recheck the same scenario/reference, masks, threshold and grouping. Preserve
   `recheck_policy_sha256`; if it changes, label the run a new comparison policy, not proof of
   the old finding's repair. Actual build/capture identity may change. Open the new images and
   retain the original finding, with independent semantics/interaction/accessibility evidence.
