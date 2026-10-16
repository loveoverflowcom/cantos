# Develop eight-commit SSOT and verification review

Status: source-bound review and bounded repair; not production or phase-exit acceptance.

## Pinned scope

The Mac review starts from fetched `origin/develop` at
`1b394e5cc9e641865073a5107d8ac9c606370658` in an isolated worktree and branch
`fix/review8-ssot-20261016`. The requested `git log -8 origin/develop`, with its ordinary
merge traversal, selected exactly these commits in this order:

| SHA | Subject |
| --- | --- |
| `1b394e5cc9e641865073a5107d8ac9c606370658` | Merge pull request #216 from loveoverflowcom/fix/theatre-catalog-pagination-172-20261016 |
| `98e5d9bb573f118ff6ce645e01569dcdd67aa4e3` | Merge pull request #215 from loveoverflowcom/feat/206-live-auth-acceptance-20261016 |
| `238b7618942603e406c2fda81d6b78b6655e7ecd` | test(theatre): verify catalog walks across publication changes |
| `9045f4006bc89fbc7fd86e848736fa9122f1811c` | feat(identity): accept real Kanidm Theatre sign-in; trust a private issuer CA |
| `64b5a60d3301f3cfe0fbc459afba17bdbd3dcf4f` | Merge pull request #214 from loveoverflowcom/feat/206-firebase-runtime-bridge-20261016 |
| `c3df1272af0d8e9fadf120379370cd3c5595f18e` | Merge pull request #213 from loveoverflowcom/feat/212-infra-layout |
| `adc2a6b6cb449c32b0be32421c1347ac3813535c` | fix(forge): bind raced requests and verify outcome reconciliation |
| `cbf7a2756923e182ea5360d3f52af3a7a51ff789` | Merge latest develop and reconcile enrollment documentation and evidence |

This is not eight first-parent merges or eight independent patches. #216 and #215 each also
include their implementation commit in the list. The review reads each selected commit's
first-parent contribution and the relevant incoming branch/runtime context, without counting
those duplicate changes twice. In particular, `cbf7a27` imports Forge, listener and documentation
changes, while Firebase's main implementation is visible in `64b5a60^..64b5a60`; simply diffing
`cbf7a27^..HEAD` would miss it. The common ancestor of the selected first parents is
`25b1c701b686018503d0f341d3966d3d35eaaf4b`, used as a deliberately broader local verification
baseline. The final PR itself targets the pinned develop base above.

Live GitHub metadata was read for issues #172, #206, #203 and #212, including comments.
#172/#206/#212 were open; #203 was closed, which does not establish its requested runtime.
PR #216 was confirmed merged at `1b394e5`, with its original head `238b761` retained.
No issue/comment/approval/Project update, deployment or CI dispatch was performed.

## Findings and repair

### P1: expired enrollment could pass a receipt-write wait

Class: **confirmed defect**; category: **logic**; confidence: **confirmed-from-source** and
reproduced against the original adapter. Introduced by the Firebase runtime slice in #214.

Owner: accepted [ADR0036's one-shot setup fence](../decisions/0036-firebase-enrollment-proof.md#selected-one-shot-setup-continuation-and-recovery),
the registration application and its PostgreSQL adapter. `advance()` sampled the database
clock in its UPDATE predicate. Its AFTER trigger could then wait on the immutable receipt
table until after expiry, yet the transaction still committed and returned `true`. This could
permit a later provider mutation or release a setup continuation after the operation expired.

The repair samples `expires_at > clock_timestamp()` again after the complete UPDATE/trigger
work, under the same existing enrollment row lock. An expired result rolls back the entire
transition and receipt and returns `false`. No schema, phase graph, domain rule or HTTP
contract changes. `ready` and `created` intentionally retain acknowledged external facts;
their non-fresh transitions are unchanged. Prepare dispatch has a separate post-write
`dispatchable()` fence; action claiming only opens non-consuming inspection, followed by
the fresh `begin_consume` fence before mutation.

The new real-PostgreSQL regression blocks the receipt table, observes the exact relation lock
while the enrollment is still fresh, waits for the database clock to reach expiry, and releases
the lock. For all eight fresh transitions it requires `false`, the unchanged phase and an
unchanged receipt count. The original code failed at `ready -> send_started`; this is not a
test of an in-memory repository or a Firebase response fixture.

The affected internal read is an exact primary-key lookup for one existing enrollment in the
same transaction: zero or one row, namespace/project/id-bound, with immutable expiry under
the held row lock. This is a documented read-performance exception for a correctness fence,
not a read-model optimization. Read frequency, EXPLAIN/scale timings and latency overhead are
unmeasured; no speedup or numeric budget is claimed. DBSP is rejected for this current-time,
strongly consistent point fence, which has no reusable join or aggregate. The controlled
PostgreSQL regression is the semantic oracle; workload/overhead acceptance remains separate.

### Factual SSOT drift

Class: **confirmed defect** in current documentation; category: **rule**; confidence:
**confirmed-from-source**. These facts are repaired without accepting new product behavior:

- Forge's local artifact CLI now coexists with a library-only attempt/recovery core and mock
  provider. Root, architecture, ADR and skill descriptions no longer imply only the old CLI
  exists. Real provider, Functions, Drive, persistent recovery and API handoff remain absent.
- Current #203 requests a Forge tab, primary Drive `appDataFolder` and service relocation.
  ADR0022 retains its accepted choice and explicitly identifies the required amendment before
  that different storage/runtime slice. Closed issue status does not substitute for code.
- The registration behavior inventory and source references identify the independently
  implemented, default-disabled Firebase runtime/migration0028. Its model remains `draft`,
  abstract and without a runtime observer; no Firebase conformance or accepted lifecycle is
  inferred from the model or its source references.
- Current service, persistence and review guides distinguish implemented OIDC/storage/SQL
  and application gates from separately pending activation. The API guide's complete migration
  count is corrected from 27 to 28. Historical evidence, migration bytes and archived outcomes
  are preserved. Some stale paragraphs predate these eight commits and are not attributed as
  new runtime regressions.

### P2: the imported narrow gate test failed workspace Clippy

Class: **confirmed defect**; category: **evidence**; confidence: **confirmed-from-source**
and reproduced by the broad local gate. The narrow PostgreSQL gate assertion imported with
#214 used `[gate.clone()]`, rejected by Rust1.98.1's `cloned_ref_to_slice_refs` under the required
`-D warnings`. The initial broad campaign stopped at workspace lint, leaving later gates
NOT_RUN. The assertion now independently expects `[Gate::TheatreFirebasePostgres]`, preserving
the exact plan oracle without cloning or suppressing the lint. Production gate behavior is
unchanged. The original failure is retained separately from the final campaign.

## Assurance ledger and coverage

| Claim | Owner / target | Required by | Evidence / disposition |
| --- | --- | --- | --- |
| Fresh effect fences reject expiry during durable receipt work atomically | Registration PostgreSQL / API | adr:0036 | held — integration-tested (local PostgreSQL 17.4); red/green and final result below |
| Proof possession, exact purpose/email/UID and one-shot setup precede provisioning; no Firebase session authority | Registration types/application/HTTP, sealed action values | adr:0036 | Source and constructor/bypass review; native/SQL scope below; effective vendor permissions remain live-pending |
| Private issuer CA is confined to the pinned Theatre identity clients and replaces roots | OIDC trust/config/registration | policy:AGENTS.md architecture boundaries | Source review and native tests below; historical attributed TLS/live sign-in is not rerun here |
| Losing a Forge reservation race never dispatches; mismatched requests conflict; unknown outcomes require exact reconciliation | Forge domain/application/attempt store | adr:0022 | Native suites below; memory/mock provider only; no persistent store or cross-process claim |
| Quint expectations and sensitive controls do not establish runtime conformance | Behavior catalog/models/runner | adr:0027; policy:cantos-behavior | Source review and runner unit tests below; actual Quint stage remains explicit |
| Catalog anchors/order/cursors survive withdraw, republish, insert, reorder and expiry | Media/listener application/PostgreSQL / Web contracts | adr:0035; issue:#172 | Native plus actual Router/SQL walk tests below; `VerifiedDeliveryFake` is delivery observation only |
| Infra relocation does not change runtime or auto-deploy | Infra assets/xtask / all apps | issue:#212; adr:0021 | Source/config/generator review and tooling tests below; no service activation |
| Current docs describe implemented scope and accepted decisions accurately | Product/architecture/skills | policy:AGENTS.md source of truth | Factual corrections above; historical evidence unchanged |
| UI adoption is currently qualified | Shared components / Web/CMP | policy:cantos-ui-design preview qualification | gap-required: identical 20 baseline errors; no qualification hash rewritten |

Coverage is complete for the declared changed-rule passes: Forge domain/application/reconciliation,
its Quint capability and runner bindings; listener keyset/domain/application/SQL/cursor and catalog
walk helpers; Firebase bootstrap/action/config/application/SQL/HTTP/contracts and Theatre Web
signup/browser/history/shell; OIDC trust; infra/xtask/Kani tooling; active owning docs and skills.
Moved archives and historical evidence are reviewed for lifecycle, links and preservation, not
revalidated as current implementation specifications. No changed Kotlin application source is
present in the selected contributions; native device behavior and untouched reconcile kernels
are outside this source review. The broader verification baseline also exercises prerequisite
changes imported by the merges. Generated compiler output is checked through its existing
independent oracle. No private manuscripts, credentials or generated media are committed.

The final repair and SSOT diff received a separate read-only review: no-actionable-findings for
the scoped final source. This is not approval or production acceptance.

## Local verification

The final results below retain each failed/interrupted attempt separately. Full command output remains ignored under
`artifacts/agent-runs/review8/`, with native PostgreSQL runner evidence under
`target/revision-evidence/`. Rust is pinned to 1.98.1; PostgreSQL is Homebrew 17.4;
`TMPDIR=/private/tmp`. The installed audio tools and Gradle 8.14.3 are placed on PATH, without
installation or service configuration. No real Firebase, Kanidm, storage/CDN or paid provider
call is made by this review.

### Retained attempts and independent results

Commands run from `/private/tmp/cantos-review8-20261016`. The initial input manifest and
subsequent SHA-256 checks confirm that the reviewed repair/test/model metadata remained
unchanged across the reconnect. Only the factual report and evidence-index text were added
later; runtime evidence is not invalidated by those documentation additions.

- `cargo xtask verify repository`: FAIL on the clean pinned base, exactly 20 existing shared
  component qualification errors. The later broad repository attempt has the identical list.
- `cargo xtask verify postgres --suite theatre-firebase`: the first sandbox attempt failed
  before tests because loopback binding was forbidden. Its permitted disposable-cluster run
  then genuinely failed the new regression at `ready -> send_started` against the unfixed
  adapter. Both logs are retained; neither is relabeled PASS.
- `cargo xtask verify behavior`: UNSUPPORTED, missing the exact locked toolchain.
- `cargo xtask preflight --changed-only --base 25b1c701b686018503d0f341d3966d3d35eaaf4b`:
  UNSUPPORTED at prerequisite discovery for behavior; every other gate NOT_RUN.
- Adding `--skip behavior` reached repository: FAIL with the same 20 baseline lines; later
  gates NOT_RUN. Adding `--skip repository` exposed the inherited workspace Clippy failure.
- After the one-line gate-test correction, that broad command's inner formatter, Script IR
  reference, independent Protobuf generation/drift/lint, workspace Clippy and workspace tests
  completed. Native library results include API 630 passed/99 ignored, Forge 199, Studio 239,
  Theatre 221 and xtask 409 passed/2 ignored, plus their independent integration-test targets.
  The Mac connection was then lost during DBSP tests. No test failure or aggregate summary
  was produced for that stage; the outer campaign is **interrupted**, not PASS.
- `python3 -m unittest discover -s scripts -p 'test_*.py'`: 536 tests OK, 5 optional skips.
  Its Kani/Quint messages come from runner fixtures/doubles, not actual proof execution.
- On reconnect no build/test/cluster process from that campaign survived. The branch still
  points to the pinned base with the intended dirty changes. No remote review branch or PR
  existed, and remote develop still matched the pinned base. Previously completed results
  are reused only for their identical applicable inputs; remaining gates run independently.
- The fixed regression passed in the fresh full PostgreSQL campaign: Firebase SQL 16/16,
  including all 8 receipt-wait transitions. The full `cargo xtask verify postgres` gate also finished PASS, including 51 Catalog HTTP
  cases, the separate recovery cases, private-byte/socket-HTTP checks and the complete
  process-kill/PostgreSQL-restart campaign. Its owned cluster was stopped.

### Final continuation and layer gates

The completed full PostgreSQL run printed:

```text
verify summary
  PASS         postgres
verify: PASS
```

The continuation command deliberately reused the completed unchanged steps:

```sh
PATH="/tmp/cantos-audio-tools:/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:$PATH" RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp GRADLE_USER_HOME=/tmp/cantos-gradle-user-home cargo xtask preflight --changed-only --base 25b1c701b686018503d0f341d3966d3d35eaaf4b --skip behavior --skip repository --skip rust-format --skip script-ir-reference --skip protobuf-reference --skip rust-lint --skip rust-tests --skip postgres
```

It completed DBSP, WAV reference, real synthetic-byte audio inspection, WASM lint, Studio Web
build, Theatre Web build, Leptos preview build and shared CMP UI preview. The shared CMP
command printed `BUILD SUCCESSFUL in 1m 37s`; its fresh JUnit XML has 205 tests, zero failures,
errors or skips. These are host tests/builds, not device or screenshot/runtime acceptance.

A second Mac transport loss interrupted Theatre CMP demo. At the next reconnect no build/test
process survived. The log has no final summary: `:shared:desktopTest` has fresh XML for 150
passing tests, while `:preview-sandbox` has no completed test XML. Therefore the entire
`theatre-cmp-demo` gate is **BLOCKED (interrupted, no xtask gate result)**, not PASS; its shared
subset is independently example-tested. `mutation` is **NOT_RUN**, not reached. No campaign
or already passed gate was replayed after that reconciliation. The missing final aggregate is
retained explicitly, rather than manufacturing a full-preflight PASS.

| Layer / claim | Result | Evidence level / scope |
| --- | --- | --- |
| L1 architecture/ownership | held | documented source review; existing registration adapter, no new dependency or API group |
| L2 domain/types/errors | held, unchanged | existing sealed proof/new-person values and phase graph; repair preserves the existing expiry rule |
| L3 pure/static/contracts | completed inner checks reused | statically-checked, example-tested, property-tested and independent reference oracles at identical source inputs; outer preflight interrupted |
| L4 application/adapter orchestration | completed native tests reused | mock-integration-tested with `Memory`, `Mailbox`/`HeldExchangeMailbox`, `FakeIssuer` and synthetic loopback Firebase replies; no vendor calls |
| L5 PostgreSQL/Router/private bytes/restart | PASS | integration-tested (fresh local PostgreSQL 17.4 and private filesystem); Firebase 16/16 and Catalog HTTP 51/51 plus separate recovery cases; `VerifiedDeliveryFake` does not establish public delivery |
| Web/preview builds and shared CMP host | completed | compiled; shared CMP JUnit 205/205, Theatre shared JUnit 150/150; no browser/device/visual qualification claim |
| Full Theatre CMP demo | BLOCKED | transport interruption; preview-sandbox test completion and gate exit unavailable |
| Mutation | NOT_RUN | not reached before interruption; no new Script IR validator change in this repair |
| Repository | FAIL | 20 identical clean-baseline qualification errors; final focused hygiene comparison retained |
| Quint | UNSUPPORTED | exact locked runtime/tool missing; selected behavior/conformance steps NOT_RUN |
| Kani | NOT_RUN | optional bundle unavailable; Python runner doubles are not proofs |

Final repair hashes (SHA-256):

- `postgres.rs`: `3a70f3ed781fc89189f99e03d43a90363ce5a1b4ceb5256a16766f825bc99558`
- `postgres/tests.rs`: `48b963ae924f003424927502feb7cd0bcbd980fd0d20f5e30f4230ad3bb5ff6e`
- `xtask/src/gate.rs`: `c50c33c6c9a0bcc8bb509a3d9d28ffee6b5fe675057c034fecd2d578904c5f0d`

The final 23-path diff received independent read-only review with no-actionable-findings.
Final-diff documentation/whitespace checks are separate from the retained runtime results;
commit/push/PR packaging does not repeat those unchanged tests.

## Safe cache cleanup

Before the campaign, 26 exact reproducible directories were deleted from idle old Cantos
checkouts: **15,018,131,456 allocated bytes** / **14,945,176,768 logical bytes**. The #172
checkout lost only `target/debug`; its head remains
`238b7618942603e406c2fda81d6b78b6655e7ecd`. All 15 processed old worktrees retained their HEAD,
Git status and preserved target/build metadata. Unique logs/evidence/reports, tracked evidence,
symlinked paths, branches/stashes/history, runtime/databases, credentials, toolchains and backups
were retained. The active review worktree was excluded.

The local allowlist/measurement report is `/private/tmp/cantos-cache-clean-20261016-report.json`.
These are measured removed path footprints, not a whole-volume free-space claim during
concurrent builds/APFS accounting. New verification output consumes new space afterward.

## Deferred acceptance and residual risk

Repository qualification remains FAIL until real shared-component requalification; updating
its hashes without the required renderer evidence would not repair it. The exact locked
Node 24.19.0/npm 11.9.0/Quint 0.33.0 toolchain and Kani bundle are unavailable here; no toolchain is
installed to manufacture a pass. Actual Quint/Kani execution remains unproven in this campaign.

Full Theatre CMP demo completion and mutation remain deferred after the interrupted campaign.
On a stable Mac, first reconcile existing processes, then run `cargo xtask theatre-demo verify`
and `cargo xtask verify mutation`; require nonzero passing tests and all nine killed mutants.
Next owner: unassigned — assignment required before accepting those gates. No Kotlin or Script IR
validator source changed in the repair; this does not waive the conservative broad gate selection.

Real Firebase proof/mail/setup and effective Kanidm privilege protection, safe action ingress,
cross-device/account-switch browser recovery, public storage/delivery and Android/iOS native
playback retain their existing separate acceptance gates. The Web account-switch concern is
an unresolved acceptance risk: the signup capability has its own epoch, while the accepted
mailbox-proof contract intentionally remains independent of Theatre cookies. This review does
not invent session-bound enrollment authorization. Forge has no persistent attempt store,
cross-process fence, Drive adapter, Functions consumer or provider/runtime acceptance.

The post-trigger check closes the demonstrated SQL wait; it does not prove wall-clock validity
at every later scheduler, COMMIT or remote-provider instant. No throughput or performance claim
follows from the added point read. Remote CI is not checked, polled or dispatched. No merge,
issue closure, live deployment, credential/grant change or tester notification is performed.
