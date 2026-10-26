# DOM, browser and media interop

> **Scope.** Direct browser work in Cantos Leptos code: node refs, `web-sys` helpers, listener and
> observer cleanup, focus and dialogs, keyboard shortcuts, tooltips and disclosures, storage, file input, and Theatre Web
> playback through `HTMLMediaElement`, the Media Session API, autoplay policy, byte-range seeking
> and signed-URL refresh. Use when code touches `web-sys`, `wasm-bindgen`, an event listener, the
> `<audio>` element or anything the renderer does not express declaratively.

Playback *meaning* (states, user intent, completion, resume) is owned by
[`playback-semantics.md`](../../cantos-listening/references/playback-semantics.md) and progress by
[`progress-sync.md`](../../cantos-listening/references/progress-sync.md). This reference owns the Web
adapter: how browser events become reducer inputs and how reducer commands become browser calls.

## 1. Declarative first, imperative behind named helpers

| Rule | Failure mode | Good | Counterexample | Oracle | Status |
|---|---|---|---|---|---|
| Express attributes, classes, events and conditional content in `view!` | the next reactive update discards a manual mutation | `aria-invalid=move \|\| has_error.get()` | `element.set_attribute("aria-invalid", "true")` on a renderer-owned node | review | manual |
| Reach elements through a typed `NodeRef`, never a document query | a refactor makes `query_selector` match a different element | `let audio = NodeRef::<html::Audio>::new();` | `document().query_selector("audio")` | review; static check (proposed) | manual |
| Every `web-sys` call sits in a `platform/` helper returning a typed error | `unwrap()` on `window()` panics in tests, SSR or restricted contexts | `fn show_modal(d: &HtmlDialogElement) -> Result<(), DialogError>` | `window().unwrap().local_storage().unwrap().unwrap()` in a component | helper unit tests; review | proposed |
| Never `set_inner_html` with user or provider content | script text, synopsis or provider error text becomes markup injection | render as text nodes | `div.set_inner_html(&dialogue.text)` | static check for `inner_html` (proposed) | always; rich text needs a server-side allowlist sanitizer recorded in an ADR |

Treat an unmounted `NodeRef` (`get()` returns `None`) as an ordinary state, not a panic. Convert
`JsValue` errors into domain-shaped errors at the helper (`PlayRejected::NotAllowed`,
`StorageError::QuotaExceeded`) so the component can localize them.

## 2. Every listener, observer and timer has a teardown

Window and document listeners, `ResizeObserver`, `IntersectionObserver`, `MutationObserver`,
intervals, timeouts, `requestAnimationFrame`, media-query listeners, `EventSource`, Media Session
action handlers and object URLs all outlive a component unless released.

- Prefer owner-scoped helpers (`window_event_listener` returning a handle, `set_interval_with_handle`,
  `set_timeout_with_handle`) and release them in `on_cleanup`.
- Leptos 0.7+ `on_cleanup` takes a `Send + Sync` closure while most `web-sys` handles are not
  `Send`; wrap browser handles (for example with `send_wrapper::SendWrapper`) instead of leaking them.
- Keep a `Closure` alive exactly as long as its registration; `forget()` only for genuinely
  process-lifetime listeners, and say so in a comment.
- The leak test is behavioral: enter and leave the screen many times, then assert one active
  listener or poll (a counter in the helper, or network/heap observation in a real browser).

## 3. Focus, dialogs and navigation

- After client-side navigation, move focus to the new page's heading (`tabindex="-1"`) or main
  region and update `document.title`; otherwise keyboard and screen-reader users stay on a link
  that no longer exists.
- Use the native `<dialog>` with `show_modal()` through a `NodeRef` helper for confirmations
  ("Xuất bản tập Một lời hẹn, phiên bản 4?"); it provides focus containment and Escape. Restore
  focus to the invoking control on close, as the [UI system](../../../../docs/design/ui-system.md#components-and-interaction-states) requires.
- Keep focused elements visible above the fixed mini-player (`scroll-padding`, see [`styling.md`](styling.md)).

## 4. Keyboard shortcuts never fire while typing

Single-key player shortcuts are forbidden while typing in the script editor
([UI system](../../../../docs/design/ui-system.md#components-and-interaction-states)). Put the guard
in one pure-ish helper used by every global shortcut:

```rust
// Illustrative. keyCode 229 is the legacy "IME is processing" signal some engines still send.
pub fn is_typing_context(event: &KeyboardEvent) -> bool {
    event.is_composing() || event.key_code() == 229 || target_is_editable(event.target())
}
```

`target_is_editable` covers `input`, `textarea`, `select` and `contenteditable`. Space as
play/pause is allowed only when focus is on the player or the page body, never in a field. Test
both the allowed and the suppressed case with trusted key events ([`web-testing.md`](web-testing.md)).

## 5. Storage, files and unload

| API | Rule |
|---|---|
| `localStorage` / `sessionStorage` | every access is fallible (private mode, quota, disabled); versioned keys; store preferences only, never tokens, signed URLs or private manuscript text unless an ADR decides a scoped local draft backup cleared on sign-out |
| File input (import, 010) | client-side type/size checks are feedback only; the server parses DOCX/Markdown and returns the preview; do not read a large file into WASM memory to "preview" it; upload progress needs `XMLHttpRequest.upload` because `fetch` has no portable upload progress (candidate) |
| `beforeunload` | register only while the Studio draft is dirty, unregister when clean; browsers show their own text |
| `visibilitychange` / `pagehide` | pause polling when hidden; take a progress checkpoint per [`progress-sync.md`](../../cantos-listening/references/progress-sync.md) — `sendBeacon` cannot send auth headers or read the revision response, so choose `fetch(..., keepalive)` or accept a fire-and-forget checkpoint deliberately |
| Clipboard | requires a user gesture and secure context; a rejection becomes a typed, localized error |

## 6. Theatre playback through `HTMLMediaElement`

One `<audio>` element, owned by the shell ([`web-architecture.md`](web-architecture.md#6-shell-owned-state-that-must-survive-navigation)),
is the Web media engine for the shared player reducer. The reducer, its `PlayerEvent` and
`PlayerEffect` vocabulary and its vector corpus are owned by
[`playback-semantics.md`](../../cantos-listening/references/playback-semantics.md); it lives in a
Rust crate with no Leptos or `web-sys` import. This adapter **translates, never decides**:

```text
<audio> DOM events ──► PlayerEvent ──► reduce(state, event) ──► (state, Vec<PlayerEffect>)
                                                                        │
UI renders status(state)  ◄─────────────────────────────────────────────┘ effects executed by
                                                                           the media helper
```

Bind media events declaratively on the shell-owned element so they are removed with it, and keep
the imperative calls (`play()`, `pause()`, `currentTime`, `playbackRate`, `src`) in one helper
that executes `PlayerEffect`s:

```rust
// Illustrative, proposed. The view translates events; it never decides that audio is playing.
view! {
    <audio
        node_ref=audio
        preload="metadata"
        on:loadedmetadata=move |_| session.dispatch(engine_ready(&audio))
        on:playing=move |_| session.dispatch(PlayerEvent::EngineRunning)
        on:pause=move |_| session.dispatch(PlayerEvent::EnginePaused)
        on:waiting=move |_| session.dispatch(PlayerEvent::EngineStalled)
        on:timeupdate=move |_| session.observe_position(&audio)   // throttled EnginePosition
        on:seeked=move |_| session.dispatch(seek_completed(&audio))
        on:ended=move |_| session.dispatch(PlayerEvent::EngineReachedEnd)
        on:error=move |_| session.classify_failure(&audio)       // may consult the access adapter
    />
}
```

| DOM signal | `PlayerEvent` | Web pitfall |
|---|---|---|
| `loadedmetadata` / `durationchange` | `EngineReady { duration }` | `duration` can be `NaN` or `Infinity`; treat non-finite as unknown, never as zero |
| `playing` | `EngineRunning` | the only proof that audio is actually playing |
| `pause` | `EnginePaused` | also fires for hardware keys, OS surfaces and the browser itself; the reducer attributes it, the adapter does not |
| `waiting`, `stalled` | `EngineStalled` | a stall is buffering, not a failure and not a pause |
| `seeking` / `seeked` | — / `EngineSeekCompleted { at }` | the seek was requested by an `EngineSeek` effect; position is final only after `seeked` |
| `timeupdate` | `EnginePosition { at }` | fires several times per second; throttle, and never write to the server per event |
| `ended` | `EngineReachedEnd` | media end is not the product's completed-listen rule |
| `error` (`MediaError.code` 1–4) | `EngineFailed { reason }` | no HTTP status is exposed: decode errors map to `Undecodable`, but network and "source not supported" may mean expired access, so ask the access adapter before choosing `AccessExpired` or `Offline` |
| `play()` promise rejection | report it as an event the corpus covers | `NotAllowedError` is autoplay policy, `AbortError` an interrupting load or pause; propose a missing variant to `cantos-listening` with a vector instead of inventing a Web-only meaning |

Effects in the other direction: `LoadSource` sets `src` from the opaque source handle the access
adapter holds (the reducer never sees a URL); `EnginePlay` calls `play()` and handles its promise;
`EngineSeek` sets `currentTime` after clamping; `EngineSetSpeed` sets both `playbackRate` and
`defaultPlaybackRate`, because a new `src` resets the rate to the default; `UpdateNowPlaying`
feeds the Media Session below.

| Rule | Failure mode | Oracle |
|---|---|---|
| Rendered status derives from `status(state)`, never from a click flag | the button shows "Tạm dừng" while the browser blocked playback | real-browser test without autoplay permission: click → `play()` rejected → play control and reason still shown |
| The adapter only translates; every decision is a reducer arm with a vector | a Web-only branch decides whether to resume after a `pause` event | review; shared vectors pass on `Rust wasm32` |
| Playback never starts without a gesture after load or reload | autoplay rejection, or audio surprising the listener | interaction test: reload with saved progress shows "Tiếp tục nghe từ 12:34" and no audio |

### Media Session (candidate)

`navigator.mediaSession` can show title, work and artwork and receive play, pause, seek and skip
actions from hardware keys and OS surfaces. Feature-detect it, verify support in each target
browser, and expect `web-sys` to gate these bindings behind unstable-API configuration. Each
action handler dispatches the same `User*` event as the on-screen control; `UpdateNowPlaying`
sets metadata and a clamped `setPositionState` (it throws when position exceeds duration);
handlers are cleared on cleanup. It is a convenience layer: Theatre Web has no background-audio
guarantee ([040 non-goals](../../../../docs/work-plan/040-theatre-web-listening.md#non-goals)).

### Seeking and byte ranges

Seeking beyond the buffered range issues HTTP range requests. The delivery path must answer
`206 Partial Content` with `Accept-Ranges: bytes`; verify it through the real storage/CDN path
([030 acceptance](../../../../docs/work-plan/030-mix-review-and-publish.md#acceptance-criteria)), not a
local file. Clamp seek targets with a pure function over duration and `seekable` ranges. Do not set
`crossorigin` on the element unless the CDN sends matching CORS headers: plain playback does not
need CORS, and a mismatch blocks playback entirely. Delivery policy itself belongs to
[`storage-and-delivery.md`](../../cantos-publication/references/storage-and-delivery.md).

### Access refresh preserving position

The element may issue a new range request at any moment with the same URL, so an expiry can break
a seek in the middle of an episode. The bounded refresh rule is the reducer's (R7 in
[`playback-semantics.md`](../../cantos-listening/references/playback-semantics.md)) and the error
contract is the listener API's `access_expired` row
([`listener-api.md`](../../cantos-listening/references/listener-api.md)). The Web adapter's part:

1. Emit `AccessExpiring` ahead of the expiry the server returned with the authorization.
2. Execute `RequestAccess` for the *same* publication; report `AccessRefreshed { source }` or
   `AccessRefreshFailed { reason }`.
3. Execute the resulting `LoadSource { start_at }`: set the new `src`, wait for
   `loadedmetadata`, set `currentTime`, reapply speed, and call `play()` only when an
   `EnginePlay` effect follows.
4. Never swap to another publication to "fix" an error; a new release is an offer
   ([`progress-sync.md`](../../cantos-listening/references/progress-sync.md)).
5. Keep URLs out of logs, routes, storage, error text and traces.

## 7. Player controls in HTML

- Transport controls are `<button>`s with labels that name action and item: "Phát Một lời hẹn".
- The seek control is an `<input type="range">` (or an equivalent with full keyboard support)
  with `aria-valuetext` such as "12 phút 34 giây trên 45 phút", and elapsed/total time visible as
  text; waveform art is never the only way to seek ([UI system § Theatre](../../../../docs/design/ui-system.md#theatre-compact-listening)).
- Speed is a labeled control whose current value is visible ("1,25×" in Vietnamese formatting).
- Live position updates are not announced continuously; announce state changes (playing,
  paused, failed, resume offered) once.

## 8. JavaScript interop

Use typed `wasm-bindgen` bindings and convert at the boundary: `JsValue → typed Rust → domain
type`. A JS shim stays small, versioned with the app and documented beside its binding; an
undeclared global function is an invisible dependency. Do not test browser behavior by mocking
`web-sys`; put the browser behind a port for component tests and use a real browser for the rest.

## 9. Contextual help and disclosures

The copy roles, mechanism choice and examples belong to
[contextual-help](../../cantos-ui-design/references/contextual-help.md); this section is the DOM
side. Reuse existing parts before adding markup: `IconButton` for an icon-only action,
`FieldHelp`/`FieldError` for field text, `ButtonState` reasons for blocked actions.

- **Label tooltip.** `IconButton` already renders its name as hidden text and as `title`. Treat
  `title` as a pointer-hover convenience: it does not appear on keyboard focus or touch and screen
  readers handle it inconsistently. Never add `title` as the only name, description or reason.
- **Optional explanation.** A visible `<button>` with `aria-expanded` and `aria-controls`
  revealing an adjacent region in reading order, or native `<details>`/`<summary>`. The trigger
  says what it reveals ("Vì sao bản thu này đã cũ?"); the content may contain links.
- **Richer content.** An app-owned popover follows the
  [Studio account menu](../../../../apps/cantos-studio/web/src/account_menu.rs): a named trigger
  with `aria-expanded`/`aria-controls`, Escape closes and returns focus to the trigger, a focus
  move outside closes it, and opening never shifts the layout beneath. A modal choice uses
  `<dialog>` (§ 3).
- **No hover-only path.** No `mouseenter`-only listeners for content; anything shown on hover is
  also shown on focus, stays while hovered, and Escape dismisses it without moving focus.
- **No new shared primitive by stealth.** A reusable tooltip, toggletip or popover is the
  `proposed` `tooltip` specimen and needs [preview admission](../../cantos-ui-design/references/preview-catalog.md)
  on both renderers; a feature-local copy is a finding.
- **Oracle.** DOM test of `aria-expanded`, `aria-controls`/`aria-describedby` and the reason
  text; a keyboard walk (open, Escape, focus restored) and a touch-emulated or real-touch pass
  confirming nothing needs hover.

## Completion checklist

- [ ] Declarative bindings first; `NodeRef` instead of document queries; no `set_inner_html` of content.
- [ ] Every `web-sys` call is in a named helper with a typed error; no `unwrap()` on browser globals.
- [ ] Every listener, observer, timer, stream and media handler is released in `on_cleanup`.
- [ ] Global shortcuts check `is_typing_context`; focus moves on navigation and returns after dialogs.
- [ ] Help and disclosures need no hover; `title` is never the only name, description or reason.
- [ ] Playback state comes from media events through the shared reducer; intent is separate.
- [ ] Autoplay rejection, buffering, expired access and invalid seeks leave a recoverable player.
- [ ] Range requests and signed-URL refresh were observed on the real delivery path, or reported as not covered.
