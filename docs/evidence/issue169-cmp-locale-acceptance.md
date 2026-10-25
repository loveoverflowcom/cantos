# CMP locale repair and acceptance ledger — issue 169

Status: bounded repair and comparison; **issue 169 remains open**. The library/catalog
adoption decision in [#167](https://github.com/loveoverflowcom/cantos/issues/167) is OPEN.
The [candidate spike](cmp-localization-spike.md) executes Compose Resources, moko-resources
and Lyricist on the JVM without adding any of them to the application's dependencies.
This record distinguishes that experiment from application and native acceptance.

## Invariant, owner and repair

Invariant: one application preference selects every existing UI-copy path without changing
content or recreating playback, navigation, account, listening or active text/focus owners.
A pre-acceptance preference-store refusal keeps the selected locale and exposes localized,
retryable feedback; platform API acceptance does not prove a disk flush.

Owner/boundary: `UiLocale` is a plain closed value; `PreferencesController` owns the writable
application preference. Its pure reducer has no platform dependency. `LocalePreferenceStore`
is the actual key/value boundary, implemented by Android SharedPreferences, iOS UserDefaults
and an explicitly injected sandbox memory double. The presentation `ShellLocale` remains a
read-only projection; the common app mapper is exhaustive. Source content and SQLDelight/domain
storage remain outside this preference boundary. Shared UI-system primitives receive labels
and acquire no application-copy dependency. Production stays separate from `:preview-sandbox`.

Findings and changes:

- Missing persistence: valid saved canonical tag > explicit injected host/preview seed >
  Vietnamese default. Neither device negotiation nor a cross-renderer precedence policy is
  silently adopted. Android/iOS demo hosts explicitly inject their own native key/value store; the production
  Android unavailable host independently reads its app sandbox’s preference;
  ordinary previews/tests remain memory-only unless a store is injected. Theme stays memory-only.
- Split resource locale: five launch/unavailable translations now have explicit generated XML
  identities selected by the application locale. Compose 1.9.0's environment constructor and
  subtree provider are internal; this bounded adapter uses public resource IDs, without
  internal-visibility suppression, process-locale mutation or locale-keyed remounting. It is
  not a broad catalog conversion. The production unavailable host uses the same saved-locale rule in its own app sandbox,
  defaults to Vietnamese and has no language selector or listening adapter.
- English `1 episodes`, `1 works`, `1 minutes`: optional one-form resources fix the existing
  integer quantity paths, including sandbox “sample” wording. Vietnamese retains its other
  form. Decimal text `1.0` is not reclassified as integer one. This is not a general CLDR engine.
- Ignored save failures: the application exposes the last locale outcome separately from its
  selected preferences. A pre-acceptance refusal renders bilingual polite live-region feedback;
  selecting the language again retries and clears feedback on acceptance. Pure controller and
  actual JVM input tests cover this path.
- Native persistence API review: Android `apply()` and iOS `setObject()` accept into their
  platform cache and schedule persistence. No synchronous disk flush blocks a language change,
  and no transactional disk rollback is claimed. iOS's UserDefaults privacy manifest declares
  own-app settings reason CA92.1 and is wired into Xcode resources.

[Android's API](https://developer.android.com/reference/android/content/SharedPreferences.Editor#apply())
provides no disk-error callback for `apply()`.
[Apple UserDefaults](https://developer.apple.com/documentation/foundation/userdefaults)
updates memory immediately and writes asynchronously;
[`synchronize()`](https://developer.apple.com/documentation/foundation/userdefaults/synchronize())
is unnecessary and is not used. A later disk failure is outside the synchronous port result.
Controller rejection tests concern pre-acceptance refusal by the named mock, not native disk
rollback. Whole-app diagnostic reset retains its existing non-atomic scope.

## Acceptance checklist mapped to issue 169

| Issue requirement | Delivered scope | Remaining acceptance |
| --- | --- | --- |
| Three candidate fixture renders, setup/generated size, Android/iOS/preview/translator comparison | Actual JVM renders, exact 12 rows each, six inspected captures, generated-source inventory, primary-source comparison, retained reproducible inputs | Candidate Android/iOS link/runtime, native preview, KSP, translator study and release-artifact footprint |
| Decision before wide conversion | No adopted library/dependency or wide migration; #167 OPEN | Reviewed renderer-neutral ADR, selected library/version and migration/rollback decision |
| One application locale | Existing shell/auth/Membership projections plus explicit launch/unavailable XML selection | Future screens and production locale selector must use the same owner |
| Saved key/value preference and isolated mock | Native adapters, explicit host injection, saved-tag precedence and sandbox-only memory double | Target-specific reopen evidence below; asynchronous disk loss/corruption beyond the deterministic mock |
| Reactive switch preserves owners, content, input, caret and focus | Core/sandbox state regressions; actual JVM workbench selection/focus and unchanged owner snapshots | Pending real-account flow, IME and iOS-native input/focus; no OS process-death recovery inference |
| Typed error/reason mapping and previous copy invariants | Exact all auth/Membership mappings; interpolation/speed/time/parity regressions retained | General typed parameter API awaits #167 |
| CLDR/formatting and literal content | Bounded vi/en integer one/other, decimal distinction, speed/time and non-recursive literal title tests | Full CLDR formats, regional/fractional policy, dates/numbers beyond current consumers |
| Content unchanged; language accessibility spans | Literal Vietnamese titles preserved; content/state snapshots unchanged | Mixed-language native accessibility spans need content-language metadata and target observations |
| Light/dark, large font, long Vietnamese, pseudo-locale and semantics | Existing real-renderer layout/copy matrices and focused semantics, as recorded below | Pseudo-locale and TalkBack/VoiceOver remain unrun |
| Shared primitives independent of app copy | No shared UI-system source/dependency changes | Broader component admission is separate |
| Screen/namespace migration without unrelated refactor | Five existing XML strings and three count paths; no framework migration or playback/storage refactor | Reviewed staged catalog migration after ADR |
| Milestone verification | Red regression, coherent CMP checkpoint, native commands and final preflight | Native/candidate gaps retain separate gates |

## Executed evidence and provenance

Baseline: `develop` commit `2e3f29a120d2c5ab5962b8c4bf5a2dc8c92c30cb`.
Worktree: `/Users/manh.pd1/.codex/worktrees/issue-169-acceptance/cantos`, branch
`codex/issue-169-locale-acceptance`. Synthetic catalog and named mock ports are used in JVM
application tests; no API, identity provider, native audio or database is exercised.

Toolchain: Rust 1.98.1; Kotlin 2.2.20; Compose 1.9.0; Material3 1.8.2; installed Gradle 8.14.3;
Microsoft OpenJDK 17.0.19 and Homebrew Python 3.14.8 on macOS arm64. Commands below ran from the worktree root with that
Gradle `bin` on PATH, `RUSTUP_TOOLCHAIN=1.98.1` and the shared local Cargo target cache.
Final preflight additionally put `/opt/homebrew/bin` first on PATH for Python 3.14.8
with the required macOS `os.waitid`/`WNOWAIT` capability.
Full command logs remain in ignored `artifacts/agent-runs/issue169/`.

| Command / layer | Actual result | Evidence scope |
| --- | --- | --- |
| `cargo xtask theatre-demo verify`, before count repair | FAIL: 145 shared tests, exactly three quantity regressions fail | Reproduces singular episode/work/minute defects; `copy-red.log` |
| `cargo xtask theatre-demo verify`, repaired checkpoint | PASS: shared 146 + sandbox 218; 0 failures/errors/skips | L2/L3 pure decisions and L4 JVM orchestration/semantics/input; `cmp-green.log` |
| `cargo xtask theatre-demo android-build` | PASS: `BUILD SUCCESSFUL in 33s` | Android demo compilation/package only |
| `cargo xtask theatre-host android-release` | PASS: release build and artifact inspection; `BUILD SUCCESSFUL in 34s` | Production release resolved graph plus actual APK/AAB sandbox exclusion |
| `cargo xtask theatre-demo ios-build --kotlin-native-home /Users/manh.pd1/.konan/kotlin-native-prebuilt-macos-aarch64-2.4.20` | UNSUPPORTED: installed compiler is not required 2.2.20 | Pinned Native prerequisites/link; never runtime acceptance |
| `cargo xtask preflight --changed-only` | Final-diff result accompanies the PR; retained in `preflight.log` | Repository, Rust format and selected CMP gate |

The initial xtask probe reported **UNSUPPORTED** because Gradle was absent from PATH. The
already installed Gradle 8.14.3 was then selected; no toolchain was installed or upgraded.
The corrected candidate fixtures' early missing-import/transparent-capture failures and final
results are recorded separately in the spike record. Early repository preflight attempts failed
on the moved XML file's stale skill link and then a skill line-count limit; both documentation
references were corrected before the final handoff run. A later attempt reached Python tests
but the system Python 3.9.6 lacked required 3.11+ APIs (`tomllib` and process-group support):
478 tests reported 5 failures/47 errors. Bundled Python 3.12.14 supplied `tomllib` but its build also lacked `os.waitid`: that attempt
reported 492 tests, 1 failure/45 errors/5 optional skips. Capability probes confirmed installed
Homebrew Python 3.14.8 supplies both APIs; the final command selects it. Optional pinned
Pillow 11.3.0 adapter tests remain unavailable (five tests), with no image-comparator code changed. These were actual hygiene/environment failures, not CMP
runtime failures; no tests or required gates were disabled.

Kotlin has no configured formatter/linter: touched Kotlin was manually reviewed, compiled and
tested where named. Rust-format does not format Kotlin. The pre-existing compiler warning in
`PlayerControls.kt` about a non-null `when` branch is not introduced by this change.

## Runtime, visual observations and deferred acceptance

Application JVM captures were opened as two contact sheets, plus the actual Android captures
listed below. The JVM matrix covers 16 splash and 16 unavailable cells: Vietnamese/English ×
light/dark × 390×844, 320×568, 844×390 and 320×568 at 2× text. Observed language, diacritics,
contrast and wrapping match the selected inputs; no lateral text clipping. The unavailable body
at 2× needs vertical scrolling, and the executed tests prove each label reachable. These are
`screenshot-inspected`, `cross-theme-inspected` and `cross-viewport-inspected` on the JVM.
Four additional motion images were present in the splash sheet; no new native-motion claim follows.

Android demo runtime was actually driven on a task-owned, headless **Pixel_7 AVD**,
`emulator-5580`, Android **17/API 37**, `sdk_gphone16k_arm64`, 1080×2400 at 420 dpi
(approximately 411×914 dp). The emulator used `-read-only -no-snapshot`; it was stopped after
observation. No connected physical device was used or changed. Installed debug APK SHA-256:
`f9965b452b2d9b1b3e305c77580e45703d823c162f04e913ca715581f06966b0`.

Observed sequence: fresh Vietnamese → Account → Listening settings → English. UI Automator
assertions found English labels, unchanged Vietnamese `Người giữ tiếng chuông` and `12:48`.
`am force-stop` removed PID 4278; the explicit launcher cold start created PID 6370 and restored
English Home. The native preference file held only the expected nonsecret `ui-locale=en`.
Then English light → dark → Vietnamese; font scale 2.0 kept language labels and all dialog
choices/Done visible. After returning font scale to 1.0, a second force-stop/cold start restored
Vietnamese. The emulator crash buffer was empty. This is **interaction-tested / device-tested
(Android emulator)** for those exact operations, and crosses the real SharedPreferences boundary.

Opened native screenshots: `android-initial.png`, `android-reopened-en.png`,
`android-settings-en.png`, `android-settings-en-dark.png`,
`android-settings-vi-dark-text2.png`, `android-language-dialog-vi-dark-text2.png`.
Observed readable controls in both themes; the long content title ellipsizes in the 2× mini
player while its full accessibility label remains in the UI Automator tree. Explanatory settings
copy requires scroll at that scale; this is not a claim that all copy fits simultaneously.
Captures, UI Automator XML, PID/store record and checksummed runtime manifest remain ignored in
`artifacts/agent-runs/issue169/`; no generated images/media are committed.

The production release inspector independently rejected sandbox/mock/test content against
70 sandbox classes and three forbidden resource files. Required production classes were present;
APK/AAB each defined 26,007 classes. Unsigned APK SHA-256
`d0a14b2a29775c4b86fa7c4c5d6b79856d3a86a3bbae63be091fcc36a7b20cac`; AAB SHA-256
`72e23495f0fd2a84089a21089e169e87d403d47558c09c868a1a95bb79ffe9ca`.
This is packaging/isolation acceptance, not an installed production listening flow.

A JVM capture remains separate from an Android/iOS run. Candidate captures do not accept the
application. A forced Android stop/relaunch tests that named operation, not arbitrary OS
termination, crash-safe disk writes or recovery of memory-only playback/content.

| Independent layer gate | Evidence / covered scope | Gaps |
| --- | --- | --- |
| L1 ownership/architecture | Documented review; inward domain/ports/application/adapter/presentation boundaries | No library/catalog adoption |
| L2 values and pure reducer | PASS, example-tested exact canonical locale/fallback/refusal and unchanged selected state | Native asynchronous disk outcomes are outside the pure oracle |
| L3 compiler/unit tests | PASS at checkpoint, named 146 shared + 218 sandbox suites | No configured Kotlin formatter/linter; iOS compiler unavailable |
| L4 application and rendering | PASS, mock-integration-tested store/mount orchestration; semantics-tested/interaction-tested JVM caret/focus and feedback | Active real account, IME and populated Membership editing not exercised |
| L5 native preferences | Android emulator interaction/device evidence and real SharedPreferences reopen | iOS UNSUPPORTED; arbitrary disk failure/OS termination and screen readers unrun |
| L5 DB/API/provider/audio | Inapplicable: no such behavior changed or exercised | No inference from local/mock results |

Required follow-up ownership is **unassigned — assignment required before acceptance** for:

- #167's reviewed ADR and target-compatible candidate/adoption decision. Prerequisites: exact
  target metadata/transitive graphs, retained fixture inputs, pinned Native toolchain and native
  hosts. Criteria: reviewed source/parameter/fallback contract and migration/rollback; all
  intended target builds/links and same-mounted locale-switch behavior actually exercised.
- iOS compile/link and simulator/device reopen, input/focus, resources/privacy bundle and
  VoiceOver. Prerequisites: installed Kotlin/Native **2.2.20**, configured Xcode and simulator.
  Command: `cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/native-2.2.20`;
  then drive the documented launch → language → terminate/reopen → language flow. Criteria:
  correct saved preference/copy with stable input/account flow and observed privacy packaging.
- Android TalkBack, additional OS versions, IME and configured pending-account flow. Use the
  built demo APK only on an owned test emulator/device; observe selected UI language before/after
  route/locale changes and relaunch. Criteria: state/caret/focus and accessible copy retain the
  stated invariant; report each OS/device separately.
- Pseudo-locale, mixed-language accessibility spans and full CLDR/date/number formatting.
  Prerequisites: reviewed catalog/content-language contract in #167. Re-run the retained fixture
  experiments and `cargo xtask theatre-demo verify` after extending the relevant production
  composition tests; inspect expanded text and native language semantics, not just compilation.

Performance: no read query or playback work changed. Candidate generated-source inventory is
measured separately; preference API latency, startup, disk durability and release-size deltas
were not benchmarked. No performance improvement or process-death durability is claimed.

Edge classes covered: saved/default/corrupt/unknown/unavailable tags; pre-acceptance write refusal
and idempotent user retry; two locales and unchanged theme/content/other owners; 0/1/2 and `1.0`;
repeated/reordered placeholders and literal braces/backslashes; exact typed auth/Membership errors;
small/normal/landscape frames, light/dark and 2× text where executed.

Not covered/residual risk: issue 169 is not fully accepted. No library/architecture decision,
full parameter/CLDR migration, candidate native compatibility, async disk-error reporting,
arbitrary process-death recovery, real pending-account/IME continuity, pseudo-locale,
mixed-language screen-reader proof or native audio claim is supplied. Passed layers are
independent implementation evidence and do not accept deferred native/product gates.

## Executable expectation boundary

The [draft locale-preference model](../../apps/cantos-theatre/behavior/locale_preference.qnt) and
[registered scenarios](../../specs/behavior/catalog.json) describe restoration precedence, atomic
refusal/retry, accepted locale/reset and theme isolation. They are model-only expectations of the
bounded controller contract, not an accepted localization architecture or a Kotlin conformance
adapter. Reopen uses an explicit abstract read result and proves no native disk durability.
