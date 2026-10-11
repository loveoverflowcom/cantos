---
name: cantos-cmp-mobile
description: >-
  Builds and reviews the Cantos Theatre Kotlin Compose Multiplatform app for Android and iOS:
  feature-first CMP packages, one process-wide media session behind platform-native playback
  adapters, pure Kotlin reducers over immutable state, staged and verified offline downloads, a
  durable progress outbox, Material 3 Expressive theming from shared tokens and per-platform
  device evidence. Use for CMP, Compose, Kotlin, apps/cantos/mobile, mini or full player, background
  audio, lock-screen or notification controls, audio focus, interruptions, sleep timer,
  downloads, offline listening, TalkBack/VoiceOver or Android/iOS lifecycle work.
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
(the owner for mobile behavior), [`apps/cantos/mobile/README.md`](../../../apps/cantos/mobile/README.md),
[work item 050](../../../docs/work-plan/050-cmp-native-listening.md), the
[UI system](../../../docs/design/ui-system.md) and
[business rules 25–29](../../../docs/product/business-rules.md#listening-and-mobile). Link them;
when this skill and a document disagree, the document wins and the conflict is reported.

## Repository reality

- `apps/cantos/mobile/` (Theatre) and `apps/cantos-studio/mobile/` (reserved for a possible Studio
  client) each contain a README only. This skill's playback, download and progress rules are for
  Theatre; the two apps share no Kotlin module until code is actually common. There is no Gradle build, Xcode project, KMP module,
  test task, formatter or device runner. Discover `settings.gradle.kts`,
  `gradle/libs.versions.toml`, the Xcode project and CI workflows before citing any command; the
  commands in these references are **proposals** until the repository adds them.
- Every library named here (Media3, AVFoundation wrappers, Ktor, SQLDelight, Room KMP,
  WorkManager, kotest, Roborazzi …) is a **candidate**. Adopting one needs the ADR listed in
  [mobile.md § Implementation decisions](../../../docs/architecture/mobile.md#implementation-decisions-to-record),
  written with [`templates/adr.md`](../../../templates/adr.md), and a pinned version checked
  against official documentation **for every target** — Android support does not imply
  `commonMain` or iOS support.
- Platform audio, background and storage rules change between OS releases. Treat every platform
  API detail here as a question to verify against current docs and a real device, not as API
  reference.
- Never report a build, test, device run, screen-reader walkthrough or audio observation that did
  not happen. Linux hosts cannot produce iOS runtime evidence.

## Scope and mode

| The task | Use |
|---|---|
| CMP screen, Kotlin state holder, playback, downloads, progress outbox, native adapter, CMP theme | this skill + its owners above |
| Theatre Web or Studio (Leptos), CSS, browser media | [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) |
| listener endpoint, progress reconciliation rule, entitlement policy on the server | [`cantos-listening`](../cantos-listening/SKILL.md) + foundation |
| offline redistribution rights, release manifests, delivery URLs | [`cantos-publication`](../cantos-publication/SKILL.md) |
| one work-plan item or issue end to end (for example #14–#17) | [`cantos-work-item`](../cantos-work-item/SKILL.md), which loads this skill |
| review of a PR, range or diff | [`cantos-code-review`](../cantos-code-review/SKILL.md); these references supply criteria only, read-only |
| UI/UX audit with a findings report, no fixes | [`cantos-ui-inspector`](../cantos-ui-inspector/SKILL.md) |

Mobile Studio, car/wearable integrations, Rust business-code FFI and DRM are
[050 non-goals](../../../docs/work-plan/050-cmp-native-listening.md#non-goals). Flag a request
that drifts into them instead of silently implementing it.

## Non-negotiables on mobile

- **Decoupling.** `commonMain` application code depends on narrow ports, never on Media3,
  AVFoundation, Ktor engines, SQL drivers or Android/UIKit types. Adapters live in `androidMain`
  / `iosMain` or Swift and are injected at the composition root. The API client is generated from
  the contract; no handwritten divergent DTOs, no Rust memory layouts, no Studio or Narrative
  Forge types. → [cmp-architecture.md](references/cmp-architecture.md)
- **One media session owner per process.** Screens attach to it; navigation never creates or
  destroys an engine. No WebView audio, ever.
- **Immutability.** UI and application state are `val` values replaced through `copy`; a
  verified download record, an outbox operation and a playback checkpoint are written once and
  superseded, never edited in place. → [kotlin-conventions.md](references/kotlin-conventions.md)
- **Functional core.** Playback intent, interruption handling, sleep timer, resume prompt,
  download lifecycle and outbox scheduling are pure Kotlin reducers in `commonMain` returning
  effects as data; adapters interpret them.
  → [native-playback.md](references/native-playback.md),
  [downloads-and-local-state.md](references/downloads-and-local-state.md)
- **Types as proof barriers.** Value classes for `EpisodeId`/`PublicationId`; evidence types
  (`VerifiedDownload`) constructible only by the verifier; sealed errors with exhaustive `when`;
  no `!!`.
- **Material 3 Expressive.** Through generated tokens and one `CantosTheme`; expressive APIs only
  where the pinned CMP Material 3 artifact provides them on every target; dynamic color never
  replaces the lavender identity. → [cmp-design-system.md](references/cmp-design-system.md)
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
   For a decomposition, record the before/after ownership, dependency and lifetime graph first
   ([cmp-architecture.md § Decomposition record](references/cmp-architecture.md#decomposition-record)).
2. **Choose state and API ownership.** One mutable authority per fact, at its real lifetime:
   process (media session, download queue, outbox), account session, route, composable.
   Durable decisions go to a pure reducer; menu, focus and scroll state stay local. Server-owned
   policy (reconciliation, entitlement, authorization) stays on the server; Kotlin renders the
   outcome and keeps the local candidate. → [kotlin-conventions.md](references/kotlin-conventions.md)
3. **Set native design intent and baseline.** Apply
   [Material 3 Expressive](../cantos-ui-design/references/material3-expressive.md) at the
   intensity the surface deserves (player vs library vs settings). Trace token source →
   generated Kotlin → `CantosTheme` → mounted component before changing styling. Capture and open
   the current render in the scenario you will change, with Vietnamese content such as
   `Người dẫn chuyện` and `Ánh đèn cuối sân khấu`.
4. **Implement at the seam.** Route binds state and effects; screen takes values and `onX`
   events; sections stay dumb. Platform behavior goes behind the existing port, not into a
   composable. Define each effect's owner, keys, cleanup and stale-result rejection. Add a
   dependency only with a concrete consumer, a gap and verified target support.
5. **Test the changed claim.** Reducer and controller tests in `commonTest` with controlled
   clock and dispatcher; shared JSON vectors from `cantos-listening` for semantics that Web and
   mobile must agree on; Compose UI tests with a named renderer for semantics and interaction;
   device runs for background, lock screen, interruptions, downloads and process death.
   Zero selected tests or a missing iOS host is blocked. → [cmp-testing.md](references/cmp-testing.md)
6. **Open, judge and recheck.** Open the new captures, compare against the baseline using
   [visual review](../cantos-ui-design/references/visual-review.md), fix at the owning component
   or token, and rerun the *same* scenario. A desktop preview never stands in for Android or iOS.
7. **Deliver scoped evidence.** Report with the foundation block, the
   [UI completion report](../cantos-ui-design/SKILL.md#completion-report) and the additions
   below. List native gates you could not run on the residual-risk line.

## Native gates: what only a device can prove

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
| generated Kotlin theme, Material 3 Expressive in CMP, dynamic color, typography, touch targets, semantics, insets, back, reduced motion, resources | [`cmp-design-system.md`](references/cmp-design-system.md) |
| choosing CMP evidence, renderers, device runs, blocked gates, acceptance matrix, clocks and dispatchers | [`cmp-testing.md`](references/cmp-testing.md) |

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
Runner per target:      physical | emulator | simulator | JVM desktop | Robolectric — model, OS
UI-test renderer:       per Compose UI suite
Display:                dp × dp, density, fontScale or Dynamic Type size, orientation,
                        theme, locale, reduced motion, screen reader on/off
Media fixture:          publication id, format, live or fake delivery
Tests:                  selected / executed / skipped, per target
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
