---
name: cantos-behavior
description: >-
  Use Cantos's stable Quint catalog to author or review executable stateful behavior contracts,
  derive AI test scenarios, run model and optional independent pure-core conformance gates,
  and interpret counterexamples. Owns the SSOT impact procedure every stateful change runs,
  code-only diffs and unregistered capabilities included, before choosing a write. Resolves approved
  requirements, implementation facts and needs-decision gaps before modeling; requires named runs,
  reachability and sensitive negative controls; preserves Protobuf and native API/Rust/Leptos/CMP
  test boundaries and distinguishes sampled model evidence from implementation conformance.
---

# Cantos executable behavior

Working-method owner for registered stateful behavior contracts, shared by API, Leptos and
CMP agents. It adds no product rule or outward-action permission. Compose
[the foundation](../cantos-engineering/SKILL.md) for L1–L5 order, evidence vocabulary and report.
Product rationale/unresolved decisions stay in owning docs; accepted `.qnt` declarations own
executable expectations. [ADR 0027](../../../docs/decisions/0027-quint-executable-behavior.md)
and [the inventory](../../../specs/behavior/inventory.md) explain that division.

## SSOT impact procedure

The one procedure for deciding whether a change touches executable stateful behavior. The
[foundation](../cantos-engineering/SKILL.md#ground-rules) triggers it whenever a task can change
a state, guard, outcome, permission, lifecycle, retry/recovery step or the oracle that tests one,
including a code-only diff with no `.qnt` file and a capability nobody named. It is a lookup
and a classification: it never mandates a model edit and grants no write the task lacks.

1. **Look up before choosing a write.** Catalog → root pointer and module manifest (`owner`,
   `sourceRefs`, `ownedSources`, `implementationTests`) → module inventory and the owning
   requirement → `.qnt` model and named runs → actual code and callers. Search by the touched
   paths and domain, not only for a capability you already know. As an optional aid,
   `cargo xtask preflight --changed-only --base "$BASE_REF" --dry-run` lists the
   `behavior (CAPABILITY)` gates the registry's reverse index selects. Unmodeled entries
   (`model: null`) and paths no record names select nothing: an empty selection is not "no impact".
2. **Classify before writing.** At the time of writing no capability is `accepted` and
   ADR 0027 is proposed, so a model is a proposed expectation: compare it and the code with the
   inventory's approved-requirement rows. Only an `accepted` model is itself the oracle.
   Classes can combine (a code defect over an unmodeled guard); report each.

| Class | Selected by | Write, when the task grants it | Never |
|---|---|---|---|
| code defect | code diverges from an approved requirement the model also states | fix the implementation with a failing regression; reuse the run ID; model unchanged | edit expected results, witnesses or controls to bless the code |
| model defect | model contradicts an approved requirement; source and counterexample cited | minimal model/run/inventory fix through the write bundle; lifecycle unchanged | rewrite the requirement or read it from code or UI |
| approved requirement change | the user or a named owner accepted the new rule | the write bundle and the implementation as one delta | promote lifecycle from a PASS |
| missing model, scenario or capability | no entry, `model: null`, or no run covers the changed guard | reuse the entry; author a bounded draft only with scope, permission and a clear approved requirement; otherwise record the gap, owner and next step | per-renderer copies or a second reducer |
| needs-decision | requirement absent or conflicting | record the question and its source; continue independent work | adopt the UI's or the code's behavior as authority |
| no semantic change | presentation, copy, layout or an outcome-preserving refactor | none; state the reason and still run what the reverse index selects | edit `.qnt` to satisfy the procedure |
| read-only task | review, inspection or prompt authoring | report the class and the proposed delta | mutate code, model or catalog; widen the grant |

3. **Write only the classified delta** through the
   [write bundle](references/authoring-and-evidence.md#impact-write-bundle); model work then
   follows the required order below, and control returns to the calling workflow.
4. **Report** the foundation's `Behavior SSOT impact` field: capability ID and lifecycle (or
   absence), class, authority source, delta or no-model-change reason, checks actually run and
   open gaps. A draft, a model PASS or an unrun gate never stands for requirement acceptance or
   implementation conformance.

## Required order

1. Read [the catalog](../../../specs/behavior/catalog.json), locate the stable capability ID,
   then its sourceRefs, inventory, owner and lifecycle. A root pointer's `manifestPath` names the
   module-owned manifest that carries model, runs and controls ([federation](../../../specs/behavior/README.md#federated-registry-and-module-owned-manifests));
   never duplicate root-owned fields there. Check actual production code separately.
   `draft` or `needs-decision` is not accepted behavior just because a model runs.
2. State the behavior claim and scope in the foundation ledger. Separate approved requirements,
   real implementation facts and explicit finite assumptions. Report missing/conflicting
   semantics as needs-decision; do not choose a UI outcome as authority.
3. Read its `.qnt` model and named runs. Derive scenarios from guards, state transitions,
   exact outcomes and relational invariants. Reuse stable run IDs across native/platform
   tests where meaningful; keep new scenarios proposals until reviewed.
4. Pair each new property with an exact named run, a reachable antecedent/state and a
   typechecking negative control that violates that property. The catalog owns selection;
   do not create per-renderer model copies or a hand-synchronized reference reducer.
5. Run `cargo xtask verify behavior [--capability ID] [--conformance]`. `--setup` explicitly installs only
   the locked development tool. The default checks only models. The registered `--conformance` adapter calls real private
   admission decisions with event-only synthetic fixtures, without DB/provider/device access;
   they never provision infrastructure, start audio generation or call a solver.
6. Investigate counterexamples in source context. Preserve concrete ITF, command, seed,
   source/tool/config hashes and exact observed status. Classify requirement/model error,
   real implementation mismatch, adapter error or unresolved assumption before changing code.
   Never silently rewrite expected behavior to make conformance green.
7. Retain native typed unit/integration/UI tests for transport, constructors, runtime and
   platform boundaries. Model PASS does not demonstrate any implementation call; use the
   [observation protocol](../../../specs/behavior/observation-protocol.md) for independent observations.
8. Review freshness and report each actual stage plus exclusions. Catalog lifecycle changes
   need requirement acceptance evidence; execution alone never promotes it.

## Implementation observations

The registered backend observers (private audio admission; native Identity mailbox proof and
session admission, the latter two with a per-checkpoint `core`/`shell` origin label) consume only
stable trace IDs and replay events plus input facts, never expected states/outcomes. Decode the
exact pinned ITF representation, preserve every assertion checkpoint without repeating effects,
compare exact errors/state and retain full actual receipts. Read
[the protocol](../../../specs/behavior/observation-protocol.md) before adding an adapter.

Positive expectations use original-model named runs, sampled trace and reachability witnesses.
A mutant counterexample is deliberately wrong behavior: replay prefixes through the first
semantic mismatch and label it sensitivity-only. Parsing, illegal later continuations, runtime
failures or zero selected tests never count as a kill. Add an original-trace observation fault
control that demonstrably exposes adapter drift, separately from model mutations.

Keep model `report.json` and `conformance.json` status/provenance independent. A new observer
joins the dispatcher's finite allowlist (`adapters()`) with a declared codec, per-control detection
reasons and fault controls; never a general plugin engine. The current scope is
`pure-core-synthetic-shell`. For the audio adapter Complete(false)/Abandon are harness effects and
failed media masks the decision's raw result in the Quint oracle; for the native Identity adapters
the store/shell rules listed in the protocol are synthetic. No production application
orchestration, authentication, database, actual bytes, object store, restart, public delivery or
client/device claim follows. Preserve native constructor, foreign-binding, numeric and
event-matrix tests.

## Enforcement and exceptions

| Rule / failure prevented | Good / counterexample | Oracle and current enforcement | Exception |
| --- | --- | --- | --- |
| Catalog lookup prevents accidental model or ID duplication | reuse registered run ID / create a new UI-owned copy | catalog validates IDs, local refs and selection in the behavior gate | unmodeled capability stays explicitly scoped/needs-decision |
| Reachability prevents vacuous properties | trace reaches admitted/replay/reject antecedent / `if false` makes everything pass | every registered witness must yield actual Quint invariant violation of its negation and raw trace | no unwitnessed property may be reported meaningful |
| Negative controls check assertion sensitivity | mutated action typechecks and breaks its property / parser failure called killed mutation | one control per property, exact substitution and actual invariant counterexample | controls cover inserted faults only, not all bugs |
| Independent observation prevents a mirrored oracle | call actual production seam and compare Quint checkpoints / second handwritten Rust reducer | `--conformance` compares original traces against event-only real core observations; full receipt/call-count and semantic sensitivity checks | native codec/numeric/platform tests may keep their existing distinct oracles |
| Honest tiers prevent invented acceptance | sampled model PASS; API NOT_RUN / model PASS called backend verified | existing status vocabulary, stage report, retained source/tool hashes | no tier promotion without actual attributable execution |

Read [authoring and evidence](references/authoring-and-evidence.md) for the impact write bundle
and lookup examples, the bounded recipe, scenario partitions and a counterexample example.
[Execution guide](../../../specs/behavior/README.md) owns actual command/budget/output
contracts. Skills own method, not a second product spec.
