# Accessibility

> **Scope.** Make and prove a Cantos screen usable with keyboard, touch, screen readers, zoom and
> large text on either renderer: measuring the documented targets, accessible names, disabled
> reasons, focus, keyboard and drag alternatives, shortcuts that respect Vietnamese input,
> announcements, the Web/CMP split and the two accessibility evidence levels.

The targets are owned by [UI system § Accessibility and motion acceptance](../../../../docs/design/ui-system.md#accessibility-and-motion-acceptance)
and [§ Components and interaction states](../../../../docs/design/ui-system.md#components-and-interaction-states),
by [mobile § UI and accessibility](../../../../docs/architecture/mobile.md#ui-and-accessibility)
and by [business rule 29](../../../../docs/product/business-rules.md#listening-and-mobile). They
are targets to verify, not claims of compliance. This reference says how to meet and measure them.

## 1. Measuring the targets

| Target (owned by the UI system) | How to measure | Common false pass |
|---|---|---|
| text and non-text contrast floors | token contrast record per theme ([`tokens.md`](tokens.md#9-contrast-records)), then inspected captures where text sits on cover art, scrims or tinted containers | a token check passing while the title sits on a bright cover |
| touch targets on Android, iOS and touch Web | bounds of the **interactive node**, not the drawn icon: DOM bounding box, Compose node bounds in a semantics test | a 24 px icon inside a 24 px button "because the design is compact" |
| 200% browser zoom, 320 CSS px reflow, native large text | captures at each setting; no horizontal page scroll, no clipped label, no hidden action | testing at 125% only; testing zoom but not OS font scale |
| names carry action, item and state | read the accessibility tree or semantics tree, not the screen | an icon button whose name is "button" |
| reduced motion | [`motion.md`](motion.md#4-reduced-motion) | shortened transitions reported as removed |

## 2. Accessible names: action + item + state

| Control | Name (vi-VN) | Name (en) | State exposed |
|---|---|---|---|
| episode row play | `Phát tập 3: Một lời hẹn` | `Play episode 3: Một lời hẹn` | — |
| full-player primary control | `Phát` / `Tạm dừng` (label switches) | `Play` / `Pause` | the label itself; no separate pressed state |
| bookmark toggle | `Đánh dấu vị trí 12:05` | `Bookmark position 12:05` | pressed or selected when saved |
| download | `Tải xuống tập 3` | `Download episode 3` | `Đang tải xuống, 42%` / `Downloading, 42%`; `Đã tải xuống` / `Downloaded` |
| generate | `Tạo thoại đã chọn: 2 dòng trong cảnh Sau buổi diễn` | `Generate selected dialogue: 2 lines in scene Sau buổi diễn` | busy while the request is in flight |
| publish | `Xuất bản tập Một lời hẹn, bản sửa đổi 4` | `Publish episode Một lời hẹn, revision 4` | unavailable plus its reason (§ 3) |
| stale clip marker | `Bản thu đã cũ: dòng đã sửa ở bản sửa đổi 4` | `Clip out of date: line edited in revision 4` | — |

- A control that switches its label (`Phát` ↔ `Tạm dừng`) must not also expose a pressed state;
  the two contradict each other when announced. A toggle that keeps its label (bookmark) exposes
  pressed or selected.
- The visible label is contained in the accessible name, starting with it, so voice control
  ("click Publish episode") works.
- Names come from localized templates with parameters, never concatenation
  ([`localization.md`](localization.md#4-parameters-plurals-and-formatting)).
- Content inside a name keeps its own language (the episode title `Một lời hẹn` in an English
  label); tag it where the renderer allows.
- Decorative icons are hidden from assistive technology; meaningful icons carry text.

## 3. Disabled with a reason

| Field | Contract |
|---|---|
| Scope | every control unavailable because of permissions, QC, rights, approvals, budget or missing input |
| Why / failure | a dead control with no explanation stalls the creator; a reason in a hover tooltip is invisible on touch and to many screen-reader users |
| Good | visible reason text beside the control, programmatically associated: `Xuất bản tập` with "Còn 2 lỗi QC chặn xuất bản" / "2 blocking QC findings remain" and a link to the findings |
| Counterexample | a greyed Publish button; a reason only in a tooltip; a reason invented by the UI instead of supplied by the backend |
| Oracle | presentation table test: every `BlockReason` variant maps to a message ([`component-states.md`](component-states.md#2-availability-is-a-type)); DOM or semantics test of the association |
| Enforcement / exception | manual · none |

Renderer technique differs: on the Web a control kept focusable with `aria-disabled` and a
described-by reason lets keyboard and screen-reader users discover why, at the cost of blocking
activation in code; the native `disabled` attribute removes it from the tab order. On CMP, whether
a disabled Compose control stays focusable and how its reason is read varies by version and
platform — put the reason into the node's semantics or into adjacent text in reading order, and
confirm with TalkBack and VoiceOver. Recipes: renderer skills.

## 4. Focus

| Rule | Good | Counterexample | Oracle |
|---|---|---|---|
| **visible** | a `color.focus` indicator that meets the non-text floor against its surroundings | removing outlines globally; a focus ring the same lavender as the selected state | focus-tour captures, light and dark |
| **unobscured** | scroll padding so the mini-player and sticky bars never cover the focused control | the last episode row hidden behind the mini-player while focused | keyboard walk to the last row with the mini-player open |
| **contained** | modal dialogs and sheets keep focus inside until dismissed | tabbing behind an open publish confirmation | driven tab cycle inside the dialog |
| **restored** | on close, focus returns to the initiating control; after removing or regenerating an item, to the next logical item | focus dropping to the document start | assert the active element or focused node after close |
| **moved deliberately** | route change → main heading; mini → full player → full-player heading; failed save → error summary or first invalid field | focus left on a control that no longer exists | assertion after each transition |
| **preserved** | Studio script position, selection and unsaved edits survive pane changes and recoverable errors ([UI system § Studio](../../../../docs/design/ui-system.md#studio-text-first-production)) | the inspector opening resets the caret to line 1 | driven flow: select, open inspector, close, assert selection |

## 5. Keyboard and input alternatives

- Tab order follows reading order; no positive tab indexes.
- **Every drag has an alternative.** Reorder dialogue or scenes with "Move up", "Move down" and
  "Move to scene…" commands; place cues with fields or arrow-key nudges; collapse the full player
  with a button as well as a swipe; seek with arrow keys (small step), Page Up/Down (large step),
  Home/End and skip buttons. Waveform art is never the only way to seek or read progress.
- The seek control exposes readable value text, not a raw number: `12 phút 5 giây trên 47 phút
  30 giây` / `12 minutes 5 seconds of 47 minutes 30 seconds`.
- **No single-key player shortcuts while typing.** Vietnamese input methods type tone and vowel
  marks with ordinary keys — Telex uses letters such as `s`, `f`, `r`, `x`, `j`; VNI uses the
  digits `1`–`5` — and some input methods rewrite text without composition events. A single-key
  shortcut therefore collides with Vietnamese typing even when no composition is reported. Allow
  character-key shortcuts only when focus is outside editable content, ignore events during
  composition, prefer a modifier, and let users turn character-key shortcuts off.
- Escape dismisses overlays; Enter and Space activate buttons through native behavior.
- Touch gestures have visible alternatives; long-press menus have a visible entry point. Tooltips,
  disclosures and other optional help follow [`contextual-help.md`](contextual-help.md).

## 6. Announcements without spam

Announce significant asynchronous changes once; never narrate progress.

| Announce | Do not announce |
|---|---|
| a production run started, paused (with reason), failed, cancelled, completed or awaits review | each clip finishing during a 40-line run |
| publication succeeded or failed | progress percentages, except on request |
| download completed or failed; playback failed; a resume prompt appeared | the playback position ticking; sleep-timer countdown |

- Use a polite region for progress outcomes; reserve assertive announcements for blocking errors
  that need action now.
- Keep the status region outside the editor's editable text, and present it before content
  changes; text inserted into a freshly created region is often not announced.
- Decide announcements in a pure function of the previous and next facts, so "no spam" is a test
  rather than a hope. Run states follow the
  [pipeline's suggested states](../../../../docs/product/production-pipeline.md#jobs-and-failure-recovery):

```rust
// Illustrative, proposed — no Studio status module exists yet.
pub fn announcement(previous: RunState, next: RunState) -> Option<Announcement> {
    if previous == next {
        return None; // progress inside a state is shown, not announced
    }
    let key = match next {
        RunState::Queued => return None,
        RunState::Running => MessageKey::RunStarted,
        RunState::WaitingReview => MessageKey::RunWaitingReview,
        RunState::Paused(reason) => MessageKey::RunPaused(reason),
        RunState::Failed(reason) => MessageKey::RunFailed(reason),
        RunState::Cancelled => MessageKey::RunCancelled,
        RunState::Completed => MessageKey::RunCompleted,
    };
    Some(Announcement::polite(key))
}
```

Tests read as theorems: `progress_inside_running_is_not_announced`,
`every_state_change_announces_exactly_once`, `pause_reason_reaches_the_message`.

## 7. Renderer split

Mechanism names only; implementation belongs to
[`cantos-leptos-web`](../../cantos-leptos-web/references/dom-and-media-interop.md) and
[`cantos-cmp-mobile`](../../cantos-cmp-mobile/references/cmp-design-system.md).

| Concern | Web (DOM and ARIA) | CMP (Compose semantics) |
|---|---|---|
| role | native `button`, `a`, `input type="range"`, `dialog`, headings — before any ARIA role | Material components' semantics, `Role`, `heading()` |
| name | visible text; `aria-labelledby`; `aria-label` only without visible text | text merged into the node; `contentDescription` when there is no text |
| state | `aria-pressed`, `aria-current`, `aria-expanded`, `aria-valuetext`, `aria-disabled` | `selected`, toggleable state, `stateDescription`, `disabled()`, progress range info |
| reason / description | `aria-describedby` | merged semantics or adjacent text in reading order |
| announcements | `role="status"`, `role="alert"` | `liveRegion` (polite or assertive); confirm the iOS mapping |
| focus | `:focus-visible`, modal `dialog`, `inert`, deliberate focus after route changes | focus requesters, focus properties, dialog focus behavior |
| row actions | buttons inside the row; never interactive elements nested in a link | custom accessibility actions for TalkBack and VoiceOver |
| content language | `lang` on content elements | locale-aware text spans; verify what reaches TalkBack and VoiceOver |
| assistive technology | NVDA, JAWS, VoiceOver, TalkBack with a browser | TalkBack; VoiceOver through CMP's iOS accessibility bridge, whose coverage must be verified |

A Compose semantics tree in a host test is not what TalkBack or VoiceOver receives; a DOM tree is
not what a screen reader announces.

## 8. Evidence

Use the foundation levels exactly
([§ 6](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)):

| Level | Requires | Never implies |
|---|---|---|
| `DOM-tested` / `semantics-tested` | assertions on names, roles and states in a live DOM or a named Compose renderer | keyboard behavior or screen-reader output |
| `accessibility-checked` | semantics asserted **and** a keyboard (or external-keyboard / switch) walk **and** focus behavior observed, on a named renderer and target | that a screen reader announces it usefully |
| `screen-reader-walked` | a named reader and version on a named platform, build and scenario, by a named reviewer, with what was heard | other readers, platforms or locales |

Automated rule engines (axe-core over a live DOM, Android Accessibility Scanner, Xcode
Accessibility Inspector audits — candidates) contribute findings for the rules they cover; alone
they are not `accessibility-checked`. Agents rarely can produce `screen-reader-walked`; when it
did not happen, the report says "screen reader: not walked" and the gap goes on the residual-risk
line.

## 9. Common mistakes

- A clickable `div`, or a link used as a button.
- Placeholder text as the only field label.
- An error shown only in a toast that disappears.
- A custom slider without value text or keyboard steps.
- An `aria-label` that contradicts the visible label.
- Live-region updates on every progress tick.
- The mini-player covering the focused control or the last row.
