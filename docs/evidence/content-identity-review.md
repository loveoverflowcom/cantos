# Content Identity assurance review

Scope: base/merge-base `b620109d93f7ced11628ce380c282ccf7a9c351c`; code HEAD
`44bd556ea2f0d1ed981384e41c4be965e8570671`, tree
`5acb12e2dd3948ed42347c833f180a4c3e5e0a57`. Three-dot contribution plus full touched
functions, constructor/serde/row paths, existing Catalog/editorial/ingestion consumers and
owning docs. No source change was made during this final read-only pass.

Method: [cantos-code-review](../../.agents/skills/cantos-code-review/SKILL.md), foundation and
Script IR owners. This was a separate assurance pass in the implementing Codex session;
no second agent or human review is claimed. Assessment: **no-actionable-findings** for the
declared Content Identity boundary. This is not production/rights/audio acceptance.

## Assurance ledger

| Claim / owner | Required by | Evidence held / level | Disposition |
| --- | --- | --- | --- |
| Work/unit/content/script authority; Catalog core and application | ADR0001/0018; ADR0019; task scope | pure private values, owner entrypoints; repository boundaries statically-checked | held |
| IDs never derive from text/order; immutable content independent of metadata | business rules1–3; Script IR SIR1/8 | typed IDs; metadata edit and recording/text cases example/integration-tested | held |
| Refined values cannot bypass admission through DTO/SQL | SIR5; foundation boundary-hardening | unknown/extra fields, byte limits, direct constructors; re-admitted row/receipt plus digest equality | held |
| Canonical content vs complete revision identity | SIR7; task stable/revision/digest distinction | independent Python literal c1/r1 oracle; NFC/LF and rights/provenance sensitivity example/differentially-tested | held |
| Current access/retries/concurrency are authoritative | AGENTS correctness; Catalog permissions | actual Router/app-role PostgreSQL; original receipt after new head; revoked membership; two waiting writes | held, integration-tested |
| Source/Script IR exact pinning preserves existing ownership and bytes | SIR9/10; ADR0018 historical compatibility | ingestion/editorial entrypoints; wrong checksum/export/owner rejected, old source/export unchanged | held, integration-tested |
| Atomic append; SQL immutability and load integrity | foundation persistence/immutability | failure after content insert rolls back head/receipt/history/epoch; admin UPDATE/DELETE/TRUNCATE refused; corrupt row →500 | held, fault-injected/integration-tested |
| Additive migration and restart recovery | AGENTS; SIR8 | old migration/export checksums retained; server kill + real PostgreSQL restart + pinned/head/original retry equality | held, integration-tested |
| Studio-only routes with JSON negotiation and private response policy | ADR0015; API surface rules | real Router isolation before SQL; actual HTTP/PG auth/body/Origin/negotiation tests | held, example/integration-tested |
| Local gates and evidence are fresh | local handoff policy; task request | Rust1.87 suites and changed-only preflight PASS; no selected skip; remote CI not checked | held |

Coverage: complete-for-declared-scope for added Catalog domain/application/SQL/HTTP, raw shared
DTOs/schema/fixtures, additive migration registry, PostgreSQL runner extension and affected
authority/contract/handoff docs. Existing consumers were traced through compatibility paths;
Studio compiled through WASM preflight. UI runtime, Theatre runtime, Android/iOS, providers,
object storage, briefs, uploaded audio, QC and publication were not affected or accepted.

## Resolved findings and disproved candidates

- **Confirmed defect / security:** an internally tagged unit `recording` variant could discard
  extra fields despite `deny_unknown_fields`. The literal recording-plus-script rejection
  fixture failed first. An empty struct variant now rejects that same payload; schema/DTO and
  HTTP admission evidence pass.
- **Required evidence gap / rule:** initial SQL file placement and HTTP direct domain imports
  violated the repository boundary check. SQL moved under `catalog/postgres/`; HTTP delegates
  pure construction through the existing Catalog wire facade. The checker was unchanged and
  now passes, with no new dependency edge or abstraction.
- **Confirmed defect / compatibility:** optional DBSP/rkyv trait implementations made
  `Option<&str> != Some(&String)` ambiguous in the new script-ref wire admission. Explicit
  `value.id.as_str()` fixes that build; preflight's DBSP clippy/tests pass on actual code.
- **Disproved candidate:** a suspected direct-constructor uppercase ScriptRef bypass was
  checked against the existing `platform::ids::canonical_uuid`; it already refuses alternate
  spellings. No redundant domain barrier or artificial test was retained.
- **Pre-existing execution issue:** native xtask loopback tests need unsandboxed local sockets.
  Its parallel port-ownership test also raced in this environment. Authorized serial full-suite
  execution passes; no assertion, tool prerequisite or protection was relaxed.

## Residual risk

This is a same-session review, not an independently staffed review. The global Catalog clock
limits concurrent reads/writes and invalidates unrelated list cursors; the warm 100-revision
baseline cannot establish production SLOs. Compatibility source/script links can diverge from
managed content, so the next brief must pin ContentRevision and revalidate underlying bytes.
Rights/provenance are declarations, not clearance; Work access does not grant separate source/
script endpoint access. Intake0010 integration and Production Brief/producer permissions remain
with their subsequent owners. No broad issue is closed and no audio/publishing gate is claimed.
