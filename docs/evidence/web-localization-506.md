# Web semantic localization — #506

Work item: [#506](https://github.com/loveoverflowcom/cantos/issues/506). Integration base:
`develop@bf04e14c0415a82e42c930623af8818aa1c0b602`. Isolated branch:
`fix/506-web-localization-boundaries`, Mac Apple Silicon, Rust 1.98.1, Trunk 0.21.14,
wasm-bindgen 0.2.100, Node 22.15.0. No service, deployment, provider or native mobile operation.
Local full logs live in ignored `artifacts/agent-runs/issue506/`; commands below run from the
repository root unless a directory is given. Checks apply to the final source tree, with
individual earlier checkpoints called out explicitly.

## Decision and scope

[ADR0050](../decisions/0050-localization-semantic-catalog.md) records explicit owner approval
at 16:26 UTC on 26 October 2026, message `Sentinel_e5fd476bc7fc8191b0647efa4e9b9a6f`.
It selects versioned JSON, app-owned generated APIs, pinned ICU4X/CLDR and saved choice before
Vietnamese default. It does not establish native bindings or all formatter capabilities.
The [resource/runtime contract](../../packages/localization/README.md) defines the implemented
schema, generator command, preferences and typed refusals. #167/#505 remain open for their
broader acceptance; #507 owns native binding qualification and CMP adoption.

## Migration map

| Owner | Implementation |
|---|---|
| Canonical resource | `packages/localization/catalog/v1/{catalog,schema,manifest}.json`; complete release `vi-VN`/`en` and explicit preview exposure |
| Generator | `scripts/localization_resources.py`, called by `localization_catalog.py --resources`; whole-resource guard joins existing repository localization gate |
| App projections | Each Web app's `src/localization/generated.rs`; resource-derived locale enum/metadata, aliases, plain keys and required named parameter constructors |
| Studio adapters | Root `messages.rs`, all `messages/*`, live/workspace/dashboard/artwork/delegation copy; boolean locale APIs removed, existing domain mappings retained |
| Theatre adapters | Shell/public discovery/detail/library/player messages, auth and membership preview; scan-once interpolation replaced by generation-time parsed typed constructors |
| Shared UI | `ThemeCycleButton` requires caller-supplied labels; no application catalog/runtime import |
| Preferences and roots | App-specific Web storage keys, explicit storage/unsupported warning, resource-derived picker, document/container language and direction; locale does not key a form/player remount |
| Content/error boundary | Content tags, titles, drafts and service payloads unchanged; `zh-Hant` stays opaque. Studio field-rule guidance is localized before unchanged escaped path/rule diagnostics. Unknown rules get localized generic guidance. |

The earlier independent field-rule repair is local commit `2f3ae2465f277221936c0199dc844dd281a8ca68`
and remains part of this single PR. Technical IDs, hashes, revisions, protocol enum values,
raw byte/millisecond receipt facts and punctuation remain exact diagnostic notation; they are
not claims of locale-aware prose. Existing neutral-literal exceptions have exact path, literal
and source context; no wildcard exception or global lint suppression was introduced.

The test-only third catalog has three real Japanese translations and goes through the same
generator and runtime, without adding a view branch. It is a small namespace; release resources
do not pretend to contain a complete Japanese application. Full nested/select message syntax,
translator tooling and arbitrary currency/calendar/duration styles remain outside this schema.
Closed domain states continue mapping exhaustively to generated plain keys.

## Formatter qualification

The shared core pins ICU4X 2.3 data from CLDR 48.2.1, with exact Cargo.lock resolution.
Before adopting affected Web presenters, native Rust tests and an actual WASM/Node probe checked:

- Exact decimals in Vietnamese, English and Japanese; English fractional `1.0` differs from
  integer `1` for plurals, and Arabic `two`/`few` exercises full ICU category selection.
- USD cents/VND whole units, zero and `u64::MAX`, including grouping and nonbreaking spacing.
- Gregorian UTC/fixed-offset instants, negative timestamps, offset date boundaries and invalid
  instant/zone refusals. Browser adapter supplies the host's offset at the actual instant;
  native HTML uses explicit UTC. IANA rules are typed unsupported requests.
- Numeric player clock boundaries at 59:59/1:00:00 and speed hundredths without float rounding.

The reproducible [WASM probe](../../packages/localization/qualification/wasm/README.md) has 15
hand-authored exact observations/refusals. Its first stripped module was 3,008,801 bytes
(SHA256 `64a808fe69f7e2bd5f56308aedc80b67b4ee7fdaece7bb3c48a57922d633f292`), a bounded
standalone probe, **not** an optimized app-size delta or a native binding measurement.
The final FE boundary displays an explicit localized formatting-unavailable message for a typed
refusal; no English/ISO/handwritten localized success substitutes for an unsupported formatter.

## Verification ledger

| Check | Actual result |
|---|---|
| Studio native unit lane | PASS 392, including typed field-rule guidance and unchanged payload/diagnostic regressions |
| Theatre native unit lane | PASS 514 on the final functional migration, with the bounded per-package test optimization |
| Shared runtime | PASS 3 formatter tests plus 3 generated catalog/preference tests |
| Shared action/navigation integration | PASS 12, including reactive caller-injected theme labels |
| Native Clippy | PASS affected crates, tests and Theatre preview, warnings denied |
| WASM Clippy | PASS affected crates, all targets, UI/Theatre preview, warnings denied |
| Catalog Python tests | PASS 78 tests, one original optional Kotlin compiler test skipped |
| Typed Rust compiler controls | PASS valid constructor; wrong type E0308, missing argument E0063, missing key E0599 |
| WASM/Node | PASS final 15 exact vectors/refusals; stripped module 2,868,311 bytes, SHA256 `8c764a4b59da6a60cf28fc1389ee2dbfb8831a04bebb25a408a7e08746494aef` |
| Studio Trunk debug bundle | PASS after task-only cache recovery; initial ENOSPC attempts remain recorded |
| Theatre Trunk debug bundle | PASS after freeing completed Studio output; initial bindgen ENOSPC remains recorded |
| Theatre browser DOM | PASS production root locale/status/error/title/search-draft/persistence smoke; static loopback has no API |
| Studio browser DOM | PASS authoring fixture locale/draft/caret/focus/escaping and root saved preference reload, described below |
| Repository baseline | FAIL 44 existing qualification findings, reproduced on clean main at the integration base; no qualification record was waived |

The baked dependency READMEs identify the exact CLDR patch as **48.2.1**. The initial probes
used a `48.2` family label; the catalog/schema/runtime metadata was corrected to the exact patch
without changing the pinned dependency bytes, translations or formatter algorithms.
Final metadata/drift checks accompany handoff. Native string rendering, WASM/Node, browser DOM,
mock playback and real device observations are separate evidence tiers.

The registered `cantos.presentation.locale-preference` model remains a **draft CMP controller**
contract. This Web change does not change or promote that model, register a conformance observer,
or claim model conformance from fixture tests. Discovery content-language naming still displays
unknown tags unchanged; no retired discovery filter is restored.

## Resource constraints and handoff

Disk exhaustion occurred during initial parallel native links. Logs/source were preserved;
only task-owned obsolete compiler output and failed objects were removed, with exact file/hash
manifests in the ignored run directory. Later test lanes run serially. Theatre's bounded native
unit lane uses `--config 'profile.test.package.cantos-theatre.opt-level=1'` to reduce link size;
this changes no production source, dependency lock, assertion or check selection.

A Cargo.lock/root membership change conservatively selects every preflight gate. Full local
workspace/backend/Gradle requalification needs more disk than this task owns. The known baseline
has 44 shared UI qualification findings and three xtask assertion failures; do not attribute
those to this migration or rewrite qualification records without their required observations.
Scoped checks and actual preflight statuses below do not replace missing acceptance.

#507 handoff: reuse the canonical resource owner and semantic meanings, implement/qualify a
Kotlin projection and ICU4X native bindings for each actual target, compare exact vectors,
then migrate CMP namespaces. Preserve its existing saved-preference controller until replacement
passes. Rust, Node/WASM and any Web DOM observation establish no Android/iOS or native playback
acceptance. No #507 implementation was started in this PR.

## Browser observations — 26 October 2026, after the 18:14 UTC bundle build

The actual built Studio debug bundle ran on loopback `127.0.0.1:18506`, in this task's new
in-app browser tabs. The locked Mac prevented native app interaction, but the in-app DOM
surface remained usable. The pre-existing unrelated fixture tab/process was untouched.

- `?preview=studio-authoring` uses the documented synthetic editor fixture and no API/session/storage.
  The first spoken-text textarea held `Draft 506 · Lời hẹn {time} <b> 🍃`, still uncommitted
  (`Draft commits: 0`). Selecting English changed headings, field labels, actions and inspector
  copy. `document.documentElement.lang` became `en`, direction remained `ltr`; the textarea
  retained `lang=vi-VN`, value, active ID `preview-dialogue-01-Text` and selection `[33,33]`.
  The DOM contained zero `<b>` elements. This is browser draft/focus/caret evidence, not real IME.
- The root's resource-derived custom listbox offered Vietnamese/English. Choosing English
  changed the sign-in-unavailable UI and shared theme button accessibility text, kept focus on
  `studio-language-trigger`, and survived an actual page reload. Document language was `en`,
  direction `ltr`, title `Cantos Studio`. This static loopback has no configured API, so sign-in
  unavailability is expected and proves neither authenticated sessions nor production startup.
- Screenshots were captured in the task transcript for the retained English draft and the
  root after reload. Real IME composition, native screen-reader speech, long-text/RTL layout,
  live authentication/network races, real playback and CMP/device behavior are NOT_RUN.

Theatre's production debug bundle ran on `127.0.0.1:18507` after its 18:27 UTC build.
Switching to English preserved the unsubmitted search value `Lời hẹn {time} <b> 🍃` and
selection `[21,21]`; focus returned to `theatre-live-locale-trigger`. The page, navigation,
search accessibility name, localized connection error and status changed together. Document
metadata became `lang=en`, `dir=ltr`, title `Discover · Cantos`; zero `<b>` nodes appeared.
After a real reload, English and the English error dialog were restored. No real API/catalog,
authentication or media playback was supplied. The task-owned tabs and loopback servers were
closed after the observations; the unrelated fixture remained untouched.

## Scoped handoff checks and exact limits

All Cargo commands reused `/tmp/cantos-492-target` with `CARGO_INCREMENTAL=0`; no second target
was introduced. The app `trunk build --locked` commands used `CARGO_BUILD_JOBS=1`, separately
in each app's `web` directory. Native commands were:

```sh
cargo test --offline --locked -j 1 -p cantos-studio --lib
cargo test --offline --locked -j 1 -p cantos-theatre --features preview --lib --config 'profile.test.package.cantos-theatre.opt-level=1'
cargo test --offline --locked -j 1 -p cantos-localization
cargo test --offline --locked -p cantos-ui --test actions_navigation
cargo clippy --offline --locked -p cantos-localization -p cantos-ui -p cantos-studio -p cantos-theatre --tests --features cantos-theatre/preview -- -D warnings
cargo clippy --offline --locked -p cantos-localization -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-theatre/preview,cantos-ui/preview -- -D warnings
cargo fmt --all -- --check
python3 scripts/localization_catalog.py --resources --check
python3 -m unittest discover -s scripts -p 'test_localization*.py'
```

A count-only catalog additionally passed the actual WASM compiler metadata stage, with both
host proc-macro and target dependency paths. This detects an unused generated lifetime without
substituting a formatter stub. The positive native constructor and the three expected compiler
refusals have independent retained logs. Optional Kotlin compiler re-execution was skipped;
this PR makes no new Kotlin compiler claim.

Actual handoff preflight ran against the pinned base and **FAILED at repository** with exactly
the same 44 findings as clean main (set comparison: zero new/removed findings). Static service
manifest validation passed. `rust-format` was NOT_RUN inside preflight after that failure; its
independent command above passed. To avoid repeating already-scoped checks or starting unrelated
workspace/backend/mobile campaigns on the low-disk Mac, these gates were explicitly NOT_RUN
through `--skip`: `behavior`, `script-ir-reference`, `protobuf-reference`, `rust-lint`,
`rust-tests`, `forge-functions`, `turso`, `wav-reference`, `audio-inspection`, `web-lint`,
`web-build`, `ui-preview`, `cmp-ui-preview`, `theatre-cmp-preview`, `mutation`. The scope-specific
commands above supply their stated evidence, not a preflight PASS. The full selected dry-run
plan and actual failure/skip report are retained in `preflight-plan.log` and `preflight-final.log`.

No required gate, protection or qualification record was weakened. The three previously
reported xtask assertions were not globally rerun locally; remote CI is reported on its actual
final commit in the PR handoff. #506 remains open for the unverified acceptance listed above;
merging source under the owner's explicit authorization does not declare those results passed.
