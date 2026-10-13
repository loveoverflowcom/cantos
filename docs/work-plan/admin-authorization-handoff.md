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

## Current independent integration handoff

Implementation source `97efec8b358bcc7e7b79249fe55687933af6e67f`, tree
`82fba427a0bd4af02f4a48e78a69dd8bbcb1281f`, normally inherits the reviewed Studio and Theatre
integration, including Theatre's test-only immutable TRUNCATE graph repair. Historical source
and gate descriptions above remain tied to their original checkpoints. The
[current evidence appendix](../evidence/admin-authorization.md#independent-integrated-review-and-local-acceptance)
owns this snapshot's final command counts, raw hashes, reviewed repairs and residual gates.

The integration keeps the dedicated Admin confidential credential and current-group contract,
exact registry actor, Admin-only block/receipt and append-only audit. One shared ES256 signature
kernel and OAuth Basic component encoder supply plumbing only; caller-specific issuer, audience,
client, ID/access purpose and policy remain separate. Studio/Theatre Web/native session/resource
code and immutable UUID request/response fences retain their reviewed behavior; producer grants
remain independent. No Admin role bypasses owner/resource rights. Shared Studio transport keeps
JSON default, explicit Protobuf opt-in and legacy MessagePack decoding; Admin remains plain JSON.

The combined migration registry is 1–14,16–22,24,25 (23 entries). Original Admin0022 and all
published identity migration bytes/checksums remain unchanged; artwork0023 and private storage0015
belong to their separate work. The actor-disable-after-lock regression and signed special-secret
provider regression are retained. The strengthened audit fixture now checks exact `P0001` and
unchanged complete rows across all 9 protected mutations; Registry still checks `55000`.

Final actual local verification: full `cargo xtask preflight --changed-only` PASS for all 17 gates
with no skips; mutation killed 9 with 0 survivors. Disposable PostgreSQL records 13 Admin tests
plus 1 actual restart recovery, Theatre Web 15, native 6 and Studio OIDC 18. All inherited actual
HTTP/process-kill/restart/migration/replay/codec suites PASS. Earlier failures and exact fail-before/fixed source
snapshots remain available. The final 13-record manifest is
`/private/tmp/cantos-identity-campaign/pr103-acceptance-manifest.json`, SHA256
`dd49dbc36279bf64c8c50b98e860977ec1e96e2a3ecc6e076408b44dd074cead`. The historical 38-record
manifest stays unchanged at SHA256
`723980a1a14cbc180ab08f726fcbc1376a14bd8bf0cd5e232f5df0f512d01acf`.

The original PR3 missing-source matrix above is historical. Current Theatre Web/native
exchange/resource/session consumers are implemented in the inherited reviewed branch; their
[current handoff](theatre-identity-handoff.md#independent-integrated-acceptance-handoff--13-october-2026)
distinguishes actual Web/PG/Android mock-build evidence from unrun native host/device acceptance.
Android INTERNET/callback registration and dependent host activation remain unapplied after
automatic approval rejection, awaiting the direct human exception. PR101 remains draft for that
source blocker; PR103 stays draft with its unresolved dependency. A mock build does not accept
native authentication, system-browser delivery, secure-store lifecycle or playback.

Next work remains the held Android source decision, then the actual named native OS/secure-store
and provider lifecycle gates. Before Admin production activation, verify live current group
removal, dedicated confidential client/credential delivery, issuer key rotation and normal TLS,
and accept bearer/backup protection, audit retention/quotas/tamper and recovery budgets. Provider
membership is a bounded observation of at most five seconds, not atomic with PostgreSQL. Keep
Admin unavailable if the custom live contract is unproved; never use token-time groups alone.
Local Admin blocks remain immutable and grant/unblock/global logout are outside this slice.
No issue is closed, provider/account permission changed or complete four-PR merge claimed here.
GitHub owns execution status; the queue owns recommended order and rationale.
