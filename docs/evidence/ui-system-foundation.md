# UI system foundation — session 1

Date: 2026-10-11. Local-only implementation, based on develop
`dc7c2de311c400a7581347a80b33d9b8b34c25a4` (PR26/PR28 included).
No backend, TTS, database, authentication, production gate or playback change.
Merge of the research/provider slices does not grant production eligibility.

## Implemented boundary

[`packages/ui-system`](../../packages/ui-system/README.md) replaces the token-only package.
`shared/tokens.json` v0.2.0 is the sole active semantic source; stdlib Python generates scoped
CSS, typed Rust names/values and breakpoint-substituted component CSS. The Leptos crate pins
0.7.8 and has UiRoot, a minimal non-submit Button and eight decorative official outlined icons.
Studio consumes typed Theme and the shared CSS directly. Theatre remains a skeleton; CMP has
documentation only. Component breadth and app integration belong to the
[next sequence](../work-plan/015-leptos-ui-system.md).

| Invariant | Failure mode | Owner / oracle | Evidence level |
| --- | --- | --- | --- |
| One source produces the same mappings regardless of JSON key order/whitespace | app themes drift or generate unstable diffs | pure generator + deterministic fixtures/drift check | example-tested, statically-checked |
| Values, semantic types, theme roles, aliases and runtime names are validated before writes | malformed CSS or ambiguous typed mappings | validator/refusal tests | example-tested |
| Named opaque foreground/background pairs meet their floor in both themes | unreadable content/boundaries | independent anchored sRGB math, source pair table | example-tested |
| Disabled action carries non-empty visible reason and does not become submit | inaccessible blocked operation or accidental form mutation | refined reason type, string-render smoke, native attribute/callback guard | type-enforced, example-tested |
| SVGs are official licensed geometry, decorative and offline | placeholder drift, unnamed controls, unsafe markup | pinned original hashes, restricted anatomy generation, render smoke | example-tested, statically-checked |
| Package migration leaves Studio compilable without backend dependencies | broken include/path/theme API | focused WASM lint and Trunk builds | compiled, statically-checked |

The functional core is token validation/rendering; file I/O is the shell. Components contain
only presentation state and dispatch. No durable app rule needed extraction in this slice.

## Source provenance

Authenticated Drive metadata and supported raw fetch/materialization were used read-only.
The two complete Web HTML files, shared manifest, review and four package ZIPs were inspected
as source; PNGs are static boards, not HTML screenshots. No raw HTML, artwork/font bytes,
private URL/ID or signed URL enters this commit. Temporary references remain outside Git.

| Original reference artifact | SHA256 |
| --- | --- |
| Shared design-tokens JSON | `3067f69e081ff4db002f1b9cd1b0325f7cd24960b326b8735217e245fedc4098` |
| Shared component manifest | `0b1a3a01b67814db55c19b64eca3abb687aa18bebc3bddd8d53b282411bc1cf5` |
| Theatre Web HTML | `5fdd41bd99460fdad12d3092277568a2b624c0e2139790a96e66f90067d37fd4` |
| Studio Web HTML | `de39cc8d0a032a1a2abd9d021b4703ec7af288c0124abab585c41533bf5b1ae6` |
| Studio Web design ZIP | `907c09a344dc3b1f300d7c3ac7859680518fd00e67c3f66383d4e81f9046de88` |
| Preserved old Studio v0.1.0 JSON | `c320a5b95afa41adc59f9c2a84e3dd2ae780a86dd9ea7f558f191963d992e571` |

Reference palette values map without palette changes; role renaming and new foundation decisions
are documented in the [shared contract](../../packages/ui-system/shared/README.md).
All 51 pack icons were compared to the pinned official Google source in a supporting read-only
investigation; this commit includes only eight exact originals, their manifest and full Apache2
license. The generator independently checks the committed original hashes. Font family names
are retained with local/system fallbacks; embedded font/art distribution rights remain unresolved.

VOT develop was read at `f3fea641a5fd226b883c4e32224ad8c97b8323f0`, including its actual
`ui-system/primitives/{styles,leptos,cmp}` and `ui-system/workspace` icon boundary. VOT's CSS
canonical source/native parser/palette/layer graph are not copied. The
[decision](../decisions/0014-ui-system-foundation.md) records the smaller Cantos choice.

## Executed checks

Tool versions: Python 3.13.5, Rust/Cargo 1.87.0, Trunk 0.21.14 on macOS Apple Silicon.
Commands run against the local foundation dirty tree; command outputs are in this task's tool
transcript. Generated CSS headers bind the exact canonical source hash and template hash.
Default cwd is repository root unless listed. Build jobs are limited to2. Initial disk available
3.9GiB; no blanket cache/target deletion, Docker or SDK/model/toolchain installation occurred.

| Command | Result / scope |
| --- | --- |
| `python3 scripts/ui_tokens.py --check` | PASS: all three mappings match source/template |
| `python3 scripts/studio_tokens.py --check` | PASS: compatibility CLI checks the same outputs |
| `python3 scripts/ui_icons.py --check` | PASS: eight source hashes and generated SVG anatomy |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 125 tests including semantic-type and CLI/migration regressions |
| `CARGO_BUILD_JOBS=2 cargo test -p cantos-ui --locked` | PASS:5 native string-render/type smoke tests;0 doctests (not presented as test evidence) |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio --target wasm32-unknown-unknown --features cantos-ui/csr --locked -- -D warnings` | PASS: focused shared/Studio WASM lint |
| `cargo fmt --all -- --check` | PASS after final code edits |
| `python3 scripts/reference_script_ir.py` | PASS: both existing independent fixtures agree; no domain change |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui --all-targets --locked -- -D warnings` | PASS: native foundation/test lint |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui --all-targets --target wasm32-unknown-unknown --features preview --locked -- -D warnings` | PASS: CSR preview/shared target lint |
| Studio `CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build --locked` | PASS; output in `apps/cantos-studio/web/dist` (ignored), debug CSR |
| Foundation preview `CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview` | PASS; cwd `packages/ui-system/leptos`, debug CSR |
| `python3 scripts/check_repository.py` / `git diff --check` | PASS after complete evidence/staging |

The initial native smoke failed because AnyView string rendering requires Leptos's `ssr` feature.
A native-only dev dependency supplies it; the final smoke passed. This is not an SSR application
deployment. A direct module-style Python invocation initially failed import lookup; the documented
discovery invocation passed. Initial offline Cargo resolution/Trunk metadata lacked locked registry
artifacts; a network-enabled retry resolved the small registry requirements. No versions
already pinned in the lock were changed; the new native smoke adds `inventory` via Leptos SSR.

The preview first attempted a Cargo example target; Trunk0.21.14 did not discover its artifact.
A feature-gated development binary now provides the actual build entry. Its final WASM file is
1,369,165 bytes; the Studio WASM is 32,190,913 bytes in debug mode. Both built distributions carry
identical token CSS SHA256 `f973a75ed82c22ed076db10483026a4d309026ceea3deb095825dd25822c1343`.
No app bundle-size optimization or production release claim is made. Final available disk is
approximately2.5GiB; subsequent sessions should inspect their budget again.

A supporting source review found that named roles could accept an unrelated type. The three
specific wrong-type mutations failed the new regression tests before the fix; the validator now
binds family/suffix to type and rejects incompatible aliases. The compatibility CLI also has a
failure-exit regression, and the historical source hash/public role coverage is checked.

## Opaque contrast measurements

These are the 21 declared pairs in both themes (42 evaluations), computed from source values.
They do not include artwork, compositing or the appearance of a focus ring against a control.

| Foreground / background | Floor | Light | Dark |
| --- | --- | --- | --- |
| `color.onPrimary` / `color.primary` | 4.5 | 6.917 | 8.671 |
| `color.onPrimaryContainer` / `color.primaryContainer` | 4.5 | 10.693 | 7.694 |
| `color.onSurface` / `color.background` | 4.5 | 13.759 | 13.951 |
| `color.onSurfaceVariant` / `color.background` | 4.5 | 5.305 | 8.725 |
| `color.focus` / `color.background` | 3 | 5.925 | 9.811 |
| `color.onSurface` / `color.surface` | 4.5 | 14.879 | 12.373 |
| `color.onSurfaceVariant` / `color.surface` | 4.5 | 5.736 | 7.739 |
| `color.focus` / `color.surface` | 3 | 6.407 | 8.701 |
| `color.onSurface` / `color.surfaceContainer` | 4.5 | 12.652 | 10.788 |
| `color.onSurfaceVariant` / `color.surfaceContainer` | 4.5 | 4.878 | 6.747 |
| `color.focus` / `color.surfaceContainer` | 3 | 5.448 | 7.586 |
| `color.onSurface` / `color.surfaceContainerHigh` | 4.5 | 11.704 | 8.948 |
| `color.onSurfaceVariant` / `color.surfaceContainerHigh` | 4.5 | 4.512 | 5.596 |
| `color.focus` / `color.surfaceContainerHigh` | 3 | 5.040 | 6.292 |
| `color.error` / `color.errorContainer` | 4.5 | 5.536 | 7.566 |
| `color.warning` / `color.warningContainer` | 4.5 | 5.638 | 7.593 |
| `color.success` / `color.successContainer` | 4.5 | 5.924 | 7.199 |
| `color.outline` / `color.surface` | 3 | 4.223 | 4.664 |
| `color.outline` / `color.surfaceContainer` | 3 | 3.591 | 4.066 |
| `color.primary` / `color.surface` | 4.5 | 6.759 | 8.749 |
| `color.accent` / `color.surface` | 4.5 | 5.124 | 7.599 |

## Runtime boundary and residual risk

A loopback server was started for the complete reference HTML, then Computer Use reported no
available browser surface and that the Mac was locked. Therefore reference HTML rendering,
preview/Studio screenshot capture/inspection, cross-viewport/cross-theme appearance, keyboard,
focus/text200% scaling, OS IME, forced colors, reduced-motion interaction/recording and screen-reader
walkthrough are **NOT_RUN**. The attempted browser entry is not evidence of rendered pixels.
No `DOM-tested`, `interaction-tested`, `accessibility-checked` or native runtime claim is made.

Native smoke checks HTML strings only. Actual `color-mix` hover/press compositing, text/font
clipping, wrapping and focus occlusion still need runtime review. Status/disabled backgrounds
must not be confused with production authorization. The eight icon names and three Button
variants are deliberately small; later components must use source roles rather than invent
app palettes. Session3/4 must preserve valid numeric zero, including the existing intensity0
limitation documented in [Google adapter evidence](google-tts-adapter.md), without expanding TTS scope.

Full backend/workspace Rust tests/lint, PostgreSQL, mutation/WAV/provider/audio checks and mobile
build/tests are **NOT_RUN** for this UI-only bounded change. Existing AGENTS requirements remain
applicable when those owners change; this resource-constrained slice runs focused UI checks.

## Successor boundary

The [session 2 component evidence](ui-system-components.md) records the reusable Leptos library,
state gallery, focused checks and session 3 API handoff on this foundation. This session 1
record remains historical evidence; successor builds and runtime limitations are recorded there.
