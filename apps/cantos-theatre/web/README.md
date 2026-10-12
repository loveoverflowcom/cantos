# Cantos Theatre Web mock

`cantos-theatre` is the existing Leptos 0.7.8 CSR/WASM build target, extended with a
**session-only listener UI mock**. Listener-facing names remain **Cantos**. No backend or
listener contract is integrated, and this mock does not complete listening issues #12/#13.

## Scope

- Hash-routed discovery, local search, three work/episode lists, library, mock full player,
  queue, listening settings, account-unavailable and unknown-route recovery.
- Deterministic Vietnamese sample titles and synopses, including a long title and an explicitly
  unavailable sample episode. These are presentation fixtures, not published or approved assets.
- Session-only save toggles, bounded manual seeking, mock play/pause/ended states, repeated-safe
  episode selection, deduplicated queue/bookmarks, bookmark return and reset/empty-library controls.
- A shell-owned mini-player keeps the chosen sample across navigation. **There is no audio or
  playback clock.** Play changes the mock status; only manual input changes the position.
- Vietnamese/English typed UI resources, content tagged `vi-VN`, system/light/dark presentation,
  locale-formatted mock speed and spoken duration, native links/buttons/search/range controls.
- Loading/empty/error catalog fixtures selected in Settings. Loading waits for the explicit
  Show sample data action; Retry only switches back to local fixtures. Neither performs I/O.

No API calls, authentication, session tokens, audio element, playback adapter, downloads,
sleep timer, sync, persistence, billing, provider calls or backend changes exist in this app.
Sign-in/sync/download/timer actions are disabled with visible and associated reasons. Reload
returns the seeded demo; resets do not affect the separate interface locale or theme choices.

## Boundaries

| File | Responsibility |
| --- | --- |
| `src/model.rs` | Pure fixtures, hash-route parsing and in-memory presentation reducer |
| `src/messages.rs` | Exhaustive typed Vietnamese/English UI resources and mock formatters |
| `src/platform.rs` | Fallible browser hash navigation and focus, with native-test fallbacks |
| `src/view.rs` | Theatre-only screen compositions and session signals |
| `theatre.css` | App layout through existing canonical semantic roles |
| `assets/icons/` | Pinned, licensed Google Material Icons Outlined markup and provenance |

This is a presentation boundary for a future listener adapter, **not** a provisional production
API or playback/sync reducer. Before real playback, follow the owning
[listening work item](../../../docs/work-plan/040-theatre-web-listening.md),
[API boundaries](../../../docs/decisions/0015-api-surface-boundaries.md) and
[listening rules](../../../.agents/skills/cantos-listening/SKILL.md). Theatre must only consume
`/api/v1/theatre/*`; no Studio session, private draft or service module is imported here.

## Design and component admission

The [2026-10-11 Web reference](https://drive.google.com/file/d/1UEMLvfNdorz4P9qZgfoVkp2HwQV_Ipwt/view)
and [handoff](https://drive.google.com/file/d/1d2LA4MxQmcW02gUJElz9RseBWV0sAAah/view)
were read, and the reference board plus all three original artwork images were visually inspected.
The board is a manually composed design board, not a browser screenshot.

The mock follows the adopted v0.2.0 paper/purple roles, serif hierarchy, tonal grouping and
responsive rail. It deliberately uses **typographic cover placeholders**: embedded artwork/fonts
are reference inputs, not redistributed repository assets. There is no font/icon CDN. Exact
reference fidelity and actual browser font rendering remain unverified.

The existing `UiRoot`, `Container`, `Surface`, `Stack`, `Select` and `EmptyState` imports stay at
`src/view.rs`, the exact compatibility site. Private Theatre screen compositions use native HTML
and canonical tokens. No shared API, token, catalog, baseline exemption or qualification changes.
The shared catalog remains **0/29 qualified** for new adoption; this mock grants none.
Future reusable controls must complete [both-renderer admission](../../../packages/ui-system/shared/README.md#preview-registration-and-adoption)
before new app adoption. App composition is not a replacement UI-system acceptance claim.

## Build, run and checks

Use the existing [xtask command surface](../../../xtask/README.md), Rust 1.87,
`wasm32-unknown-unknown` and Trunk 0.21.14:

```sh
cargo xtask doctor
cargo xtask build cantos-theatre --web
cargo xtask run cantos-theatre --web
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify web
cargo xtask preflight --changed-only
```

The run command serves `127.0.0.1:8082`. No backend/proxy is needed or added. Existing native
workspace tests include pure reducer/resource tests and Leptos string-render composition tests;
they are part of `verify rust`, not a second test entrypoint.

**Verification is incomplete for this handoff.** The assigned dot cloud environment has no
Cargo/rustc/Trunk, so required xtask commands cannot start. No toolchain was downloaded or installed.
The app has not been compiled, built, run or interaction-tested. Supplemental source/catalog/link
checks do not satisfy those gates. See the exact [evidence and remaining checks](../../../docs/evidence/theatre-web-mock.md).

## Manual browser scenarios still required

After the Web gate passes, use the supported run command and inspect real app pixels at
1440×1000, 1024×768 and 390/320 CSS px, light/dark, both locales and 200% text zoom.

1. Discovery → a work → each available sample → Player → Library → Back/Forward. The sample
   identity/position should remain stable; opening a different sample intentionally resets position.
2. Tab to Skip to main content; confirm it focuses main without changing the route. Inspect focus
   rings and ensure no focused control is hidden by the mini-player at narrow/zoomed widths.
3. Seek with arrow keys, skip past either bound, reach the end and replay explicitly. No sound or
   automatic progression should occur. Play/pause repeated actions only toggle the written mock state.
4. Add the same bookmark/queue item twice; remove focused rows, including the last row; verify
   usable focus and empty recovery. Resume a bookmark at its exact sample position.
5. Change catalog scenario, return to discovery/work/search, retry and clear filters. Existing
   library/player state should survive. Try the empty library, then Reset.
6. Check unavailable and disabled controls, all native select fields, hash deep links, an unknown
   route, Vietnamese diacritics/long titles, reduced motion and forced colors.

Real audio, reload persistence, release pinning, server synchronization, CDN delivery and
Android/iOS device acceptance remain separate future work. This mock closes no production gate.
