# Studio sign-in/reauth Fields v3 adoption, compact geometry and copy trim — #425

Status: sign-in/reauthentication adopted; **Studio metadata/authoring fields and the Select
dropdown remain outside this PR** (separate `BufferedText`/`ChoiceField` adoption, higher risk —
see "Not in this PR"). Related: [#423](https://github.com/loveoverflowcom/cantos/issues/423)
(shared component owner), [#424](https://github.com/loveoverflowcom/cantos/issues/424) (Theatre
consumer, not yet adopted), [package README](../../packages/ui-system/leptos/README.md#web-floating-fields-fields-v3--preview-only).

## What changed (base `139de160f43ba6943b6b6adb6ae0f7b67fdc9031`)

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

## Why (live user feedback during this PR, not from the issue body)

A first pass shipped the unmodified 64-unit Fields v3 geometry. The user, looking at an actual
render, called it too tall and asked for something closer to Google's sign-in fields, plus asked
to drop the decorative always-on status icon and shorten the cookie-explainer paragraph — the
right-hand column read as too busy. The reduction keeps the issue's non-negotiable anatomy rules
(no underline in any state, one single-perimeter stroke, no stroke at rest) and only compresses
the box-model constants.

## Evidence

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

### Pre-existing flake, not a regression

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

## Not run / not in this PR

Android/iOS N/A (Studio is Web-only). Real screen reader, OS IME composition, autofill, and
Firefox/Safari were not exercised; only Chromium (the built-in browser pane). Studio's
`authoring_view.rs` metadata/structured fields (`BufferedText`) and `ChoiceField`'s raw `<select>`
are **not migrated here** — they sit outside the shared `TextField`/Select system entirely and
need a dedicated adapter that preserves the buffered-text/IME/blur-commit invariants the issue
explicitly protects; that is real, separate adoption work, tracked as a follow-up against #425.
Theatre Web's own #424 adoption is untouched; the shared geometry token change affects it only
once #424 lands.
