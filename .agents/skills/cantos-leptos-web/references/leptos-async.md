# Leptos async: reads, mutations, races and progress

> **Scope.** Design and review asynchronous behavior in Cantos Leptos screens: resources and
> actions, explicit remote states, stale-response guards, duplicate submits, bounded retries,
> save flows that keep the creator's text, effects, optimistic updates and production-job progress
> tracking. Use when a screen loads data, submits a change, polls or streams, or shows async status.

Async code is where durable rules hide. Retry policy, conflict handling, poll scheduling and
"may this be applied" are decisions: keep them as pure functions in a core crate per the
foundation's [`functional-core.md`](../../cantos-engineering/references/functional-core.md), and let
the resource or action resolve facts, call the rule once and interpret the result. Apply the
canonical [typed-error policy](../../cantos-engineering/references/typed-errors-and-composition.md).
The Web manifests pin **Leptos 0.7.8**. `LocalResource::new` takes one reactive fetcher;
`Action::new_local` supports a non-`Send` future. `Resource::new` and `Action::new` have
different serialization/`Send` bounds; do not copy 0.8 examples or force browser futures into
them. The hypothetical ports below illustrate signatures, not an implemented resource migration.

## Rules at a glance

| Rule | Failure mode | Oracle | Status |
|---|---|---|---|
| Reads and mutations are separate (resource vs action) | a save hidden in a loader re-runs on every dependency change | review; component test counts no write on reload | manual |
| Idle, loading, ready, empty and failed render distinctly | a 500 shows "No episodes yet" and the listener gives up | component test per state with a fake port | proposed |
| A stale response never replaces a newer one | episode 1 arrives late and overwrites episode 2 | deterministic out-of-order test (below) | proposed |
| Every mutation states its duplicate-submit behavior | a double click starts two billable generation runs | component test: two clicks, one port call (or one keyed operation) | proposed |
| Retries are bounded, error-class aware and a pure decision | a 403 retried forever; a non-idempotent write retried blindly | table test over `retry_decision` | proposed |
| A failed save keeps the editor data | a timeout reloads the server copy and erases a paragraph | component test: failing port, text unchanged, retry offered | proposed |
| No effect writes its own trigger | request loop, frozen tab, provider cost | review; a request counter in the fake stays bounded | manual |
| Optimistic UI only with server revision reconciliation | a bookmark shows saved and silently vanishes | component test: rejected write renders a failed item with retry | proposed |
| Polling and streams are bounded and cancelled on unmount | leaked intervals keep polling after leaving the page | real-browser test: leave the page, network shows no further polls | proposed |

## 1. Reads versus mutations

A resource owns remote *read* state keyed by its reactive inputs. An action owns one *write* and
its pending/value state. The component derives what to render from both.

```rust
// Illustrative CSR wiring for Leptos 0.7.8; the port/types remain proposed.
let ports = expect_context::<TheatreContext>();
let catalog = ports.catalog.clone();
let progress = ports.progress.clone();
let episode = LocalResource::new(move || {
    let id = route_episode_id.get(); // tracked before awaiting; nothing writes this key back
    let catalog = catalog.clone();
    async move { catalog.episode(id).await } // Result<Option<EpisodeView>, CatalogFailure>
});
let add_bookmark = Action::new_local(move |intent: &BookmarkIntent| {
    let progress = progress.clone();
    let intent = intent.clone();
    async move { progress.add_bookmark(intent).await } // Result<BookmarkReceipt, BookmarkFailure>
});
```

Do not fire the same read from a parent and a child; load once and pass the value or resource down.

### Typed results and error projection

- Resource/action output keeps a capability-specific `Result<T, E>`. `Result<Option<T>, E>` is
  justified only when absence is a successful contract outcome: unresolved resource, `Ok(None)`
  and `Err(E)` are three different states. Do not use `.ok()` to make a request fit `Option`.
- Action `value` may be `None` before any completed result; `pending` is a separate fact.
  Neither is the success/failure outcome. Retain submitted snapshot/request identity and decide
  whether an existing result belongs to it; a prior successful action value cannot announce
  “saved” for a newly edited or in-flight snapshot.
- Project every owned error variant exhaustively into a message key, field findings,
  reconciliation facts and permitted recovery action. The pure projection performs no retry,
  navigation or logging. Keep retry decisions in section 5; a localized message does not
  establish retryability. Do not collapse conflict/auth/validation/unknown-write outcomes into
  one `Unavailable` banner.
- Current Studio [`ClientError`](../../../../apps/cantos-studio/web/src/live/command.rs) includes
  `Cancelled`, `SessionChanged`, `ReceiptNotEstablished`, `Transport`, `Protocol`, `Http` and
  `InvalidInput`. These are implementation facts, not the proposed `PortError`/`UiError` names
  in later examples. Preserve its ambiguous-write/receipt semantics and the public `ErrorCode`
  contract when extracting mappings. Malformed/unknown wire errors become protocol failures at
  the HTTP adapter, never fabricated empty data or a default domain error.
- `<ErrorBoundary>` can render explicit result errors; it is not a general Rust panic or
  JavaScript exception containment mechanism. JS/browser exceptions stay in concrete platform
  helpers and map to typed failures under foundation ERR-04, not in view closures.

For a changed projection, test every variant's exact key, findings, recovery and form state;
compile exhaustive matches. Include malformed wire errors, failed refresh with cached data,
cancelled writes and lost save/finalize responses. Host projection tests do not establish browser
interaction or real request cancellation.

## 2. Make every remote state explicit

```rust
// Illustrative view state. `stale` keeps the previous value visible while refreshing.
pub enum LoadState<T> {
    Idle,
    Loading { stale: Option<T> },
    Ready(T),
    Empty,
    Failed { error: UiError, stale: Option<T> },
}

/// Pure: emptiness is a decision about the payload, not about the transport.
pub fn classify_library(page: LibraryPage) -> LoadState<LibraryPage> {
    if page.entries.is_empty() { LoadState::Empty } else { LoadState::Ready(page) }
}
```

Render with an exhaustive `match` (no `_` arm) so a new state fails compilation instead of
rendering nothing. Keep stale data only for the same account, resource and compatible revision;
clear it on logout/context switch so a failed refresh cannot disclose prior-session content.
`<Transition>` keeps the previous content during a refetch; use it when the
stale value is still meaningful (a library list) and `<Suspense>` when it is not (a different
episode). Empty and failed copy are different strings with different actions: "Bạn chưa nghe tập
nào" with a discovery link versus "Không tải được thư viện" with "Thử lại". All copy comes from the
locale resources owned by [`localization.md`](../../cantos-ui-design/references/localization.md); never
render a raw transport or provider error string.

## 3. Stale responses

```text
request A (episode 1) ─────────────────────────────►  slow response A
request B (episode 2) ───────────►  fast response B         ← must win
```

- Prefer a resource keyed by its input: a re-run supersedes the previous request.
- When work is hand-rolled (polling, a search box, an action whose result targets a selection),
  tag each request with what it answers and apply the response only if that is still current.
- Key by the full question, including the revision: a save acknowledgement for base revision 3
  must not be applied after the editor has rebased onto revision 5.
- Fence late errors as well as successes; an old failure must not clear new validation findings,
  sign out a new session or overwrite a current success. A keyed resource does not fence side
  effects performed inside its fetcher or other independently written signals.

```rust
// Illustrative pure guard: a monotonic issue counter plus the question being answered.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Issued { pub seq: u64, pub key: RequestKey }

pub fn should_apply(latest: Issued, answered: Issued) -> bool {
    answered.seq == latest.seq && answered.key == latest.key
}
```

On unmount or navigation, drop the pending write of the result. Writing into a disposed signal is
at best a no-op with a warning; prefer the fallible `try_` setters or an owner check, and confirm
the behavior for the pinned version. The regression test is in [`web-testing.md`](web-testing.md#the-stale-response-regression).
An action abort handle stops polling its future; it does not promise that a browser request or
server write stopped. Concrete request adapters own abort/cleanup, while mutation receipts and
idempotency own reconciliation. Report cancellation as cancellation, not a retryable error.

## 4. Duplicate submits: state the behavior per mutation

| Mutation | Second submit while pending | Mechanism |
|---|---|---|
| Save script draft | **rejected**; typing continues and marks the draft dirty again | control `aria-disabled` with reason "Đang lưu…", see [`script-editor.md`](script-editor.md#4-draft-save-and-version-states) |
| Generate selected dialogue, start production run | **rejected**; the same click intent reuses one operation ID on retry | operation ID minted once per intent, server deduplicates ([`durable-jobs.md`](../../cantos-production-pipeline/references/durable-jobs.md)) |
| Approve, publish | **rejected**, after an explicit confirmation naming the episode and revision | server gate is authoritative |
| Search or filter query | **replaces**; the older request is superseded | resource keyed by the query |
| Progress checkpoint | **replaces** locally; submission follows the sync contract | [`progress-sync.md`](../../cantos-listening/references/progress-sync.md) |
| Add bookmark | **queued** with a local operation ID each | per-intent operation IDs; duplicates collapse server-side |

Use `aria-disabled="true"` plus a visible reason rather than the `disabled` attribute when the
control must stay focusable and explain itself ([component states](../../cantos-ui-design/references/component-states.md)).

## 5. Retries are a pure, bounded, class-aware decision

The request's retry safety is a type, so an unkeyed write cannot be retried by accident:

```rust
// Illustrative. Jitter is an input, so the decision is deterministic under test.
pub enum Retryability { Read, IdempotentWrite(OperationId), UnkeyedWrite }

pub enum RetryDecision { RetryAfter(Duration), Stop(UiError) }

pub fn retry_decision(
    error: &PortError,
    attempt: u32,
    safety: &Retryability,
    policy: &RetryPolicy,
    jitter: f64,
) -> RetryDecision {
    let may_repeat = !matches!(safety, Retryability::UnkeyedWrite) && attempt < policy.max_attempts;
    match error {
        PortError::Timeout | PortError::Transport if may_repeat => {
            RetryDecision::RetryAfter(policy.backoff(attempt, jitter))
        }
        PortError::RateLimited { retry_after } if may_repeat => {
            RetryDecision::RetryAfter((*retry_after).min(policy.max_delay))
        }
        PortError::Timeout | PortError::Transport | PortError::RateLimited { .. } => {
            RetryDecision::Stop(UiError::Unavailable)
        }
        PortError::Conflict { current } => RetryDecision::Stop(UiError::Conflict(*current)),
        PortError::Unauthenticated => RetryDecision::Stop(UiError::SignInAgain),
        PortError::Forbidden { reason } => RetryDecision::Stop(UiError::NotPermitted(reason.clone())),
        PortError::Invalid(findings) => RetryDecision::Stop(UiError::Validation(findings.clone())),
    }
}
```

Test it as a table: every `PortError` variant × {read, keyed write, unkeyed write} × {first, last
attempt}, asserting the exact `RetryDecision`. After the automatic budget, show a manual "Thử lại"
that reuses the same operation ID. A browser timeout on a generation request does not mean the
server did nothing; the retry must be keyed so the server can recognize it.

## 6. Save flows keep the creator's data

The draft the creator is typing is the source of truth until the server acknowledges a specific
snapshot. A failed request changes the *status*, never the *content*.

- The action input is an immutable snapshot of the draft plus its base revision; the editable
  draft keeps evolving while the save is in flight.
- On success, compare the acknowledged snapshot with the current draft: equal → clean at the new
  revision; different → dirty against the new revision. Never overwrite newer typing with the
  acknowledged copy.
- On failure or conflict, keep the draft, show the reason and a recovery action. Do not refetch
  "to be safe" and do not clear the form.
- Attach field findings to the submitted snapshot/field version. Preserve newer typing and
  selections, and reject stale findings; do not let an older failing response overwrite the
  current draft's errors. Context/account changes still clear private data deliberately.

The full editor state machine lives in [`script-editor.md`](script-editor.md#4-draft-save-and-version-states).

## 7. Effects

Use `Effect` only for genuine side effects without a rendered result: focus, document title,
`beforeunload` registration, a local backup write. Never let an effect write a signal it reads.

```rust
// Counterexample: the effect reads `job` to schedule a poll and writes `job` with the result,
// so every response schedules another request immediately.
Effect::new(move |_| {
    let current = job.get();
    spawn_local(async move { set_job.set(production.status(current.id).await.ok()); });
});
```

The fix is to make the input (a job ID, a poll tick) the source of a resource and let a pure
schedule decide the next tick (section 9).

## 8. Optimistic UI

| Change | Optimistic? | Reason |
|---|---|---|
| Bookmark add or delete | yes, as a *pending* item with its operation ID; replaced by the server record, or shown as failed with retry | cheap, reversible, reconciled by revision and tombstone ([`progress-sync.md`](../../cantos-listening/references/progress-sync.md)) |
| Local draft edits, reorder, speaker change | not optimistic: they are local draft state until saved | the status region says "Có thay đổi chưa lưu" |
| Save script | no; "Đang lưu…" until acknowledged | the revision identity comes from the server |
| Start generation, approve, publish, retract | never | cost, rights and gates are server decisions |

An optimistic item that the server rejects must stay visible in a failed state; silently removing
it is a lost write from the user's point of view.

## 9. Production progress: bounded polling or streaming

Production continues without an open browser ([UI system § Studio](../../../../docs/design/ui-system.md#studio-text-first-production)).
The browser only observes. Whether to poll or stream (polling, Server-Sent Events, WebSocket) is a
decision to record when the job API exists; either way:

- Render server facts: stage, affected revision, attempts, estimate or "Chưa có ước tính chi phí",
  actual cost, uncertain provider outcomes, retry actions. Never infer a stage from elapsed time.
- Apply events in server sequence order and ignore older sequence numbers (the same guard as
  section 3). Distinguish execution status from editorial and publication status, as the
  [pipeline](../../../../docs/product/production-pipeline.md#jobs-and-failure-recovery) does.
- Bound the observer: stop on a terminal state, back off while nothing changes, pause while the
  document is hidden, give up after a bounded number of errors and offer manual refresh.
  `EventSource` reconnects on its own indefinitely; close it after the error budget.
- Cancel on unmount: clear the timeout or interval handle and close the stream in `on_cleanup`.

```rust
// Illustrative pure schedule. None means "do not poll".
pub fn next_poll(view: &RunView, unchanged_polls: u32, page_visible: bool) -> Option<Duration> {
    if view.is_terminal() || !page_visible {
        return None;
    }
    let step = 2_u64.saturating_pow(unchanged_polls.min(4));
    Some(Duration::from_secs((2 * step).min(30)))
}
```

Announce significant transitions (run failed, needs review, completed) once through a polite live
region; do not re-announce every poll ([accessibility](../../cantos-ui-design/references/accessibility.md)).

## Completion checklist

- [ ] Reads and mutations are separate; no read is issued twice for one screen.
- [ ] Every remote state renders distinctly with localized copy and a recovery action.
- [ ] A stale response cannot overwrite a newer one, and a test proves it out of order.
- [ ] Each mutation states rejected, queued or replaces, and the control reflects it.
- [ ] Retry decisions are pure, bounded, class-aware and refuse unkeyed writes.
- [ ] A failed or conflicting save leaves the draft untouched.
- [ ] No effect writes its own trigger; polling and streams stop on terminal state and unmount.
- [ ] Optimistic items are reconciled with server revisions and stay visible when rejected.
