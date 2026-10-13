# ADR 0022: Cantos Forge is a producer-side CLI outside the API runtime

- Status: proposed
- Date: 2026-10-13
- Decision owner: Cantos project
- Related issue or PR: [#82 — Cantos Forge vision](https://github.com/loveoverflowcom/cantos/issues/82)
- Supersedes / superseded by: N/A. Narrows the wording of [ADR 0018](0018-external-audio-production.md) without changing its API rules.

## Context

[ADR 0018](0018-external-audio-production.md) removed TTS, provider workers, mixing and encoding
from `cantos-api` and made external producers responsible for audio. Producers and AI hosts now
lack one place that resolves instructions, keeps local drafts and artifact revisions, records
evaluation evidence and prepares a handoff. Issue #82 records the owner's request for such an
application under `apps/`, named **Cantos Forge**, covering stories, scripts and audio.

Known facts at `develop` 64979f3: `apps/` holds only Studio and Theatre; the API implements
Production Brief read grants and the private Audio Lifecycle (owner-created submission, exact
upload bearer, one binary `PUT` that admits). No route accepts a story or script from a
producer. Assumption: Forge's first users are AI hosts calling a CLI, not people in a GUI.

"External" in ADR 0018 means outside the API's runtime and trust boundary. It does not require
the producer tooling to live outside this repository or product.

## Decision

1. `apps/cantos-forge` is a Rust CLI (one crate, package and binary `cantos-forge`) that runs in
   the producer's environment. It is part of the Cantos product family, not of the API runtime.
2. **The API, Studio and Theatre still never execute generation**, call a provider, spawn Forge
   or depend on Forge running. ADR 0018's API rules are unchanged; only Forge may orchestrate
   producer-side tools, and only in a later, separately reviewed slice.
3. Dependencies point inward: Forge uses `packages/*` and pinned workspace dependencies, never
   `services/` code, another app or PostgreSQL. A future handoff talks to the API over HTTP
   using the producer credentials the API already defines; it never uses a Studio cookie.
4. Forge has a functional core (`domain/`) with no I/O, clock, randomness, process, CLI or serde,
   and an imperative shell. Local identities are distinct from any server identity; server IDs
   become separate types only when a handoff receives them.
5. Execution, evaluation, handoff and publication are independent. Forge records evidence and
   never turns an AI assessment, a local check or an upload into Studio approval or publication.
6. `cargo xtask` builds Forge and owns repository verification; Forge owns production workflows.
   xtask never runs Forge or a provider inside build, test, preflight or CI.
7. The first increment is the crate, its wiring and the verified pure core plus a truthful
   `capabilities` command. Workflow commands, adapters and handoff follow #82's sequence.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Forge as a CLI app in this workspace | shares pinned toolchain, contracts and gates; one place for producer rules | the workspace grows a native binary consumers must not confuse with the API | selected |
| Keep producers fully outside Cantos (skills only) | nothing new to maintain | every AI host re-implements revisions, evidence and handoff differently; no typed safety | rejected by the owner's request in #82 |
| Add producer workflows to `cantos-api` | one deployable | reverses ADR 0018; credentials and provider cost enter the server | rejected |
| MCP server or daemon first | richer host integration | long-lived process and transport before any workflow exists | deferred; an adapter over the same core when a host needs it |
| Separate domain crate for Forge now | compile-time isolation | no second consumer today ([ADR 0001](0001-modular-monolith.md)) | rejected until a second consumer exists |

## Consequences

- Product and user impact: none for listeners or creators. AI hosts get a discoverable
  `cantos-forge capabilities --json` that truthfully reports only `capabilities` as implemented.
- Compatibility and migrations: N/A. No schema, route, contract or persisted data changes.
- Operations, storage, and production cost: a seventh workspace member built by `verify rust`
  and CI; no provider, network call, credential or runtime service.
- Implementation and recovery requirements: later slices must keep local and server state
  apart, record attempts before billable effects, reconcile ambiguous outcomes before retry,
  constrain paths, processes and output sizes, and keep secrets out of bundles and logs (#82).
- Deferred questions: workspace storage format; instruction-bundle resolution from
  `.agents/skills/`; the shared pure Script IR validator package; the first provider adapter;
  self-service producer credentials (needs a separate API/permission design).

## Validation and revisit conditions

Validated by `cargo test -p cantos-forge` (domain examples, properties, a compile-fail doctest
and lexical purity checks) and by the xtask test that every workspace dependency points inward.
Revisit if a second consumer needs the Forge core (split a package), if an API route starts to
depend on Forge, or if a host requires a long-running transport.
