# ADR 0002: Initial Script IR contract and canonical content boundary

- Status: proposed
- Date: 2026-10-10
- Decision owner: unassigned; production adoption needs a named owner
- Related issue or PR: [part of #1](https://github.com/loveoverflowcom/cantos/issues/1)
- Supersedes / superseded by: N/A

## Context

The repository's `0.1.0-draft` example had no schema or executable validator. It mixes content
with casting, rights status, lifecycle fields and floating-point delivery. Issue #1 requires a
versioned contract before persistence and a Studio consumer. There are no existing accepted
revisions or application endpoints in this checkout. A CLI and deterministic tests are concrete
consumers of one domain module in the existing server directory; no separate service/crate is
needed beyond that server package.

## Decision

Implement an experimental `0.1.0` JSON Schema (Draft 2020-12) and a pure Rust admission path:
version probe → closed raw DTO/shape checks → accumulating semantic validator → immutable
`ScriptContent`. Only the validator constructs content; there is no public field, Deserialize,
Default or mutation path for content. Use array order for sequence, globally unique opaque IDs,
an explicit narrator, same-scene cue anchors, typed respellings and preserved source/rights
record references. The [contract](../../services/cantos-server/contracts/README.md) owns exact spellings, bounds,
diagnostic order, normalization and field classification.

Keep casting, mutable rights status, timestamps and revision/workflow metadata outside content.
Keep source/generation/rights references in canonical exports but outside performed-content
digests. A digest does not establish evidence integrity or rights eligibility; the persistence
slice must store the complete export immutably and check referenced records under authorization.

Read/write only `0.1.0`; reject the untouched illustrative `0.1.0-draft` example explicitly. Do
not infer a converter that drops fields or guesses rights/casting. A converter needs its own
mapping, provenance notes and compatibility evidence. Freeze c1 as compact JSON over a bounded
subset with fixed ASCII keys, normalized text and integer delivery; hash the tagged content
projection with SHA-256. Use [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) as the serialization
reference for this subset after admission normalization, not as a claim that arbitrary input
JSON or floats are supported.

Pin direct dependencies and commit Cargo.lock, using the installed Rust 1.87 toolchain:

| Dependency | Consumer / reason | Source checked |
| --- | --- | --- |
| serde 1.0.228 | Closed wire DTOs; no domain deserialization bypass | [versioned API](https://docs.rs/serde/1.0.228/serde/) |
| serde_json 1.0.145 | Strict JSON decoding at the wire edge | [versioned API](https://docs.rs/serde_json/1.0.145/serde_json/) |
| unicode-normalization 0.1.24 | Derived text NFC, Unicode 16.0.0 | downloaded pinned package API/tables and [upstream](https://github.com/unicode-rs/unicode-normalization) |
| sha2 0.10.9 | ContentDigest SHA-256 | [versioned API](https://docs.rs/sha2/0.10.9/sha2/) |
| jsonschema 0.33.0, dev only | Independent Schema/Rust shape differential tests | [versioned API](https://docs.rs/jsonschema/0.33.0/jsonschema/) |

Disable jsonschema default features; tests need no HTTP/file reference resolution. Use a small
explicit encoder and a Python stdlib oracle rather than adopting a generic JCS dependency. The
bounded source-mutation runner adds no dependency. This implementation is locally exercised;
the ADR is not recorded as accepted because no owner has accepted production adoption.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Preserve draft wire shape | Similar to old example | Mixes mutable metadata and floats into content | Rejected for the first implemented contract |
| Auto-convert the illustrative draft | Easier intake | Missing provenance and ambiguous dropped metadata require human decisions | Deferred until a real import consumer needs it |
| Generate schema from DTOs | Less structural duplication | Extra generator; semantics still need an independent validator | Hand-authored schema plus differential corpus chosen for this bounded slice |
| Only JSON Schema | Simple client validation | Cannot resolve IDs, speakers, cue locality or evidence references | Rejected |
| Hash the complete export as content | Binds evidence to the hash | Rights/provenance changes falsely appear to alter performed content | Separate immutable revision/evidence binding required instead |

## Consequences

- Product and user impact: a structured original Vietnamese episode can be admitted/exported
  offline. Incomplete editorial states remain outside `ScriptContent`; an editor must keep its
  own draft and render all submission blockers. No Studio flow is delivered here.
- Compatibility and migrations: first contract only; no supported stored version is retired.
  Future changes require new exact versions and explicit conversion/rejection, never rewriting
  immutable historical records or c1 digests.
- Operations, storage and production cost: no DB, provider, media or network operations in
  validation; no paid generation. Record existence, rights and authorization remain external.
- Implementation and recovery requirements: retain original source bytes, complete exports,
  immutable revision records and actor evidence; stale writes and retry identity are session 2.
- Deferred questions: owner acceptance, broader language/Unicode profile, cue-markup review,
  pronunciation phoneme support, old draft conversion, per-span provenance and persistence/API
  envelope. Voice/cast capability checks belong to the production input boundary.

## Validation and revisit conditions

Use [the evidence ledger](../evidence/script-ir-contract.md): exact rejection fixtures,
Schema/Rust parity, independent byte/digest goldens, reorder/Unicode/field-sensitivity examples
and bounded mutation checks. Review before storing production revisions; revisit when a
converter, new language/performance field or canonical scheme is needed. Full-stack saves,
provider behavior, rights gates and publication are outside this evidence.
