# Copy economy and contextual help

> **Scope.** Decide how much text a Cantos screen shows and where optional explanation lives:
> classifying copy by role, removing redundant prose, choosing between visible text, a label
> tooltip, an inline disclosure, a popover or sheet and first-use guidance, the Web and CMP
> interaction choices, accessibility and localization requirements, worked examples and evidence.
> Use when adding or reviewing labels, helper text, descriptions, empty states, tooltips,
> info affordances or onboarding on either renderer.

[UI system § Copy and contextual help](../../../../docs/design/ui-system.md#copy-and-contextual-help)
owns the requirement. This reference is the working method. It never weakens the
[accessibility](accessibility.md), [component-states](component-states.md) or
[localization](localization.md) contracts: when concision and one of them conflict, they win.
Fewer words is not the goal; a faster correct decision is.

## 1. Repository reality

| Existing part | Behavior today | Use it for |
|---|---|---|
| Leptos `IconButton` ([`button.rs`](../../../../packages/ui-system/leptos/src/button.rs)) | visually hidden name inside the `<button>`, the same name as a native `title`, a visible disabled reason tied by `aria-describedby` | an icon-only action whose name is the whole explanation |
| CMP `IconButton` ([`Actions.kt`](../../../../packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/Actions.kt)) | `contentDescription` plus Material `TooltipBox`/`PlainTooltip` showing the same name; a JVM host test keeps adjacent icon buttons named and pressable, not tooltip timing or placement on a device | the same, on Theatre native |
| `FieldHelp` / `FieldError` on both renderers | visible supporting and error text; Web associates them through the field's description IDs | a constraint that prevents an error, and validation |
| `ButtonState::Disabled(reason)` / `Loading(reason)` | the reason stays visible beside the action | blocked actions ([accessibility § 3](accessibility.md#3-disabled-with-a-reason)) |
| Studio account menu ([`account_menu.rs`](../../../../apps/cantos-studio/web/src/account_menu.rs)) | app-owned disclosure trigger with `aria-expanded`/`aria-controls` and a floating group | a Web-owned pattern to follow, not a shared export |
| design inventory `tooltip` specimen | `proposed`, no shared API or admission | nothing yet; a rich tooltip or toggletip is a new component ([preview admission](preview-catalog.md)) |

There is no shared rich tooltip, toggletip, popover or bottom-sheet export. Reuse the rows above;
anything more follows [component-first lookup](preview-catalog.md#lookup-before-product-markup).

## 2. Classify each piece of copy by role

Write the role before choosing words or a mechanism.

| Role | Examples | Placement | Never |
|---|---|---|---|
| **essential** | action label, current state, error and validation, a required-input constraint, permission/QC/rights blocker, disabled reason, destructive or irreversible consequence, empty-state recovery | visible in place, programmatically associated, in both locales | tooltip-only, long-press-only, hover-only, auto-dismissing, or moved to another screen |
| **supportive** | one-line context that speeds a decision: a format hint, an estimate label, what a filter includes | concise visible text first; contextual help only when it is genuinely optional and the screen stays decidable without it | a paragraph where a phrase works; repeated on every field or row |
| **instructional** | how to use a control or workflow | first improve the affordance (label, grouping, icon + text, state); then a disclosure, popover, sheet or first-use guide by § 3 | a permanent tutorial paragraph above a simple control |
| **redundant** | text that repeats a visible label, an obvious icon, a success already shown, the session identity, or a description elsewhere on the screen | remove it when meaning and accessibility remain | replacing it with another icon or tooltip by reflex |

Prose to cut or shorten — each is a finding, not a style preference:

- a caption repeating an obvious icon or label ("Nhấn nút Phát để phát");
- several sentences explaining one simple action;
- generic encouragement or filler that adds no decision value ("Hãy bắt đầu hành trình của bạn!");
- permanent helper text on every field when only one field has a real constraint;
- a success or session fact restated in a banner, a toast and a heading;
- the same description in several regions of one screen; one region owns each explanation.

Keep helper text that prevents an error or a lost edit. This is not a ban on visible
instruction: a creator facing an unfamiliar, costly or irreversible step needs it in place.

## 3. Choose the lightest mechanism that keeps the content reachable

| Need | Mechanism | Conditions |
|---|---|---|
| the control is unclear | better label, icon + text, grouping or state | always first; a tooltip does not repair an ambiguous control |
| a name for an icon-only control | label tooltip (the existing `IconButton`) | text equals the accessible name; the icon is familiar in context; primary actions carry visible text ([M3 § 8](material3-expressive.md#8-familiar-patterns-stay-familiar)) |
| one optional phrase about a control | short visible supporting text; a label tooltip only if it is a name | never essential content |
| a few sentences, a link or an example | inline disclosure (expand in place) or an explicit info button | opened by click/tap/Enter/Space; state exposed; content stays in reading order |
| choices, interaction or rich content | popover (Web) or bottom sheet / dialog (CMP) | focus moves in and returns to the trigger; Escape/back dismisses |
| a consequence the person must accept | confirmation dialog ([component-states § Confirmation dialogs](component-states.md#confirmation-dialogs)) | essential; never a tooltip |
| a genuinely unfamiliar interaction | first-use guidance | dismissible, re-findable from a visible entry, never blocking the primary action, never the only documentation |
| a secondary action, not an explanation | overflow menu | the trigger is visible and named; the menu is not a place for help prose |

A tooltip carries a short name or hint, contains no link, button or form control and is not the
only source of anything. Content on hover or focus is dismissible without moving focus,
hoverable and persistent until dismissed (WCAG 2.2 SC 1.4.13).

## 4. Renderer choices

Mechanism names only; implementation is owned by
[Web DOM interop](../../cantos-leptos-web/references/dom-and-media-interop.md#9-contextual-help-and-disclosures)
and [CMP design system](../../cantos-cmp-mobile/references/cmp-design-system.md#tooltips-and-contextual-help).

| Concern | Web (Studio, Theatre) | CMP (Theatre Android/iOS) |
|---|---|---|
| label tooltip | `IconButton`'s name; native `title` is a pointer-hover convenience only | `IconButton`'s `TooltipBox`; long press and pointer hover are conveniences only |
| discovery without hover or long press | the visible name, visible text, or an explicit info button | the visible name, visible text, an explicit info button, overflow menu or sheet |
| optional explanation | button with `aria-expanded` revealing an inline region, or native `<details>`; an app-owned popover following the account-menu pattern | an expandable section or an info button opening a sheet/dialog (candidate APIs, verified per target) |
| touch | hover never exists on some devices; nothing is hover-only | long press is not discoverable and conflicts with scrolling, dragging, selection, seeking, platform edge gestures and screen-reader gestures |
| assistive technology | the name and description come from DOM text and `aria-describedby`, never only from `title` | `contentDescription`, `stateDescription`, expand/collapse and custom actions; verify TalkBack and VoiceOver separately |

Choose by context, not one universal gesture: the same explanation can be a disclosure on Studio
Web and a sheet on Theatre native.

## 5. Accessibility and localization

- **Essential content is never deferred.** Errors, blockers, disabled reasons, QC findings,
  destructive confirmations and irreversible consequences stay visible and associated
  ([accessibility § 3](accessibility.md#3-disabled-with-a-reason), [§ 6](accessibility.md#6-announcements-without-spam)).
- **Every gesture has a visible or accessible alternative** ([§ 5](accessibility.md#5-keyboard-and-input-alternatives)):
  a long-press tooltip duplicates a name that is already in the semantics; an info button is a
  named control in tab and reading order.
- **Disclosures expose state.** Expanded/collapsed is announced; focus is visible, unobscured,
  contained in a modal popover or sheet, and restored to the trigger
  ([§ 4](accessibility.md#4-focus)); nothing opens on focus alone in a way that traps it.
- **Targets and scaling.** An info affordance meets the touch-target floor; tooltip and disclosure
  text wraps at 200% zoom, 320 CSS px and large font scales instead of truncating.
- **Reduced motion.** Opening and closing help never delays an action ([`motion.md`](motion.md)).
- **One copy owner.** Tooltip, disclosure, panel and accessible text use the caller's existing
  copy owner with paired vi-VN/en values ([localization § 0](localization.md#0-current-owners-interim-rules-and-open-choices),
  [§ 7](localization.md#7-what-must-be-localized)); a label tooltip reuses the accessible-name key
  rather than a second, diverging string.
- **Measure in Vietnamese.** Short English often becomes longer Vietnamese; check the longer
  locale before deciding text "fits".

## 6. Worked examples

Guidance examples, not claims about live defects in the current screens.

| Case | Keep visible | Shorten or defer | Web | CMP | Fallback | Why better |
|---|---|---|---|---|---|---|
| secondary icon-only action (bookmark) | the icon button and its saved/unsaved state | drop a caption "Nhấn để đánh dấu vị trí hiện tại"; the name `Đánh dấu vị trí 12:05` carries it | `IconButton` with name and `title` | `IconButton` with `contentDescription` and tooltip | name and pressed state in DOM/semantics; if the icon is unfamiliar, use icon + text, not a tooltip | one control, no prose, same meaning for every input |
| auth form with helper prose | labels `Email`, `Mật khẩu`; the real password rule once, on sign-up; every error | remove the welcome paragraph and per-field "Nhập email của bạn" hints | `FieldHelp`/`FieldError` associated through descriptions | `FieldHelp`/`FieldError` in reading order, error semantics | errors visible and announced on submit; a placeholder never replaces a label | faster scan; the one real constraint stands out |
| blocked primary action (Publish) | `Xuất bản tập`, the blocker list "Còn 2 lỗi QC chặn xuất bản" and a link to the findings | per-finding detail lives in the QC view | `ButtonState::Disabled(reason)` with `aria-describedby` | Studio is Web-only; the Theatre analogue is a download unavailable with a visible reason | reason focusable or in reading order, never tooltip-only | the creator knows why and where to fix it |
| empty Library | heading "Thư viện chưa có tập nào", the cause when not obvious (filter, offline) and one action "Khám phá truyện" | remove multi-paragraph product explanation | heading, text and a link/button | the same in Compose, heading semantics | the action is a named control | recovery in one step instead of reading |
| compact Theatre player action (speed, sleep timer) | play/pause, elapsed/total time, the active value as text ("1,25×", "Còn 15 phút") | timer and speed options move to a menu | menu button with `aria-expanded`, options as a list | overflow menu or sheet; no long press on the seek bar or the player surface | named controls with state descriptions; custom actions on the mini-player | the transport stays uncluttered and no gesture fights seeking |
| complex Studio workflow (stale external audio after an edit) | the stale marker with source revision, "publication blocked" and the next action | the explanation of stale rules and producer steps goes behind "Vì sao bản thu này đã cũ?" | disclosure button revealing an inline panel with links | not applicable (Studio is Web-only) | keyboard-operable disclosure, state announced, content in reading order | the decision stays on screen; the theory is one step away for those who need it |

## 7. Review questions

Use when building, inspecting or reviewing; cite findings to this heading.

1. Does each sentence have a role from § 2? Is any redundant text left?
2. Can the screen be understood and the primary action completed without opening any help?
3. Is any essential content reachable only by hover, long press, a tooltip or a toast?
4. Does every tooltip, disclosure and info affordance work by keyboard, touch and screen reader,
   with its state and focus behavior observed?
5. Does any long press or tap target conflict with scrolling, dragging, selection, seeking or
   platform/screen-reader gestures?
6. Is the same explanation repeated in more than one region?
7. Do both locales fit at 320 CSS px, 200% zoom and large font scale without truncating?

Severity and kind follow the [inspector report contract](../../cantos-ui-inspector/references/report.md#7-kinds):
redundant copy is usually `polish`; essential content that is hidden, hover-only or
long-press-only is an accessibility defect of a control.

## 8. Evidence

| Claim | Evidence | Level | Does not prove |
|---|---|---|---|
| the help is associated and its state exposed | DOM test of `aria-expanded`/`aria-describedby`; Compose semantics test of description, expanded state and actions | `DOM-tested`, `semantics-tested` | keyboard or reader behavior |
| keyboard and focus path through the help | driven walk: open, read, Escape, focus restored | `accessibility-checked` | touch or screen-reader output |
| nothing is hover-only or long-press-only | touch input on Web and a named Android and iOS target, each separately | `interaction-tested`, `device-tested` | other devices |
| a screen reader reaches it | named TalkBack, VoiceOver or NVDA walk | `screen-reader-walked` only when it happened | other readers |
| copy length and wrapping | opened captures in both locales at the affected widths and scales | `screenshot-inspected` | interaction |

No automated oracle judges verbosity; copy economy is a manual review. Source inspection of a
tooltip or disclosure is not runtime acceptance.
