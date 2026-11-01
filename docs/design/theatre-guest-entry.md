# Theatre guest entry and session navigation

**Approved requirement:** owner decision of 1 November 2026 in
[#691](https://github.com/loveoverflowcom/cantos/issues/691) and
[#692](https://github.com/loveoverflowcom/cantos/issues/692). Implementation and renderer
acceptance remain separate. This supersedes the guest portions of #641, #527 AC-22/23,
#583 AC-05 and #690's preferences-only-in-Me placement; their signed-in policy remains.

## Entry and admission

An admitted guest enters a distinct **Landing** page, similar in content structure to Discover:
brand/intro, Sign in/Register, search/kind filters and real public Works with loading, error,
empty and retry states. Landing reuses the catalog controller, search and cards, with its own
route and composition. Public catalog reads do not await successful account lookup. Only
the account owner's admitted Guest phase offers authentication actions.

Only a confirmed SignedIn account shows primary application navigation. Guests have no rail,
rail toggle, header Menu, drawer or bottom navigation at any width. There is no navigation
gutter, hidden focus stop, scrim or body-scroll lock. Web retains signed-in adaptive rail/drawer;
CMP retains native signed-in destinations. Removal releases listeners and focus/scroll effects
without recreating account, catalog, preferences or player owners. Detail/auth keep local Back.

Idle, Checking, Failed and LogoutPending are unresolved phases, never aliases for Guest. They
retain intended destination and the owner's status/retry/cleanup actions. Private presentation
and primary navigation remain unavailable until admission. A network failure alone establishes
no logout; registration/mail proof establishes no login.

## Routing and recovery

| Input / transition | Required settlement |
| --- | --- |
| Empty root | Landing for Guest/unresolved lookup; Discover once SignedIn is confirmed |
| Guest private Library/History/Me URL | Replace with Landing; retain an allowlisted typed return intent when opening auth |
| Unresolved private destination | Preserve intent and render session recovery without private presentation/reads |
| Login success | Authorized typed return, or Discover; Landing cannot trap the admitted listener |
| Auth cancel / Back | Guest-safe source; invalidate pending intent before leaving |
| Confirmed logout / expiry | Owner clears private presentation, then route reconciles; old history cannot revive an old account |
| Public Work/search/player or shared link | Preserve destination through lookup; shared/private reads keep their existing gate |
| Verification/auth/recovery link | Existing typed route/proof handling; malformed links have recovery |
| Repeated address/session/destination | Idempotent settlement; no duplicate history write or route notification |

Public Work returns carry only canonical IDs, never arbitrary redirects or private presentation.
Search survives Landing/Discover settlement and browser history. Cancellation and account
generations reject late completions even after reopening the same route. Back/Forward uses
current admission instead of restoring an old account.

## Preferences, player and presentation

Landing/auth offer a compact preferences disclosure using the existing owner/chooser for
VI/EN and Light/Dark/System; signed-in Me retains its preferences. This is a local utility.
One shell-owned player/media session survives navigation visibility. Public mini-player and
playback are independent of primary navigation; route/session/preferences changes never start
audio or replace a public source. Private teardown retains its existing access/session fences.
Use the existing Material 3 Expressive tokens and Vietnamese typography, with keyboard/touch
focus, reduced motion, narrow/wide viewports and enlarged text.

## Shared scenarios and evidence

The single draft capability [cantos.presentation.guest-navigation](../../apps/cantos-theatre/behavior/guest_navigation.md)
provides shared `run.session-root`, `run.unresolved-session`, `run.auth-return`,
`run.auth-cancel-late`, `run.logout-expiry`, `run.public-deep-link`, `run.repeated-navigation`
and `run.preferences-player` IDs. It is an expectation, not another router.
Model runs/properties/witnesses/negative controls, Web unit/string-render/build, compiled Leptos
browser interaction, CMP common/JVM/build and Android/iOS runtime are independent evidence.
Parity requires matching observations at compatible source revisions.
