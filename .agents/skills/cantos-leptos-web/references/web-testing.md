# Web testing and evidence

> **Scope.** Choose and run evidence for Leptos Web changes: the evidence layer per claim,
> component tests with fakes injected through context, deterministic time and IDs, readiness
> waits, the stale-response regression, real-browser checks for interaction, focus, IME and media,
> the visual verification loop with scenario files, the viewport/theme/zoom matrix, exact evidence
> terms and where captures are stored. Use when adding a Web test, driving a browser or reporting
> Web evidence.

The vocabulary is the foundation's [§ 6](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified);
oracle selection is [`verification-strategy.md`](../../cantos-engineering/references/verification-strategy.md);
visual judgement is [`visual-review.md`](../../cantos-ui-design/references/visual-review.md). No Web test
harness exists yet. Tools named here are **candidates** to choose, pin and record in an ADR, after
checking their current documentation; never cite a test command the repository does not define.

## 1. Choose the evidence layer

| Claim | Cheapest adequate evidence | Evidence level | Tool candidates |
|---|---|---|---|
| Validation, editor reducer, merge, retry decision, poll schedule, playback reducer, seek clamp | plain `#[test]` and property tests on the host target | `example-tested`, `property-tested` | `cargo test`; `proptest` |
| Every error variant has a message | exhaustive `match` without `_` | `type-enforced` | compiler |
| Adapter maps DTOs and statuses to `PortError` | contract test over fixtures generated from `packages/contracts/` | `example-tested`; `differentially-tested` against the schema | host tests |
| A screen renders idle/loading/ready/empty/failed from a fake port | Leptos component mounted in a real browser DOM with a `TestWorld` | `DOM-tested` | `wasm-bindgen-test` in headless browser mode |
| A slow older response does not win | component test with a manually completed fake | `DOM-tested` (plus the pure guard `example-tested`) | as above |
| Tab order, focus return, shortcuts, IME composition, drag alternative, media controls | trusted input driven in a real browser | `interaction-tested`; `accessibility-checked` when semantics, keyboard and focus are all covered | Playwright or a CDP driver |
| Web against a real backend, PostgreSQL or object storage | browser journey over the real services | `integration-tested` | browser driver plus local services |
| Range requests and signed-URL refresh on the delivery path | browser journey with network observation through storage/CDN | `integration-tested` | as above |
| Layout, hierarchy, theme, zoom, long Vietnamese text | capture, then open and judge | `screenshot-captured` → `screenshot-inspected`, `cross-viewport-inspected`, `cross-theme-inspected` | driver screenshots; scenarios below |

Rules that keep the levels honest:

- A fake or in-memory backend is never `integration-tested`, even in a real browser.
- Events created with `dispatchEvent` are untrusted (`isTrusted = false`): they do not move focus
  on Tab, run IME composition or trigger every default action. A component test that dispatches
  synthetic events is `DOM-tested`; in this skill `interaction-tested` requires trusted input from
  browser automation.
- A local static file server is not CDN evidence, and a headless run with an autoplay-permitting
  flag is not autoplay evidence.
- A screenshot proves nothing about a screen reader; `screen-reader-walked` needs a named
  NVDA/VoiceOver/TalkBack walkthrough that actually happened.
- `wasm-bindgen-test` runs in Node unless configured for a browser; Node has no DOM, so a "DOM
  test" there tested nothing about the DOM.

## 2. Component tests with fakes injected through context

The port boundary from [`web-architecture.md`](web-architecture.md#2-layering-inside-a-surface) is
the seam. Production provides HTTP adapters at the surface root; the test shell provides small
stateful fakes through the same context type. Grow one `TestWorld` vocabulary; do not let each
feature invent its own fake ecosystem, and do not create a support crate before two consumers.

```rust
// Illustrative, proposed test support — not existing code.
#[wasm_bindgen_test]
async fn failed_save_keeps_the_typed_line_and_offers_retry() {
    let world = TestWorld::new()
        .with_locale(Locale::Vi)
        .with_script(fixtures::mot_loi_hen())   // derived from packages/contracts/examples/episode-draft.json
        .with_save_result(Err(PortError::Timeout));
    let ui = world.mount(|| view! { <ScriptEditorPage episode=fixtures::EPISODE_DEMO_01/> });

    ui.set_text(Row("dialogue-02"), "Ngày mai, mình có diễn tiếp không? Nhé?");
    ui.press_button("Lưu bản nháp");
    ui.wait_for_status("Chưa lưu được").await;

    assert_eq!(ui.text_of(Row("dialogue-02")), "Ngày mai, mình có diễn tiếp không? Nhé?");
    assert_eq!(world.script.saved_revisions(), 0);
    assert!(ui.button("Thử lại").is_enabled());
}
```

- Assert roles, accessible names, visible text, attributes (`aria-invalid`, `data-draft-status`)
  and fake state — the contract, not signal counts, internal callbacks or large DOM snapshots.
- Prefer stateful fakes (`InMemoryScriptPort`, `FakeCatalog`) to call-count mocks; count calls only
  when the count is the contract (one generation request per click).
- Name tests like theorems: `edit_during_save_survives_the_acknowledgement`.
- For a bug, make the test fail first, and say so in the report.

## 3. Determinism and readiness

| Smell | Replacement |
|---|---|
| wall clock, `Date.now()`, `Instant::now()` | injected `ManualClock` port for countdowns, sleep timer, poll schedules |
| random IDs for operation IDs or new rows | `SequenceIds` in tests; production minting behind a port |
| fixed sleeps | wait for an observable condition with a bounded timeout: a status text, an attribute, a fake's recorded call |
| network latency | `ManualPort` whose responses the test completes explicitly |
| shared fixtures mutated by tests | a fresh `TestWorld` per test |

After a signal update, await the reactive scheduler (an executor tick or the pinned version's
equivalent) instead of sleeping. A wait that times out fails with the last observed state in the
message, so a failure explains itself.

## The stale-response regression

Required for every screen with a hand-rolled request key, a search box, polling or an action whose
result targets a selection ([`leptos-async.md`](leptos-async.md#3-stale-responses)).

```rust
// Illustrative. The fake lets the test choose completion order.
#[wasm_bindgen_test]
async fn slow_older_episode_response_does_not_replace_newer_selection() {
    let (catalog, gate) = ManualCatalogPort::new();
    let ui = TestWorld::new().with_catalog(catalog).mount(|| view! { <TheatreRoutes/> });

    ui.navigate("/works/anh-den-cuoi-san-khau/episodes/1");
    ui.navigate("/works/anh-den-cuoi-san-khau/episodes/2");
    gate.complete(EpisodeId::from("2"), fixtures::episode_two()).await;
    gate.complete(EpisodeId::from("1"), fixtures::episode_one()).await;   // late and stale
    gate.all_settled().await;
    ui.settle().await;

    assert_eq!(ui.heading(1), "Tập 2: Trước giờ mở màn");
}
```

Assert only after the stale future has completed *and* the scheduler has flushed; asserting
earlier passes vacuously. Remove the guard once locally and confirm the test fails.

## 4. Real-browser checks

Choose the driver in an ADR (candidates: Playwright, `wasm-bindgen-test` with WebDriver for
component tests, a CDP-based driver, an agent browser tool for exploration). Record browser
engines and versions with every result; Chromium-only evidence says nothing about Firefox or WebKit.

| Behavior | What to drive | Notes |
|---|---|---|
| Keyboard and focus | Tab traversal, Escape in dialogs, focus return, focus after navigation, shortcut suppression in fields | capture a focus tour; check focus is not under the mini-player |
| IME composition | CDP `Input.imeSetComposition` + `Input.insertText` (Chromium) as a regression for composition handling | simulation is not a real IME; also record a manual walkthrough per OS input method (macOS Vietnamese Telex, Windows Unikey or EVKey, Android keyboard) with versions |
| Keystroke-replacement input | rapid backspace + insert sequences | asserts no validation flash, announcement or save per keystroke |
| Media | play/pause, seek, speed, buffering under throttling, `play()` rejection without a gesture, error recovery | use generated test audio, never committed audio; do not enable autoplay flags for autoplay claims |
| Navigation persistence | play, navigate across Theatre routes | one `audio` element, position advances |
| Leaks | enter and leave a screen many times | listener/poll counters or network and heap observation |

## 5. Visual verification loop

Follow the [shared UI loop](../../cantos-ui-design/SKILL.md#shared-ui-loop) and judge with
[`visual-review.md`](../../cantos-ui-design/references/visual-review.md). Web specifics:

```text
start the app with `cargo xtask run|app <app> --web` (never a hand-rolled server)
→ run the scenario: cantos-ui-inspector scenarios, e.g. studio-script-editor-v1.json or
  theatre-web-player-v1.json, copied to working notes when adapted
→ open every capture; record path, state, viewport, theme, zoom, reviewer (agent | human)
→ write findings with the rule each violates
→ fix the smallest coherent cause
→ rerun the SAME scenario unchanged; open the new captures; record the recheck per finding
```

Scenario format and ownership are in [`scenarios.md`](../../cantos-ui-inspector/references/scenarios.md);
reuse [`studio-script-editor-v1.json`](../../cantos-ui-inspector/scenarios/studio-script-editor-v1.json)
and [`theatre-web-player-v1.json`](../../cantos-ui-inspector/scenarios/theatre-web-player-v1.json)
when they cover the changed state. A scenario edited between runs cannot show that a fix worked.

## 6. The matrix — choose from what changed

| Dimension | Values | When |
|---|---|---|
| Viewport | 320 CSS px reflow width; narrow phone (e.g. 360×800); Studio compact window; wide desktop (e.g. 1440×900) | layout, panes, toolbars, dialogs |
| Theme | light and dark | any token, surface or state color |
| Zoom | browser page zoom 200%; text-only zoom where the engine offers it | text containers, fixed heights, toolbars |
| Motion | default and `prefers-reduced-motion: reduce` | any transition, mini-player expansion |
| Text | realistic Vietnamese: "Người dẫn chuyện", "Ánh đèn cuối sân khấu", "Ngày mai, mình có diễn tiếp không?", stacked diacritics, long titles, missing cover art | every changed text surface |
| State | idle, loading, ready, empty, failed, retry, conflict, expired access, stale clip | the states the change touches, plus the normal state |

Fewer cells are fine when a dimension cannot differ; state which were skipped and why. Product
conditions per surface are in the [UI system validation matrix](../../../../docs/design/ui-system.md#future-validation-matrix).

## 7. Fixtures and evidence storage

- Fixtures are original or permitted content, starting from
  [`packages/contracts/examples/episode-draft.json`](../../../../packages/contracts/examples/episode-draft.json). Never a
  private manuscript, real voice sample or provider output.
- Test audio is generated at test time (a tone or silence of known duration) into an ignored
  directory. Audio extensions are ignored by Git; do not force-add them.
- Captures, traces and reports go under an ignored run directory such as
  `target/ui-evidence/<date>-<short-sha>-<scenario>/` or `artifacts/`; check with
  `git check-ignore -v <path>` before writing. Keep the scenario, report and images of one run together.
- Commit a capture only as a deliberately approved baseline with an owner, a comparator and an
  update procedure; a folder of unreviewed images is not a regression suite.
- Redact signed URLs, cookies and tokens from traces and HAR files before sharing them.

## 8. What to report

Use the [Web report additions](../SKILL.md#web-report-additions) on top of the shared UI report.
State per claim the exact level reached, the browser engines, the backend (fake ports, local
services or a shared environment) and what was not run. "Tested in the browser" is not a level.

## Existing PNG comparison

After a real browser capture, use `cargo xtask visual-compare` with the existing approved design
PNG under the [reference-pairing method](../../cantos-ui-design/references/visual-review.md#10-reference-pairing-and-local-comparison).
Record browser/version, actual served build SHA/dirty state, CSS viewport and physical PNG size,
DPR, font readiness and exact family/version, page/text zoom, locale/theme, fixture/state and
capture timestamp. Record original HTML/MJS source revision and capture inputs for the design
reference. Wait for fonts and the specified stable UI state; review dynamic masks explicitly.
No comprehensive Web capture harness is introduced by the comparator; two existing PNGs suffice.

Attach observed DOM roles/locators to region findings only when the current DOM establishes the
mapping. A static HTML calibration is HTML evidence, not a Leptos production render. Preserve the
comparison JSON and open numbered overlay/detail crops. Recheck through the same browser,
scenario/reference/policy, with semantics, keyboard/focus and interaction reported independently.
