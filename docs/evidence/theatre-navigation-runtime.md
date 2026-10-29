# Theatre navigation runtime regression

Implementation and bounded renderer evidence, 29 October 2026. Base:
`develop@59cbea353b02b00034374a785492f6db3ca50b9e`; code commit:
`aafdc3404af9677243d70d4aff045c4a9d0f6c48`. This is local implementation
evidence; deployment and live acceptance are separate records.

## Failure and repair

The production Work → Home → new search journey logged WASM `unreachable` and
kept old results. An actual local Theatre WASM build with temporary panic
diagnostics identified `feedback.rs`'s disposed `EdgeProgress` derived signal.
The diagnostic hook and listener counters were removed before the final build.

Two deterministic regressions failed before their respective fixes:

- Re-reading the same discovery address notified route/query readers again.
  History now publishes each fact only when its value changes, preserving
  query-before-route ordering, auth cancellation and existing request fences.
- Holding the actual `EdgeProgress` view through owner cleanup panicked at
  exactly the production diagnostic location. Its view now retains its own
  reference-counted projections until the readers are dropped. A second test
  observes completion during teardown without showing an obsolete loading label.

No panic is swallowed, no blanket `try_get` fallback was added, and no service,
wire contract, authorization, search timing, CSS, token or CMP implementation changed.

## Independent checks

Rust 1.98.1 and Trunk 0.21.14 were used with a task-owned target directory,
`CARGO_INCREMENTAL=0`, one build job and canonical `TMPDIR=/private/tmp`.
The small disk required sequential lanes; full logs and exact bundles/hashes
are retained in the task's ignored `artifacts/agent-runs/runtime-search` directory.

| Layer / claim | Result / evidence |
| --- | --- |
| Architecture and state ownership | Read-only review of navigation, browser history, shared projection, discovery/detail callers and fences; dependencies unchanged |
| Native tests / static checks | PASS: affected Studio, Theatre and UI format, lint and 1,175 tests across 19 targets; includes the three new regressions |
| WASM lint | PASS: both clients and shared UI, preview and Theatre production lanes |
| Builds | PASS: Studio and Theatre through `cargo xtask build APP --web`; gallery through `cargo xtask ui-preview leptos run` |
| Actual Theatre renderer | PASS: five Work → Home → Enter-search round trips, exact 10 catalog / 5 Work / 5 units requests, Back/Forward, clear/focus, loading-chip duplicate guard, Explore → Discover, older search completing last and route exit with units pending |
| Backend boundary | Synthetic HTTP catalog/units with controlled delays; actual production client and WASM. This is mocked integration and browser interaction evidence, not Server/Identity/CDN acceptance |

The native handoff lane was:

```sh
cargo xtask preflight --changed-only --base 59cbea353b02b00034374a785492f6db3ca50b9e \
  --skip repository --skip behavior --skip web-lint --skip web-build --skip ui-preview
```

The complementary Web lane skipped the already-passed native gates and recorded
WASM lint PASS. Its first Studio packaging attempt failed with `No space left
on device`; after preserving evidence and freeing only closed task-owned caches,
the individual canonical Studio and Theatre builds and gallery build passed.
The native lane also had an earlier disk failure, retained separately from the
successful final run. No combined all-gates PASS is claimed.

Repository qualification remains FAIL at the same 42 diagnostics as the clean
base after the bounded records below are refreshed. Python ran 1,319 tests:
1,299 passed, 18 skipped and the same two failures reproduced on the clean base
(BSD `sed` in the retired VPS cutover test and missing `flock` in reconciliation).
The registered retired-language Quint gate is NOT_RUN: this environment lacks
its pinned Node 24.19.0/npm 11.9.0 toolchain. Its model and language rewrite are
unchanged. A baseline comparison does not make any failing gate pass.

## Bounded shared qualification refresh

The whole-renderer digest invalidates five previously current Leptos records:
Button, FilterChip, LoadingState, EdgeProgress and SurfacePulse. Only those
records are refreshed; older evidence and unrelated stale records stay intact.
The actual component bodies before and after `EdgeProgress`, every stylesheet,
token and component declaration were byte-compared with the base and are unchanged.
CMP source and tokens exactly match its retained records, which are reused,
not rerun or relabeled as new native-device evidence.

Reviewer: Codex. Actual IAB Chromium 156.0.0.0; viewport 1280×720 CSS px, DPR 1,
fonts loaded. Recipe frame 390 CSS px is distinct from the browser viewport.
The gallery was built from the code above and the recipes were driven in the
actual DOM; 18 named observations and ten opened captures are retained locally.

- EdgeProgress: pending, success, error, cancel, hidden, background, disposed,
  remount and list placement. The reserved edge measured 2 px; inactive states
  painted no edge/status, retained content stayed available, and the injected
  reduced-motion recipe had no segment animation. Light/Dark images were opened.
- Button / ListenHero: actual Enter followed by native Return left one intent
  while resolving and kept focus. Light and Dark captures retain the visible
  localized loading reason and decorative MicroArc.
- FilterChip: native Space selected the long Vietnamese label, retained focus
  and the 3 px outline; Light/Dark specimens were opened. The actual Theatre
  loading-chip journey also retained focus and sent only one request for two Enters.
- LoadingState: visible polite status, no self-suppressing busy region and a
  decorative indicator. Light/full-motion and Dark/injected-reduced captures
  were opened; the latter had zero animation duration.
- SurfacePulse: twelve decorative fills shared the same sampled opacity,
  revealed the real content and stayed aria-hidden. Dark/injected-reduced
  motion held opacity 0.7 with zero animation duration; Light/Dark captures
  were opened. No frame-pacing or OS preference-binding claim follows.

The unchanged token digest is
`95b95845cbecab0827fd5d7387045a93f48ae75a18edb4624e84262c6bf6dbc7`;
Leptos source is `03d1673bdb2917d0af23be0cb5e35719f3d186b39d303fcc38cf52e2640a32f4`;
CMP source remains `401086ab7ca9c5ce6425fa32ee54040fe56d7e83a46d26dfdb3a1a5fb6e658cc`.
Qualification-only pointer/record changes were checked not to change these digests.

## Limits

The local product and gallery runs had zero new console errors in their declared
journeys. Other engines, screen readers, real IME/touch, OS motion/lifecycle,
Android/iOS, authenticated account behavior and eligible audio playback were not
accepted by this campaign. No API, VPS, Identity or APK operation was performed.
The next production boundary is the separately authorized fresh develop build,
Pages deployment and live repetition of the failing journey.
