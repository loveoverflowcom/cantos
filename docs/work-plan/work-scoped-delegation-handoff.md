# Work-scoped delegation review handoff

The [decision-pending RFC](../architecture/work-scoped-delegation-rfc.md) and
[dated research evidence](../evidence/work-scoped-delegation-research.md) support
[#113](https://github.com/loveoverflowcom/cantos/issues/113). GitHub owns live execution status.
This handoff recommends order, not a selected credential/API/schema or new authorization.

## One-PR review boundary

Deliver the source-bound RFC comparison and synthetic evidence as **documentation only**.
Preserve existing Studio/Admin/producer/storage boundaries and production gates. Do not create
live grants/credentials, migrate a database, change identity/security settings, deploy, close the
issue or describe the architecture decision as resolved. No final ADR is appropriate until the
owner selects a direction and accepts the revoke/lifecycle contract.

The source branch starts at fetched `origin/develop`
`b6066acc659b6d4cd030badb9c1abdad08b6dcf2`. Re-read current `develop` and issue/comments before
the independent second session; reconcile source drift without overwriting unrelated work.
Two existing issue127/audio-intake worktrees remain untouched. Their changes and the unrelated
Mac work are not prerequisites or part of this PR.

## Second-session independent review

1. Reconcile the issue's research-only/non-goal boundary against every changed path. Check that
   the matrix chooses nothing and does not conflate platform role, owner, member or producer.
2. Re-read cited source families, especially Studio cookie middleware, Catalog operation/cursor
   identity, independent Script/source relations, Brief grant checks, lifecycle final checks,
   artwork owner facade and publication mutations. Verify the inventory names actual code, not
   guessed proposed modules.
3. Inspect and rerun the retained disposable A/B fixture. Validate exact errors, signature
   rejection partitions and zero synthetic effect after current-parent revoke. Report its
   limitations: no production modules, SQL, HTTP, S3 or complete wire interoperability.
4. Challenge the comparison under identical current-rights assumptions. Signed/offline proof
   cannot silently weaken revoke/owner/resource guarantees; attenuation cannot authenticate an
   asserted caller or prevent a holder reusing a parent. Do not claim package maturity from docs.
5. Review exact local fencing versus remote observation age, stream/URL leakage bounds,
   lost issuance response, child lifetime/revoke, immutable operation attribution and audit
   outage behavior. Any missing product semantics stays needs-decision, not an implementation.
6. Make only in-scope documentation/evidence corrections, run the gates below against the final
   diff, inspect it and commit on this same branch. Publish one develop-targeted draft PR only
   under the user's authorized parent-coordinated publication step. Use `Refs #113`, not a
   closure keyword, because the decision and implementation gates remain unresolved.

## Minimal owner decisions before implementation, not blockers to this PR

| Decision | Options still open | Why indispensable later |
| --- | --- | --- |
| Primary objective | Central revoke simplicity; distributed proof; offline sub-agent attenuation | Determines whether new cryptographic/issuer/evaluator machinery has a real consumer |
| Attribution | Bearer-only credential holder; separately authenticated registered delegate | A bearer or arbitrary `sub` cannot establish the actual presenter identity |
| Operations / resource relation | Exact action-by-use-case map; Work → unit/revision/Script/source relationship | Existing owner-wide operations must not be inherited by a delegate |
| Revoke/lifecycle | Local admission fence; accepted remote observation age; archived/owner-transfer/child policy | Defines final-check, current-rights and stale/offline bounds without false instant-revoke claims |
| Issuance / UX | Studio owner; trusted operator CLI; independently resource-authorized Admin; combination | Ownership/identity/re-auth, one-time reveal, replay/lost-response and recovery need one policy |
| Mechanism / format | A; PASETO/JWT B; Biscuit/Macaroons C; justified hybrid | Requires evidence, reviewed exact protocol and trusted key custody before dependencies/schema |
| Optional limits / direct upload | Defer; hard quotas with atomic reservation; separate presigned-media evaluation | Counters/rate checks and provider signatures cannot replace domain admission |

No immediate owner answer is needed to finish this evidence PR. For implementation, start with
the objective question, then resolve the applicable semantics as one bounded review rather than
asking the owner to pick a hash/algorithm in isolation.

## Verification and later acceptance

For this Markdown-only change, use the already-installed repository toolchain and canonical
entrypoints from [xtask](../../xtask/README.md):

```sh
cargo xtask verify repository
cargo xtask preflight --changed-only --dry-run
cargo xtask preflight --changed-only
git diff --check
```

Record actual printed `PASS`, `FAIL`, `NOT_RUN` or `UNSUPPORTED`. A docs check proves link/text
hygiene, not Rust policy compilation or API/DB/provider behavior. No Rust/application gate is
required merely to accept wording; a changed path outside Markdown must be classified anew.

After owner acceptance, later bounded work should proceed in this order:

1. Pin approved semantics and scope in the existing Quint catalog/model lifecycle. Named runs
   cover Work A/B, unit/revision pins, child rights subset, expiry/not-before/clock regression,
   parent revoke during lock/I/O, owner transfer/disable/archive, replay payload conflict and
   audit outage; add reachable witnesses and typechecking negative controls. Do not promote
   a draft on simulator PASS or copy a handwritten fixture reducer into another SSOT.
2. Disposable Rust candidate compatibility/official vectors, bounded parse/evaluator tests and
   comparable measurements. Record source/lock/tool identity and exclusions; choose a final ADR.
3. Pure typed principal/policy/permit and exact deterministic error/property tests before I/O.
   Existing Studio/Admin/producer proofs remain distinct and no raw DTO constructs authority.
4. Thin owning-context application adapters and synthetic orchestration tests, then disposable
   real PostgreSQL locks, audit atomicity, duplicate/replay, commit-ack loss and restart tests.
5. Separate HTTP cross-surface/mixed credentials/caller attribution/cursor/resource exposure,
   actual object integrity/final reauthorization and production identity/operator acceptance.
   A presigned-write optimization needs separate authorized provider acceptance and cannot
   bypass the existing intake inspector, receipt or publication gate.

Later acceptance commands depend on selected files: `cargo xtask verify behavior` for registered
models, `cargo xtask verify rust`/`verify contracts` for code/contracts, `verify postgres` for real
SQL/HTTP persistence and changed-only preflight. Optional Kani applies only to an appropriate
reviewed pure kernel. These are future gates, all **NOT_RUN for delegation implementation** here.
Architectural owners are Catalog/Editorial/Production/Media and identity/operator boundaries;
individual assignees remain unassigned until GitHub/project review. Do not invent an operator
or a live provider budget from this handoff.
