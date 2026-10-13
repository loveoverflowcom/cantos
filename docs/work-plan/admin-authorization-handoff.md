# Admin PR4 → combined identity review

The delegated PR4 delivers the [bounded Admin slice](../architecture/admin-authorization.md):
dedicated verified resource credential, exact registry actor, allowlisted/current system-group
role, separate capability and Admin-only revocation, atomic append-only audit and operation replay.
It adds no Library/UI work, live client/grant/secret/settings bootstrap or merge authority.

## Pinned integration source

Fresh Cantos origin/develop `00eab6d456e2fb306e0b5f2009b3f01da7ef75d1` was fetched before branching.
Registry PR92 exact`277e28c1ab3a06e3c2fae6093e3278bdece1a0bb`, Studio PR98
exact`9794ed9ddcd376680a01b37938eabdde564290e1`, and Theatre PR101 exact
`41c3b752e61ecc767a2a9895c525ea80efc90ce1`/tree`be21bbacf44725078fb4eea8b51d2dd7d90b7d08`
were read as open/draft. A **normal merge of published PR101** into this branch keeps inherited
PR92/98/101 commits unrewritten; initial integrated baseline is
`09e496e44e5cfac9b7b3e34b9f988643f37f23dd`. Own code/docs are the diff above that baseline;
PR-to-develop diff also contains those unmerged dependencies. Exact final source identity and
verification logs are in [evidence](../evidence/admin-authorization.md) and the PR body.

Worktree `/private/tmp/cantos-admin-authorization`, branch `feat/admin-authorization`. Original
checkout and earlier worktree changes, refs, databases/audio/raw fixtures remain untouched.
VOT origin/develop was freshly fetched at`f3fea641a5fd226b883c4e32224ad8c97b8323f0`; source-only
proof/identity audit, no copied Gateway or client-header trust. No new ADR number is allocated:
this implements the existing separate Admin boundary with the concrete policy in architecture,
after fresh Protobuf develop integration the inherited Studio ADR is renumbered0024, preserving
Protobuf ADR0023 and all historical published commit references. No new Admin ADR is allocated.

## Final-review order

1. Parent's separately authorized Ultra reviews/fixes all four PRs together. This PR does not
   launch that review or merge. Check own and inherited commits, combined migrations/checksums,
   security/type barriers, backend host composition, actual consumer wiring and evidence freshness.
2. Preserve PR3's explicit incomplete **Web mounting/native resource IO**: its Web port is not
   mounted; native trusted discovery/durable flow/public-client exchange/resource revocation/
   registry consumer/Kotlin-Swift-host wiring is missing. Missing device acceptance does not
   explain those code gaps. Follow [Theatre handoff](theatre-identity-handoff.md); do not mark them
   accepted because this Admin slice passes.
3. Resolve and verify the actual live Admin current-group contract before enabling configuration.
   The signed-loopback provider in tests intentionally emits `cantos_admin_groups` on both JWT and
   introspection. Real Kanidm current membership/role removal is NOT_RUN and unsupported by the
   evidence so far. If that semantics is unavailable, keep Admin unavailable, not token-group-only.
4. Production readiness needs authorized clients/PKCE and credential delivery, live key rotation/
   account disable/revoke/group removal, audit operational quotas/retention/backup/tamper threat,
   workload/clock/lock budgets and local recovery review. Local blocks are immutable and there is
   no role grant or unblock endpoint. Never delete an audit/block record to restore authority.

Migration0022 is additive and separate. Preserve0015Storage/0016Publication/0017Discovery/
0018Registry/0019Library/0020Studio/0021Theatre. This combined registry is1–14 plus16–22;
independent Storage0015 remains its owner. No historical migration or checksum is modified.

## Acceptance and evidence boundaries

- L1/L2/L3: pure immutable role/temporal/action kernels, private verified witnesses, closed errors,
  exhaustive error projection and deterministic/property tests. Kani was considered but not
  selected: the role matrix is finite and fully partitioned; no bounded-model claim is made.
- L4: production HTTP/ES256 verifier/current introspection/application against a named signed
  loopback mock. This is not Kanidm-live or end-to-end Admin login/client evidence.
- L5: actual disposable PostgreSQL/runtime-role and Axum tests for current admission/role/block/
  disable/expiry, canonical locks, concurrent replay, audit redaction/immutability, named failures,
  rollback, application reopen and server restart. Exact executed counts/gates are in evidence.
- Baseline Darwin Python process-group fixture failures and compatible Gradle blockers stay visible.
  Disk-induced prior failures require fresh affected available gates; no mock/skip is promoted toPASS.

Next owner: parent Ultra for combined code review/merge decisions; provider/production/device
acceptance remains unassigned until explicitly authorized. GitHub owns execution/merge status;
this file owns recommended sequence and rationale.

## Fresh Protobuf integration checkpoint

At parent request, develop`b28b87856b30c78c2921aa1134ac657b0584ebd7` (merged PR102) was freshly
fetched and normal-merged at`fc0fe1869895a68c1668dbb3d48bea077279484c`. Admin feature checkpoint
is`4a33f1833f9e1943d254cfe8578190f218131bfe`/tree`7be8a8f9bb4556680af3ea663d08d4a8aee2d2bd`.
Additive README/work-queue conflicts retain both scopes; published Protobuf interfaces, runner and
selector are preserved. JSON stays default; Admin remains plain JSON and its verifier/policy/audit
modules are byte-identical through this integration. Inherited Studio ADR moves to0024, keeping
Protobuf0023. The prior complete available gate results belong to4a33; post-integration results
are recorded separately in the evidence/PR. No prior PASS is relabeled as a fresh integration run.

Draft [PR103](https://github.com/loveoverflowcom/cantos/pull/103) is the execution handoff. Fresh
post-Protobuf Rust/contracts and the available preflight passed on25f78ba; the final test-only
JWKS/introspection outage addition passed focused format/clippy and the full PostgreSQL gate
(12 Admin initial tests plus1 actual server-restart recovery). Production Admin inputs are
unchanged. Final repository remains FAIL on the Darwin fixture baseline (322 tests,37 EPERM
errors and1 dependent assertion); unskipped preflight remains UNSUPPORTED for compatible Gradle.
The PR body pins exact final SHA/tree, own/inherited commits and raw-evidence manifest. Independent
review of both the production fixes and the final test addition reports no actionable findings;
parent Ultra review and the explicit live/provider/native gaps above remain outstanding.
