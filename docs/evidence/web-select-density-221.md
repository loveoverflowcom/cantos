# Web Select density — #221 continuation

Status: implemented, bounded browser observations; **shared admission and acceptance pending**.
Related: [#221](https://github.com/loveoverflowcom/cantos/issues/221), merged
[PR #225](https://github.com/loveoverflowcom/cantos/pull/225),
[previous evidence](web-select-221.md), [v3 reconciliation](../design/ui-token-v3-reconciliation.md).

## Contract, owner and audit

Invariant: toolbar Select has compact painted geometry without shrinking its interaction target;
forms retain Regular geometry, and both use the existing controlled Select/menu/native fallback.
Surface: Theatre topbar (Medium-Low secondary tool), Web UI-system preview. No domain, API,
persistence, i18n policy, provider, audio, rights or publication behavior changes.

At base `26fd84c52d4b57f68beeaa7c9c78b2b68a3d1e0c`, `view/live_shell.rs` rendered a native
locale select and native Settings theme. `features/account.rs` rendered another native theme;
`features/live_player.rs` retains native listening-speed selection. `membership_preview.rs`
retains two native diagnostic selectors. The five shared Select sites in `view.rs` (mock
preferences/scenario and authorization preview) use native option children; the migrated live
topbar is the Theatre caller opting into typed options/MenuSelect. The topbar/theme/speed rule
in `styles/theatre.css` applied 48 height, 8/16 padding and Web control radius 24. The shared
`fields.rs` already delegated typed `options` to `select_menu.rs`; native option children and
SSR/unsupported-engine fallback existed. This patch extends those owners, without a second menu.

- `SelectDensity::Regular` remains default: minimum 48 and Web reading radius 12.
- `Compact` uses the existing 48 target with transparent 4-unit block borders: painted minimum
  40, 12-unit Web reading corner and 20-unit icon. Inline inset uses the existing root density's
  `space.controlInline`; text/icon and inline-label gaps use `space.sm` (8). Wrapped text can grow.
- `SelectLabelPlacement::Above` remains default; `Inline` retains a real semantic label and
  spans descriptions below the whole group. The app only hides its label visually on narrow
  layouts, preserving the accessible name.
- Popup radius is Web container 18; option radius is reading 12; option target remains 48.
  Intrinsic option/check width can exceed the trigger, bounded by the visual viewport. Existing
  selected/active/hover/focus/disabled, keyboard, Popover and native fallbacks remain owned here.
- Tokens stay canonical v0.6.0. Only `foundation.css.in` changes geometry; the existing generator
  emits `foundation.css`. Token CSS/Rust/Kotlin outputs have no value delta. Catalog-generated
  Rust/Kotlin inventories add three Web diagnostic states, without changing CMP layout.

On 2026-10-19 the task owner explicitly authorized **locale-only migration in this PR after
browser review despite missing CMP/screen-reader acceptance**, retaining the gate and pending
acceptance, with no merge/deploy. Topbar now calls shared Select with Compact/Inline and existing
`Locale::ALL`, autonyms, language tags, callback and state. Only its obsolete native-select rule
is removed. Account/Settings theme remains native because its acceptance condition is unmet.
No qualification records, compatibility exemptions or verification requirements are changed.

## Reproduction and browser observations

Build/serve: Rust 1.98.1, `CARGO_BUILD_JOBS=2`, `CARGO_TARGET_DIR` pointing at the existing local
Cargo cache. `cargo xtask ui-preview leptos run` (8083); `cargo xtask app cantos-theatre --web`
(8082). Local CSR debug builds; no API/DB is running. Theatre shows the same unavailable-catalog
state in both images; it is not a catalog/playback acceptance test.

Renderer: Codex in-app Chromium, UA Chrome **155.0.0.0**, macOS, DPR 1; local Noto font readiness
reported `loaded`. Early Chrome-extension baseline is separate: its facts were 1600×1125/DPR 0.8,
then its page automation timed out. Those images are not used as the matched comparison.
Native computer-use permission was unavailable. No assistive-technology walkthrough occurred.

Actual Theatre pairs were captured and opened by Codex:

| Before → after files | Conditions | Observed result |
| --- | --- | --- |
| `theatre-before-1280-dark-vi.jpg` → `theatre-after-1280-dark-vi.jpg` | 1280×720, dark, vi-VN, closed, same unavailable Home | Pill-like native 127×48/radius 24 becomes shared 134.47×48 target, 40 painted, compact 20 icon/radius 12; other content remains aligned |
| `theatre-before-390-dark-vi.jpg` → `theatre-after-390-dark-vi.jpg` | 390×844, dark, vi-VN, closed; 375-wide content plus scrollbar | Label remains named while visually hidden; compact surface, no second chevron or horizontal overflow |
| `theatre-after-1280-dark-vi-open.jpg` | 1280×720, dark, vi-VN | Trigger left 1049.53/bottom 72; menu left 1049.53/top 80, width 154.47, selected check and active boundary visible |
| `theatre-after-320-dark-vi-open.jpg` | 320×800, dark, vi-VN; 305-wide content | Header wraps; menu stays within viewport at x16, width 154.47; document scroll width equals content width |
| `theatre-after-768-light-en.jpg` | 768×900, light, en | Inline label fits; English copy and layout remain readable |

Resizing the open Theatre menu 320→768 reanchored it to x537.53/y80. Enter/End/Enter selected
English; both `<html lang>` and app `lang` became `en`, and focus remained on the trigger.
Home/Enter restored vi-VN. The existing Settings selector still changes the root theme.

Preview recipe version 1: `component=select`, `mode=isolated|matrix`, `state=enabled`,
`fixture=standard|long-vietnamese`, `locale=vi|en`, `theme=light|dark`, `frame=320|390`,
`text=100|200`, `motion=system|reduced`. Frame width is a specimen constraint, not a viewport.
Observed:

- Regular target 48 / icon 24; Compact target 48 / 4-unit borders / icon 20. Compact text and
  chevron use intrinsic width. Open popup shows selected check separately from keyboard-active.
- Compact End/Escape retained vi-VN and emitted no intent. Enter/End/Enter committed `en` once;
  Space/Home/Tab committed vi-VN once and focused the explicit native baseline.
- Regular Enter/Home/Down/Enter committed `light` once; Space/End/Tab committed `dark` once
  and advanced focus to Compact. Typeahead `e`/Enter committed `en` once.
- Disabled controls expose native `disabled` and their reason; loading additionally exposes
  `aria-busy=true`. Opened disabled and large-text screenshots retain readable explanations.
- At 390×844, 200% **specimen text** computed to 32px. Long Vietnamese values wrap and controls
  grow; the oversized menu scrolls within the viewport and flips above. This is not browser zoom.
- Reduced-motion recipe suppresses transitions; the menu adds no animation. OS preference was
  still false. No recording/frame-pacing or OS reduced-motion claim is made.

Artifacts are ignored and retained outside the disposable worktree at
`artifacts/ui-evidence/20261019-select-density-221/` in the original checkout. These are actual
browser JPEGs, DOM/facts and fingerprints, not design-reference approval. No screenshot is committed.

## Verification ledger and limits

| Layer | Evidence held | Limit |
| --- | --- | --- |
| L1 ownership | source audit; existing Select/MenuSelect/CSS/token pipeline; no new runtime/dependency | no shared admission implied |
| L2 presentation types | closed density/label-placement enums; existing controlled values/states | no domain rule changed |
| L3 deterministic regression | initial typed-contract test failed before implementation; checkpoint `cargo xtask ui-preview leptos verify` PASS (113 native examples + 5 doctests, WASM Clippy and Trunk build); popup width boundary examples | SSR/compilation do not establish pixels, browser fallback or AT |
| L4 component/application | trusted keyboard/pointer input, DOM names/state, locale/language/focus observations above | no backend, durable preference or full account flow acceptance |
| L5 renderer | named opened browser images at 320/390/768/1280, light/dark, VI/EN and 200% specimen text | matrix below remains required |

Final changed-only preflight results and exact source commit are recorded in the PR, independently
from this component checkpoint. Logs are retained under `artifacts/agent-runs/20261019-select-density-221/`.
The clean-base repository check already reports stale qualifications/adoptions; the new Select
import and changed shared catalog retain their admission failures. Do not treat a skipped
repository gate or passed available-gate run as full acceptance. Default Python 3.9 failed the
checker's existing type annotations; installed `/opt/homebrew/bin/python3` (3.14) is used for
subsequent gates. No tool was installed.

Outstanding acceptance, owner **unassigned — assignment required before acceptance**:

- Actual browser 200% zoom; screen-reader walkthrough (VoiceOver/NVDA) with native names,
  announcements, active/selected and focus behavior; touch and other engines.
- Unsupported-Popover feature-probe fallback and native form validity. The explicit native
  specimen and SSR regression are separate evidence, not an unsupported-engine test.
- Full popup scroll/edge/resize, menu-option hover, forced-colors and OS reduced-motion matrix;
  reviewed design reference and visual acceptance; cross-renderer qualification.
- Installed supported Gradle/JDK and `cargo xtask ui-preview cmp verify`, then refreshed, honest
  admission records. Keep Account/Settings migration pending and rerun final preflight after
  acceptance work. No merge, deployment or issue closure is authorized by this record.
