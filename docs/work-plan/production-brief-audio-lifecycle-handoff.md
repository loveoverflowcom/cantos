# Production Brief → AudioLifecycle handoff

Session2's bounded implementation is [Production Brief v1](../../packages/contracts/production-brief-v1.md).
It follows fresh `develop` PR70 and preserves [Content handoff](content-identity-production-brief-handoff.md).
Evidence, independent review and scope are in [the report](../evidence/production-brief.md).

## Available boundary

- Production's pure `domain::brief` has distinct Brief/Grant IDs, exact digest admission,
  guidance/profile/capability rules, immutable snapshots, freshness and producer-scope checks.
- Owner create/read and exact external bearer export copy admitted Content and resolve source/
  Script bytes only through owning context entrypoints. Narration/recording need no cast or
  Script IR. The wrapper never overrides Script IR effective line languages.
- Brief digest `pb-r1` covers full Content `cc-r1`, Work rights `pb-rights1`, explicit profile,
  complete copied instructions, guidance revisions and audit identity/time.
- `read_instructions` grants pin Work/unit/Brief ID/digest, expire within24hours, and retain
  append-only revocation. They are separate from Studio sessions and authorize no upload.
- Owner reads retain exact historical documents and report current freshness; producer reads
  block stale inputs, expiry/revocation, wrong scope, archived unit/work or inactive owner.
- Migration0013 adds Brief/head/grant/revocation storage without touching old SQL. Current
  registry:1–9,11,12,13/count12. Source/script exports and applied checksums stay intact.

## Session3 dependencies

PR69 is a separate unmerged draft at `86e2dde58f2028a777da27bfe007aa89db7f3708` when session2
inspected it. Do not infer accepted intake/runtime evidence from its existence. Fetch fresh
`develop` and its current PR state. Preserve0010/0011/0012/0013 SQL checksums. When integrating
0010, reconcile the registry to1–13/count13 and retain old-gap and already-applied migrations
tests. Neither a maximum migration number nor a count proves that a gap was applied.

Its intake currently binds a Studio script-owner session and exact Script revision. AudioLifecycle
must explicitly bridge that to Catalog Content/Brief identity, without rewriting intake or using
read grants as upload grants. The four profile IDs match, with no AAC/default; a copied profile
instruction does not prove received media meets it. Verify actual private object/codec facts.

## Required next review boundary

Bind immutable admitted renditions and provenance to exact ContentRevision UUID/`cc-r1`,
Brief UUID/version/`pb-r1`, profile and declared rights. Preserve append-only QC/editorial
decisions tied to the exact stored checksum. Current scope, rights and stale-input gates must
be checked again for review/publication; old read/export success grants no approval.

Separate upload, technical acceptance, editorial/rights approval and release state machines.
Keep the prior immutable release playable on any failure. Do not infer exactly-once external
production, content fidelity or listening acceptance from an upload receipt. No TTS/provider
adapter/worker/server mixer belongs in Cantos.

## Limits to preserve

Development identity and safe credential delivery still require production acceptance. The
global Catalog fence limits concurrency. Large Source receipts exceeding256KiB and legacy
text-only sources without extraction receipts intentionally block source export. Rights and
capabilities are declarations; no listening or audio measurement happened in this session.
Live Leptos/CMP API integration is the user's subsequent task; do not start it here. Keep #64,
#66 and wider roadmap issues open unless their full acceptance criteria are proven separately.
