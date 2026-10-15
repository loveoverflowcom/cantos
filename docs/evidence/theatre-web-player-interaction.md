# Theatre Web player interaction boundary

Issue [#154](https://github.com/loveoverflowcom/cantos/issues/154) concerns the explicit
`?mode=mock` Leptos player. The base is `develop@87b49a3` (fetched after the issue's
`ff5a86a` snapshot). This is an implementation and deterministic work-count record;
browser, visual, assistive-technology and live-audio acceptance remain separate.

## Ownership and behavior

The existing root `PlayerSession`, fake `MediaPort`, pure playback reducer and
`theatre-mock-playback@1` corpus retain playback meaning. No new engine, provider, port,
shared playback event, HTTP route, publication access or persisted fact is introduced.
The live account/discovery entrypoint never mounts this player. There is no audio element,
sound, playback clock, preload request or network seek in these scenarios.

The Web-only pure presenter is `application/scrub.rs`; `features/player_timeline.rs`
owns its Leptos shell. A native range keeps a local `Preview`, while the root player
retains `PendingSeek` across route disposal. Accepted engine-position observations stay
distinct from the reducer's requested position. Source/duration/generation binding fences
every draft and pending target. An actual matching position callback acknowledges a seek;
a stale callback cannot do so. A newer callback generation/source invalidates the old pending target,
and a current engine-ended observation clears it. None of this changes playback intent.

Native `input` updates the thumb, elapsed time and localized `aria-valuetext` locally.
Native `change` or pointer release consumes the draft before dispatching one seek; a
second release/change cannot dispatch it again or erase pending acknowledgement. Arrow,
Home/End and Page keys retain native behavior. Escape/pointer cancel cancels the draft;
blur abandons an uncommitted draft. Canceled late input is ignored and the native DOM value
is restored even if the controlled value has not changed. Bounds use the bound `Source`,
including its actual duration, rather than a mutable catalog lookup. Unknown/zero duration
cannot begin a scrub. Native browser event ordering still needs the observations below.
The event distinction follows [MDN change](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/change_event)
and [pointercancel](https://developer.mozilla.org/en-US/docs/Web/API/Element/pointercancel_event);
these references do not establish this application's runtime acceptance.

`TransportView` narrows intent/status/replay reads without replacing #153's source/status
projections. A pause action during requested play is available while the engine is Loading;
only the existing engine callback confirms simulated running. Seeking back after Ended
continues to label the action Play. The mini-player shares the root pending position,
source metadata and transport; its information anchor and action button are siblings.
It adds existing cover/fallback artwork, work/episode context and a decorative progress
line. Titles wrap. An owner-scoped `ResizeObserver` reserves the actual overlay height
for content and focus scrolling, and disconnects/removes its owned CSS property on unmount;
existing conservative CSS clearance remains the fallback. Browser cleanup is not yet observed.

## Component lookup and presentation

The existing catalog declares `range-field` (`RangeField`), `button`, `status`, and the
app-owned `listening-progress`, `mini-player` and `expanded-player` specimens. RangeField's
shared runtime qualification remains pending. Reuse the existing `gallery-range`,
`gallery-stepped-range` and `gallery-prototype-position` specimens. The gallery's field-state,
error, theme, large-text and reduced-motion controls already drive native RangeField states;
keyboard input reaches zero/maximum. These fixtures cover the existing range API and local
intents, not the app's seek protocol or an engine error. No range-specific URL recipe is added.
No shared component API or new shared product import site is added; the existing native
semantic HTML compositions remain explicitly Theatre Web-owned. No CMP qualification is claimed.

Existing catalog entrypoint (use its own gallery controls for range state and text scale;
the selected Button recipe is not a range recipe):

```text
?v=1&component=button&state=enabled&theme=dark&density=comfortable&mode=catalog#gallery-fields
```

Full player intensity is Medium-High (primary transport, secondary tools); mini-player
intensity is Medium-Low (compact continuity). Existing Lavender Stage color, shape,
control-size, spacing, focus and feedback-motion roles are consumed. No token/font/palette
or animation is added. Vietnamese/English resources add preview/pending status and an
action-plus-episode accessible name. Time values never enter a live region; the full-player
status memo changes its text only when the status key changes. Existing reduced-motion and
forced-colors rules remain. This is source review, not visual or accessibility acceptance.

## A/B/C decisions

| Topic | A | B | C | Decision / evidence |
| --- | --- | --- | --- | --- |
| Scrub | Seek on every input, as at the base | Local draft, one final commit, root pending acknowledgement | Throttle/coalesce seeks during drag | Adopt B for the explicit mock; native replay measures 1000 versus 1 engine command. Browser/reader qualification remains pending. Defer C until audible-preview demand and traces justify it. |
| Position consumers | Retain #153's narrow source/status and real timeline updates | Narrow transport projection | Animation-frame scheduling | Retain A and adopt B for transport after deterministic work counts; defer C without browser evidence. |
| Preload | Native `none`/`metadata` candidate in #13's live adapter | Compare native policies/selected hint on the same browser/network | Custom prefetch, Ogg index or Range cache | All comparisons NOT_RUN: no live adapter/delivery in this boundary. Defer to #13/#120; add no preload or cache behavior here. |

No dependency, custom drag gesture, timer/throttle, WebAudio, Service Worker, SSR, Islands,
cache policy or release-size speed claim is adopted. Static assets, compiled-WASM cache,
public API data and private/session data retain their existing separate owners and policy.
New presentation facts live in the current session/route and are never browser-persisted.

## Deterministic measurements and regression ledger

Rust 1.98.1, Leptos 0.7.8, host debug tests; one deterministic replay per row, without a
wall clock. The before seek handler is replayed through the same production context,
session and reducer with `DeferredMedia` recording effects. The broad transport dependency
is retained as an independent before oracle. These are host work counts, not browser
main-thread, DOM-write, elapsed-minute or interaction-latency measurements.

| Scenario / claim | Before | Candidate | Oracle / evidence |
| --- | --- | --- | --- |
| 1000 alternating 55 s / 5 s inputs while paused | 1000 engine seek commands | 0 before commit, 1 after commit plus duplicate release | `drag_previews_leave_playback_untouched_and_change_plus_release_seeks_once_paused`; mock-integration-tested with DeferredMedia; state/progress unchanged during preview, no start command |
| 3600 one-second position observations | 3601 broad transport-consumer evaluations | 1 narrow consumer evaluation | `transport_consumer_does_not_rerun_for_an_hour_of_position_ticks`; example-tested; play/confirm/pause/end/seek-back/rebind positive controls |
| 1000 unrelated Library saved toggles | No player change expected | Narrow transport count remains 1 | Same native test; example-tested, no browser DOM cost claim |
| Source/restore/rebinding, zero/unknown duration, bounds, canceled input | Exact rejection/cancellation expected | Pure state assertions | `application::scrub::tests`; example-tested |
| Old/wrong/out-of-bounds acknowledgement, rapid second seek, pause, ended | Old callbacks cannot replace the active target | Exact state/generation/pending assertions | `features::player_timeline::tests`; mock-integration-tested with DeferredMedia |
| Route disposal during draft or pending seek | Drop draft; preserve root pending/session | Exact root state, command count and mini/full position agreement | Same orchestration suite; not a real browser Back/Forward observation |
| Native range states, labels and sibling mini link/action | Exact HTML anatomy | Existing gallery plus player host string-render assertions | Leptos gallery/player tests; example-tested, not DOM-tested |

Commands and full local logs are retained under ignored `artifacts/agent-runs/issue154/`.
The PR's final preflight summary is the final-diff gate record; the native tests also run
the unchanged shared playback corpus. The optional release asset inventory uses
`cargo xtask web-performance cantos-theatre`; inventory bytes establish no runtime benefit.

## Verification and deferred acceptance

| Executed command (Rust 1.98.1, `CARGO_BUILD_JOBS=2`) | Result |
| --- | --- |
| `cargo test -p cantos-theatre --locked --lib -- --nocapture` | 178 passed, 0 failed; includes the shared playback corpus |
| `cargo xtask ui-preview leptos verify` | PASS — tests, WASM lint and preview build |
| `cargo xtask web-performance cantos-theatre --output artifacts/web-performance/issue154` | PASS — provenance, release build and static inventory |

The inventory contains 32 output files (2,451,461 raw bytes, 903,584 modeled gzip bytes);
the six HTML-linked files total 2,154,274 raw / 631,776 modeled gzip bytes. Brotli estimates
were unavailable. No paired byte-size improvement, deployed compression, latency or cache
hit is claimed. After inventory, evidence text changed and a preview-host-only experiment
was removed; built application/shared runtime component inputs did not change. Final preflight
covers the complete final diff.

The first preflight reported `repository: FAIL` and later gates `NOT_RUN` because the
preview-host experiment invalidated existing Leptos qualification records. Diagnosis compared
the clean base renderer digest (`82e7446300057dc01e60a0a0947255a3b9433256a60d73b4330b7c6d36f46eec`)
with the recorded digest (equal) and experimental digest (different). The experiment was
removed and existing specimens reused. Qualification hashes, evidence, policies and import
baselines were not changed. The final gate result is reported separately in the PR.

| Layer | Evidence boundary |
| --- | --- |
| L1 Architecture | Source review: same root player, inward application/presenter boundary, mock remains explicit, no cross-surface or backend import |
| L2/L3 Types, pure policy and static checks | Pure presenter tests, existing reducer/vector corpus, localized key parity, native/string-render tests and final preflight |
| L4 Application | Production TimelineControl → context → PlayerSession → reducer with named DeferredMedia; FakeMedia retains synchronous deterministic confirmation |
| L5 Browser/DB/API/media | NOT_RUN for browser acceptance; no DB/API/storage/production media path is changed or exercised |

Rendering is CSR/Trunk 0.21.14; native string rendering is a test adapter, not an SSR
deployment. Diagnostic commands are `cargo xtask ui-preview leptos verify` and
`cargo xtask ui-preview leptos run`; app build/serve remains
`cargo xtask build cantos-theatre --web` / `cargo xtask run cantos-theatre --web`.
No app server, identity, provider or infrastructure was started for this record.

No connected browser was available: computer-use inventory returned no browsers, Chrome
and in-app browser creation reported unavailable, and the Opera connector reported not
connected. No browser installation or substitute audio adapter was attempted. Consequently:

| Deferred check / status | Prerequisite and next command / scenario | Acceptance / next owner |
| --- | --- | --- |
| Browser interaction, DOM writes/main-thread and p50/p95: NOT_RUN | Connected Chromium/Firefox/Safari; run the existing app/preview commands above; paired base/candidate builds, fixed public fixture and cache/profile conditions | 320×800 and 1440×900; keyboard, rapid reversal, touch release/cancel, source switch mid-drag, route Back/Forward and 3600 simulated ticks; log seek/ack order and distributions. Unassigned — reviewer assignment required. |
| Accessibility/visual/motion: NOT_RUN | Same browser builds; light/dark, vi/en, long Vietnamese title, no cover, 200% zoom/text, normal/reduced motion | Capture and open attributed images; inspect focus, touch bounds, contrast, wrapping, status deduplication and overlay clearance; repeat route mounts and observer cleanup. Walk a named screen reader separately. Unassigned — renderer reviewer required. |
| Live media/recovery/preload: NOT_RUN | Accepted #13/#120 immutable publication delivery and their actual live adapter | Range, first audible sound, p50/p95 real seek, rebuffer, error/access-expiry recovery and native preload comparison on each supported engine; no mock substitutes. Owning #13/#120 acceptance, assignment required. |

Residual risk: native range event order, touch cancellation, real DOM focus/layout, resize
cleanup, reader output, browser latency and actual media recovery remain unobserved.
The implemented mock flow is not full #154 browser acceptance or completion of #13/#120.
