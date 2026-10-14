# ADR 0026: Quint executable stateful behavior contracts

- Status: Proposed for model/architecture review; tool selection is authorized by issue 106.
- Date: 2026-10-14
- Decision owner: Cantos project
- Related issue or PR: [issue 106](https://github.com/loveoverflowcom/cantos/issues/106), PR A foundation
- Supersedes / superseded by: N/A

## Context

Cantos already has native Rust, Leptos and CMP tests, versioned Protobuf-first transport,
private media admission and an explicit evidence vocabulary. They need one reviewable source
for shared stateful expectations without another handwritten reference reducer. Current
implementation and UI behavior cannot silently settle missing product decisions.

The [capability inventory](../../specs/behavior/inventory.md) distinguishes approved intake
requirements, real code seams and bounded modeling assumptions. The pilot starts with one
valid private AudioSubmission. It does not migrate schema, HTTP or database behavior.

## Decision

1. Use [Quint](https://quint.sh/docs/language-basics) `.qnt` files as executable stateful behavior
   SSOT **once their requirements/model are accepted**. Prose owns product rationale and
   unresolved decisions. [Catalog metadata](../../specs/behavior/catalog.json) owns stable
   capability/property/run/witness/control IDs, owner, lifecycle and source references.
   Metadata rows inherit their capability's owner/lifecycle/source references. A model PASS
   never promotes `draft` to `accepted` or proves that a feature exists.
2. Keep Protobuf-first wire/schema ownership, existing transport goldens and native typed
   Rust/Kotlin/Leptos tests. Quint adds no production dependency, API/DB field or second
   application reducer. No Gherkin/Cucumber or custom modeling language is introduced.
3. Deliver one runnable private admission/replay pilot with exact errors, immutable receipts,
   current-authority rechecks, separate owner-history projection, named scenarios, positive
   reachability traces and one sensitive deliberate mutation per selected property.
   The boolean inspected-media input is an abstract shell fact, not an actual-byte witness.
4. Pin official npm `@informalsystems/quint` **0.33.0**, Node **24.19.0**, npm **11.9.0** and
   transitive integrity lock in [tools/behavior](../../tools/behavior/README.md). The upstream
   [0.33.0 release](https://github.com/quint-co/quint/releases/tag/v0.33.0) fixes action/effect
   regressions present in 0.32.0. Explicitly select the **TypeScript** simulator; the current
   CLI otherwise defaults to Rust. No solver, Java distribution or simulator download runs.
5. `cargo xtask verify behavior [--capability ID] [--setup]` uses existing gate/pipeline/status
   architecture. `--setup` alone authorizes locked development-only npm installation, with
   lifecycle scripts disabled. Ordinary runs are offline with installed tools. Model/tool
   changes select the behavior gate in changed-only preflight; the foundation is outside
   ordinary full preflight/CI until its toolchain is separately provisioned.
6. Gate type/effect checks, every selected named run, bounded sampled property simulation,
   every required witness and every negative control. Witness queries ask Quint to violate
   `not(witness)` and retain the actual reachability trace. A control must typecheck and
   produce an invariant violation plus raw ITF; syntax/runtime/tool errors never kill it.
   Retain commands, configured/concrete seeds, source revision, source/input/tool SHA-256
   identities, logs and raw ITF under ignored `target/verification/behavior/`.
7. Follow the shared [behavior skill](../../.agents/skills/cantos-behavior/SKILL.md) for API,
   Leptos and CMP work. Generate scenarios from accepted declarations and retain their IDs;
   treat counterexamples as evidence to investigate, never permission to rewrite requirements.
   [PR B](../work-plan/quint-conformance-handoff.md) will compare independent observations
   from existing production seams. Wider shared UI/CMP scenarios remain later work.

The CLI distinguishes [sampled simulation and solver checking](https://quint.sh/docs/quint).
This gate is deterministic bounded **sampled sequential model evidence**, not exhaustive
proof, fairness/liveness, Rust refinement, SQL/provider acceptance or device evidence.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Native tests only | Already useful for implementation/platform boundaries | No single executable shared stateful expectation | Retained alongside Quint |
| Hand-maintained Rust/Kotlin/JS model reducers | Familiar execution | Duplicated semantics drift or mirror implementation mistakes | Rejected for migrated capabilities |
| Solver backend or generic model platform | Stronger exploration under a reviewed model | Additional solver/runtime, budgets, codecs and proof obligations | Deferred; no demonstrated pilot need |
| Quint TypeScript CLI | Official typed actions/runs, raw traces, narrow existing orchestration | Sampled evidence and pinned output format maintenance | Selected for this bounded foundation |

## Consequences

- Product and user impact: no visible product behavior changes; model lifecycle stays draft.
- Compatibility and migrations: no API/Protobuf/schema/database change.
- Operations, storage, and production cost: a local development-only npm dependency; no
  infrastructure, cloud writes, provider calls or paid resources.
- Implementation and recovery requirements: fresh evidence for fixed inputs, output/timeout
  bounds and existing POSIX owned-process cleanup. Partial/failing records are retained.
  Child processes clear ambient global module lookup and reject Node preloads/options;
  npm uses isolated empty user/global config and the official registry.
- Deferred questions: real observation codec and implementation conformance; provider/SQL/
  authorization/byte acceptance; complete publication/listening and cross-client migration.

## Validation and revisit conditions

Run the positive model gate and its witness/mutation sensitivity checks before acceptance;
keep model and implementation results separate. Upgrades must rerun output-contract tests and
all positive/negative stages. A model and its assertions may share a misconception: negative
controls cover the inserted faults only. Independent product review and real observations are
still necessary. Issue 106 remains open across the conformance/client work.

The VOT [ADR 0003](https://github.com/loveoverflowcom/vot-workspace/blob/develop/docs/adr/0003-executable-behavior-models.md)
and [authoring guide](https://github.com/loveoverflowcom/vot-workspace/blob/develop/specs/behavior/README.md)
were reviewed as reference patterns, not copied as Cantos product authority or tool pins.
