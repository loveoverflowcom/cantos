---
name: cantos-cmp-mobile
description: >-
  Builds and reviews the Cantos Theatre Kotlin Compose Multiplatform app for Android and iOS:
  feature-first CMP packages, one process-wide media session behind platform-native playback
  adapters, validated domain types, sealed ADTs, pure reducers, explicit effects, structured
  concurrency and reproducible verification. Covers Library/outbox, future downloads and native acceptance
  across implemented native owners and the isolated JVM preview. Use for CMP, Compose, Kotlin, apps/cantos-theatre/mobile, mini or full player, background
  audio, lock-screen or notification controls, audio focus, interruptions, sleep timer,
  downloads, offline listening, TalkBack/VoiceOver, Android/iOS lifecycle work, emulator-free
  Compose preview capture, Android CLI render-compose-preview or PNG/semantics inspection.
---

# Cantos CMP mobile

This skill is the **behavioral contract and router** for Cantos Theatre on Android and iOS. It
owns the native working method: where Kotlin code lives, how playback and downloads are modeled,
how platform adapters are isolated and which evidence each claim needs. It composes three owners
and redefines none of them:

| Owner | Supplies | Never restated here |
|---|---|---|
| [`cantos-engineering`](../cantos-engineering/SKILL.md) | required order, decoupling, immutability, functional core, [evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified), [completion report](../cantos-engineering/SKILL.md#9-completion-report) | evidence levels, tiers, residual-risk rule |
| [`cantos-ui-design`](../cantos-ui-design/SKILL.md) | [shared UI loop](../cantos-ui-design/SKILL.md#shared-ui-loop), Material 3 Expressive, tokens, type, motion, accessibility, localization, visual review, [UI completion report](../cantos-ui-design/SKILL.md#completion-report) | design intent, contrast targets, copy policy |
| [`cantos-listening`](../cantos-listening/SKILL.md) | meaning of playback states, progress reconciliation, download entitlement, listener API | what a state means, which progress wins, when a download is allowed |

Product truth lives in [`docs/architecture/mobile.md`](../../../docs/architecture/mobile.md)
(the owner for mobile behavior), [`apps/cantos-theatre/mobile/README.md`](../../../apps/cantos-theatre/mobile/README.md),
[work item 050](../../../docs/work-plan/050-cmp-native-listening.md), the [UI system](../../../docs/design/ui-system.md) and
[business rules 25–29](../../../docs/product/business-rules.md#listening-and-mobile). Link them;
when this skill and a document disagree, the document wins and the conflict is reported.
For registered stateful scenarios, compose [cantos-behavior](../cantos-behavior/SKILL.md):
reuse catalog IDs and accepted `.qnt` expectations, keep platform-native tests and observations,
and never infer client/device acceptance from model simulation.

## Repository reality

- `apps/cantos-theatre/mobile/` separates production-safe UI/domain/application code and ports
  in `:shared` from mock adapters, fixtures, previews and diagnostic composition in the KMP
  JVM-only `:preview-sandbox` module. Only the Desktop Preview selects the sandbox; Android/iOS have no mock host.
  Shared composition has no implicit mock fallback. The [implementation map](../../../docs/architecture/repository-layout.md#platform-scope)
  and [mobile guide](../../../apps/cantos-theatre/mobile/README.md) own production catalog,
  native account/player and durable SQLDelight Library scope. The JVM preview uses the real
  Library store in memory and mocked external ports; it accepts no device/process-death behavior.
  The aggregate `--mobile` command remains `UNSUPPORTED`.
  The sole mobile product is Theatre; Studio is Web-only under the
  [platform matrix](../../../docs/architecture/repository-layout.md#platform-scope).
  Shared libraries may serve Theatre Mobile and Studio Web.
- The one Android application `androidApp` (debug `com.cantos.theatre.dev`, release `com.cantos.theatre`; no flavors) composes `:shared` only and mounts `CantosTheatreApp` (the one `CantosPresentation`
  over live catalog/account/Library/player; `CantosListeningUnavailable` without an origin; never a mock). Rerun `cargo xtask theatre-host android-release` ([testing](references/cmp-testing.md), [#122 record](../../../docs/evidence/issue122-cmp-sandbox-isolation.md)).
  A demo APK, debug flag or R8 removal cannot accept production exclusion.
  Use `Mock` for Cantos-owned adapters/fixtures; retain third-party names such as Okio
  `FakeFileSystem`. Add no directory, SQLite, file-system or HTTP mock dependency without a consumer.
- Motion today: the pure `MotionPolicy.kt` (intent × property × preference, token-fed) in the
  shared CMP package, adopted by the Auth action press and the Theatre launch entrance and replayed
  on a manual clock in tests. There is no mini ↔ full player, sheet, dialog, drag or seek-thumb
  consumer, and no Android/iOS reduce-motion adapter; the pinned Material3 1.8.2 has no
  `MotionScheme`. Method and labels: [motion.md](../cantos-ui-design/references/motion.md).
- Discover commands from the [mock guide](../../../apps/cantos-theatre/mobile/README.md#local-commands)
  and [xtask](../../../xtask/README.md#theatre-preview). `theatre-preview verify` runs common/desktop
  tests in both modules (the sandbox is JVM-desktop only; there is no Android or iOS mock host). Host
  tests establish no native runtime evidence. Only `ui-system-cmp` has Android preview wrappers
  (Theatre has none, #287); capture additionally needs CLI/Quail 2+ readiness. No native runtime test gate exists.
- Inspect actual Gradle pins and platform adapters before proposing a dependency; for example,
  SQLDelight is already adopted for Library storage. An unadopted library (Media3, Ktor, Room KMP,
  WorkManager, Kotest, Roborazzi …) remains a **candidate**. New adoption needs the ADR listed in
  [mobile.md § Implementation decisions](../../../docs/architecture/mobile.md#implementation-decisions-to-record),
  written with [`templates/adr.md`](../../../templates/adr.md), and a pinned version checked
  against official documentation **for every target** — Android support does not imply
  `commonMain` or iOS support.
- Platform audio, background and storage rules change between OS releases: verify every platform
  API detail here against current docs and a real device. Never report a build, test, device run,
  screen-reader walkthrough or audio observation that did not happen; Linux cannot produce iOS evidence.

## Scope and mode

The separate [`packages/ui-system/cmp`](../../../packages/ui-system/cmp/README.md) Gradle package
has a user-requested JVM desktop component preview, shared-token mapping and an isolated Android
library host with thin `@Preview` wrappers around the actual shared components and fixtures.
`cargo xtask ui-preview cmp android-build` compiles that library; `cargo xtask compose-preview
ui-system-cmp` captures an existing wrapper through Android Studio. The [PR43 Android host record](../../../docs/evidence/cmp-pr43-android-preview.md) records an actual
Android build and four inspected captures with asserted static semantics. These consumers produce
no mobile APK or product app and verify no Android/iOS device behavior. Qualification is only what `qualifications` in the
[preview catalog](../../../packages/ui-system/shared/preview-catalog.json) records and the
repository gate keeps fresh; every other API is unqualified, and a change to the CMP package
invalidates those records until they are re-run and re-reviewed. The gallery also holds a
read-only tab for the [design inventory](../../../packages/ui-system/shared/design-inventory-v3.json):
proposals are not public APIs. New shared components must be declared/tested in both previews
and follow [preview admission](../cantos-ui-design/references/preview-catalog.md) before real app
adoption; appearance, density, font scale and motion reach components as injected values
([CMP design system](references/cmp-design-system.md#injected-preferences-and-scope)).
The [gallery recipe contract](../../../packages/ui-system/cmp/README.md#scenario-recipes-and-replay)
and [Theatre desktop workbench](../../../apps/cantos-theatre/mobile/README.md#scenario-workbench)
own selection, reset, intent replay and synthetic snapshots. The Theatre workbench stays in
`preview-sandbox/src/desktopMain`; its move preserves the existing replay and ownership semantics.
[#122](https://github.com/loveoverflowcom/cantos/issues/122)'s production host release graph and
APK/AAB exclusion are re-proved by `theatre-host android-release`, not by any demo build.

| The task | Use |
|---|---|
| CMP screen, Kotlin state holder, playback, downloads, progress outbox, native adapter, CMP theme | this skill + its owners above |
| Native login/restore, opaque Bearer coordinator, secure storage, Web email verification and return | [native auth owner](../../../apps/cantos-theatre/mobile/README.md#opt-in-native-identity) + foundation [identity/session method](../cantos-engineering/references/identity-and-session-boundaries.md), starting with its v1 amendment; reuse the existing session engine and preserve account/late-result fences |
| Theatre Web or Studio (Leptos), CSS, browser media | [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) |
| listener endpoint, progress reconciliation rule, entitlement policy on the server | [`cantos-listening`](../cantos-listening/SKILL.md) + foundation |
| offline redistribution rights, release manifests, delivery URLs | [`cantos-publication`](../cantos-publication/SKILL.md) |
| one work-plan item or issue end to end (for example #14–#17) | [`cantos-work-item`](../cantos-work-item/SKILL.md), which loads this skill |
| review of a PR, range or diff | [`cantos-code-review`](../cantos-code-review/SKILL.md); these references supply criteria only, read-only |
| UI/UX audit with a findings report, no fixes | [`cantos-ui-inspector`](../cantos-ui-inspector/SKILL.md) |

Studio Mobile is excluded by the platform matrix. Car/wearable integrations, Rust business-code FFI and DRM are
[050 non-goals](../../../docs/work-plan/050-cmp-native-listening.md#non-goals). Flag a request
that drifts into them instead of silently implementing it.

## Non-negotiables on mobile
For Kotlin/domain/controller changes, **MUST** load
[Kotlin rules](references/kotlin-conventions.md) (TYPE-01–05, FP-01–04, KT ownership/lifetimes),
[architecture](references/cmp-architecture.md) (ARC boundaries, SOLID) and
[testing](references/cmp-testing.md) (VER-01–08). They are the canonical Kotlin definitions;
legacy KT anchors route to them. For dependency decisions also load
[tooling evaluation](references/cmp-tooling.md). Review mode uses the same references read-only.
For fallible flow, also load the foundation's
[typed errors and composition](../cantos-engineering/references/typed-errors-and-composition.md);
Kotlin rules below apply its taxonomy, containment and cancellation policy.

- **Decoupling.** `commonMain` application code depends on narrow ports, never on Media3,
  AVFoundation, Ktor engines, SQL drivers or Android/UIKit types. Adapters live in `androidMain`
  / `iosMain` or Swift and are injected at the composition root. The API client is generated from
  the contract; no handwritten divergent DTOs, no Rust memory layouts, no Studio or Narrative
  Forge types. → [cmp-architecture.md](references/cmp-architecture.md)
- **One media session owner per process.** Screens attach to it; navigation never creates or
  destroys an engine. No WebView audio, ever.
- **Immutability.** UI/application state uses invariant-preserving value replacement; `copy`
  is not automatically safe. Future accepted records such as a
  verified download record, an outbox operation and a playback checkpoint are written once and
  superseded, never edited in place. → [kotlin-conventions.md](references/kotlin-conventions.md)
- **Functional core.** Playback intent, interruption handling, sleep timer, resume prompt,
  download lifecycle and outbox scheduling are pure Kotlin reducers in `commonMain` returning
  effects as data; adapters interpret them.
  → [native-playback.md](references/native-playback.md),
  [downloads-and-local-state.md](references/downloads-and-local-state.md)
- **Types as proof barriers.** Distinct meaningful identities, validated factories/aggregates,
  closed ADTs and typed errors; audit constructor/copy/decoding bypasses. Preserve current
  transactional playback rejection and vectors; compose typed failures without losing
  refinements. Arrow Either/Raise remains an evaluated candidate, subject to target checks and
  an adoption ADR, with no dependency added for syntax alone. No native refinement-proof claim.
- **Material 3 Expressive.** Through generated tokens and one `CantosTheme`; expressive APIs only
  where the pinned CMP Material 3 artifact provides them on every target; dynamic color never
  replaces the lavender identity. → [cmp-design-system.md](references/cmp-design-system.md)
- **CMP localization before Composables.** Before adding a label, accessible name, state or
  dialog, search shared `presentation/model/ShellCopy.kt`, `AuthCopy.kt`, `MembershipCopy.kt`
  and `composeResources/values/strings.xml`; sandbox-only diagnostics use injected
  `DemoShellCopyOverrides` in `:preview-sandbox`. Reuse keys within the actual owner; update both
  languages and focused parity tests, not a new `*Copy` enum or hard-coded `Text("...")`. Launch
  and unavailable presentations use explicit `stringResource` IDs selected from the same
  app-owned locale projection; native preference adapters/evidence are in the
  [#169 ledger](../../../docs/evidence/issue169-cmp-locale-acceptance.md). Compare Compose
  Resources, moko-resources and Lyricist in [#169](https://github.com/loveoverflowcom/cantos/issues/169)
  before adoption under [#167](https://github.com/loveoverflowcom/cantos/issues/167). Broader catalog/formatting and device-negotiation policy remain open; the bounded saved
  preference rule does not select a localization library or domain-cache store. Keep mock preview
  state injectable; a demo build never accepts #122's production APK/AAB exclusion. Do not
  translate creator content or clear playback on locale changes.
- **Honest evidence.** Cheapest deterministic test in `commonTest` first; OS behavior needs a
  named device, emulator or simulator; a missing environment is *blocked*, never *passed*.
  → [cmp-testing.md](references/cmp-testing.md)

## Production development loop

Follow the [shared UI loop](../cantos-ui-design/SKILL.md#shared-ui-loop) with these native
steps. Skipping a step is a decision you state, as the foundation requires.

1. **Resolve the screen and contract.** Name the feature, production entrypoint (route, screen,
   controller), source sets and targets, and the listening contract it consumes
   ([`listener-api.md`](../cantos-listening/references/listener-api.md),
   [`playback-semantics.md`](../cantos-listening/references/playback-semantics.md)). Read the
   actual code and call sites. Write the invariant and failure mode in one sentence and seed the
   ledger from the [mobile acceptance matrix](../../../docs/architecture/mobile.md#future-acceptance-matrix).
   Pin the actual mock/production boundary, toolchain and relevant TYPE/FP/ARC/VER IDs;
   record rule → enforcement/test and remaining gaps. For a decomposition, record the before/after
   ownership, dependency and lifetime graph first
   ([cmp-architecture.md § Decomposition record](references/cmp-architecture.md#decomposition-record)).
   **Look up before writing a control:** search the public APIs, the preview catalog and the
   [design inventory](../../../packages/ui-system/shared/design-inventory-v3.json) (also the
   gallery's Design v3 tab) per the [component-first lookup](../cantos-ui-design/references/preview-catalog.md#lookup-before-product-markup),
   and record the IDs/variants/states reused, or why none fits. Today the Theatre mock's
   `features/shell/presentation` screens compose their own Material controls over the shared theme
   and tokens, while Membership already composes the shared Button, TextField, Surface, Badge,
   BackButton and Logo. Moving a shell control onto a shared composition needs declaration,
   preview and admission first, and never a copied button, field, card or palette in a feature.
2. **Choose state and API ownership.** One mutable authority per fact, at its real lifetime:
   process (media session, download queue, outbox), account session, route, composable.
   Durable decisions go to a pure reducer; menu, focus and scroll state stay local. Server-owned
   policy (reconciliation, entitlement, authorization) stays on the server; Kotlin renders the
   outcome and keeps the local candidate. → [kotlin-conventions.md](references/kotlin-conventions.md)
3. **Set native design intent and baseline.** Apply
   [Material 3 Expressive](../cantos-ui-design/references/material3-expressive.md) at the
   intensity the surface deserves (player vs library vs settings). Trace token source →
   generated Kotlin → `CantosTheme` → mounted component before changing styling. Capture and open
   the current render in the scenario you will change. For static Android host evidence, prefer
   [Compose preview without an emulator](references/compose-preview.md) when its prerequisites
   exist; keep native runtime gates separate. Use Vietnamese content such as
   `Người dẫn chuyện` and `Ánh đèn cuối sân khấu`. For interactive diagnostics, select a validated
   recipe, restore its bounded snapshot and record the seed, injected appearance, constraint frame
   and intent sequence using the [preview method](../cantos-ui-design/references/preview-catalog.md#replay-a-preview-scenario).
4. **Implement at the seam.** Route binds state and effects; screen takes values and `onX`
   events; sections stay dumb. Platform behavior goes behind the existing port, not into a
   composable. Define each effect's owner, keys, cleanup and stale-result rejection. Add a
   dependency only with a concrete consumer, a gap and verified target support. Reset-to-selected
   and replay recreate the recipe's mount; keep owned cleanup separate from borrowed injection,
   fence old callbacks and preserve editing/scroll state when only filtering or appearance changes.
5. **Test the changed claim.** Reducer and controller tests in `commonTest` with controlled
   clock and dispatcher; shared JSON vectors from `cantos-listening` for semantics that Web and
   mobile must agree on; Compose UI tests with a named renderer for semantics and interaction;
   device runs for background, lock screen, interruptions, downloads and process death.
   Test recipe/snapshot rejection and A→B→A/reset/dispose during delayed effects with virtual
   time; memory recreation is not process-death evidence. Zero selected tests or a missing iOS
   host is blocked. → [cmp-testing.md](references/cmp-testing.md)
6. **Open, judge and recheck.** Open the new captures, compare against the baseline using
   [visual review](../cantos-ui-design/references/visual-review.md), fix at the owning component
   or token, and rerun the *same* scenario. A desktop preview never stands in for Android or iOS.
7. **Deliver scoped evidence.** Report with the foundation block, the
   [UI completion report](../cantos-ui-design/SKILL.md#completion-report) and the additions
   below. List native gates you could not run on the residual-risk line.

## Native gates: what only a device can prove

Keep OS launcher icons, native splash/LaunchScreen and CMP brand components separate. Reuse the
checksum-pinned official Logo Kit through the existing generator; inspect source artwork before
changing native wrappers. Resource/asset-catalog checks and APK inspection establish packaging
only; launcher acceptance needs opened Home/drawer-or-App-Library/Settings captures with OS, mask,
density, theme, source hash and installed build identity ([native icon review](../cantos-ui-design/references/visual-review.md#native-launcher-icons)).

| Claim | Cheapest deterministic evidence | Native evidence still required | Blocked when |
|---|---|---|---|
| interruption, focus loss or headset removal never becomes "user playing" or an unexpected resume | `example-tested` reducer table over interruption × intent sequences | `device-tested` per platform; call/route cases usually need physical hardware | no device with the needed audio route |
| playback continues locked/backgrounded with correct OS controls and metadata | `example-tested` metadata projection from the active publication | `device-tested` with a recording, secrets redacted | background capability not configured or no device |
| cold launch offers resume, never autoplays | `example-tested` restore reducer from a persisted checkpoint | `device-tested` after an OS-initiated process kill | cannot kill/restore on the target |
| a partial download is never offered as complete | `fault-injected` staging protocol over a fake file system at each crash point | `device-tested` process death and airplane mode mid-transfer | no device storage/network control |
| offline progress never silently overwrites newer progress | `differentially-tested` against shared vectors; outbox `property-tested` | `integration-tested` against the listener API, then two devices plus Theatre Web | no server build exposing the contract |
| screen reader can operate the player | `semantics-tested` on the production player (renderer named) | `screen-reader-walked` with TalkBack and VoiceOver | no device or simulator with the reader |

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| package layout, shared application layer, ports, expect/actual, the single media session, generated client, decomposition records | [`cmp-architecture.md`](references/cmp-architecture.md) |
| component API, state ownership, immutable values, `StateFlow.update`, effects and lifetimes, sealed types, errors, coroutines, formatting | [`kotlin-conventions.md`](references/kotlin-conventions.md) |
| playback port, pure reducer, Android/iOS media adapters, interruptions, sleep timer, restore, URL refresh, lock-screen metadata | [`native-playback.md`](references/native-playback.md) |
| download staging and verification, local metadata store, account scoping, revocation, progress outbox, background execution | [`downloads-and-local-state.md`](references/downloads-and-local-state.md) |
| generated Kotlin theme, Material 3 Expressive in CMP, dynamic color, typography, touch targets, semantics, tooltips/long press/contextual help, insets, back, reduced motion, resources | [`cmp-design-system.md`](references/cmp-design-system.md) |
| choosing CMP evidence, renderers, device runs, blocked gates, acceptance matrix, clocks and dispatchers | [`cmp-testing.md`](references/cmp-testing.md) |
| Arrow, Kotest Property, persistent collections, Konform, Turbine, Lincheck, Detekt/ktlint/Compose Rules compatibility and adoption cost | [`cmp-tooling.md`](references/cmp-tooling.md) |
| Android CLI/Quail 2+ setup, emulator-free PNG and semantics capture, preview inspection, JVM alternative | [`compose-preview.md`](references/compose-preview.md) |

Load foundation references through the foundation router; the ones mobile work reaches most are
[`decoupling.md`](../cantos-engineering/references/decoupling.md),
[`functional-core.md`](../cantos-engineering/references/functional-core.md),
[`immutability.md`](../cantos-engineering/references/immutability.md),
[`types-as-proofs.md`](../cantos-engineering/references/types-as-proofs.md) and
[`property-and-differential-testing.md`](../cantos-engineering/references/property-and-differential-testing.md).

## Evidence report additions

Use the foundation report and the [UI completion report](../cantos-ui-design/SKILL.md#completion-report)
unchanged, then add the fields that make a native judgment reproducible:

```text
Production entrypoint / callers:
Build:                  source revision + working-tree state, variant, pinned Kotlin/CMP versions
Targets / source sets:  touched and executed (commonMain, androidMain, iosMain, commonTest, …)
Runner per target:      physical | emulator | simulator | JVM desktop | Robolectric | Studio layout renderer — model/OS or CLI/IDE versions
UI-test renderer:       per Compose UI suite
Display:                dp × dp (the real constraint, never inferred from PNG pixels), density,
                        fontScale or Dynamic Type size, orientation, theme mode and resolved
                        theme, locale, reduced motion, screen reader on/off
Component lookup:       catalog/inventory IDs, variants and states reused or extended; the
                        missing-kind rationale; scenario IDs; qualification or blocker
Recipe / replay:         schema/version, seed, snapshot/import result, ordered intents,
                        owned or injected mount and cleanup owner; simulated clock/faults
Capture record:         JVM sidecar path and png_sha256 | official capture.json (per renderer)
Media fixture:          publication id, format, live or fake delivery
Tests:                  selected / executed / skipped, per target
Isolation:              shared/sandbox source and Gradle graph checks; production release
                        compile/runtime classpaths and actual APK/AAB inspection, or blocker
Native gates:           selected / executed / blocked (reason)
Secrets check:          logs, checkpoints and saved state scanned for tokens and signed URLs
```

A JVM desktop or Robolectric result is never `device-tested`. An Apple framework that compiled
or linked is `compiled`, not launched. Captured but unopened images are `screenshot-captured`.

## Compose with

[`cantos-engineering`](../cantos-engineering/SKILL.md) for every Kotlin decision and the report;
[`cantos-ui-design`](../cantos-ui-design/SKILL.md) for any visible change;
[`cantos-listening`](../cantos-listening/SKILL.md) whenever playback meaning, progress, bookmarks
or downloads change; [`cantos-publication`](../cantos-publication/SKILL.md) for offline rights and
delivery. A shared token or contract change also needs current Web evidence through
[`cantos-leptos-web`](../cantos-leptos-web/SKILL.md). Inspection scenarios for the Theatre player
live with [`cantos-ui-inspector`](../cantos-ui-inspector/SKILL.md).
