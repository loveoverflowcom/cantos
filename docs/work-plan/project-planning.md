# GitHub planning and execution

[Project](https://github.com/users/loveoverflowcom/projects/6) ·
[Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) ·
[Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2) ·
[Issues](https://github.com/loveoverflowcom/cantos/issues)

GitHub owns assignments, completion and review state. [Current priorities](current-priorities.md)
owns the recommended next boundaries. The [12 October delivery chain](../archive/superseded-plans/project-planning-2026-10-12.md)
is historical; do not restart it at completed #66.

## Dependency chain

| Existing capability | Next acceptance / consumer |
| --- | --- |
| Catalog → Content → immutable Brief | Reuse the implemented contracts and Studio consumers; #6/#8 track remaining instruction/UI scope, not a new Catalog |
| Scoped private Audio Lifecycle and filesystem/S3 adapters | #7/#9/#10: real integrity, authorization, review and private-provider acceptance |
| Publication core and immutable release pointers | #11/#170: accepted delivery policy, configured real storage/Range/expiry/withdrawal, retained-release safety |
| Discovery and separate Library/Progress backend | #172: public-state/pagination gaps; #13/#14 real Web/native audio; #15/#16/#17 background/offline/sync |
| Kanidm identity adapters and default-disabled Firebase enrollment | #174/#191/#206: accept real signup/login under [ADR 0036](../decisions/0036-firebase-enrollment-proof.md); reuse the implemented mailbox-proof/one-shot-setup flow, with privilege, action-ingress and live origin/session gates still separate |
| Forge local artifact/register/validate/export foundation | #203: shared local/Functions producer execution with private Drive drafts, then explicit API admission |
| Manual Hosting and demo distribution tooling | #199/#184: separately authorized live-origin or tester acceptance |

Public-cover (#157/#171), P1 metadata/transcripts (#177/#178), optional membership (#175/#176),
UI previews and behavior catalog work retain their independent owners. See the queue rather
than adding another dependency/status table per session.

## Phase exit and review evidence

- **Catalog:** plain novel chapters and unscripted podcast episodes use existing Catalog/Content
  APIs without mandatory Script IR/casting. #66 is completed; remaining Studio QA is tracked by
  #131/#179/#180 and public client states by #172.
- **Brief/intake:** exact immutable content/brief/profile binding, separate scoped credentials,
  verified private bytes, replay/conflict and stale/revoked/wrong-owner behavior.
- **Review/publication:** rights and named listening remain distinct from measured technical
  admission; failed replacement preserves the old release. Private storage success is no public
  publication witness.
- **Theatre:** real authorized releases, seek/expiry/withdrawal, native lifecycle and durable
  progress/offline behavior on the named clients. A mock player or empty discovery page is not
  listening acceptance.
- **Identity:** authorized mailbox proof, safe setup delivery and genuine OIDC/API session
  evidence under reviewed provider/issuer authority and origins; local fixtures and a registration
  receipt never establish authenticated access. Keep unaccepted activation disabled.
- **Forge:** local/cloud parity, authorized private draft I/O, recorded billable attempts and
  uncertain-outcome recovery, followed by explicit admission; generation never publishes.

## Working convention

Branch from and target `develop`. Read the relevant source and decision before specifying a
route, schema or tool. Link `Part of #...` for partial delivery; close an issue only for its
actual completed scope. A proposal or merged core does not satisfy the phase exit above.

At handoff, record attributable commands and results in the PR under the existing verification
cadence. Keep provider/device/SQL/runtime gaps explicit. Do not copy test transcripts, SHA logs,
worktree coordination or obsolete session permissions into the active queue.
