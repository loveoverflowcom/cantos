---
name: cantos-behavior
description: >-
  Use Cantos's stable Quint catalog to author or review executable stateful behavior contracts,
  derive AI test scenarios, run model and optional independent pure-core conformance gates,
  and interpret counterexamples. Resolves approved
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

## Required order

1. Read [the catalog](../../../specs/behavior/catalog.json), locate the stable capability ID,
   then its sourceRefs, inventory, owner and lifecycle. Check actual production code separately.
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

Read [authoring and evidence](references/authoring-and-evidence.md) for the bounded recipe,
scenario partitions and a counterexample example. [Execution guide](../../../specs/behavior/README.md)
owns actual command/budget/output contracts. Skills own method, not a second product spec.
