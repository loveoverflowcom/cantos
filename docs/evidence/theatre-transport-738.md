# Theatre transport and seek — issue 738

Source-bound review of [#738](https://github.com/loveoverflowcom/cantos/issues/738),
5 November 2026. Base `origin/develop@26419642c93d47583d947eb8dc7140bdb14e8a71`
includes #737's shared native-submit change. One PR, no deployment.

## Ownership and decision

The full player is Medium-High intensity: continuing listening is the primary goal,
Play/Pause the primary action, skip/seek secondary. The persistent mini-player is
Medium-Low. The existing [UI system](../design/ui-system.md#theatre-compact-listening)
and [live playback boundary](../../apps/cantos-theatre/web/README.md#live-guest-playback)
own appearance and semantics.

`features/live_player.rs` now uses catalog `button` / `Button` Primary for transport,
Tonal for the two skip controls, with `icon` / `IconName::{PlayArrow, Pause,
FastRewind, FastForward}`. The visible ±15-second labels still describe exact skips;
the shared directional glyphs replace the app-private replay/rotate artwork.
No shared component, API, variant, state or token changes. No auth capsule geometry.
The existing shared child slot carries the localized visible label and the existing
action-plus-unit accessible name; no copied Button internals or raw aria override.

The current Button qualifications remain attributable to unchanged inputs:

| Input | SHA-256 / evidence |
| --- | --- |
| Token source | `79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218` |
| Leptos renderer | `e92cc3969ad8f6b2dd7947917bb21ab48c01e63fab2fdb078428c0bf8053590b`; [#737 record](ui-requalification-737-button-leptos.json) |
| CMP renderer | `f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a`; [#615 record](ui-requalification-615-button-cmp.json) |

`python3 scripts/ui_preview_catalog.py --digests` confirms these hashes. Existing
admission must pass the repository gate for the new explicit import site; neither
baseline consumers nor qualifications are rewritten. No CMP runtime changes.

## State and token map

Both consumers use the same root `PlayerSession`, `transport_key` and
`press_transport`. The current state is read at activation, rather than captured
from an older label. The mounted native button stays stable across name/icon updates.

| Actual owner state | Label / icon | Action | Blocked / focus |
| --- | --- | --- | --- |
| No source | Consumers absent | None | No target |
| Paused, including interruption/cold restore | Play / PlayArrow | PlayRequested | Enabled, native keyboard target |
| Loading with Play intent | Pause / Pause | PauseRequested | Enabled; pending Play remains cancelable |
| Playing / Buffering with Play intent | Pause / Pause | PauseRequested | Enabled; engine state never becomes Button Loading |
| Ended at duration | Replay / PlayArrow | PlayRequested | Enabled |
| Failed, including Network/PlayRefused/Unavailable | Retry / PlayArrow | PlayRequested | Enabled; existing owner decides recovery |
| Pending seek | Current transport projection | Current intent action | Unchanged availability |
| Skip backward / forward | Localized ±15 seconds / FastRewind, FastForward | SkipRequested ±15000 | Enabled as before; reducer retains rejection/clamping |

Changing action names do not also expose `aria-pressed`. Button's default Enabled
is deliberate: loading/buffering describe the engine and cannot block Pause.
`PlayerTimeline` remains the app-owned native range with its existing scrub draft,
one final commit, cancellation, pending acknowledgement and source/intent binding.
No reducer, autoplay, network, media/session fence, progress/history, API or CMP edits.

The shared owner supplies `size.control`, `size.icon`, `space.sm`,
`space.controlInline`, action color pairs, Web control radius, focus and motion
roles. App CSS supplies only placement and a stable label envelope. All possible
localized action labels share one grid cell so changing state cannot shift the
target; noncurrent labels are invisible and the envelope is decorative. The
single localized accessible name remains exposed. Narrow mini-player insets use
`space.sm`, the decorative thumbnail yields room to the title, and the transport
slot stretches within the existing single-column layout.

Behavior SSOT lookup: the root catalog and Theatre manifests register no
transport/seek capability. This is an outcome-preserving presentation refactor;
no model/lifecycle change. The preflight reverse index derives no registered
assurance obligation, which is not proof of absence of semantic risk. Existing
player/session/scrub regressions retain that boundary's evidence.

## Execution and evidence

All commands run from the repository root in this issue's isolated checkout.
Run records, driver, JSON observations, input hashes, failures and captures are
local-only under `artifacts/agent-runs/issue738/`; no media or screenshots in Git.
Cargo uses `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

The original two shared-anatomy regressions failed on the base (9 pass / 2 fail),
then passed with the renderer migration. Browser review found the first boolean
`data-current` mapping produced an empty attribute and hid the label; the current
implementation uses explicit strings and retains a native rendering assertion.
The initial 320px/text200 observation exposed the mini-player's inherited inset
and thumbnail squeezing its content. Its failure capture is retained and the
same scenario is rechecked after the bounded layout repair.

The documented `cargo xtask app cantos-theatre --web` refused: API catalog was
unavailable at 8080 and the existing listener on 8082 was already in use. No
unrelated process was stopped. The browser harness instead serves actual compiled
frontend assets through CDP Fetch with explicit synthetic API responses; this is
bounded frontend evidence, not a real Server/Identity or product E2E result.
Only the WASM custom `name` section is omitted in the browser fixture for size;
all retained executable/data sections and both complete files are hashed in
`wasm-fixture-provenance.json`. Product build output is unchanged by that fixture.

Generated local Ogg Opus mono 24kbps/48kHz audio is separately decoded by the real
HTMLAudioElement, without autoplay-enabling flags. API publication/grant facts and
range delivery are synthetic. Explicitly dispatched `waiting` events exercise the
registered buffering callback and are labeled synthetic, not a real network stall.

## Final checks and observed scope

Checked source input SHA-256: `live_player.rs`
`bd261a51ec49c04f0681fac72e1e422bda1b1b8451d19568802fa0ba4c3d5d4a`;
`theatre.css` `1dffcfc9872aaf44169cc0ee5402a437b467c4be7626b12bf943f808da7a8c93`.
The PR and issue comment bind these inputs to the final Git commit.

| Command | Actual result / target |
| --- | --- |
| `cargo test -p cantos-theatre --features preview features::live_player::tests --locked` | First base regressions: 9 pass / 2 expected fail; migrated checkpoint: 11 pass. The final visibility assertion is also covered by the full suite below. |
| `cargo xtask build cantos-theatre --web` | PASS, development CSR bundle; final CSS also built by preflight. |
| `TMPDIR=/private/tmp/cantos738-tests cargo xtask preflight --changed-only --base origin/develop` | PASS: repository, rust-format, rust-lint (Theatre), rust-tests (Theatre), web-lint (Theatre), web-build (Theatre). No skipped or unsupported selected gate. |
| Repository tests within preflight | 1,386 tests, 20 internal optional skips; PASS. Current two-renderer admission and new import pass unchanged. |
| Theatre tests within preflight | 612 unit tests and 6 doctests; PASS. Includes stale source/intent callbacks, canceled/late scrub commits and mini/full pending-position observations. |
| `node artifacts/agent-runs/issue738/browser-check.mjs` | Final 24 observation rows PASS in headless Chrome 156.0.8078.80, with focus emulation explicit; zero runtime exceptions. |

The browser runs the real production frontend, native buttons and range. Enter
cancels pending Play; Pause works after the synthetic buffering callback; native
Space dispatches one play effect; one Pause dispatches one pause effect. A source
switch retains one audio owner and one mini button, and 404 followed by Retry
authorizes the selected second source exactly once. Accessible names contain
the current action plus its own unit, decorative icons stay hidden, and no busy,
disabled or pressed state is invented. Keyboard range + forward/back skips finish
at the expected one-second position with pause intent retained.

Sixteen matrix environments cover 320/390 CSS px × Light/Dark × VI/EN × root text
100/200%, each in mini/full mode (32 captures). Viewport height is 1,400 CSS px,
DPR 1. Text scaling is not browser zoom. Reduced motion is emulated in text200
rows: actual Button transition duration is `0s`. Every observed action is at least
48px tall/wide and fits its horizontal bounds; normal icons/gaps measure 24/8px,
text200 48/16px. Play/Pause retains its focused node and exact bounding box.

Reviewer: Codex, one automated reviewer. Opened final captures: mini/full pairs
for `320-light-vi-VN-2`, `320-dark-en-2`, `390-light-en-1`, and `390-dark-vi-VN-1`.
These eight show readable actions, separate target/label space, canonical themes,
visible focus and stable envelopes. Other captures remain captured-only. Image
review additionally found the short active label was top-aligned inside the
reserved envelope; `align-self: center` fixes it. The identical scenarios were
rerun after this final CSS-only correction; no product Rust inputs changed.

The Work cover outside this cluster still makes the document 352px wide at
320px/text200. It has the same 352px width when the entire mini-player is hidden;
the cover rules are unchanged. The mini-player itself now fits (273px outer width,
241px content/action at this scroll-bar geometry). This pre-existing Work cover
finding is retained, not hidden by a global overflow rule or fixed in a player PR.

## Review, layers and remaining acceptance

Self-review covered the complete four-file change, consumers, shared admission,
current-intent callback, localization, native focus/IDs and source/seek fences.
No remaining actionable defect was found in the changed transport scope.
No durable rule changed: L1/L2 are source/static review; L3 is the passing native
and WASM checks; L4 is actual DOM/interaction/accessibility-check and the eight
opened captures above; L5 real Server/Identity/public delivery is NOT_RUN.
There is no latency, cache, frame-pacing, native-device or screen-reader claim.
All existing localization resources, API/schema/migrations, media identities,
cost, rights and approval owners remain unchanged. No ADR or queue-order edit is
needed for this bounded reuse decision.

AC-01/02/03 have source and regression evidence. AC-05 is completed by this record
and the fresh issue checklist/comment sync. AC-04 remains open: the standard
product launch was unavailable, real publication/CDN/range acceptance is absent,
and the broader keyboard/assistive-technology/short-height coverage is not inferred
from programmatic focus, one Chromium engine, a tall viewport or generated audio.
The unrelated Work-cover overflow is also pending. Resume with a ready local
Theatre API and a free 8082, `cargo xtask app cantos-theatre --web` (or
`cargo xtask run cantos-theatre --web` for the owned native stack), then repeat the
same mini/full state matrix with a real publication and the missing targets.
Next runtime acceptance owner is unassigned. No deployment or issue closure.
