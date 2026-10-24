# Web Floating fields and control chrome — #423

Status: implemented in the shared Leptos package; **bounded browser observations, acceptance
pending**. Related: [#423](https://github.com/loveoverflowcom/cantos/issues/423), adopters
[#424](https://github.com/loveoverflowcom/cantos/issues/424) and
[#425](https://github.com/loveoverflowcom/cantos/issues/425), [package README](../../packages/ui-system/leptos/README.md#web-floating-fields-fields-v3--preview-only).

## What changed (base `97d192de010761d62158190f360c198d21fafd1b`)

- `TextFieldVariant::Floating`: one shell paints fill and state stroke; control and label inside;
  messages outside below. Legacy variants keep their DOM.
- Field stroke rules for every `.cui-field-control` (including Select): underline bar and offset
  focus ring removed; invalid is a 1-unit inset perimeter, focus/expanded a 2-unit inset perimeter.
- Select popup: decorative border removed (elevation only; forced-colors border restored).
- Preview: the `auth-tonal` recipe now shows Floating beside the AuthTonal baseline. The shared
  catalog gains the `floating` text-field state; generated Rust and Kotlin inventories changed
  metadata only (no CMP layout or behavior).
- `Button`, `select_menu.rs` and `navigation.rs` needed no source change: Primary/Tonal with the
  existing `PersonAdd`/`Login` registry icons already provide the required CTA pair.

## Observations (local `cargo xtask ui-preview leptos run`, Chromium in the Claude browser pane, headed)

Source SHA `97d192de` + uncommitted working tree for this PR.

| Scenario | Result |
| --- | --- |
| Floating, light, value present | shell 64 high, 12 corner, label box 0–30 (12 inset + 18) at 12px, value box starts 30 + 2 gap = 32; no stroke |
| Floating, cleared + focused (real click) | label floated, one 2-unit inset perimeter, caret visible |
| Floating, empty + blurred | label rests at 16px inside the 64 shell, no stroke |
| Floating, error, dark, long Vietnamese label, text 200, frame 320 | label wraps to two lines, shell grows to 101, no clipping, one 1-unit error perimeter, error text below |
| Regular TextField, real focus | 2-unit inset perimeter (`outline-offset: -2px`), radius followed, no box-shadow |
| Select regular/compact at rest | tonal fill, no outline |
| Compact Select expanded | 2-unit perimeter at the 40-unit painted surface (offset −6 = −(4 + 2)), popup border 0 with elevation |
| Button loading, long Vietnamese, frame 320 | icon+label group equal 12/12 inline gaps while wrapped (110 high) |

## Not run — recorded gaps

Forced-colors and reduced-motion emulation (no browser switch available here; reduced motion is
trivial because the label does not animate), real OS IME composition, real browser autofill,
screen reader, keyboard-only walkthrough of every state, the Drive design boards (private links not
opened here), Firefox/Safari, and the CMP renderer. Static string-render tests do not establish any
of these. Consumer rollout (#424/#425) and CMP (#421/#229) are separate.
