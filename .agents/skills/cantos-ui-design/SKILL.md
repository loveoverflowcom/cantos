---
name: cantos-ui-design
description: >-
  Shared UI policy for Cantos Studio Web and Theatre Web/Mobile, using Leptos and Compose
  Multiplatform: Material 3 Expressive as an attention system with a per-surface intensity
  model, the calm Lavender Stage identity, one versioned semantic token source mapped to CSS and
  Kotlin, Vietnamese-safe typography, intent-based motion with reduced motion, accessibility,
  vi-VN/en localization, component state matrices and inspected visual evidence. Use for visual
  design, tokens, color, type, shape, motion, copy, accessibility, UI states or screenshot
  review; routes renderer work to cantos-leptos-web and cantos-cmp-mobile.
---

# Cantos UI design

This skill owns the **working method** every Cantos screen shares on both renderers: how to decide
a screen's hierarchy, express it through semantic tokens and shared component contracts, keep
text, accessibility and motion correct, and prove the rendered result. It is a behavioral contract
and router. Renderer mechanics — Leptos components, CSS, DOM, Compose, native adapters — belong to
[`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) and
[`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md).

It composes [`cantos-engineering`](../cantos-engineering/SKILL.md): the required order, the
[evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
and the [completion report](../cantos-engineering/SKILL.md#9-completion-report) are defined there
and reused here unchanged.

Apply the [platform matrix](../../../docs/architecture/repository-layout.md#platform-scope) before
routing renderer work: Studio is Web only; Theatre supports Web/Mobile. Shared CMP components
and preview fixtures never authorize a Studio mobile client or mock screen.

## Product truth lives in the docs

| Requirement | Owner |
|---|---|
| direction, token families and roles, Studio and Theatre regions, component states, accessibility and motion targets, validation matrix | [UI system](../../../docs/design/ui-system.md) |
| native semantics, focus order, large text, touch targets, safe areas, reduced motion on CMP | [mobile § UI and accessibility](../../../docs/architecture/mobile.md#ui-and-accessibility) |
| lavender/purple palette, rounded and pill controls, compact adaptive layouts, restraint | [brief § Design direction](../../../docs/product/brief.md#design-direction) |
| evidence a UI pull request carries | [CONTRIBUTING § UI changes](../../../CONTRIBUTING.md#ui-changes), [AGENTS.md § Verification](../../../AGENTS.md#verification) |
| meaning of playback, job, QC, approval and download states | the domain owners in [Compose with](#compose-with) |
| Studio Forge workspace, independent generation/storage/review/handoff states | [Forge ADR 0022](../../../docs/decisions/0022-cantos-forge-producer-cli.md), [acceptance boundary](../../../docs/deployment/forge-worker.md); planned UI, not local worker acceptance |

Link these; never restate a competing copy of a target, token family or state list. When this
skill and a document disagree on product behavior, the document wins; report the conflict.

## Naming on screen

Listener-facing strings (Web `<title>`, headings, navigation, splash, PWA/mobile manifest and
labels, localization keys' values) say **Cantos**; creator-facing strings say **Cantos Studio**.
`cantos-theatre` and "Cantos Theatre" are technical and documentation names only. Check every
listener surface for the technical name before calling UI work done.

## Repository reality

Studio Web, Theatre Web's scoped live account/Library boundary and isolated Web/CMP listening
mocks, and `packages/ui-system` Web/CMP previews exist. The
[Leptos package](../../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay) and
[CMP package](../../../packages/ui-system/cmp/README.md#scenario-recipes-and-replay) own their
scenario recipes, supported specimens and diagnostic modes; inspect those contracts and actual
code rather than treating inventory proposals as implemented controls. The
[Theatre desktop workbench](../../../apps/cantos-theatre/mobile/README.md#scenario-workbench)
uses the existing mock composition and memory sessions; it establishes no production persistence.
The authenticated design handoff supplies palette/font roles and official outlined icons.
Native listening remains deferred; existing JVM/Studio host captures establish no device
acceptance. Optional [local PNG comparison](references/visual-review.md#10-reference-pairing-and-local-comparison)
measures attributed pairs without approving UX. Therefore:

- discover the actual manifests, generator and evidence before citing a command; illustrative
  paths/APIs remain **proposed** until the owning package implements them;
- consume selected values from the canonical source; never invent a parallel palette or silently
  treat proposed spring/native values as implemented;
- no visual-fidelity claim is possible until actual reference/runtime pixels and viewport are
  inspected ([UI system § Direction and reference](../../../docs/design/ui-system.md#direction-and-reference));
- the [UI sequence](../../../docs/work-plan/015-leptos-ui-system.md) owns component and responsive
  integration order, with the foundation decisions linked below.

| First-slice decision | Detailed in |
|---|---|
| token source location, format and generator for CSS and Kotlin | [`tokens.md`](references/tokens.md) |
| palette values with recorded contrast per theme; dynamic-color policy | [`tokens.md`](references/tokens.md), [`material3-expressive.md`](references/material3-expressive.md) |
| font family with verified Vietnamese coverage; type scale | [`typography.md`](references/typography.md) |
| localization framework, key convention, locale set confirmation | [`localization.md`](references/localization.md) |
| reduced-motion adapters and the motion mapping per renderer | [`motion.md`](references/motion.md) |
| capture tooling and an ignored evidence location | [`visual-review.md`](references/visual-review.md) |

Record each with [`templates/adr.md`](../../../templates/adr.md) or the design issue form; a
decision made silently inside a component is a defect.

**Current localization is real, not a new first-slice framework choice.** Before
adding any visible/accessible text, inspect the live owner map and interim workflow in
[localization.md](references/localization.md#0-current-owners-interim-rules-and-open-choices).
Follow the caller to its existing owner, reuse a key only when its meaning matches,
update both vi-VN and en, include parameter/accessible copy checks, and avoid a new
per-feature translation mechanism. This preserves existing ownership; it does not
select shared semantic catalogs, storage or locale precedence. Keep all library and
catalog alternatives open under [#167](https://github.com/loveoverflowcom/cantos/issues/167)
([Web #168](https://github.com/loveoverflowcom/cantos/issues/168);
[CMP #169](https://github.com/loveoverflowcom/cantos/issues/169)) until a
small spike and ADR explicitly accept one.

## Route the work

| The change touches… | Load next | Evidence boundary |
|---|---|---|
| Studio or Theatre Web: Leptos view, CSS, DOM, browser media | [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) + the references here it needs | DOM and browser; never native |
| Theatre Android/iOS: Compose, `CantosTheme`, native adapters | [`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md) + the references here it needs | Compose semantics and named devices; never Web |
| a semantic token, shared component contract or shared copy key | [`tokens.md`](references/tokens.md), then **both** renderer skills | each consumer rendered and inspected; CSS proves nothing about Kotlin |
| a domain state shown in UI (playback, job, QC, approval, download) | the domain owner first, then [`component-states.md`](references/component-states.md) | the owner's tests for meaning; the UI only maps it |
| audit and findings report, no fixes | [`cantos-ui-inspector`](../cantos-ui-inspector/SKILL.md) | report-only |
| review of an existing diff | [`cantos-code-review`](../cantos-code-review/SKILL.md) | read-only; these references are criteria |

If you arrived here from a renderer skill, continue in that skill; do not reload it. A
documentation-only change to this skill needs no app render: validate links and routing, and say
that no production behavior was exercised.

## Material 3 Expressive is an attention system

Expressive design steers attention to what matters on *this* screen with five levers — **color,
shape, size, motion, containment** — while keeping familiar controls. It is not "round every
corner": a screen of equally loud pills has no hierarchy. Before styling, answer the intent
questions — primary goal, primary action, primary content, secondary tools, tone — and pick the
surface's intensity:

| Cantos surface | Intensity | Expression budget |
|---|---|---|
| Studio script editor, work navigation | Low | text dominates; quiet tools; stable rectangular reading areas; no card per utterance |
| Studio production status, QC, approval, publish | Low-Medium | trust and clarity; explicit revision identity; confirmations name episode and revision |
| Studio casting and voice preview | Medium | selected voice and preview action stand out; still one primary action |
| Theatre discovery and library | Medium | cover art, title and availability lead; no oversized banners or gradients |
| Theatre full player | Medium-High | expressive transport: size and shape morph on play/pause; playback state always legible |
| Theatre mini-player | Medium-Low | compact, persistent, never obscures content or focus |

No MVP surface is High; adding one is a product decision. The identity is **Lavender Stage**, a
working label for the direction in the docs: soft lavender/purple brand, low-chroma reading
surfaces, one primary action hue per screen, status never by color alone, dark theme designed
rather than inverted, and dynamic color that never silently replaces the identity. Depth,
M3 Expressive specifics and the candidate-component table: [`material3-expressive.md`](references/material3-expressive.md).

Semantic names and selected values come from the canonical source, with the current Web/CMP
mappings in [`tokens.md`](references/tokens.md) and the shared package contract. A proposal
does not replace those mappings or implement a native spring merely by naming one.

## Shared UI loop

Use [component-first lookup and preview admission](references/preview-catalog.md) before product
markup: find the API/variant/state, reuse or extend its contract, and declare a missing component
with preview scenarios before product adoption. Shared APIs retain two-renderer admission;
Web-owned compositions retain their explicit owner. Design status, source registration and
runtime qualification are independent. Registration/parity/source tests never replace runtime QA.

Every UI change follows this order on either renderer. Skipping a step is a decision you state.

```text
1  name the screen/flow, its surface intensity and its consumers (Web, CMP or both)
2  keep domain decisions out of UI: the backend or core decides, the UI renders typed facts
3  look up API + component ID + variant/state; reuse, extend, or declare and preview a missing kind
4  implement/localize the owned preview through semantic roles and injected facts/intents
5  select a reproducible recipe; drive state/theme/motion inputs, capture and open affected previews
6  complete admission before product import; then drive and inspect the production surface
7  fix at the responsible owner and recheck the same scenario, unchanged
8  format and lint with each touched toolchain
9  report with the completion report below
```

1. **Name it.** Route, screen, flow and every consumer of a touched token, component or key;
   write the intent answers and the intensity row. Seed the ledger from the
   [validation matrix](../../../docs/design/ui-system.md#future-validation-matrix).
2. **Decouple.** Approval staleness, publish eligibility, regeneration scope, cost, progress
   reconciliation and permissions are decided by the backend or a pure core
   ([foundation § 4](../cantos-engineering/SKILL.md#4-functional-core-imperative-shell)). The UI
   receives values such as `Unavailable(BlockingQcFindings)` and maps them through a pure,
   exhaustive presentation function. Element state (open menu, focus, scroll) stays local.
3. **Lookup before markup.** Record the catalog ID/API, variant and states that fit the consumer.
   If one is missing, follow the [declaration/admission sequence](references/preview-catalog.md);
   do not hide a reusable control or palette inside a feature to bypass it.
4. **Implement the owned preview.** Consume semantic roles and root-injected preferences, immutable
   projections and intents; never hand-edit generated output. Classify copy by role and place
   optional help by [`contextual-help.md`](references/contextual-help.md). Localize visible/accessibility copy
   with semantic keys, parameters and resources; map errors instead of exposing internals.
5. **Test and inspect previews.** Use the cheapest presentation-mapping/DOM/semantics regression,
   then select and restore a named recipe through the
   [preview replay method](references/preview-catalog.md#replay-a-preview-scenario). Drive
   state/theme/motion scenarios with realistic Vietnamese content (`Người dẫn chuyện`,
   `Ngày mai, mình có diễn tiếp không?`, `Ánh đèn cuối sân khấu`), the changed states, the
   viewports, themes and text scales the change can affect. Preserve the recipe/version, seed,
   injected owner, ordered intents and snapshot/import outcome; open each image and record findings
   with [`visual-review.md`](references/visual-review.md).
6. **Admit, then adopt.** Preserve the required renderer records and freshness checks; unsupported
   or unrun evidence leaves shared APIs unqualified. Product use needs its own flow observations;
   a gallery pass never proves an application flow.
7. **Recheck the same scenario.** Reset to the selected recipe, repeat the same input sequence and
   inspect new captures. Preserve fixture, frame width and actual viewport, theme, locale, text
   scale and motion preference; only the source differs. A scenario changed between runs proves
   nothing about the fix. An owner recreation must cancel the old mount and reject its callbacks;
   filtering or changing appearance is not an implicit reset.
8. **Format.** Each touched toolchain's formatter in check mode, then lint and tests.
9. **Report** every UI field, including what was not observed.

## Rules

Qualitative UI acceptance remains a review obligation. Existing token/catalog/source checks and
optional pixel comparison cover only their stated claims; proposed oracles below are not
implemented runtime gates.

| Rule | Failure mode | Good / counterexample | Oracle · enforcement | Exception |
|---|---|---|---|---|
| **UI-INTENT** write intent answers and intensity before styling | equal-weight chrome; decoration mistaken for hierarchy | full player: "goal keep listening; primary play/pause; secondary speed, timer, bookmark, download; Medium-High" / adding a gradient and rounder cards "to look expressive" | orientation test on inspected captures · manual | copy-only change with no layout effect |
| **UI-CORE** the UI renders typed facts and never decides domain state | Web and CMP diverge; a UI-only gate is bypassable | API returns `Unavailable(StaleApproval { approved, current })` / Leptos comparing revision numbers to decide staleness | review for duplicated rules; mapping table test · manual | presentation-only state at its lifetime |
| **UI-COMPONENT** lookup → reuse/extend → missing declaration + preview → state/theme/motion evidence → admission → product adoption | feature-local controls evade shared contracts or unqualified prototypes reach apps | record `button` + `AuthPrimary` and host-driven states / copy a filled button into a feature | [preview admission](references/preview-catalog.md), import gate + reviewed scenario evidence | app-owned compositions retain an explicit owner; no shared-admission exemption |
| **UI-TOKEN** visual values come from one versioned semantic source; generated output is never hand-edited; a role change maps both consumers | Web/CMP palette drift; theme change misses literals | `var(--cantos-color-primary)` / a hex literal in a component; patching generated Kotlin | drift test and literal scan (proposed) · manual | renderer-local geometry with a stated reason |
| **UI-STRING** every visible and accessibility string is a localized resource with a semantic key | untranslatable UI; English fallbacks; leaked provider errors | `studio.publish.action` in vi-VN and en / `"Publish"` literal; `error.to_string()` shown | literal scan, key and placeholder parity (proposed) · manual | logs and developer-only diagnostics |
| **UI-COPY** classify copy by role; essential content stays visible; cut redundant prose; optional help uses the [lightest reachable mechanism](references/contextual-help.md) | walls of instruction; or a blocker hidden in a hover tooltip or long press | icon button named `Đánh dấu vị trí 12:05`, no caption / a QC blocker only in a tooltip; a paragraph above a familiar control | [review questions](references/contextual-help.md#7-review-questions) · manual | none for essential content; no ban on helper text that prevents errors |
| **UI-STATE** each component has a state matrix; disabled carries a reason; status is never color alone | dead-end controls; QC blocks invisible to color-blind users | icon + text + tone for `Blocking`, reason beside a disabled Publish / grey button, red dot only | matrix tests and per-state captures · manual | — |
| **UI-A11Y** names state action + item + state; focus visible, contained, restored; drag and gestures have alternatives | keyboard or screen-reader dead ends; focus lost after dialogs | `Phát tập 3: Một lời hẹn` / an unnamed ▶ icon | `accessibility-checked`; `screen-reader-walked` only when it happened · manual | — |
| **UI-MOTION** motion uses intent tokens, never delays an action or moves a target under the user, stays interruptible; reduced motion keeps feedback | blocked or mis-tapped input; vestibular harm | play dispatches on press and the morph decorates it / waiting for the morph to finish | `MotionPolicyTest` exhaustive mapping and virtual-clock replay on CMP (Leptos mapping not implemented), runtime recording · manual | — |
| **UI-PIXELS** a visual claim needs opened, attributed captures of the changed states and a same-scenario recheck | "it compiles" passes for "it looks right" | before/after at the same 320 CSS px, dark, vi-VN / rechecking with a shorter English string | review ledger · manual | no rendered effect |

## Evidence selection

Use the foundation vocabulary verbatim; this table only selects among it.

| Claim | Cheapest suitable evidence | Level | Does not prove |
|---|---|---|---|
| domain state → label, icon, tone, availability | exhaustive match + table test over every variant | `statically-checked`, `example-tested` | rendering |
| paired token roles meet the contrast floors per theme | contrast checker over the token source (proposed) | `example-tested` | contrast over images, scrims, gradients |
| generated CSS and Kotlin match the source | regenerate-and-diff drift test (proposed) | `statically-checked` | that components consume the roles |
| a component exposes the right name, role, state | DOM test on Web; Compose UI test naming its renderer | `DOM-tested`, `semantics-tested` | pixels, screen readers |
| a flow works by input | events driven through the real flow | `interaction-tested` | screen readers, other targets |
| keyboard, focus and semantics | driven keyboard/focus walk plus semantics assertions | `accessibility-checked` | real assistive technology |
| a screen reader can operate it | named TalkBack/VoiceOver/NVDA walkthrough | `screen-reader-walked` | other readers or versions |
| layout, theme, locale, text scale | capture **and open** each image | `screenshot-inspected`, `cross-viewport-inspected`, `cross-theme-inspected` | interaction, motion, semantics |
| motion, interruption, reduced motion | recording or frame profile on the target while reversing mid-flight; a manual-clock replay covers ordering, intent-before-clock and final state only ([labels](references/motion.md#6-evidence)) | `interaction-tested` or `device-tested`, artifact in provenance | smoothness, frame pacing, the OS reduce-motion signal, other hardware |
| insets, back, native input, media controls | run on a named device, emulator or simulator | `device-tested` | other OS versions |

Captured but unopened images stay `screenshot-captured`. A desktop or JVM render is never
`device-tested`. A screenshot never establishes accessibility, motion or interaction. There is no
separate "motion verified" level: report the recording as provenance under the level it supports.

## Completion report

Emit the foundation block with these UI fields inserted before the residual-risk line. Every field
is required; write "not observed" rather than deleting one.

```text
Invariant:
Owner / boundary:
Evidence level:                  <from foundation § 6>
Evidence:                        test / property / tool → pass | fail
Command:                         <exact, reproducible>
Provenance:                      source revision, fixtures, live vs mocked, artifacts
Edge classes covered:
Screen(s) / flow / consumers:    e.g. Studio script editor (Web); Theatre full player (Web, CMP)
Component lookup / admission:    IDs/APIs/variants reused or extended; missing-kind rationale; scenario IDs and evidence; qualification or blocker
Surface intensity:               <intensity row> — checked on <which captures>
Renderer / target:               Leptos Web <browser, version> | CMP Android <device, API> | CMP iOS <device, OS>
Tokens:                          roles added/changed · source version · Web and CMP mapped? · contrast record
Localization:                    keys added/changed · vi-VN and en updated · literal scan result
Accessibility:                   accessibility-checked (what was walked) | screen-reader-walked (reader, platform, reviewer) | not checked
Visual:                          images opened: path · state · viewport · theme · locale · text scale · reviewer
Motion:                          recording/profile artifact and reduced-motion run | not observed
Findings / same-scenario recheck: ledger rows with their recheck result
Not covered / residual risk:
```

Report blocked evidence as blocked. A missing reference image, renderer, device or screen reader
belongs on the residual-risk line, never in silence.

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| hierarchy, the five levers, intensity per surface, Lavender Stage, dynamic color, shape morphing, emphasized type, springs, candidate M3 Expressive components | [`material3-expressive.md`](references/material3-expressive.md) |
| the token source, generated CSS and Kotlin, naming, versioning, contrast records, drift and contrast tests | [`tokens.md`](references/tokens.md) |
| type roles, Vietnamese diacritics, font coverage, line height, long names, tabular figures, reading measure, test strings | [`typography.md`](references/typography.md) |
| motion intents, springs, interruptibility, reduced motion, mini ↔ full player, recording evidence | [`motion.md`](references/motion.md) |
| names, focus, keyboard, drag alternatives, live regions, targets, Web vs CMP semantics, evidence levels | [`accessibility.md`](references/accessibility.md) |
| strings, keys, parameters, plurals, formatting, error presentation, the framework decision | [`localization.md`](references/localization.md) |
| how much text to show, redundant copy, helper text, tooltips, disclosures, popovers/sheets, first-use guidance, Web vs CMP help interactions | [`contextual-help.md`](references/contextual-help.md) |
| anatomy and state matrices: buttons, chips, navigation, fields, episode rows, player, job status, QC, dialogs, stale indicators | [`component-states.md`](references/component-states.md) |
| the screenshot review pass, Studio and Theatre checks, ledger, same-scenario recheck, evidence | [`visual-review.md`](references/visual-review.md) |
| registering a new component in both preview apps, named-token inspection, qualification before app adoption | [`preview-catalog.md`](references/preview-catalog.md) |

## Compose with

- [`cantos-engineering`](../cantos-engineering/SKILL.md) for every state, validation or async
  decision behind a screen; [`functional-core.md`](../cantos-engineering/references/functional-core.md)
  and [`types-as-proofs.md`](../cantos-engineering/references/types-as-proofs.md) for presentation
  mappings.
- Domain meaning: [`cantos-listening`](../cantos-listening/SKILL.md) (playback, progress,
  bookmarks, downloads), [`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md)
  (jobs, cost, stale clips, QC), [`cantos-publication`](../cantos-publication/SKILL.md)
  (approvals, publish gates), [`cantos-script-ir`](../cantos-script-ir/SKILL.md) (dialogue identity,
  Vietnamese text normalization).
- Renderers: [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) and
  [`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md). Audits:
  [`cantos-ui-inspector`](../cantos-ui-inspector/SKILL.md).
