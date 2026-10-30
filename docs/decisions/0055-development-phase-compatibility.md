# ADR 0055: Development-phase compatibility — synchronized breaking changes

- Status: **accepted (owner, 30 October 2026)**, stated during the
  [#647](https://github.com/loveoverflowcom/cantos/issues/647) decision review; repository-wide
  working policy with no runtime change by itself
- Date: 2026-10-30
- Decision owner: Cantos product owner
- Related issue or PR: epic [#634](https://github.com/loveoverflowcom/cantos/issues/634), S01
  [#647](https://github.com/loveoverflowcom/cantos/issues/647); the epic-scoped wording is
  [ADR 0054 "Owner direction for this epic"](0054-publication-generation-and-access.md#owner-direction-for-this-epic-breaking-changes-during-development)
- Supersedes / superseded by: replaces the general requirements to keep older clients or older
  data working that skills, templates and the contract index stated (listed under Consequences).
  Superseded when the owner ends the development phase (see revisit conditions)

## Context

Cantos is in development. Its current clients are built from this repository and ship with it:
Theatre Web, Theatre CMP Android/iOS and Studio Web (Leptos). Several owners nevertheless asked
for compatibility as if released clients and existing data had to keep working: a support window
for the previous listener API major because installed mobile clients lag releases, removal or
retyping only with a migration window, expand → backfill → switch readers → contract migration
sequences, PR questions about old code running on a new schema, and Script IR converters with
permanently readable old versions. Contract decisions added presentation levels to keep older
strict readers unchanged. Those lanes cost code, fixtures and review in every change.

During the #647 review the owner directed breaking changes for epic #634 and asked that the
direction be recorded once for the whole repository. This record is that policy; ADR 0054 keeps
the epic's own application of it.

## Decision

While Cantos is in development:

1. **Synchronized breaking changes.** A breaking wire, API or contract change is allowed when
   every current client — Theatre Web, Theatre CMP Android/iOS and Studio Web — and every other
   in-repository consumer of that contract is updated and fully tested in the same synchronized
   delivery: one pull request, or one integration merge. `develop` never carries a Server shape
   that a current client cannot read. Prefer replacing the current representation in place.
   Do not keep code, compatibility aliases, presentation levels, version lanes, strict-reader
   fixtures or dual-read lanes only for previously released APKs or builds; remove the replaced
   shape, its DTOs and its fixtures in the same delivery.
2. **Data follows the code.** Existing data needs no compatibility either: data follows the new
   code and wire. No conversion, backfill command or old-row reader is required, and a new schema
   registry version may drop or replace obsolete tables. Prefer the best code quality and
   architecture over preserving an old shape.
3. **Unchanged invariants.** The direction relaxes none of these:
   - fail-closed authorization and no disclosure of hidden or restricted data. Code rejects, or
     ignores by an explicit rule, an old request, row or token it no longer understands; it never
     reinterprets one. A change that keeps a shape but changes its meaning or unit makes the old
     form fail;
   - revocation;
   - rollback that never drops a gate. An older binary refuses a newer database, so a rollback is
     a restore of a matching binary and data pair or a forward fix, never a binary swap that
     reopens a closed gate;
   - immutable settled records (revisions, approvals, manifests, receipts). A kept record is never
     rewritten in place; when a reader is dropped, the record is rejected with an actionable error;
   - rights and consent provenance for whatever is kept;
   - the append-only, checksummed schema registry: changes append versions and registered
     migration SQL is never edited;
   - production data conversion, migration and activation remain separate, explicitly authorized
     tasks.
4. **Scope.** A current client is one built from this repository. A contract with a consumer
   outside the repository (none today; the producer handoff is proposed) is not covered, and
   breaking it needs its own decision. Where a skill, template or contract README requires keeping
   an old shape, version, alias or fixture only for older clients or older data, this policy
   replaces that requirement; their facts about current shapes, limits and security rules stay
   authoritative.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Version lanes and a support window for released builds | Installed older APKs and cached Web builds keep working | Parallel DTOs, fixtures, presentation levels and dual-read code; review of shapes no current client uses | Rejected by the owner for the development phase |
| Additive-only evolution with migration windows and backfills | Safe for rolling deploys and field clients | Multi-PR sequences, conversion commands and dead fields | Rejected for development; reconsider when the phase ends |
| Record the direction only for epic #634 (ADR 0054) | Narrow change | Skills and templates keep demanding compatibility elsewhere; contradictory guidance | Rejected: the owner asked for a repository-wide record |
| Synchronized breaking change with unchanged invariants | One current representation; simpler code and review | Installed older builds can fail; schema rollback is restore-based | **Selected** |

## Consequences

- Product and user impact: after a breaking delivery an installed older APK or cached Web build
  may stop working. It receives errors, never another subject's data; testers install the
  current build.
- Compatibility and migrations: these owners now point here instead of requiring old-client or
  old-data compatibility: `AGENTS.md`, `CONTRIBUTING.md`, the pull request template, the
  [Server contract index](../../services/cantos-server/contracts/README.md), the engineering
  HTTP boundary, diff review and persistence references, the listener API reference, the
  work-item slicing and pull request references, Script IR schema versioning and routing case
  R34. Existing compatibility code, such as the deprecated unprefixed Studio `/api/v1/*` alias,
  legacy MessagePack and the Discovery presentation levels, is not removed by this record; each
  removal is its own synchronized delivery that applies this policy to that contract.
- Operations, storage and production cost: none by itself. Schema rollback is a restore;
  production data conversion and activation stay separate.
- Implementation and recovery requirements: a breaking pull request names every current consumer
  it updates and the tests that ran for each, removes the replaced shape and states how old forms
  fail closed. Registered SQL is never edited.
- Deferred questions: when the development phase ends, and the support window, versioning and
  data-migration policy for field clients and production data after it.

## Validation and revisit conditions

Each breaking pull request is reviewed for the complete consumer list with tests, fail-closed
handling of old requests, rows and tokens, and the unchanged invariants above. Revisit when the
owner ends the development phase — for example before a public store release, before a client
outside this repository consumes a contract, or when production data must survive a change. The
owner then records the replacement compatibility policy and supersedes this record.
