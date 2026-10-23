# Executable stateful behavior contracts

Start with [catalog.json](catalog.json), then the [authority inventory](inventory.md).
The catalog is the only ID/lifecycle/selection owner; `.qnt` owns executable expectations
for accepted capabilities. Companion `_tests.qnt` files contain named runs of the same
model, never another reducer. Product prose accepts requirements and explains context.
Protobuf owns transport/schema; native tests keep their implementation/platform boundaries.

The private AudioSubmission pilot is **PR A model foundation plus PR B pure-core observations of issue 106**. Its lifecycle is draft:
model-only PASS neither accepts the model nor demonstrates Rust/DB/API/client conformance.
`--conformance` reports independent Rust pure-core observations separately; it does not establish DB/API/client acceptance.
Read [ADR 0027](../../docs/decisions/0027-quint-executable-behavior.md) and the
[PR B observation handoff](../../docs/archive/handoffs/quint-conformance-handoff.md).

A **draft, model-only** producer-side capability, `cantos.production.forge-attempt` (#203,
[Forge](../../services/cantos-forge-functions/README.md#generation-attempts)), describes when a billable provider call
may start and what an uncertain outcome permits. It has no Rust observer; native mirror tests
carry each named run's ID and `tests/behavior_binding.rs` keeps the lists equal.

Two **draft** native Identity v1 capabilities (#334/#335,
[ADR 0045](../../docs/decisions/0045-v1-email-password-sign-in.md)) describe mailbox-proof
consumption, `cantos.identity.native-mailbox-proof`, and fact-to-session issue plus per-admission
security-state rechecks, `cantos.identity.native-session-admission`. Each registers a test-only
**pure-core observer** (#106): `--conformance` replays the original-model traces through the real
`MailboxProof::plan_*` and `validate_fact`/`SecurityState::from_response`/`decide_issue`/
`decide_admission` seams with a small, disclosed synthetic shell. This is pure-core conformance,
not DB/RPC/E2E acceptance; read the [native protocol](observation-protocol.md#native-identity-auth-observation-protocol-v1).
Their [inventory rows](inventory.md#draft-cantosidentitynative-mailbox-proof-334335-v1-signup)
report two contract/implementation mismatches and a needs-decision case that no property constrains, and map
the unchanged legacy `cantos.identity.bearer-lifecycle` scope.

Two further **draft, model-only** capabilities describe public media rules:
`cantos.media.public-playback-delivery` ([ADR 0032](../../docs/decisions/0032-theatre-origin-playback-delivery.md),
accepted for step-1 origin integration only) and `cantos.media.public-cover-projection`
([ADR 0033](../../docs/decisions/0033-release-pinned-public-cover.md), proposed).
Neither has a registered Rust observer, so `--conformance` does not apply to them; their
[inventory rows](inventory.md) separate implemented kernels from model assumptions.

## Run locally

```sh
# Provide the exact Node/npm versions in tools/behavior/package.json separately.
# Once, or after the integrity lock changes: explicit development-only installation.
cargo xtask verify behavior --setup
# Installed tool only: no npm fetch, Rust evaluator, solver, browser or service.
cargo xtask verify behavior
cargo xtask verify behavior --capability cantos.media.private-audio-admission
# Add real pure-core observations, semantic sensitivity and fresh implementation inputs.
cargo xtask verify behavior --conformance --capability cantos.media.private-audio-admission
cargo xtask verify behavior --conformance --capability cantos.identity.native-mailbox-proof
cargo xtask verify behavior --conformance --capability cantos.identity.native-session-admission
# Without --capability every model runs; only catalog-registered observers are replayed.
cargo xtask verify behavior --conformance
```

Each executable capability runs type/effect checks, all selected named runs, the fixed
bounded sampled property campaign, a separate reachability query per required witness and
one deliberate negative substitution per selected property. Every query fixes backend,
seed, samples, steps and timeout. A witness must yield a real trace violating `not(witness)`;
not finding it is FAIL, not proof of unreachability. A mutant must typecheck and violate its
one target invariant with a raw counterexample. Missing/zero/skipped scenarios, partial
traces, missing evidence, stale sources/tools and syntax/runtime failures fail closed.

The official Quint CLI is used unchanged. The runner only validates selected declarations,
output contracts and the raw ITF envelope/hashes. It does not parse a new model language,
evaluate actions or reproduce expected state transitions in Python/Rust. With `--conformance`,
the [registered codec](../../scripts/verify_behavior_conformance.py) decodes existing ITF outputs
and compares an [event-only Rust observer](../../services/cantos-server/src/contexts/media/quint_conformance_tests.rs).
The native Identity capabilities use a [declared codec](../../scripts/verify_behavior_conformance_native.py) and their own test-only
observers ([mailbox](../../services/cantos-identity/src/quint_conformance_tests.rs),
[session decisions](../../services/cantos-server/src/platform/identity_auth/decision/quint_conformance_tests.rs)).
The dispatcher's finite allowlist (`adapters()`) fails closed for any capability the catalog and an adapter do not both register.
The existing POSIX bounded process primitive owns timeout/output/group cleanup. Current
execution requires POSIX with waitid/WNOWAIT; another host is UNSUPPORTED at tool probing.
The behavior gate is opt-in outside full preflight/CI. Non-document changes under
`specs/behavior` or `tools/behavior`, and the behavior primitive, select it in changed-only
preflight. Full/uncertain preflight retains its existing gates and also retains behavior when
changed model/tool inputs select it; run the explicit suite for other behavior-tool changes.

## Read the pilot

[Model](media/private_audio_admission.qnt) and [named runs](media/private_audio_admission_tests.qnt)
use closed state/action/outcome/error variants. A submission is already prepared and valid.
One exact byte equality class and one conflicting class separately exercise digest and
length checks. Time partitions cover creation, admitted_at, in-window and expiry boundaries,
including backwards observations. The class values are synthetic, not literal byte metadata.

`begin` calls the modeled pure admission decision. `Inspect` reserves only harness history;
it creates no admitted receipt. `complete(true)` abstracts inspected/stored valid media and
rechecks today's authority before settlement. `complete(false)` and `abandon` preserve
settled history. `revoke`, `invalidate`, `restoreCurrent` and `clock` change external facts;
they preserve the receipt. Restoration abstracts archive/restore over the same immutable
binding, not acceptance of changed Content/rights/guidance, and never clears revocation.
Named assertions emit an explicit `Assertion`/`NoOutcome` no-effect checkpoint rather than
duplicating the previous action. Exact replay retains its identity; producer access still fails after revocation,
staleness or expiry. Owner state is a separate projection with the contract's precedence.
The sampled generator biases exact attempts so settled/replay antecedents are reached before
invalidation, while retaining all modeled input/environment actions. No actual
authentication, bytes, database transaction, object store or cancellation runs.

## IDs and authoring

Catalog property/run/witness/control rows inherit the capability's owner, lifecycle and
sourceRefs. IDs survive declaration/path renames. A semantic split requires an explicit old
ID mapping; retire IDs rather than reusing them. `draft`, `accepted`, `deprecated` and
`needs-decision` describe reviewed authority, independent of PASS/FAIL. Unmodeled entries
carry a reason and are not executable selections. Never migrate proposed UI behavior as truth.

Each new property needs a reachable positive state or antecedent, an exact named run and a
sensitive typechecking negative control. Keep budgets bounded and deterministic. AI-generated
scenarios are proposals until reviewed; never edit expected semantics merely to match code.
Follow [the shared skill](../../.agents/skills/cantos-behavior/SKILL.md).

## Evidence and reproduction

Fresh `target/verification/behavior/<capability-id>-<unique>/` bundles retain report.json,
exact commands, configured seed and actual trace seed, source HEAD plus input SHA-256 and
the exact original input bytes under `inputs/` (including dirty models/scenarios),
locked tool versions, setup receipt and installed-tree/executable hashes, type/effect output, named-run
ITF, sampled trace, witness traces, mutated sources and counterexamples. HEAD alone does
not identify dirty inputs. Retained bytes allow reconstruction after the worktree changes;
recorded commands name the original checkout, so reconstruct that snapshot before replay.
Before PASS, sources and tools are rehashed. These are endpoint
freshness checks, not filesystem isolation against concurrent mutate-and-restore writes.

Failed/partial bundles are retained. There is no reusable latest-PASS, automatic baseline
rewrite or cleanup. Concrete trace plus recorded seed/tool/model identities is the replay
input; a seed alone is not a counterexample. Keep generated bulk output out of Git. Promote
only a small sanitized reviewed regression with full model/tool/config provenance. PR B
must validate identity/schema/actions and complete observations before comparing checkpoints.

PASS means type/effect-checked, named-example-tested and bounded sampled model properties,
plus witnessed reachability and mutation sensitivity. It is not exhaustive/temporal proof,
shrinking assurance, SQL/provider/actual-media acceptance or native device evidence.
Default `implementation-conformance` remains NOT_RUN. `--conformance` separately writes
`conformance.json` with identity, original-trace replay, model-mutant sensitivity, observer-fault
sensitivity and freshness stages. It calls real Rust decisions/constructors only in test code,
with synthetic external effects; read the [protocol and limits](observation-protocol.md). A
model-only PASS cannot substitute for an independently PASS conformance report.
