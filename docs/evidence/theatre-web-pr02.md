# PR 02 — Leptos Theatre foundation validation

Date: 2026-10-13. Baseline: `develop` at `7396ede88466ef56dc9e7759956a4072d6672a5b`.
The requested Discovery/Detail/Player architecture already exists on this baseline. This slice
hardens catalog callback ownership, repairs two browser-reproduced focus losses, and validates
the existing session-only mock. Part of [#13](https://github.com/loveoverflowcom/cantos/issues/13),
without completing its real listening acceptance. No new architecture/dependency decision is
needed: Rust 2021, Leptos 0.7.8 CSR, the app-local core and current ports remain in place.

## Invariants, ownership and evidence

| Invariant / failure mode | Owner and oracle | Evidence level / result |
| --- | --- | --- |
| Only the newest issued pending catalog request settles; duplicate/future/unissued callbacks cannot replace results | `application/catalog.rs`; exact state tests and controllable `DeferredCatalog` | `example-tested`, `mock-integration-tested` — PASS |
| Retry advances generation; exhaustion cannot reuse identity; unavailable/missing/invalid-duration samples cannot become sources | catalog session with `MockCatalog` and `InvalidDurationCatalog` | `example-tested`, `mock-integration-tested` — PASS |
| Recovery does not lose keyboard focus when its button disappears | CatalogState and Explore views; Enter before/after the same scenario | `interaction-tested`, `accessibility-checked` — body before; main/search input after |
| Intent is distinct from engine confirmation; obsolete callbacks cannot override pause/restore | framework-free `core/playback`, application session, FakeMedia and deferred media port | `example-tested`, `mock-integration-tested`, `differentially-tested` — PASS on Rust native |
| Navigation reuses one player, source and position | root TheatreContext; native tests and Discovery → Detail → Player → Library → Back/Forward | `example-tested`, `interaction-tested` — PASS for simulated playback |
| Loading/error/empty are distinct, actionable mock states | deterministic MockCatalog; controlled browser scenario selection/recovery | `mock-integration-tested`, `DOM-tested`, `interaction-tested` — PASS |

Eight catalog tests replace one narrower stale-result test; the Theatre suite now has **53 passing
tests**. The core, existing library/progress/queue ownership and seven shared fixture files are
unchanged. `theatre-mock-playback@1` contains 51 hand-authored steps comparing full state, derived
status and ordered effects. Its strict native Rust decoder/replay passed; Kotlin and WASM fixture
test execution were not run here. Compiling/running the WASM application is separate evidence.
No listener API, publication, production job or no-production-work database test ran.

## Local checks

### Control-label alignment follow-up

The screenshot-reported vertical offset came from centering the full label line box, whose font
leading is asymmetric. The shared Leptos `cui-control-label` now trims to cap/alphabetic font
metrics in supporting browsers; unsupported browsers retain token line-height. Button separates
its optional decorative icon from label children, and FilterChip uses the same label anatomy.
Theatre's app-owned native actions/chips consume this hook. No pixel translation, fixed text
height, clipping, font download, shared token change or CMP renderer change was introduced.

`render-inspected` / `interaction-tested`: the Codex in-app browser rendered the gallery and
Theatre at the default desktop viewport and 320×800; the gallery also ran at 390×844. Observations
cover light/dark, compact/comfortable gallery density, English transport and wrapped Vietnamese labels. The measured 48px Theatre
transport had matching icon/button centers and label center within 0.01px; the previous 21px
line box became a 9.86px cap/baseline box at 14px font size. Inspected Vietnamese accents and
descenders remained visible, wrapped gallery chip/button labels grew with content, and the
gallery had no horizontal overflow at narrow widths. Tab retained the visible 3px focus outline;
the primary action incremented its callback count, and simulated play changed to Pause simulation.
No screen-reader, 200% zoom, other-browser or native-mobile result is claimed for this follow-up.
Local captures are ignored: `button-alignment-light-en.jpg`, `button-gallery-narrow-focus.jpg`
and `button-gallery-dark-320.jpg`, plus `button-theatre-dark-320.jpg`, under the same artifact directory below.

Follow-up `cargo xtask verify rust` and `cargo xtask verify web`: PASS all their gates.
`cargo xtask preflight --changed-only`: PASS repository, rust-format, scoped native
lint/tests for Studio/Theatre/UI, both Web lint/builds and UI preview. No gate skipped and no
GitHub CI checked.

Host: macOS; existing Rust/Cargo 1.87.0, Trunk 0.21.14, Python 3.14.8 and wasm32 target.
Commands run from the repository root with `RUSTUP_TOOLCHAIN=1.87.0`; Python-heavy commands
also use `PATH=/opt/homebrew/bin:$PATH TMPDIR=/private/tmp`, and native suites use
`RUST_TEST_THREADS=1`. This selects the already-installed Python with `tomllib` rather than the
default system Python 3.9, which failed the first repository run. No toolchain was installed.

| Command | Result |
| --- | --- |
| `cargo xtask verify rust` | PASS: rust-format, workspace rust-lint and workspace rust-tests; 53 Theatre tests |
| `cargo xtask verify web` | PASS: WASM web-lint, Studio/Theatre Trunk builds, shared Leptos preview build |
| `cargo xtask verify repository` | PASS: repository checks, 221 Python tests and staged/unstaged whitespace checks |
| `cargo xtask preflight --changed-only` | PASS: repository, rust-format, Theatre rust-lint/rust-tests/web-lint/web-build; no skipped gate |
| `cargo xtask verify contracts` | UNSUPPORTED: audio-inspection requires absent ffmpeg/ffprobe; script-ir-reference, contract-tests and wav-reference NOT_RUN because suite prerequisite checking stops before execution |
| GitHub CI | Not checked; no polling, dispatch or rerun |

The first workspace Rust run failed the existing xtask localhost port-release assertion. The
subsequent complete serial run passed without skipping tests or changing xtask. Final source
passes formatting/Clippy/tests.
No contracts, fixtures, manifests, migrations, backend or mobile files changed. Media inspection
and PostgreSQL are outside this Web mock boundary; the contracts-suite limitation remains explicit.

## Integrated-browser observations

Serve command: `RUSTUP_TOOLCHAIN=1.87.0 cargo xtask run cantos-theatre --web`, debug CSR,
`http://127.0.0.1:8082`. Browser: Codex in-app browser on macOS; engine/version not exposed by the
selected automation surface. Pointer and trusted keyboard actions were driven through CUA.
Backend/media: MockCatalog/FakeMedia only, no API or `<audio>`, no network/media/CDN acceptance.

- Discovery → work → long-title episode → simulated play → keyboard seek to 0:01 → bookmark →
  Library → Back/Forward preserved the sample, position and simulated play state. Unavailable
  episode controls had visible associated reasons. Reloading the Player deep link reset the demo
  to the paused 12:48 seed, as advertised.
- Buffering, confirmation, interruption, no-auto-resume and in-memory restore displayed their
  explicit mock statuses. Restore remained paused. No actual audio, clock or OS interruption ran.
- Loading stayed controlled until Show sample data; empty and error remained distinct. Retry
  retained the selected player. Enter originally left focus on BODY; the same error-retry scenario
  after the fix left focus on MAIN `theatre-main`. Loading and empty recovery also returned there.
- Search for `đèn` found the accented title. A missing query exposed Clear search and filters.
  Enter originally dropped focus to BODY; after the fix at the same 390×844/light/en setting,
  results returned and focus stayed on INPUT `theatre-search`.
- Skip to main content focused main without changing the route. Removing the last bookmark
  returned focus to main. At 320×800, the focused last bookmark action ended at y=403 while the
  long-title mini-player started at y=571: the control remained unobscured. Scroll width was
  305 CSS px with the scrollbar, below the 320 viewport.
- Unknown-route recovery rendered its explanation and discovery link. No new console warning
  appeared after changing the static EmptyState title read to an explicit untracked locale read;
  no runtime error was observed.

Screens/consumers: Theatre Web only. Discovery/library Medium, full player Medium-High,
mini-player Medium-Low. Canonical v0.2.0 token roles, official icon assets, shared component
compatibility site and vi-VN/en resources are unchanged. No new shared primitive/admission or
contrast qualification is claimed. English UI scopes its language on the app container, while
Vietnamese sample titles retain their language tags.

Agent-inspected JPEGs are local-only under ignored `artifacts/pr02-2026-10-13/`:

| Capture | State / viewport / theme / locale / scale |
| --- | --- |
| `desktop-dark-library.jpg` | long-title current progress and mini-player; 1440×1000; dark; vi-VN; default |
| `narrow-dark-focus.jpg` | keyboard-focused bookmark removal above mini-player; 320×800; dark; vi-VN; default |
| `narrow-light-empty.jpg` | empty catalog with persistent player; 390×844; light; en; default |
| `narrow-light-search-focus.jpg` | search recovery focus; 390×844; light; en; default |
| `compact-light-player.jpg` | paused full player; 1024×768; light; en; default |
| `desktop-light-player.jpg` | long-title simulated full player; 1440×1000; light; en; default |

Evidence: `screenshot-inspected`, `cross-viewport-inspected`, `cross-theme-inspected` for these
named cells; semantics/keyboard/focus checks cover the named controls only.

## Residual risk and next boundary

200% browser zoom could not be established: zoom shortcuts produced no observable viewport
change in this browser, so no zoom pass is claimed. Screen readers, real OS IME, forced colors,
reduced-motion runtime/recording, Firefox/WebKit comparisons and native/mobile were not tested.
Motion and exact-reference fidelity remain unobserved. No real asynchronous network scheduler,
unmount cancellation or retry exhaustion was exercised; callback order is controlled in fake-port
tests. Shared components remain unqualified under their existing admission policy.

There is no audio, real media publication identity/access refresh, listener authentication,
reload persistence, CDN delivery, synchronization or offline playback. Those retain the owning
[040 work plan](../work-plan/040-theatre-web-listening.md) and publication/API prerequisites.
This PR changes no wire schema, persisted data, cache/audio revision, provider cost, rights or
approval. The implementation handoff has a named contracts-suite limitation and closes no
production listening or UI-system gate.
