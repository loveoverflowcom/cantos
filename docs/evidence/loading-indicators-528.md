# Loading, busy and progress indicators — inventory and contract (#528)

Source: `develop` at `24b04d2aaa2044815e066b6b0db7e21df1994eae` plus this PR. The inventory
is a code-review ledger built from `rg` over `apps/cantos-theatre/{web,mobile}`,
`apps/cantos-studio/web` and `packages/ui-system/{leptos,cmp}` for `loading|busy|pending|spinner|
progress|indeterminate|skeleton|shimmer|buffering|refresh|retry|upload|download|submit|fetch`, then
reading each owner; it is not an AST proof of completeness. Nothing here is browser or device
acceptance: see [Evidence](#evidence-and-not-run).

## Contract

| Part | Leptos (`cantos-ui`) | CMP (`com.cantos.ui`) |
| --- | --- | --- |
| Indeterminate indicator | package-internal `LoadingIndicator` → `.cui-loading-indicator` arc | package-internal `LoadingIndicator` composable arc |
| Section state with text | `LoadingState` = indicator + visible label | `LoadingState(text)` = indicator + label, `stateDescription` |
| In an action | Button / IconButton / Select busy state mounts the indicator | `ButtonState.Loading` composes the indicator |
| Full motion | CSS rotation, `--cantos-web-loading-turn` 1000 ms (a Web design seed) | one `infiniteRepeatable` turn per `PendingRingTurnMillis` (1000 ms) |
| Reduced motion | `animation: none`; stationary visible arc | no transition is composed at all; stationary visible arc |
| Lifecycle | loop exists only while the node is mounted; a hidden ancestor ends it | loop exists only while composed; leaving composition cancels it |
| Semantics | `aria-hidden` arc; adjacent text/reason is the status | arc has cleared semantics; label/reason is the status |

The two renderers share intent, the MicroArc geometry (Loading v1 2/4, #586: `size.loadingArcCompact` 16 in a
button, chip or select slot, `size.loadingArc` 20 for `LoadingState`, `size.loadingArcStroke` 2, a 270° arc with round
ends; a glyph slot keeps the 24-unit `size.icon` footprint) and the
1000 ms turn; they share no runtime. Not a canonical token: no continuous-motion role exists yet, so
each renderer keeps one named constant (see [motion](../../.agents/skills/cantos-ui-design/references/motion.md)).

The indicator is deliberately **not** a public component: it is consumed through the registered
Button, Select and `LoadingState`, so the catalog inventory gains no entry. Consequently `LoadingState`
is the only shared loading API an app can import, and an app region that needs an indicator uses it.

Decided shapes, from real consumers only:

| Variant | Decision |
| --- | --- |
| inline / button / select trailing | **implemented** (existing consumers, now one component) |
| page / section with a label | **implemented** (`LoadingState`) |
| determinate | **not added**: no loading consumer reports a real percentage; the Theatre Web `progress` elements are listening position, kept |
| skeleton | **not added**: Theatre Web home keeps its three static placeholder blocks (layout-shift guard, no shimmer); Studio has none |

## Inventory

`reuse` = already routed through the shared component, so the change in this PR reaches it.
`replace` = changed here. `keep` = intentionally unchanged, with the reason.

### Shared / preview

| Path | Flow → state owner | Component | Action | Verification |
| --- | --- | --- | --- | --- |
| `packages/ui-system/leptos/src/button.rs` Button, IconButton | action `ButtonState::Loading` | indicator | replace raw span with `LoadingIndicator` | `actions_navigation.rs` |
| `packages/ui-system/leptos/src/select_menu.rs` (menu + native indicator) | `ChoiceState::Loading` | indicator | replace raw span | `fields.rs`; gallery |
| `packages/ui-system/leptos/src/feedback.rs` `LoadingState` | section loading | indicator + label | replace dotted `.cui-loading-mark` | `layout_feedback.rs`, `gallery.rs` |
| `packages/ui-system/leptos/foundation.css(.in)` | — | one `@keyframes`, one reduced-motion override | add / delete `.cui-loading-mark`, `.cui-action-progress` | CSS contract test |
| `packages/ui-system/cmp/.../Actions.kt` `PendingRing` | `ButtonState.Loading` | indicator | replace with shared `LoadingIndicator`; reduced no longer creates a ticker | `PendingRingAndFieldIndicatorTest`, `LoadingIndicatorTest` |
| `packages/ui-system/cmp/.../Feedback.kt` `LoadingState` | section loading | indicator + label | replace `expand_more` glyph | `LoadingIndicatorTest` |
| `packages/ui-system/cmp/.../Choices.kt:238` | select trailing chevron | `Icon("expand_more")` | keep: hidden while loading; a real disclosure | `SelectAffordanceTest` |
| preview samples (`foundation_samples.rs`, `PreviewApp.kt`, `TheatreSamples.kt`, `AuthAccountSamples.kt`) | gallery fixtures | production components | reuse; no second gallery | gallery / preview tests |

### Theatre Web

| Path | Flow → state owner | Action |
| --- | --- | --- |
| `features/public_home.rs` `Region::Loading` | discovery first read → `Region` | **replace** `<p>` with `LoadingState` (polite) |
| same, `.theatre-public-skeleton` blocks | same | keep: static placeholders, no shimmer, reserve layout |
| same, Load more (`more_state`) | next page → `Tail` | reuse (Button busy) |
| `features/public_work.rs` `Shape::Loading` | work detail read | **replace** `<p>` with `LoadingState` |
| same, units status `<p>` | first unit page, no rows yet | keep: a text-only focus/status target combining several messages; only `LoadingState` is importable, so no indicator is added (follow-up) |
| same, Load more / retry | next unit page | reuse |
| `features/library/live.rs` refresh status | any Library section reading | keep: text status plus the refresh `<button>` `aria-busy`; no indicator (follow-up, see above) |
| same, per-section status text | section `PagePhase::Loading` | keep: the refresh status owns the one visible indicator for the same state |
| same, row Button `Loading` (`rows.rs`) | save / sync of a row | reuse |
| `features/auth.rs` `theatre-auth-status` | session `Checking` | keep: text-only live region (follow-up) |
| `features/signup.rs`, `account_menu.rs` | submit / logout | reuse (Button busy) |
| `features/library.rs:39`, `features/player.rs:168`, `features/live_player.rs:438` `progress` | listening position / mini-player position | keep: determinate playback position, not loading |
| `features/live_player.rs:281` busy | progress sync commands | keep: disables only; no loader (`UnknownOutcome` is not a spinner) |
| Player buffering | Web player has no buffering surface in live mode (`status_key` is preview only) | `NOT_APPLICABLE` |

### Studio Web

| Path | Flow → state owner | Action |
| --- | --- | --- |
| `ui.rs` `ActionButton` / `FormSubmit` | every Studio action (save, import, upload, Forge, download, publish, sign-in) | **replace** raw span with `LoadingIndicator`; native submit and IME unchanged |
| `view.rs:794` session checking | `SessionState::pending` | **replace** `Status` with `LoadingState` (polite) |
| `live/dashboard.rs:119` | catalog read `Loading` | **replace** `<p>` with `LoadingState` |
| `forge/view.rs:354` | Forge library `Loading` | keep: shared multi-state status `<p>` (follow-up) |
| `live/workspace.rs:1060` | request `Phase::Pending` | keep: shared multi-state live region beside Cancel (follow-up) |
| `production/browser.rs:330` `Status` busy | production request | keep: stable polite live region; the action Button owns the visible indicator |
| `view.rs:89,484,565`, `import.rs`, `adaptation.rs`, `inspector*` `busy` | state guards that disable controls | keep: guards, not visuals |
| editor / reading areas | — | keep: no shimmer or loop in reading zones |

### Theatre CMP

| Path | Flow → state owner | Action |
| --- | --- | --- |
| `features/shell/presentation/CatalogFooter.kt` `ListFooter` | next / first catalog page → `ShellListPhase.Loading` | **replace** text with `LoadingState` (polite) |
| same, `LibraryFooter` | Library section reads | **replace** with `LoadingState` |
| `CantosPresentation.kt` `WorkScreen` detail | `ShellDetailPhase.Loading` | **replace** with `LoadingState` |
| same, `CatalogRecovery` | `ShellCatalogMode.Loading` | **replace** the title with `LoadingState`; note and Retry unchanged |
| `PlayerSection.kt:65` | `PlaybackUiStatus.Loading|Buffering` | **replace** plain text with `LoadingState` (polite) only while the engine waits; other statuses stay text |
| `PlayerSection.kt:117` mini-player | `stateDescription` | keep: text-only state in a dense row; no loop in a persistent bar |
| `auth/presentation/{VerifyEmailScreen,RegistrationForm,LogoutSheet,AuthScreens}.kt`, `membership/.../MembershipPreview.kt` | `ButtonState.Loading` | reuse |
| `AndroidMediaEngine.kt`, `IosMediaEngine.kt`, reducers | engine facts | keep: no UI |
| sync / download | no implemented surface | `NOT_APPLICABLE` |

## Admission and the repository gate

`python3 scripts/check_repository.py` (run under Python 3.14; the default 3.9 cannot import it) already
fails on clean `develop` at `24b04d2a`: the Leptos qualification records of button, text-field, surface,
badge, back-button, logo, select, icon, status, dialog, checkbox and error-state are stale, and the
same stale records make existing import sites report "new use requires both preview qualifications".
This PR does not refresh them: a stale record is rerun and reviewed, not re-hashed. Its only delta is the
`LoadingState` import in the Theatre/Studio files above, because `LoadingState` has no qualification
record on either renderer. Those adoption edits are isolated in their own commit so they can be dropped
or held until `LoadingState` is qualified; the shared-component fixes reach every existing consumer
without a new import.

## Evidence and NOT_RUN

| Claim | Oracle | Result |
| --- | --- | --- |
| one indicator, decorative, no glyph | `layout_feedback.rs` | run, see PR |
| one endless animation, removed under reduced motion | `layout_feedback.rs` CSS contract | run, see PR |
| CMP turns under Full, static under Reduced, label is the status, loop ends with composition | `LoadingIndicatorTest` (JVM manual clock) | run, see PR |
| Web recording, frames 0/mid/end, Light/Dark, VI/EN, forced colors | browser capture | `NOT_RUN` |
| Android emulator/device recording, TalkBack | device | `NOT_RUN` |
| iOS | Xcode simulator | `NOT_RUN` (`UNSUPPORTED` on this host for native motion) |
| CPU wake / battery under a hidden or backgrounded app | profiler | `NOT_RUN` |

Reduced motion on the Web is the real `prefers-reduced-motion` media feature; on CMP it is the
injected `UiPreferences.motion`, and no platform reduced-motion adapter is added by this PR.
