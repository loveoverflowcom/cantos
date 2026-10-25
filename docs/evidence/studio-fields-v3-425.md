# Studio Fields v3, script choices and CTA adoption — #425

Status: two slices. Slice 1 (PR #436) adopted Fields v3 on sign-in/reauthentication. Slice 2
(below) adopts the shared field anatomy, Select and CTA hierarchy in the structured script editor
(metadata, characters, acts, scenes, dialogue, pronunciation, cues, advanced JSON). **Bounded
local Chromium observations; browser, OS IME, autofill and screen-reader acceptance pending.**
Related: [#423](https://github.com/loveoverflowcom/cantos/issues/423) (shared component owner,
open), [#424](https://github.com/loveoverflowcom/cantos/issues/424) (Theatre consumer),
[package README](../../packages/ui-system/leptos/README.md#web-floating-fields-fields-v3--preview-only).

## Slice 2 — structured editor fields, script choices and CTA

Base `develop` @ `e082d2d8df51056678611361d0c39b590e650acf`.

### Inventory and decisions

| Surface | Before | Now | Exception / reason |
| --- | --- | --- | --- |
| Sign-in / reauth fields | shared `TextField(Floating)` (slice 1) | unchanged | — |
| Sign-in / reauth submit | `FormSubmit`, always tonal | `FormSubmit variant=Primary` + existing `Login` icon | the form is the screen's current task; other `FormSubmit` callers (revision read, delegation issue) keep the tonal default |
| Metadata, titles, character/dialogue/cue text, pronunciation, advanced JSON | local `BufferedText` in a bare `div`, `aria-invalid` rendered from a bool | same `BufferedText` buffer/commit, wrapped in the shared external-label anatomy `cui-field` (`data-variant="regular"`, `data-state`), literal `aria-invalid="true\|false"`, error message inside the field, `cui-textarea` on textareas | external label kept on purpose: editor and long labels (issue rule); `BufferedText` is **not** replaced by the controlled `TextField` |
| Language, role, speaker, emotion, cue kind/anchor/edge | hand-rolled native `<select>` + chevron | shared `cantos_ui::Select` with `options` (menu presentation, native fallback) | read-only maps to `ChoiceState::Disabled` with a short localized reason ("Chỉ đọc"/"Read only"): native select has no readonly state |
| Editor tools | every tool an outlined `secondary` button | add actions `tonal`; undo/redo, move, remove, cancel, findings `text`; remove confirmation `danger` | M3 Expressive low intensity for the editor: quiet tools, fewer borders; no new icons (registry admission) |
| Studio CSS | global `label` margin and outset `:focus-visible` ring applied to every native control | both exclude shared field anatomy (`.cui-field > .cui-field-label`, `.cui-field-shell > .cui-field-label`, `.cui-field-control`) | other native controls keep the existing ring and label spacing |

The speaker and cue-anchor options are authored script text and carry the document's content
language; interface-copy choices (emotion, kind, edge, language names) stay in the interface
language. An unresolved speaker/anchor stays selected as one disabled entry in interface copy and
is explained by the field error; it is never dropped or offered as a choice.

### Root causes found by measuring the running app

- **Outset focus ring on every Studio field.** `studio.css` loads after `foundation.css`, and its
  `.studio-shell :where(… input, textarea, select):focus-visible` rule (same specificity) replaced
  the shared inset perimeter with a 3px ring offset +4px — the halo Fields v3 forbids — on authoring,
  import, production and other app-composed fields, and on invalid fields it was focus-colored
  instead of error-colored. Measured before: `solid 3px 4px`; after: `solid 2px -2px` (error color
  when invalid).
- **Invalid buffered fields were neither announced nor painted.** `aria-invalid=<bool>` renders
  `aria-invalid=""` in tachys 0.1.9, which ARIA reads as `false` and which `[aria-invalid="true"]`
  does not match.
- **Sign-in Floating shells grew from 58 to 74 when focused or filled.** Studio's global label margin
  (8 + 8) reached the floated label *inside* the shared shell. Slice 1 reduced the shared Fields v3
  geometry 64 → 58 after feedback that the field was too tall; that feedback was given on this
  74-unit render. The shared geometry is left as slice 1 set it; whether 58 or the 64-unit design
  is preferred now that the inflation is gone is an open review question for the #423 owner.
- **Pre-existing panic on `develop`.** Making the draft invalid JSON while the Metadata pane is
  mounted panicked (`RuntimeError: unreachable`; "tried to access a reactive value … already
  disposed", from the old `ChoiceField`): the parse change and the pane teardown run in one flush.
  `ChoiceField` now projects value/options/finding from one `Memo` that holds its last value while
  the draft does not parse, so neither it nor the shared Select re-runs on disposed signals.

### Evidence

Runtime: `cargo xtask run cantos-studio --web` (Trunk 0.21.14, debug), driven through a small
local CDP script in headless Google Chrome 156.0.8078.47. The authoring editor needs a Studio
session that this machine does not have, so the new **debug-only** composition
`?preview=studio-authoring` mounts the real `StructuredEditor` over the accepted Script IR fixture
`two-scenes.json` with four recipes (`editable`, `invalid`, `readonly`, `long`), synthetic findings
using real rule codes, and a visible draft-commit counter. Like the revision shell, an accepted
edit clears findings in the same flush. It makes no API, session or storage request and is absent
from release builds. Baseline: the same preview added to an isolated `origin/develop` checkout,
served by Trunk on another port, same script.

| Claim | Oracle | Baseline (`develop`) | This change |
| --- | --- | --- | --- |
| Buffered fields reuse shared anatomy, literal `aria-invalid`, error inside the field | `authoring_view::tests` (native SSR): `buffered_fields_reuse_…`, `a_finding_marks_…` | — | PASS; both fail when `aria-invalid` returns to a bool or the wrapper drops `cui-field` (hand mutation) |
| Script choices use the shared Select; unresolved value kept, disabled, explained; content language on authored labels | `script_choices_render_the_shared_select_…`, `choice_projection_keeps_…` | — | PASS; fail when the placeholder is enabled (hand mutation) |
| Read-only: native readonly, quieter tone, choices disabled with reason | `read_only_text_stays_native_readonly_…`; CDP: 8-character selection copied from a readonly textarea | — | PASS |
| Quiet editor tools | `editor_tools_drop_outlined_buttons_…` | — | PASS |
| Studio CSS leaves focus and label spacing to the shared anatomy | `studio_css_leaves_field_focus_…` | — | PASS; fails when the focus exclusion is removed (hand mutation) |
| Sign-in/reauth submit is primary; other forms stay tonal | `signin_form::tests::the_current_sign_in_…`, `ui::tests::native_form_submit_…` | — | PASS |
| Invalid idle = 1px inset error perimeter; invalid + focus = 2px inset error perimeter | CDP computed style | `none` / `solid 3px +4px` focus color | `solid 1px -1px` / `solid 2px -2px` error color |
| Active IME composition writes neither model nor control; composition end acknowledges once; plain typing stays buffered; blur commits once; second blur none; caret kept after a mid-text composition; pane switch keeps the edit; Escape restores | CDP `Input.imeSetComposition`/`insertText` (simulated IME, not an OS IME) + commit counter | PASS (same results) | PASS (same results) |
| Select keyboard: open, Escape closes without commit, ArrowDown ×2 + Enter commits once, focus returns to the trigger with a 2px inset perimeter; a speaker finding focuses the enhanced trigger | CDP keyboard | native select | PASS |
| No horizontal page scroll at 390 CSS px and at 200% (720 CSS px, DPR 2), long Vietnamese titles, EN | CDP | PASS | PASS |
| Invalid JSON disposes the structured panes without a panic | CDP + console | **FAIL** (panic, Metadata pane) | PASS (Metadata, scene and character panes; editable and invalid recipes) |
| Sign-in shell height when filled/focused; submit hierarchy | CDP | 74; tonal | 58; primary (light `rgb(103,80,128)`, dark `rgb(217,189,241)`) |

Opened captures (kept with the CDP driver in the ignored run record
`artifacts/agent-runs/425-studio-authoring-fields/`; not committed): editor before/after (light,
dark, VI/EN, invalid, readonly, 390, 200%, forced colors), focus perimeters, open speaker menu,
findings inspector, sign-in form light/dark, and the other Studio consumers of the shared field
anatomy (`?preview=studio-artwork`, `?preview=studio-forge`), whose only change is the label–control
gap now coming from the shared field instead of the Studio label margin.

Checks on the final diff: `cargo xtask preflight --changed-only --base origin/develop` stops at
`repository` (FAIL, see below). With that step skipped after diagnosis: `rust-format`,
`rust-lint (cantos-server, cantos-studio)`, `wav-reference`, `audio-inspection`,
`web-lint (cantos-studio)` and `web-build (cantos-studio)` PASS. `rust-tests` failed once on
`cantos-server` `test_support::port_ack_tests::production_settings_rights_snapshots_and_approvals_reconcile_real_unknown_commits`
with `Open(AlreadyOwned)` (710 passed, 1 failed); that test passes alone and the reopen race is
known on clean `develop`; this diff does not touch `cantos-server`. `cargo test -p cantos-studio --locked`:
372 unit + 2 doc tests PASS. The `authoring_view` module now compiles natively (its only browser
random source, the structural ID mint, refuses there), so its string-render tests run in the
ordinary Rust gate.

### Not run / remaining acceptance

- Real Studio session (signed-in revision and proposal panes with backend findings, failed save,
  reauthentication with an unsaved draft): no Studio-allowed account on this machine.
- OS IME (Telex/VNI), real autofill, screen readers, Firefox/Safari, touch. CDP composition is a
  simulation; forced colors were emulated.
- `repository` gate: FAIL on `develop` already (31 findings: stale Leptos/CMP qualifications and
  new-use lines). This change adds one line of the same class — `authoring_view.rs: new use requires
  both preview qualifications: ErrorState, Select` — because the shared Select adoption at a new
  product site waits for the preview qualification refresh owned by #423. No baseline entry was
  added to bypass it.
- Disabled buttons keep the shared disabled fill (`surface-container`), so a disabled text tool
  reads as a quiet pill; that styling belongs to the shared Button owner.

## Slice 1 — sign-in/reauthentication (PR #436)

### What changed (base `139de160f43ba6943b6b6adb6ae0f7b67fdc9031`)

- `apps/cantos-studio/web/src/signin_form.rs`: the email/password `TextField` calls in
  `SignInPanel` (used for both the entry sign-in form and the in-workspace reauthentication form)
  now pass `variant=TextFieldVariant::Floating`, per the README's documented consumer-migration
  note. No app-local underline/halo/placeholder override existed to remove.
- The empty-content `Status` row below the submit button (always rendered with its default
  neutral `MenuBook` icon even when there is nothing to announce) is now `hidden` until a real
  failure exists. The live-region markup and its text are unchanged for the failure case.
- The long `MemoryHelp` paragraph under the entry form (explaining the HttpOnly cookie mechanics)
  is shortened to its one actionable point: unsaved drafts live only in the current tab. VI and EN.
- Shared `packages/ui-system/leptos/foundation.css.in` (regenerated into `foundation.css` via
  `python3 scripts/ui_tokens.py`): the Fields v3 shell geometry is reduced from a 64 to a 58
  minimum height (label 12→8 from the top, value gap unchanged, bottom padding 8→6), closer to
  the 56dp Google/M3 compact field. This is a token-only change — anatomy (one shell, no
  underline, single inset perimeter) is unchanged — but it is shared with Theatre Web's pending
  #424 adoption; #424 has not adopted `Floating` yet (confirmed: no call sites), so there is no
  other current consumer to review before this ships.

### Why (live user feedback during this PR, not from the issue body)

A first pass shipped the unmodified 64-unit Fields v3 geometry. The user, looking at an actual
render, called it too tall and asked for something closer to Google's sign-in fields, plus asked
to drop the decorative always-on status icon and shorten the cookie-explainer paragraph — the
right-hand column read as too busy. The reduction keeps the issue's non-negotiable anatomy rules
(no underline in any state, one single-perimeter stroke, no stroke at rest) and only compresses
the box-model constants.

### Evidence

| Claim | Oracle | Result |
| --- | --- | --- |
| Email/password render the shared Floating shell | `email_and_password_use_the_shared_floating_fields_v3_shell` (`signin_form.rs`), asserts `data-variant="floating"` × 2 and `cui-field-shell` × 2 | PASS; fails when `variant=...` is reverted (mutation-checked by hand) |
| Status row is hidden with no failure, shown with one | `the_status_row_stays_hidden_until_there_is_a_failure_to_announce` | PASS |
| Every existing signin_form test (native form, submit/progress swap, controlled password, failure announcement, rate limit/reauth copy) | existing `signin_form::tests::*` | PASS, unchanged |
| Generated CSS matches the token template | `python3 scripts/ui_tokens.py --check` | PASS |
| `cantos-ui` package (pure string-render/layout tests, doctests) | `cargo test -p cantos-ui` | PASS |
| `cantos-server` + `cantos-studio` lib/bin tests | `CANTOS_TEST_IDENTITY_BINARY=... cargo test -p cantos-server -p cantos-studio --locked` | PASS (705/706 + cantos-studio), see "Pre-existing flake" below |
| `cargo fmt --all -- --check` | — | PASS |
| `cargo clippy -p cantos-server -p cantos-studio --all-targets --locked -- -D warnings` | — | PASS |
| `cargo clippy -p cantos-ui -p cantos-studio --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview --locked -- -D warnings` | — | PASS |
| `trunk build --locked` (production Studio Web build) | — | PASS |
| Real browser render: resting/focused/typed, light/dark (`prefers-color-scheme` emulation), 390×844 narrow viewport | `cargo xtask run cantos-studio --web` + Claude's built-in browser pane, headed, real click/type/focus | resting and focused both show one clean inset perimeter with the floated label, no second border, no clipping at 390 width; dark mode consistent; the status row and cookie paragraph are gone until a real sign-in failure occurs (reproduced via a forced-fail credential attempt against the local dev stack) |

#### Pre-existing flake, not a regression

`cargo xtask preflight --changed-only --base origin/develop` fails at the `repository` step
(`python3 scripts/check_repository.py`) with a long list of "stale source or token qualification"
findings across unrelated Theatre Web/Mobile and Studio files. Reproduced identically on
`origin/develop` with this branch's changes stashed — pre-existing, not caused by this PR; it is
the known qualification-staleness left by the recent #423/#426/#430 series (any edit under
`packages/ui-system/leptos` or `cmp` stales every consumer's recorded qualification, repo-wide).
The remaining preflight steps were therefore run individually (see table above) instead of through
the `preflight` wrapper, which stops at the first failing step.

`cargo test -p cantos-server` intermittently fails one or two of
`platform::turso::tests::business::*`, `platform::store::tests::a_production_host_refuses_...` or
`platform::identity::turso::tests::bindings_survive_reopen_...` with `Open(AlreadyOwned)`. Reran
twice: a different subset failed each time (4 of ~710, then 1 of ~710), consistent with a
parallel-test file-locking race in the shared Turso dev fixtures, not a regression from this UI-only
change (these suites do not touch `cantos-studio` or the Leptos UI-system).

### Not run / not in that PR

Android/iOS N/A (Studio is Web-only). Real screen reader, OS IME composition, autofill, and
Firefox/Safari were not exercised; only Chromium (the built-in browser pane). Studio's
`authoring_view.rs` metadata/structured fields (`BufferedText`) and `ChoiceField`'s raw `<select>`
are **not migrated here** — they sit outside the shared `TextField`/Select system entirely and
need a dedicated adapter that preserves the buffered-text/IME/blur-commit invariants the issue
explicitly protects; that is real, separate adoption work, tracked as a follow-up against #425.
Theatre Web's own #424 adoption is untouched; the shared geometry token change affects it only
once #424 lands.
