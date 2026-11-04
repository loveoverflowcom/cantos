# Guest navigation capability inventory

Capability `cantos.presentation.guest-navigation`; lifecycle **draft**. One shared Theatre
bundle edited by Web #691; native implementation/evidence belongs to #692.

| Approved requirement | Implementation facts | Model assumptions | Pending acceptance |
| --- | --- | --- | --- |
| [Owner guest decision](../../../docs/design/theatre-guest-entry.md): distinct Landing, only SignedIn primary navigation, unresolved recovery | Web Route/LiveNavigation/LiveBoundary reuse account authority | One document/process; closed routes/phases; session validity is an input fact | Deployed, native runtime, cross-renderer parity |
| Guest private route replaces to Landing; typed auth return; cancel/late results cannot restore private presentation | Existing typed allowlist and request/account generations | Four request epochs, eight pushes, two explicit public plays; no fairness/exhaustion proof | Browser BFCache/native process restoration beyond observed scenarios |
| Public catalog/player/preferences independent of navigation | Existing data/player/preference owners | Counters/flag abstract public identity; no bytes, HTTP, OS focus or storage | Real releases/media, device audio and assistive technology |

## Scenario bindings

All IDs use prefix `cantos.presentation.guest-navigation`.

| Run suffix | Property / reachable witness | Web seam | CMP seam owned by #692 |
| --- | --- | --- | --- |
| `session-root` | guest-safe, shell-admission / guest-root | Route policy, LiveNavigation and shell | Navigation.kt, TheatreShell.kt |
| `unresolved-session` | unresolved-intent / unresolved-private | Route policy and session recovery | Account coordinator |
| `auth-return` | auth-return, return-intent / returned-private, retained-private-intent | Typed return and session observation | NavigationTest.kt |
| `auth-cancel-late` | stale-ignored / cancelled-late | AuthIntentState and LiveNavigation | NavigationTest.kt, TheatreShellTest.kt |
| `logout-expiry` | guest-safe, shell-admission / signed-out-private | Owner cleanup and reconciliation | TheatreShellTest.kt |
| `public-deep-link` | unresolved-intent | Canonical Work/shared/auth routes | Native route owner |
| `repeated-navigation` | idempotent-navigation / same-destination | Address/query no-op and browser history | Reducer tests |
| `preferences-player` | public-owners-survive / playing-guest | Shell player and preference chooser | ShellLayoutTest.kt |

CMP ownedSources/implementationTests in the manifest identify the native owners for impact
selection. [#697](https://github.com/loveoverflowcom/cantos/pull/697), source
`bd0693ece1913332d8fba270103eb4f911e76d07`, binds the same IDs to production
reducer/coordinator/composition tests and records bounded JVM interaction/visual observations.
Those source-bound observations do not establish Android/iOS or full renderer parity.
Do not create a CMP model copy. The #691 Web header placement amendment is presentation-only:
it keeps the session/navigation transitions and root preference/player owners modeled here.

Every property has a named run, reachable witness and deliberate typechecking negative control.
Default execution is model-only; no implementation observer is registered for this capability.
Do not use --conformance or label unit/string-render results Quint conformance. The ignored
behavior bundle retains source/tool/config hashes, seed, commands, traces and actual statuses.
The PR records actual outcomes separately from Web/CMP renderer acceptance.
