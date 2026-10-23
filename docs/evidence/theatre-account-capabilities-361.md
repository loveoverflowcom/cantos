# Theatre Web account capability boundary — #360 and #361

Source-bound implementation for [#360](https://github.com/loveoverflowcom/cantos/issues/360)
and [#361](https://github.com/loveoverflowcom/cantos/issues/361), from
`refactor/cantos-native-turso@a898d833ccb988960c6ee13329ee1e07b01acf63` (including #365/#367).
The user selected implementing S1 first in this PR rather than waiting for separate acceptance.
GitHub owns acceptance/status; the PR records the final source identity and check results.
The work queue order and product contracts are unchanged. No new dependency or ADR is needed.

## Owner and invariant

`AccountViewState` owns private account, Library and sync memory. Session completion receives a
device-ID generator from the shell. Ordinary refusals are atomic. Exhausted security invalidation
intentionally clears private memory and cancels cleanup, enters `Failed(Unavailable)` and issues
no new generation. This is an effectively unreachable page-lifetime boundary, not an incident
or production-authentication claim. `AccountWriter` publishes the purge even when the transition
returns `Exhausted`; no clone-and-discard candidate can hide it. Durable journal bytes remain
outside the transition and recover after reload for their original actor.

Sync consumers call typed owner capabilities; mutable sync/projection callbacks are private.
User admission distinguishes signed-out, blocked restoration and absent sync. Completion
admission distinguishes a stale fence and absent sync. Completion recovery remains allowed
while restoration is blocked, preserving the previous protocol. Each admitted operation
reconciles the Library projection once. `Admitted<T>` carries its account fence and result/effect
value; a refused admission preserves the complete state and never calls its generator.

The host-compilable Leptos writer delegates to this same production owner and suppresses
admission-refusal publication. It drops guards before storage/network effects, persists the
journal before creating a send future and rechecks the fence and pending payload afterwards.
A failed persistence step keeps the pending operation as `ReadReceiptFirst`. The wasm runtime
owns HTTP, browser storage, timers and spawning; its receipt-first lookup and post-await fence
checks remain. Resume completion also checks its exact ticket at the owner. The duplicate
History prompt/cancel runtime wrappers are removed; shell callbacks use the host writer.

No authentication adapter, Server policy, endpoint, journal format, projection reconciliation
algorithm, dependency, token, localized copy, markup or component contract changes. Existing UI
controls and fixtures are reused; no new preview component admission applies.

## Evidence ledger

| Claim | Actual production owner / oracle | Evidence boundary |
| --- | --- | --- |
| Ordinary lifecycle behavior survives extraction | Characterization over session/refresh/logout/page/command paths | Host `example-tested` before and after |
| Exhaustion is fail closed and never reissues cleanup | MAX and MAX-1 tests in account/account-view, plus the actual signal writer | Host `example-tested`; not Kani |
| Device ID is an injected fact | Exact IDs and generator-not-called assertions for stale/exhausted session completions | Host `example-tested` |
| Consumers cannot mutate sync/projection arbitrarily | Private owner helpers; test-only fixture helper; no external accessor callers | `type-enforced` / source and wasm static checks in PR |
| Every capability refuses atomically | Named per-capability admission matrix, full aggregate `PartialEq` | Host `example-tested`; no cloned aggregate in the production tick path |
| A completion cannot enter B with A's fence | Every completion at logout and after B admission; resume ticket replacement | Host `example-tested` |
| Persist precedes send and failed storage retains pending | Actual `AccountWriter`, injected recording storage/send, storage reentry and readmission | `mock-integration-tested`; external storage/network are doubles |
| Recovery preserves identity and actor binding | Actual journal, logout/relogin, receipt reconciliation and unknown-outcome retries | Host `example-tested`; no actual browser durability claim |
| Sync admission stays reactive | Existing `sync_admission` regression and writer refusal subscriber counter | Host reactive `example-tested`; not DOM evidence |

The original exhaustion regression failed at the fetched base: `SignedIn` survived
`begin_refresh(Exhausted)`. The first corrected test oracle wrongly compared journal recovery
flags byte-for-byte after restore; production deliberately normalizes them to receipt-first.
The corrected oracle compares the original operation ID/payload and that required recovery mode.
The PR gives exact commands, retained log locations and later browser/gate results.

## Deferred acceptance and limits

Admitted no-op, lifecycle and storage notification refinement belongs to #362; this PR does
not claim to suppress every no-op or every selector rerun. The existing source-bound audit is
historical evidence and is not rewritten as a current plan. There is no allocation/latency,
Chrome tick profile, COW, structural-sharing or purge-of-retained-snapshots claim.

No Server/Identity/DB, real issuer, native audio, provider, rights/publication, Kani, OS IME or
screen-reader acceptance follows from these tests. Browser observations, if run, use the
existing explicitly isolated loopback API/issuer fixture and are reported separately in the PR.
A real login → mutate → logout → different-login journey still needs a named deployment and
real issuer/API acceptance. No remote service or deployment is changed. Rollback is a source
revert with no migration; operation journals stay compatible.


## Loopback browser observation

`cargo xtask verify web-bearer` passed all 27 existing scenarios on headless Linux
Chromium `155.0.8059.84`, debug CSR production modules, fixed loopback API/issuer and an isolated
profile. Its recorded source fingerprint is
`a13fc9971bd78f01c44d95b6941393588b08fda468dc3ee5de7e6494156c16d5`;
local report: `artifacts/agent-runs/web-bearer-1792590363804/report.json`.
Subsequent additions are only the `cfg(test)` page-exhaustion helper/regression and documentation;
production browser code is unchanged. The final PR lists the resulting source tree and preflight.

This exercises production writer/runtime with mocked HTTP/identity, including account A teardown,
delayed A response after B admission, Saved pending/failure controls and retained Library nodes,
focus/selection during page updates. It is `mock-integration-tested` / scoped `interaction-tested`,
not real issuer/API, browser durability under process crash or media delivery acceptance.

Opened Library captures: 390px light and 1440px dark, vi-VN, 100% text, showing the three lists,
retained selected title and controls. These captures contain only synthetic content and remain
local ignored evidence. The opened 390px dark CSS-200% capture is extremely tall with narrow
text columns; readability at that scale is **not accepted**. This PR changes no layout assets.
No screen-reader, OS IME, contrast, motion, real-media or cross-engine claim is made.
