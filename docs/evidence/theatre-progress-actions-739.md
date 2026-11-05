# Theatre progress actions — issue 739

Source-bound implementation/evidence record for [#739](https://github.com/loveoverflowcom/cantos/issues/739),
5 November 2026. Base `origin/develop@6577d1bb2043af701336e2f88df04704c105b628`
contains [#738 / PR #752](https://github.com/loveoverflowcom/cantos/pull/752).
This is one Theatre Web presentation change, with no deployment.

## Boundary and admission

The listener's primary goal remains listening. Progress actions are secondary,
Tonal controls in the Medium-High full player. The existing
[UI system](../design/ui-system.md#theatre-compact-listening) owns the direction;
`features/live_player.rs::PlayerSyncPanel` owns this composition.

The five raw `.theatre-action` buttons now reuse catalog `button` / `Button`,
`ButtonVariant::Tonal`, `ButtonState::{Enabled, Disabled, Loading}` and validated
`DisabledReason`. Restart uses the existing decorative `IconName::Refresh`,
completion `IconName::Check`. Retry and both conflict choices retain visible text.
No new component, token, shared API, dependency, localization key or CMP change.
Existing Library row reuse and #738's transport fix were inspected before editing;
neither domain nor transport code is repeated here.

Existing two-renderer admission inputs remain unchanged:

| Input | SHA-256 / owner evidence |
| --- | --- |
| Tokens | `79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218` |
| Leptos | `e92cc3969ad8f6b2dd7947917bb21ab48c01e63fab2fdb078428c0bf8053590b`; [#737 qualification](ui-requalification-737-button-leptos.json) |
| CMP | `f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a`; [#615 qualification](ui-requalification-615-button-cmp.json) |

`python3 scripts/ui_preview_catalog.py --digests` produced these inputs.
The repository handoff gate checks their admission/freshness; no qualification
record or compatibility baseline was rewritten. Button supplies canonical action
colors, target/icon/gap/type/shape, focus and reduced-motion roles. Existing app
containers supply placement; no CSS change or copied anatomy is needed.

## Action / state / reason / command matrix

| Owner fact | Presentation and reason | Intent / duplicate policy |
| --- | --- | --- |
| Guest, lost session or non-public source | Sync controls absent | No progress action |
| Idle, not completed | Restart and Mark completed enabled | Existing root callbacks request Restart / MarkCompleted |
| Confirmed completed record | Completed label and explicit disabled reason; Restart available | Completion label comes only from the confirmed record |
| Pending Restart / MarkCompleted | Only the executing action is Loading; its peer is Disabled with the pending reason | Shared dispatch guard and fresh caller projection block repeats; exactly one live Loading reason |
| Pending checkpoint / conflict resolution | Ordinary actions disabled with the saving reason; one central saving status | No unrelated action is described as executing |
| UnknownOutcome | Both actions disabled with the unknown-result reason; one central unknown status; no retry affordance added | Existing receipt-first recovery, operation identity and persistence remain authoritative |
| Failed | Error text is the sole live status, associated with shared Retry | Retry sends only the exact currently rendered failure's publication |
| ListenerChooses conflict | Only the two admitted choices and one visible prompt | ResolveHere / ResolveOther retain the owner's exact local/server positions and server revision |
| KeepServer conflict | Existing localized server-position notice | No automatic seek or merge-policy change |

Pending never becomes completed optimistically. Unknown is distinct from a live
request and from success. Disabled reasons remain visible and associated using
the shared contract; static per-control reasons do not create additional live
regions. The initial conflict migration repeated the full prompt below two
blocked ordinary actions. Opened 320px/text200 captures exposed the wasted space;
the final composition omits those refused ordinary actions until resolution.

`ProgressActionTarget` is an immutable callback projection of the existing source,
playback callback identity, account `RequestFence` and exact command. It dispatches
only if all still match. It reuses the existing memoized seek binding, so media
position ticks do not recreate the recovery choices. Source A→B→A, an updated
command/conflict and session loss invalidate old targets. AccountWriter, runtime,
sync core, receipts, reconciliation, HTTP, persistence and playback retain every
decision/effect. Main actions read current availability at activation.

Behavior SSOT lookup found `cantos.listening.progress` at `needs-decision`,
`model: null`. This change preserves that owner's outcomes and presents its
existing refusal/fence rules; it changes no requirement, model, lifecycle,
merge strategy or durable authority. Native/UI regressions remain independent
of formal assurance. The handoff preflight derives applicable obligations.

## Execution and provenance

Cwd: `/Users/manhblue/Documents/Codex/2026-11-06/task-7/cantos`.
Rust 1.99.1; Cargo uses `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
TMPDIR=/private/tmp/cantos739-tests`.
Source blob for `live_player.rs`: `5346ea0c6dd0a036b2504e137493345dd48a2964`.
The final PR and issue comment bind these inputs to the checked commit and tree.

Local logs, driver, hashes, failures and captures are retained outside Git under
`artifacts/agent-runs/issue739/`. The original anatomy regression failed against
the base's raw controls. The migrated checkpoint passed 14 player tests; final
native/format/lint/build results belong to the handoff preflight below. Initial
test-only missing-semicolon compile failures and their correction are retained.

`cargo xtask build cantos-theatre --web` passed, including the final conflict
composition. `cargo xtask app cantos-theatre --web` refused unavailable API 8080
and occupied frontend port 8082; no unrelated process was stopped.
The browser therefore runs actual compiled frontend assets over CDP Fetch with
explicit synthetic session, catalog, progress, receipt and delivery responses.
Its session facts are public test values, never real cookie/login evidence.
Conflict fixtures use the owner's 409 receipt shape; failures use 403,
unknown outcomes 503, and delayed writes are explicitly held/released.
The real HTMLAudioElement decodes a generated local Opus tone; no real publication,
Identity service, database or CDN is claimed. Only the nonexecutable WASM custom
name section is omitted in the browser fixture; retained executable/data sections
are individually hashed in `wasm-fixture-provenance.json`.

`PHASE=before node artifacts/agent-runs/issue739/browser-check.mjs` passed 53
observations on the base bundle. The final `node .../browser-check.mjs` passed
59 observations, zero runtime exceptions, in Chrome 156.0.8078.80, headless,
with focus emulation disclosed. Native Tab order/focus and accessible Button name,
Enter and Space conflict activation, one-effect repeat blocking, failed→retry,
confirmed completion, unknown outcome, delayed old-source receipt and logout
receipt rejection were observed. Guard helper tests separately cover stale
command, source A→B→A and session fences; detached synthetic-node interaction is
not claimed as a native keyboard flow.

The matched matrix is 320/390 CSS px × Light/Dark × VI/EN × root text100/text200,
for pending, failed and ListenerChooses: 48 captures per source. Viewport height
1400 CSS px, DPR 1. Locale is initialized from the real stored preference for
each isolated scenario. Text scaling is not browser zoom. Observed action targets
are at least 48×48px, fit their horizontal bounds and use 0s transition under
reduced motion in text200 rows. No frame-pacing/performance claim.

Opened final images: pending `320-dark-vi-VN-2`, `390-light-vi-VN-1`;
failed `320-dark-vi-VN-2`, `390-light-en-1`; conflict `320-light-en-2`,
`390-dark-vi-VN-1`; plus `after-keyboard-focus.png` and `after-unknown.png`.
The corresponding base pending/conflict extremes were opened. These show
readable reasons, visible focus, distinct theme roles and reachable wrapped
choices; the final conflict has one prompt. Remaining captures are captured-only.
Long English words can wrap within the 320px/text200 conflict choices.

Earlier harness failures are preserved: wrong HTTP conflict status, switching
route before source selection committed, and forgetting the real logout
confirmation. A separate base run that repeatedly changed locale through Me
also produced existing callback-disposal exceptions. The final matched matrix
uses independently initialized locales and does not accept that unrelated flow.

## Handoff and remaining acceptance

The final local handoff command is
`cargo xtask preflight --changed-only --base origin/develop`; its exact summary,
checked SHA/tree, self-review and actual issue synchronization are recorded in
the PR and issue comment, with the full output retained locally.

L1/L2: owner/static review of the presentation mapping and existing fences.
L3: named native regressions and preflight static/suite results.
L4: actual frontend/DOM/trusted-input evidence with named API doubles, plus opened
images. L5 real service/publication/delivery acceptance is NOT_RUN and outside this
presentation PR. No schema/migration, credentials, server/session policy, audio
identity, provider cost, rights/approval, cache or native-platform changes.

AC-01/02/03 have source and targeted regression evidence. AC-05 is completed by
this record, actual local checks and the fresh issue checklist/comment sync.
AC-04 remains partially observed: the complete keyboard focus-recovery tour
after pending/resolution removes a focused target and assistive-technology
interaction were not accepted from these captures or initial Tab checks.
Resume that same state matrix in the actual frontend and explicitly observe
post-request/post-resolution focus recovery. Real-service follow-up additionally
needs the local API and a free 8082, then `cargo xtask app cantos-theatre --web`.
Next acceptance owner: unassigned. The issue remains open; no deployment.
