# CMP catalog detail identity guard and concurrent integration

This is the source-bound record for the bounded [#159](https://github.com/loveoverflowcom/cantos/issues/159)
hardening in [PR #248](https://github.com/loveoverflowcom/cantos/pull/248), targeting `develop`
from `e4d6ca23a51d3f1c1465c7c2da5c2a198f1c3e14`.

## Reconciliation with PR #247

[PR #247](https://github.com/loveoverflowcom/cantos/pull/247), pinned at
`cc1e56b121c95e86f01028813df6b9f9c26408b5`, appeared during the original implementation. It was
created at 04:51:30 UTC; #248 was created at 05:02:34 UTC before the overlap notification arrived.
Both originally branched from the same develop SHA. Source comparison found 25 overlapping
paths out of #248's original 36 and #247's 79.

The pinned #247 source already supplies validated server search/kind filters, query-bound
pagination/retry/fencing, Android release HTTPS/network configuration, an iOS production
project and the shared-only framework/CLI. #248's alternative query model and second iOS
project would duplicate those owners. Its final diff therefore retains none of those changes.
#247 owns that integration; #248 supplies only the independent identity guard below, regression
tests and this record. Both PRs still target develop; #248 does not import or silently stack
the 79-file #247 commit. The guard applies to the unchanged `resolveWork` success branch in
either source, so it can be retained when reconciling their final merge order.

The original wider implementation remains auditable at
`458c574802d115ec798c28c8619fc8e7ec8e3e62` in #248's history. Its saveable route/draft/scroll,
IME and metadata-selection alternatives are retained there for a future explicit adaptation to
#247's existing query/kind-filter/account/refresh owners; they are not the current PR scope.
Only paths introduced by that original commit were restored to the pinned base. No unrelated
source, worktree, cache, log, evidence or runtime data was removed.

The [#247 ledger](https://github.com/loveoverflowcom/cantos/blob/cc1e56b121c95e86f01028813df6b9f9c26408b5/docs/evidence/theatre-live-catalog-step1.md)
and [issue #14 comment](https://github.com/loveoverflowcom/cantos/issues/14#issuecomment-6245829346)
were read. Their reported seeded 4 Work/31 unit API/storage checks, Android package checks and
PostgreSQL timeout reruns are not claimed as evidence for #248. No #247 artifacts were executed
or production resources accessed during this comparison.

## Invariant and implementation boundary

An awaited Work detail may become Ready and request units only when its returned `work_id`
equals the opened Work ID. A different request token is already stale; a matching token alone
cannot establish the resource binding. A foreign port can return a separately valid Work value
for another UUID even though the production HTTP adapter already rejects that mismatch.

The reducer now maps that foreign response to `CatalogFailure.Protocol`, preserves the
requested Work/summary and browse state, clears the awaited token and emits no unit read. Retry
asks for the original Work ID. A late response with the refused token remains stale; an exact
retry result can proceed to units. Normal Back cancellation still applies after recovery.
This matches the existing unit-page identity recheck and existing protocol-failure presentation.

Owner: shared pure application reducer, not HTTP/server policy. No API/schema, query, native
host, transport, UI/copy, component adoption, storage, playback, identity or rights rule changes.
No ADR, dependency, backend query/performance change or provider call is needed.

## Verification and acceptance limits

Two deterministic regressions exercise the real reducer and controller with the existing
`GatedPort` deferred fixture. The reducer test asserts complete refused state/effect output,
exact retry identity, stale-token fencing and successful recovery. The controller test asserts
no foreign unit read, the original summary/retry target and Back cancellation. They reproduce
the bug before the guard and pass afterward through `cargo xtask theatre-demo verify`; exact
counts and final preflight status are in the PR and ignored handoff.

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo verify
TMPDIR=/private/tmp cargo xtask preflight --changed-only
```

Evidence is `example-tested` for the pure reducer and `mock-integration-tested` for the real
controller against `GatedPort`, executed on Kotlin/JVM. The adapter remains strict, but no
claim is made that an actual server emitted a foreign detail. Current source changes have no
rendered effect beyond selecting the existing protocol-failure state, so the broad commit's
screenshots/native builds are historical evidence only, not current acceptance.

The untouched develop-equivalent tree reproduces 22 shared UI qualification rows. No new
component is adopted by the narrowed diff. Full preflight retains any repository FAIL rather
than calling the baseline accepted. Disk headroom is insufficient for a broad recompilation;
the earlier expanded campaign was stopped below 1 GiB without cleanup. Kotlin/Native 2.2.20 is
absent and no Android target is connected. None of those native/API checks is needed to prove
this pure-core refusal, and none is claimed.

Native catalog/search UX acceptance remains #159/#14 and #247's responsibility. Library,
account, media delivery and native playback retain their own issues. Next acceptance owners
are unassigned; no PR/issue is merged or closed by this record.
