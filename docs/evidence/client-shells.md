# Client skeleton evidence

Date: 2026-10-11. Scope: local `feat/client-shells-skeleton` from fetched `origin/develop`
`83eab6061554539193e4d61f7d267de2b55b095a`. Local commit only; no push, PR, deployment or merge.
The existing unmerged neural TTS work is outside this branch and its gates are unchanged.

## Implemented / unavailable

Studio remains `apps/web` with its creator routes/editor/context, real API adapter and existing
tests. Only its generated semantic palette changes. Independent Theatre Web and CMP modules
implement demo discovery/search/work/library/player navigation and unavailable-action reasons.
Pure route/navigation/search code is separate from Leptos/Compose. Shared semantic tokens generate
CSS/Kotlin without domain or framework dependencies in the token source.

All Theatre data is synthetic and visibly labeled. No listener API DTO, account, payment,
publication record, audio adapter, network request, background service or download success exists.
Disabled native buttons expose unavailable playback/library/downloads. Retry-like state reset
only restores demo content. Production crash/retry/cost/approval/lease rules remain owned by the
existing backend and planned publication/listening flows; this shell dispatches no job or provider.

## Reference and render observations

The two supplied stored HTML references were fetched through the connected Drive read-only
flow and materialized under `/tmp/cantos-shell-references`, outside Git. Their CSS and pixels
were inspected in Safari: Theatre web discovery, Studio works and the mobile Theatre phone
preview. Private IDs/download links, embedded artwork, font assets and 16 MB reference files
are excluded from the change. Covers are explicit placeholders; serif/system fonts are fallbacks.
The web reference requested some separate relative cover/logo assets that were absent from the
downloaded HTML workspace. Layout, CSS and available pixels were inspected; complete reference
asset rendering was unavailable. This proves inspection, not rights or exact fidelity.

Actual Leptos/WASM render and interaction observations used Safari on the local Mac. Inline
computer-use screenshots were inspected in the task conversation; that tool exposed no supported
file/Library export. No stored screenshot deliverable or native screenshot is claimed.

| Surface / condition | Observed evidence | Bound |
| --- | --- | --- |
| Theatre desktop, Safari window capture 1620 × 862, default scale | Discovery, work/episode, library, disabled player/mini-player and paper/lavender hierarchy rendered | Window dimensions include browser chrome, not exact document viewport |
| Theatre narrow | Actual app in an ephemeral iframe with width 320 CSS px; wrapped nav, Vietnamese headings/input/placeholder and search no-results fit without observed horizontal clipping | Scrollbar reduces usable content; wrapper is ignored generated output, not application code |
| Theatre Safari zoom 200% | Page Menu explicitly showed 200%; player text/reasons wrapped and buttons reflowed; lower controls remained reachable through Page Down | Browser page zoom, not a native font-scale test |
| Theatre keyboard/back | Work → player → Safari Back returned to work; heading received focus; Tab/Return operated demo disclosure; visible 3px focus ring | Safari Option-Tab includes links under its default keyboard preference |
| Theatre skip link | Option-Shift-Tab exposed focused skip link; Return focused h1 without replacing the listener hash route | No screen-reader announcement test |
| Theatre local state | Vietnamese `SƠN` search returned Sơn Hà Ký; unmatched query displayed empty; loading/error/not-implemented controls showed demo labels; reset restored demo and select value | No network or recovery integration |
| Theatre theme/locale | Dark render and English controls changed in place on the player route | Preferences are in memory; no account sync |
| Studio desktop | Existing proposal and saved-editor workspace rendered; clicking saved editor then Tab focused script-ID field with visible focus | Static asset server, no authenticated save/import/casting browser rerun |
| Studio narrow/themes | Same built app in 320 CSS px iframe; creator navigation/login text wrapped; light/dark palette and focus rendered | Empty unauthenticated workspace; historical live authoring evidence remains separate |

No new animation is implemented. Reduced-motion CSS is present, but motion profiling,
screen readers, IME composition and native large text/safe areas/back gestures remain **NOT_RUN**.

## Executed checks

| Command / layer | Result and meaning |
| --- | --- |
| `cargo fmt --all -- --check` | Passed; Rust formatting only |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed; workspace static checks |
| `cargo test --workspace --locked` | Passed, including 3 Theatre route/state/search cases and existing Studio/backend/contract cases; ignored database cases are covered separately below |
| `cargo clippy -p cantos-studio -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings` | Passed for both web consumers |
| `NO_COLOR=true trunk build --locked` in each web app | Passed for both final builds; compilation is separate from browser observations |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | 80 tests passed, including token generation/drift/contrast |
| `python3 scripts/reference_script_ir.py` | Passed against both independent accepted-fixture encodings |
| `TMPDIR=/private/tmp CARGO_INCREMENTAL=0 python3 scripts/mutate_script_ir.py` | Baseline 15 tests; 9 killed, 0 survived/timeouts/unviable. Runner now copies both web workspace members into its temporary build |
| `PATH=/opt/homebrew/opt/postgresql@17/bin:$PATH python3 scripts/test_postgres.py` | Passed using a newly initialized disposable native PG17 cluster; revision/import/adaptation/production tests plus HTTP restart/recovery journeys. No inherited DB URL |
| `./gradlew --offline --no-daemon :shared:jvmTest` | Passed; all common Compose UI compiled, 3 navigation/search/boundary tests executed. No native device/runtime |
| `xcodebuild -list -project apps/mobile/iosApp/CantosTheatre.xcodeproj` | Passed; target, Debug/Release configurations and shared CantosTheatre scheme recognized. No linked host build |
| `python3 scripts/studio_tokens.py --check` / `python3 scripts/check_repository.py` / `git diff --cached --check` | Passed; generated source agreement and repository/text hygiene only; generated Windows wrapper line endings normalized to repository LF |

The PostgreSQL run retained logs under ignored `target/revision-evidence/`, including this run's
`cluster-1791698265748822000` directory. Its server was stopped, and only that run's disposable
`pgdata` was removed after verifying no postmaster PID. Existing TTS/model/audio/evidence and
older clusters were untouched. The machine reached 219 MiB free during testing and recovered
to about 1.1 GiB. Further native downloads/builds were stopped at the user's request. Mutation
details remain locally at `/tmp/cantos-shell-evidence/mutations.log`.

## Native blockers and official compatibility

Android `:androidApp:assembleDebug` was attempted offline and blocked by missing cached AARs
(Material3 1.12.0-alpha03 / AndroidX 1.5.0-alpha22, ripple and graphics-shapes).
`:shared:compileKotlinIosSimulatorArm64` was attempted offline and blocked by missing Material3,
ripple, graphics-shapes and kotlinx-datetime klibs. These attempts did **not** verify native
compilation. Final NavigationEvent/common code was subsequently recompiled/tested on JVM;
Android/iOS native targets were not retried. Framework linking, Swift compilation, installation,
device/emulator rendering, audio quality and full production journeys are **NOT_RUN**.

Pins use existing installed Gradle/Kotlin/CMP/Android tools. The standard wrapper was generated
by cached Gradle 9.7.0 and includes the official distribution checksum. No SDK/emulator/model
was installed. Kotlin multiplatform library and the separate Android application host follow
the [official Android KMP plugin](https://developer.android.com/kotlin/multiplatform/plugin).
CMP target/version rules were checked against
[JetBrains compatibility guidance](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)
and the [official release list](https://github.com/JetBrains/compose-multiplatform/releases).
Material3 is an explicit prerelease pin; native compatibility remains a verification gate.

## Review and assurance ledger

Read-only source review covered the local diff/new files against the base above: all manifests,
lockfile entry, Rust/Kotlin/Swift/source/CSS, token source/generator/outputs, mutation runner,
docs and standard wrapper files. No server/Studio Rust source, API, migration, CI, skill or
production gate changes are present. Wrapper JAR was generated by the installed official Gradle
tool; opaque bytecode was not decompiled. This is a self-review, not independent approval.

Assessment: **no-actionable-findings** for the bounded skeleton; insufficient evidence for any
native runtime or production listening claim. Confirmed implementation issues found during
checks were fixed before completion: isolated mutation builds omitted the new workspace member;
obsolete CMP BackHandler was replaced by NavigationEvent; a demo select could retain a stale
value after reset; native top controls now wrap; discovery filtering is separate from search.

| Claim | Owner / required by | Evidence level | Disposition / next action |
| --- | --- | --- | --- |
| Audiences have separate entries without replacing Studio | Client map; user request / ADR0010 | example-tested routes + WASM builds + rendered workspace; Studio source unchanged | held for skeleton |
| Theatre cannot claim playback/download/auth success | Listening/UI rules; AGENTS and user request | source-traced absence of adapters; rendered disabled buttons; common boundary test | held; real adapter evidence belongs to 040/050 |
| Shared theme is deterministic and readable | Token generator/UI policy | example-tested drift/contrast; rendered light/dark | held for tested pairs; assistive technology/native contrast remains unrun |
| Backend domain/permissions/publication gates remain intact | Engineering/domain boundaries; AGENTS | source diff + existing Rust tests and fresh local PostgreSQL integration | held for unchanged boundaries; no new publication path |
| CMP hosts are authored with bounded tool use | CMP policy; user accepts native NOT_RUN when unavailable | common UI compiled/example-tested; Xcode project parsed | held as source skeleton; native build/runtime gap explicitly retained |
| Asset provenance and reference privacy | UI/rights rules; user request | source inventory excludes borrowed art/private locators/raw HTML | held; approved artwork/fonts require later rights decision |

Residual risk: native linking/rendering/back/accessibility are unverified, Material3 is prerelease,
reference fidelity is partial and media/account/delivery contracts are absent. Continue the
bounded follow-ups in the [surface map](../architecture/client-surfaces.md) and
[work queue](../work-plan/README.md); these results do not close 040/050 or authorize production.
