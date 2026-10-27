# Theatre CMP generated text resources — #507

Bounded implementation and source-bound evidence; **not completion of #507 or native formatter
acceptance**. Base: fetched `origin/develop@da2bb95bc55b5e01680d1c84644c6a9d46cf0521`.
Branch: `feat/cmp-localization-507`, isolated `/tmp/cantos-issue507` on the authorized Mac.
The current #507 body/comments (none), #167/#169/#505, actual merged #511/#512 source,
[ADR0050](../decisions/0050-localization-semantic-catalog.md), [foundation evidence](localization-foundation-505.md)
and [Web boundary](web-localization-506.md) were read. Owner approval selects the architecture;
it does not qualify Kotlin ICU4X, native targets or all #506 acceptance. No deployment or merge.

## Invariant and ownership

One saved-preference authority selects all migrated text, including launch, typed host failures,
auth, Membership, dialogs and accessible names, without recreating playback, Library, account,
navigation, pending requests, input or focus. Generated messages insert content literally.

`packages/localization/catalog/v1/catalog.json` remains the single versioned source. It adds
346 explicit CMP platform variants: `cmp_shell` 164, `cmp_auth` 109, `cmp_membership` 60,
`cmp_launch` 11, `cmp_live` 2; 16 have required named text arguments. These variants preserve the
existing CMP meanings/wording rather than repurposing Web preview keys with different semantics.
The structural v1 schema is unchanged. Whole-catalog completeness, exact argument/plural rules,
Kotlin symbol/namespace validation and deterministic drift are mandatory. Web generated bodies
are byte-identical to the base after their provenance header; their global catalog digest changes.
No service schema, HTTP DTO, content language, Script IR, SQLDelight schema or audio state changes.

`localization_kotlin.py`, invoked by the existing resource generator/repository guard, emits:

- Domain `UiLocale`: canonical tags, autonyms, direction and default from release registry rows.
- Presentation `ShellLocale` and exhaustive `app/LocaleProjection.kt` adapters from the same rows.
  The presentation-model leaf imports no domain code; these are read-only representations,
  not additional mutable authorities or locale negotiation runtimes.
- Presentation-owned `generated/TheatreResources.kt`: plain typed keys and sealed named-argument
  text messages. Generated concatenation uses parsed original segments; there is no recursive
  template pass, raw lookup map, raw backend prose or missing-key fallback.
- A text-only `commonTest` projection of the shared Japanese fixture. Japanese is absent from
  production locale choices/source sets. The fixture's plural requests are deliberately not
  projected; an attempted numeric/plural Kotlin projection fails explicitly.

`PreferencesController` retains native key/value ports, saved canonical preference > explicit
host/preview seed > Vietnamese default, atomic pre-acceptance refusal/retry and its existing
async native durability limits. Corrupt/unsupported saved tags remain a typed `Corrupt` read and
use the deliberate existing seed/default policy, without rewriting storage or choosing English.
There is no device/browser negotiation. UI language choices iterate registry-derived entries and
show autonyms. A content tag such as `zh-Hant` need not be an available UI translation.

`ShellCopy`, `AuthCopy` and `MembershipCopy` retain domain/error projections over generated keys.
Typed messages replace presentation string-vararg calls for titles, already formatted time and
speed text. Generated live variants replace the bilingual override map. Optional diagnostic
slots take caller-resolved strings. Shared UI-system source is untouched and still receives labels.
Launch/host-unavailable text now uses the same generated catalog and selected presentation
locale directly; the paired `strings.xml` resolver is removed. Compose font/drawable resources
remain. No process locale is changed and no `key(locale)` remount is introduced.

## Deliberately retained numeric boundary

There is **no qualified Kotlin ICU4X binding** in the merged dependencies or this PR. Web/WASM,
native Rust and JVM results do not supply that binding. Consequently:

| Existing owner / consumers | Preserved behavior and next boundary |
| --- | --- |
| `presentation/model/LegacyShellCopy.kt`, nine numeric keys | Existing episode/chapter/work/minute quantities, numbered episode label, minute/second words; vi/en integer one/other and literal-title behavior stay unchanged. Live episode wording retains its existing explicit variant. |
| `LegacyShellCopy.speed`, `ShellCopy.spokenTime`, `clockTime` | Existing speed comma/dot, spoken minute/second composition and clock output; no claim of general duration/locale support. |
| `features/auth/presentation/LegacyRetryAfter.kt` | Existing integer retry-after wording for login and verification; no new plural/number claim. |

Compatibility locale matches are exhaustive with no arbitrary-locale-to-English arm. Adding a
release locale generates text/registry/projections but cannot compile these consumers until its
actual numeric binding is qualified and the compatibility owners are migrated. The text generator
rejects `count` or plural requests rather than fabricating a formatter. Full multi-locale #507
acceptance therefore remains **partial**. Rollback reverts this resource/projection/consumer change
as a unit; no stored preference key, credentials, database, content or audio revision changes.

## Executed checks

All commands ran in the isolated checkout with Rust 1.98.1, installed Gradle 8.14.3,
Kotlin 2.2.20, CMP 1.9.0, Material3 1.8.2 and JDK 21.0.7. Cargo reused
`CARGO_TARGET_DIR=/tmp/cantos-492-target CARGO_INCREMENTAL=0`; Gradle reused installed caches.
Full logs remain in ignored `artifacts/agent-runs/issue507/`. No toolchain was installed.

| Layer / command | Result | Actual scope |
| --- | --- | --- |
| `python3 scripts/localization_catalog.py --resources --check` | PASS | Complete source/manifest and every Web/Kotlin/fixture projection; no drift |
| `python3 -m unittest discover -s scripts -p 'test_localization*.py'` | PASS: 84 selected, 1 optional compiler skip | Whole-catalog negatives, namespace/identifier safety, opaque Kotlin interpolation, release/test exposure and explicit unqualified-numeric rejection |
| Theatre architecture primitive (same repository guard) | PASS | Enforced inward layer imports, package placement and production/sandbox source boundary |
| `cargo xtask theatre-preview verify` | PASS: shared 570 selected / 3 skipped; sandbox 145 selected / 2 skipped | Actual shared and sandbox suites, production composables/controllers with explicit external-port doubles |
| `cargo xtask theatre-host android-release` | Audit/build/byte inspectors PASS; aggregate FAIL at disk-full evidence freeze, recovered below | Actual Android compiler, release graph, unshrunk audit, R8 release APK/AAB and exclusion inspection; no installation/distribution |
| `cargo test --offline --locked -j 1 -p cantos-localization` | FAIL: disk exhaustion while archiving pinned ICU data; no tests executed | Actual pinned Rust ICU4X tests/shared Japanese fixture; never Kotlin/native formatter evidence |
| `cargo fmt --all -- --check` | PASS | Rust only; Kotlin has no configured formatter/linter, so manual style/import review plus compiler evidence |
| `cargo xtask verify behavior --capability cantos.presentation.locale-preference` | UNSUPPORTED | Locked Quint tool absent in this isolated checkout; no setup/install. Existing draft model/expectations unchanged; no conformance observer claim. |
| `cargo xtask theatre-host ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.4.20` | UNSUPPORTED | Guard requires installed Kotlin/Native 2.2.20; installed 2.2.0/2.4.20 do not qualify. No framework/app/device build. |

Final Android recovery ran the exact failed completion primitive after removing only this
checkout's regenerable audit compiler intermediates:
`python3 scripts/android_release_evidence.py --mobile apps/cantos-theatre/mobile --report apps/cantos-theatre/mobile/androidApp/build/reports/release-boundary/release-evidence.json --api-origin __CANTOS_DEFAULT_API_ORIGIN__` — **PASS**.
The source/graph comparison, audit and optimized APK/AAB inspections and immutable evidence
snapshot all completed. Optimized APK: 4,134,266 bytes,
SHA-256 `a78e4cdf51a726c85c0dd7c091f6af92e85a41313f538839e0f48e33a5346bb2`;
AAB `32dc61ac6d9a6135f5755a6f9d50e1991d32cd8e2e762e3e1cae84a184f792a9`.
Source-content snapshot `a29e48840cefe49200b3e94a9e435cf743fb72a83ac3b9ff15b15589ec69cc25`
binds the uncommitted build inputs at the pinned base; subsequent changes are evidence text only.
The frozen record is in `artifacts/android-release/1f7128f263984e2caad815a7b22753a0/`.
A later disk failure during a source-only baseline checkout rolled that checkout back; no extra
baseline worktree remains. Exact-tree remote CI evidence supplies the comparison below.

Rust fixture re-execution also failed from disk exhaustion. A bounded retry optimized only
`profile.test.package.icu_datetime_data.opt-level=1` to reduce temporary data archives; it
then failed while archiving `icu_experimental_data`. This remains **FAIL / environment**, not
a Rust fixture PASS. No assertions/data were replaced, no global cache was cleaned, and no
Kotlin/native formatter claim follows. The changed fixture's Kotlin tests did execute above.

Final preflight used the pinned base and **FAILed at repository with exactly 44 findings**.
Normalized comparison against [the #506 repository job](https://github.com/loveoverflowcom/cantos/actions/runs/40381289531/job/121778139403)
is identical: 44 baseline, 44 current, zero new/removed. The base tree is identical to that
job's reviewed source `48788b618c71ca4e87531f5a960c35ee6a909bac` (`git diff --name-only` empty).
These are stale component qualifications/adoption findings; no qualification hash was changed.
`rust-format` was NOT_RUN inside preflight after the repository failure; its standalone check
passed. Unclassified catalog/tooling paths select every gate. To avoid unrelated backend/media
campaigns and duplicate scoped runs on the full disk, explicit `--skip` reported NOT_RUN for
`behavior`, `script-ir-reference`, `protobuf-reference`, `rust-lint`, `rust-tests`, `forge-functions`,
`turso`, `wav-reference`, `audio-inspection`, `web-lint`, `web-build`, `ui-preview`, `cmp-ui-preview`,
`theatre-cmp-preview`, `mutation`. Scoped evidence above does not make preflight PASS.
The Web generated bodies are unchanged, so Web bundles/consumers were not requalified locally.

The first compile exposed two stale old-API callers and was corrected. A new focused-input test
initially compared password semantics to cleartext; actual semantics correctly contained bullets.
The corrected test compares the masked value/focus across the switch, then explicitly reveals
its synthetic input to assert bytes before submitting. These were recorded failures, not native
or production success. A layer-guard inspection also required moving generated resources into the
presentation leaf and generating its distinct representation/projection rather than importing
domain into that leaf. Final checks cover that corrected source.

New regressions exercise the real production bookmark row with title
`Ánh đèn {time} %1$s <b> cuối sân khấu` in visible/accessibility text, canonical unsupported saved
tags, resource-only Japanese text/error, named argument order, 0/1/2 existing vi/en quantities,
decimal `1.0`, speed/time boundaries and all existing typed auth/Membership/host-error mappings.
JVM login switching preserves focused input and one pending request. Settings switches through
actual UI controls while preserving active player state, exact position, engine commands,
Library/account/navigation snapshots; refusal and retry preserve the native-store port contract.

## Rendered observations

Opened actual JVM Desktop Compose captures from the production components, with fake
access/decoder ports and no audio. These are **screenshot-inspected**, never device evidence:

- `shared/build/reports/theatre-demo/unavailable-dark-en-320x568-text2.0.png`, 320×568 dp,
  dark English at 2× text: heading/body wrap and stay horizontally inside the viewport; the
  existing scrolling test separately asserts reachability. SHA-256
  `bfdc613e851c7d035ec34a8dab8351245ace0c87cbb60ab2b334677792514563`.
- `shared/build/reports/theatre-app/theatre-light-390x844-text1.0-player.png`, 390×844 dp,
  light Vietnamese: source title/diacritics, transport, time, speed/timer/queue labels are visible.
  SHA-256 `9f9509e372f57142c3830739d95207f3614ea44092536f35e6d710d93aaa2a29`.

The suite also asserts launch/app copy under an opposing JVM system locale, reactive launch
accessible status and localized typed unavailable/host-fault semantics. Captures use injected
preferences; they establish no TalkBack/VoiceOver, actual IME, RTL or native locale annotation.

## Deferred acceptance and exact blockers

- Kotlin/native ICU4X binding and cross-target formatter vectors: **NOT_RUN / implementation gap**.
  Qualify pinned ICU4X/CLDR number, cardinal, duration and any affected formatter on Android/iOS
  before moving the named compatibility consumers. There is no success stub or new binding here.
- iOS framework: **UNSUPPORTED**, missing exact installed compiler; next command is the same
  `theatre-host ios-build` with a real installed 2.2.20 home. Xcode 16.3 was observed, not altered.
  App launch, saved-preference reopen, VoiceOver and native text/focus remain NOT_RUN.
- Android device: **NOT_RUN**; `adb devices -l` returned no device. Release packaging is not
  runtime acceptance. Reopen persistence, native player/notifications, IME, TalkBack, large-text
  device interaction and real audio/auth/network journeys need an eligible device/runtime.
- JVM skips: shared real-origin catalog, real bookmark API and authorized Library integration;
  sandbox real-server preview and opt-in OS-window Escape test. Existing doubles are named above;
  there is no real Server/Identity, network or native disk-durability acceptance in these tests.
- Repository baseline, final preflight and final-head CI results accompany the PR below/description.
  Existing stale shared-component qualifications must not be refreshed by editing hashes.

No shared UI component, Quint model, native playback/session/outbox algorithm, deployment,
provider, credentials, VPS, Long Vân privacy or another task's fixture/process was changed.
Disk headroom was inspected before builds. Only this task's regenerable Android audit/compiler and exact failed Rust compiler
outputs were removed after retaining artifacts/reports when free space fell below 250 MiB;
source, APK/AAB/mapping evidence, DBs, other worktrees and caches were preserved.
