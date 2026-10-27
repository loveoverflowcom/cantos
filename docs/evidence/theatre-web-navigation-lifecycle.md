# Theatre Web navigation and logout-dialog lifetime — 27 October 2026

This is source-bound local evidence for the header lifecycle and Discover/Library selection.
It does not accept a deployed release, Identity authentication, native playback or mobile.

## Boundary and cause

Base: `origin/develop` at `6ba3b47bb005cab099b380a4f172703743619d34`.
macOS arm64, Rust 1.98.1, Trunk, ChatGPT in-app browser (engine/version unknown).
The inspected release build initially trapped with `RuntimeError: unreachable`; its temporary
panic hook reported a disposed reactive value. A temporary probe of the Button's `data-state`
reader identified **`theatre-logout-confirm`**. Neither probe logged inputs, credentials or
session facts; both were removed. The initial hypothesis concerning the login form was
refuted and its experimental changes were reverted.

The header mounted `LogoutDialog` under `Show(signed_in || dialog_open)`. A session check moves
`SignedIn → Checking`, making that condition false even when the listener has not requested
logout. The dialog's arena-backed confirm-state signal could be disposed before its queued
render effects settled. The resulting WASM trap left the location hash changing while the
heading, document title and selected destination retained their old state.

The existing native dialog now has the header's lifetime. The browser's native closed dialog
is absent from the accessibility tree; `dialog_open` alone requests the modal, and the existing
`LogoutStep::can_confirm` guard still rejects a confirmation outside admitted/ retryable states.
No authentication, cookie, permission or backend policy changed.

## Navigation composition

The existing app-owned navigation is retained in live and isolated mock shells. Shared
`NavItem` was checked in the public API/catalog; this patch extends the existing app composition
rather than changing its shared API or admission record. The selected check reuses the bundled
licensed decorative `check` SVG, reserves its space, and stays `aria-hidden`. `aria-current`,
the destination label and the paired selected colors continue to carry state.

The old later `:hover` rule replaced only the selected background. Dark foreground
`rgb(41,19,50)` on hover background `rgb(60,39,72)` measured **1.28:1**. Hover now applies the
existing token-driven state layer over the selected background. Selection colors change together;
only check opacity transitions, using the existing 140 ms feedback token. This avoids an
intermediate mismatched foreground/background pair. Reduced motion explicitly disables that
transition. No custom palette, animator, dependency or shared token changed.

## Reproduction and regression

The native `logout_dialog_lifetime_is_independent_of_session_admission` regression failed before
the lifecycle patch (guest had zero dialogs instead of one closed, retained dialog). After the
patch, all eight `features::account_menu::tests` passed, including guest disclosure behavior,
closed dialog anatomy, confirmation guard and pending-state explanation. The navigation anatomy
regression also failed before the selected check was added (zero markers instead of two).

Commands (from the worktree, Rust 1.98.1):

```sh
cargo test --locked -p cantos-theatre --features preview logout_dialog_lifetime_is_independent_of_session_admission
cargo test --locked -p cantos-theatre --features preview features::account_menu::tests
cargo test --locked -p cantos-theatre --features preview main_navigation_has_only_discovery_and_my_library
TRUNK_BUILD_FEATURES=preview cargo xtask app cantos-theatre --web --release
```

Runtime recipe on the same release app, using its existing `?mode=auth-preview#discover`:

1. Expand the existing fixture controls and choose signed-in A.
2. Choose **Hold session A response** (`theatre-preview-delay-begin`) to enter Checking.
3. Confirm the native dialog still exists once, remains closed, and the header presents a guest
   account link while admission is unresolved.
4. Complete the held response; repeat this cycle three times.
5. Open Library, Discover, Library; URL, title, h1 and the one `aria-current` destination agree.
6. Assert no console error or warning. The three observed cycles retained one closed dialog,
   changing only the header entry from button to link and back.

A second journey used the live production Web runtime with a **synthetic loopback HTTP API** on
8080, through the existing `xtask app` addressing on 8082: guest login, JSON session confirmation,
empty Library reads, logout confirmation and login again. Credentials and response facts were
synthetic. This is browser interaction and mocked HTTP integration, **not real Identity/API
acceptance**. The old source reproduced the disposed-signal trap and stale route on this seam;
the patched source navigated correctly and kept its console clear through the same operations.

## UI observations

- Existing desktop viewport (1280×720 captures, occasionally 1265×712 due browser capture), and
  a 320×800 browser viewport override; layout client width 305 px after the scrollbar. The
  document scroll width equaled client width: no horizontal overflow in the observed states.
- Vietnamese, default text scale/zoom (exact zoom unknown), System-dark and explicit Light,
  pointer input and native Enter activation of the Discover link.
- Dark/Light selected hover remained readable, with a visible check and unchanged target bounds.
  At 320 px Discover wraps at its word boundary and both destinations retain their icons, names
  and the selected marker.
- Rapid Discover/Library reversal ended on the last destination without gating input on motion.
- Three sequential screenshot/DOM samples of check opacity included complementary intermediate
  values (for example 0.415/0.585, then 0.073/0.927, then 0.010/0.990), with the new heading and
  `aria-current` already applied. A subsequent settled frame had exactly 0/1. These are scoped
  browser frame observations, not a frame-pacing or smoothness benchmark.

Evidence-local-only: captures, console JSON, frame samples and complete command logs are retained
outside the temporary worktree in `artifacts/agent-runs/20261027-navigation-fix/`. The earlier
production symptom report remains in `artifacts/ui-evidence/20261027-library/`. Images are not
committed or uploaded by this change. The PR records final preflight results separately.

## Deferred acceptance

OS/browser reduced-motion emulation was unavailable through the current browser driver; the
media-query rule and zero-duration token mapping are source evidence only. Repeat the same
navigation recipe with the browser/OS reduce-motion preference enabled before runtime acceptance.
200% text zoom, screen-reader walkthrough, Firefox/WebKit, native/mobile and deployed HTTPS/real
Identity journeys were not run. They require their respective renderer/runtime owners. No
production deployment, catalog publication or real-account data change follows from this patch.
