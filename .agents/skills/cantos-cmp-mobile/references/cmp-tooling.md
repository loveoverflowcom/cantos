# CMP dependency and verification-tool evaluation

> **Scope.** Read before proposing Kotlin libraries or static/concurrency/property tooling.
> Investigation snapshot: 2026-10-13. No candidate below is adopted by this documentation change.

## TOOL-01 — solve a concrete gap without moving the toolchain

**MUST** for a proposed dependency/tool: record the consumer/problem, existing alternative,
exact artifact/version, compiler/plugin/stdlib compatibility, common/JVM/Android/iOS device/
simulator targets, Native restrictions, licence, maintenance risk and verification cost.
Good Kotlin: retain `PlaybackEffect.Rejected(reason)` when it already expresses the outcome.
Counterexample: rewrite `reducePlayback` as `either { ... }` solely to introduce Arrow.
**Verification:** published metadata/release docs plus actual compile/link/tests at the repository
pins; source inspection is not executed compatibility. A toolchain change needs an explicitly
justified ADR. **Exception:** JVM-only test tools may stay in desktopTest if no Native claim is
made. Never install the entire candidate list or add a generic MVI framework.

## Baseline actually inspected

[Theatre settings](../../../../apps/cantos-theatre/mobile/settings.gradle.kts) and
[shared build](../../../../apps/cantos-theatre/mobile/shared/build.gradle.kts) pin Kotlin **2.2.20**,
Compose **1.9.0**, Material3 **1.8.2**, coroutines core/test **1.8.1**, test serialization JSON
**1.7.3**. Gradle **8.14.3**, Java bytecode **17**, JDK **17+** and AGP **8.11.1** are recorded in
the [mock guide](../../../../apps/cantos-theatre/mobile/README.md#local-commands).
Desktop is always registered; Android and Native targets are explicit opt-ins. Native requires
an already installed exact Kotlin/Native 2.2.20 compiler; no implicit provisioning.

## Candidate fit and cost

Licences below describe the upstream projects inspected, not a Cantos redistribution decision.
Recheck the chosen release, transitive licences and maintenance before adoption.

| Candidate / primary source | Concrete gap; existing alternative | Licence / maintenance consideration | Verification cost and current decision |
|---|---|---|---|
| [Arrow Core / Raise](https://arrow-kt.io/learn/typed-errors/); [project](https://github.com/arrow-kt/arrow) | Compose multiple typed failures/validation; existing sealed outcomes and rejection effects already cover playback | Apache-2.0; check chosen release/API migration; avoid compiler-plugin/optics adoption by association | Moderate: boundary/cancellation/transactional behavior plus all-target compile; defer without a consumer needing composition |
| [Kotest Property](https://kotest.io/docs/quickstart/); [project](https://github.com/kotest/kotest) | Generated event traces, shrinking/replay; existing kotlin.test tables and corpus | Apache-2.0; property artifact and framework/plugin have different integration costs | Moderate: generators, independent laws, shrinking/seed replay, all-target runner discovery; promising for VER-03, no framework replacement required |
| [kotlinx.collections.immutable](https://github.com/Kotlin/kotlinx.collections.immutable) | Writable alias prevention/structural sharing for large state; existing detached lists and immutable elements | Apache-2.0; upstream labels API experimental | Low/moderate: alias/preservation tests, performance only if claimed; current small effect snapshots need no change |
| [Konform](https://www.konform.io/); [project](https://github.com/konform-kt/konform) | Accumulated field/path validation; existing factories and sealed validation outcomes | MIT; small specialist library, review release cadence and ownership | Moderate: invalid/relational inputs and construction closure; validation DSL cannot prevent later copying/forgery, defer |
| [Turbine](https://github.com/cashapp/turbine) | Readable Flow emission/cancellation assertions; existing runTest and scripted fake observations | Apache-2.0; upstream warns of unstable coroutines-test dispatcher dependency | Low/moderate: scheduler/timeout/cancellation and unconsumed-event behavior; add only for actual Flow assertion pain |
| [Lincheck](https://github.com/JetBrains/lincheck); [KMP limits](https://kotlinlang.org/docs/lincheck-kmp.html) | A concrete JVM shared-memory race/linearizability question; current serialized-owner deterministic tests | MPL-2.0; review 3.x package/artifact migration and instrumentation support | High: independent sequential specification, bounds and instrumentation/JDK; optional desktopTest only, no current need established |
| [Detekt](https://detekt.dev/docs/introduction/compatibility/); [project](https://github.com/detekt/detekt) | Forbidden imports/casts/complexity review automation; current compiler and manual review | Apache-2.0; compiler-analysis API and rule-plugin versions are tightly coupled | Moderate/high: small rule set, per-source-set resolution, violations/false positives and xtask gate; no adoption in this task |
| [ktlint](https://github.com/ktlint/ktlint), [Compose Rules](https://mrmans0n.github.io/compose-rules/rules/) / [project](https://github.com/mrmans0n/compose-rules) | Mechanical Kotlin style and specific Compose footguns; manual KT-API/STYLE review | ktlint MIT, Compose Rules Apache-2.0; rule-engine/parser pairing needs a pinned compatibility check | Moderate: existing-source sample, generated exclusions and explicit opt-ins; one formatter, no duplicate Compose rule engines |

## Target/compiler assessment

This table records upstream capability and unresolved checks, **not passing compatibility**.
All candidate compile/link/runtime probes against this repository are **NOT_RUN**: this is a
skills-only change and it alters no dependencies. "KMP" alone does not certify a particular
published iOS slice or compiler version.

| Candidate | commonMain / JVM / Android / iOS | Pinned Kotlin 2.2.20 and Native constraints to resolve |
|---|---|---|
| Arrow Core/Raise | Upstream KMP runtime library, usable from common; JVM/Android/Native artifacts must be checked for the chosen release | Select a release with compatible Kotlin metadata/stdlib and coroutines; verify iOS device/simulator KLIBs. Raise extension receivers do not require adopting experimental context parameters. No Swift ABI guarantee inferred. |
| Kotest Property | KMP test library, belongs in commonTest rather than production commonMain; JVM/Android/iOS execution depends on artifact and runner | Inspect chosen property artifact separately from framework plugin. [6.0 release docs](https://kotest.io/docs/6.0/release6/) mention Kotlin 2.2 integration, not proof of this Gradle/Native setup. [Seed/shrink configuration](https://kotest.io/docs/proptest/property-test-permutations.html) is versioned API guidance; do not copy newer DSLs into an older selected release. |
| Immutable collections | Common runtime interfaces; upstream lists JVM and Kotlin/Native, Android uses JVM artifacts | Inspected upstream README requires stdlib at least 2.3.0; do not apply that current artifact to pins blindly. Evaluate an older explicitly compatible release or defer; Native KLIB compatibility still needs linking. |
| Konform | Common runtime DSL; upstream lists JVM/JS/Native/Wasm, Android uses JVM; exact iOS slices need metadata check | Documentation advertises 0.11.0, not a certified compiler matrix. Kotlin metadata/stdlib floor and Native 2.2.20 link remain unresolved; no JVM reflection validator in common. |
| Turbine | KMP test utility, commonTest/JVM/Android/Native support must be checked at the selected tag | README advertises 1.2.1; verify compiler metadata, iOS slices and its coroutines-test dependency against pinned 1.8.1. Its UnconfinedTestDispatcher reliance means scheduler compatibility needs a test, not an assumption. |
| Lincheck | JVM host tests only; no commonMain, Android-device or iOS/Native verification | Verify selected version's JDK/instrumentation and Kotlin-generated bytecode on desktopTest. [Model checking assumes sequential consistency](https://kotlinlang.org/docs/lincheck-testing-strategies.html); no relaxed-memory or Native proof. |
| Detekt | JVM-host source analyzer; can inspect common/Android Kotlin, not an iOS runtime library | Official table lists 2.0.0-alpha.1 built with Kotlin 2.2.20, Gradle 9.1/AGP 8.13, unlike this repo's 8.14.3/8.11.1; 1.23.8 lists Kotlin 2.0.21. Neither is a verified drop-in. Native type-resolution limitations must be declared. |
| ktlint + Compose Rules | JVM-host source lint for common/platform Kotlin; no runtime artifact or native behavioral guarantee | Check parser support for Kotlin 2.2 syntax and exact engine/rule-plugin pair. Prefer non-overlapping rules; Compose Rules' Detekt-only/type-analysis rules need the corresponding engine. Native/Swift interop and OS behavior remain outside lint. |

## Adoption record

Keep this in the implementation PR/ADR, not a second dependency policy:

```text
Consumer / rule / concrete gap:
Existing alternative and why insufficient:
Artifact + exact version / source tag / licence / maintenance:
Kotlin / stdlib / Compose / Gradle / AGP / coroutines compatibility:
common / JVM / Android / iOS device / iOS simulator metadata:
Native/Swift limitations:
Commands and per-target compile/link/test counts:
Cost / exception / rollback / owner:
```
