---
name: cantos-publication
description: >-
  Rule owner for implementing and verifying Cantos rights, provenance, approvals and publication:
  typed fail-closed rights records, immutable approvals bound to exact revisions and checksums, a
  pure publication gate that returns every blocker, a sealed publishable-release witness, a
  release state machine, staged upload under immutable keys, an atomic active-release pointer
  with outbox, private/publication storage separation, signed URLs and CDN delivery. Use for
  rights records, QC or approval gates, "Publish episode", release manifests, S3 or MinIO uploads,
  CDN, cache purge, republish, rollback, retraction or delivery reconciliation work.
---

# Cantos publication

This skill is a **behavioral contract and router** for the path from an accepted render to a
listener-visible release. It composes [`cantos-engineering`](../cantos-engineering/SKILL.md):
the required order, the [evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
and the [completion report](../cantos-engineering/SKILL.md#9-completion-report) apply unchanged.
This skill adds the publication-specific method and never redefines them.

## Who owns what

| Truth | Owner |
|---|---|
| Which rights are recorded and when unknown or expired rights block | [business rules § Rights and access](../../../docs/product/business-rules.md#rights-and-access) |
| Approval scope, invalidation triggers, release completeness, republish and retraction | [business rules § Review and publication](../../../docs/product/business-rules.md#review-and-publication) |
| Upload → verify → recheck → commit → activate order | [pipeline § Upload and publication boundary](../../../docs/product/production-pipeline.md#upload-and-publication-boundary) |
| Storage separation, asset readiness, pointer atomicity, cache purge status | [architecture § Data and delivery](../../../docs/architecture/overview.md#data-and-delivery) |
| Phase scope, acceptance and review boundary | [work plan 030](../../../docs/work-plan/030-mix-review-and-publish.md) |
| Human release records | [publication checklist](../../../templates/publication-checklist.md), [production run](../../../templates/production-run.md) |
| Image admission, format transition, immutable rebind and launcher boundary | [runtime image policy](../../../docs/design/runtime-images.md) |
| **How** to model, decide, commit, deliver and verify those rules | this skill |
| Artifacts, fingerprints, QC records, durable jobs, leases, outbox mechanics | [`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md) |
| Script revisions and canonical digests | [`cantos-script-ir`](../cantos-script-ir/SKILL.md) |
| Listener API, playback, downloads that consume releases | [`cantos-listening`](../cantos-listening/SKILL.md) |

When this skill and a document disagree, the document wins for product behavior: report the
conflict and fix the stale side when in scope.

## Repository reality

[Publication v1](../../../services/cantos-server/contracts/studio/publication/v1/README.md) and the Media publication
domain/application/migrations implement the development core: clearance, exact approvals,
readiness, immutable releases, atomic active pointers and withdrawal. Real public delivery
and provider acceptance remain separately gated. Names in technique examples are proposals
unless the actual current code/contract implements them; discover those owners first. Storage/CDN vendor, signed
versus public delivery, retention, isolation strategy and the integration environment need an
[ADR](../../../templates/adr.md) before they are treated as settled. Libraries (sqlx, proptest,
trybuild, cargo-mutants, a failpoint crate, MinIO) are candidates that need a recorded decision
and a pinned, verified version.

## The publication spine

```text
shell   read: release candidate · rights ledger · QC results · approvals · staged-object and
        delivery-path observations · one clock instant · the actor's permissions
          ↓ plain typed facts (no ports, no async)
core    evaluate_rights · approval_is_current · verify_staged · assemble_manifest
          ↓
core    decide_publication(&facts) → Ok(PublicationDecision { PublishableRelease, … })
                                    | Err(every PublicationBlocker, sorted)
          ↓ witness + PublishPermit
shell   ONE PostgreSQL transaction: lock episode → re-read facts → re-decide → insert ready
        release → move active pointer if it still equals the expected one → append transitions
        → insert outbox rows → COMMIT
          ↓ after commit, at-least-once
shell   idempotent consumers: discovery index · catalog cache purge · notifications
```

A readiness preview runs the same core outside the transaction to explain blockers. Only the
decision computed from facts read **inside** the commit transaction may publish. Bytes are
uploaded and verified before that transaction; nothing after the commit can make an incomplete
release listener-visible.

## Rules this skill owns

These rows prescribe the working method, not the implementation status of every example.
Check the current contract and code for the implemented subset described above; delivery and
provider acceptance remain separate. Full rule cards with good example, counterexample,
oracle and exception live in the linked reference.

| # | Rule | Failure it prevents | Oracle | Reference |
|---|---|---|---|---|
| P1 | Rights fail closed: missing, unrecorded, pending, revoked, expired or out-of-scope rights evaluate to a typed blocker; no default grant exists | publishing material nobody cleared | one table row per `RightsBlocker` | [rights](references/rights-and-provenance.md) |
| P2 | One evaluator checks rights before paid generation and again inside the publication commit | revocation between preview and commit slips through (TOCTOU) | interleaved revoke-then-commit test on real PostgreSQL | [rights](references/rights-and-provenance.md#4-two-checks-one-evaluator-the-second-inside-the-commit) |
| P3 | Generated output inherits every input and producer restriction; scope never broadens | a render "launders" a restricted voice or track | inheritance laws as properties | [rights](references/rights-and-provenance.md#5-provenance-and-restriction-inheritance) |
| P4 | An approval is an immutable record binding actor, permission scope and the exact candidate identity; staleness is computed, never stored | an edit path forgets to flip `is_stale` | property: any change to a bound input makes it stale | [approvals](references/approvals-and-gates.md) |
| P5 | One pure gate returns **all** blockers with typed reasons; the UI renders them, the server re-runs the gate | first-error-only UX; a disabled button mistaken for enforcement | all-defects-at-once row; direct POST with blockers | [approvals](references/approvals-and-gates.md#3-one-pure-gate-returns-every-blocker) |
| P6 | Only the gate constructs `PublishableRelease`; the commit consumes it and a `PublishPermit` | a handler commits without the gate or without authorization | compile-fail test outside the module | [approvals](references/approvals-and-gates.md#4-the-witness-and-the-permit) |
| P7 | Bytes before pointer: immutable keys, checksum-verified upload, delivery-path verification, then release + pointer + outbox in one transaction | a listener receives a manifest with a missing object | fault injection asserted through the listener API | [staged](references/staged-publication.md) |
| P8 | The release lifecycle is a closed state machine; rollback is a new release over an earlier manifest, never a backward edge | two active releases; resurrected retracted audio | exhaustive state × event table | [staged](references/staged-publication.md#2-the-release-lifecycle-is-a-closed-state-machine) |
| P9 | Private and publication storage are separated by policy; publishing copies approved renditions and never widens access to production storage | drafts, voice samples or previews become fetchable | anonymous GET against private keys on a real store | [storage](references/storage-and-delivery.md) |
| P10 | Signed URLs and credentials are secrets: short-lived, never logged, persisted, placed in a manifest or used as identity | leaked access; downloads keyed to expiring URLs | sentinel scan of captured logs and serialized records | [storage](references/storage-and-delivery.md#5-signed-urls-are-secrets-with-a-lifetime) |
| P11 | The PostgreSQL pointer is publication truth; cache purge and reconciliation repair delivery, they never decide it | publish "fails" because a purge failed, or succeeds because one ran | purge adapter forced to fail; reconcile finds seeded drift | [staged](references/staged-publication.md#8-reconciliation) |

## Working order for a publication change

Apply the foundation's [required order](../cantos-engineering/SKILL.md#the-required-order) with
these publication-specific steps:

1. **Read** the owning doc sections above, the 030 acceptance criteria, the linked issue and the
   code. Note which [business-rule invariants](../../../docs/product/business-rules.md#product-invariants-to-verify)
   the change touches; "QC approval predates a new render" and "Upload is incomplete" are always
   in scope for gate or delivery work.
2. **Ledger.** Add a row per claim: the blocker or transition affected, its failure mode, the
   oracle and the evidence level you will honestly reach.
3. **Facts first.** Define the plain fact structs the core needs before touching a handler. If a
   decision reads a port, the core is not extracted yet.
4. **Name crash points** for every new effect (upload, copy, verify, commit, dispatch, consume)
   and decide what the listener API must return after a crash at each one.
5. **Witnesses at commit.** The commit function's signature takes the evidence it needs
   (`PublishableRelease`, `PublishPermit`); never a `bool` or a status string.
6. **Implement the shell** behind narrow ports (`ObjectStore`, `DeliveryProbe`, `CachePurger`)
   named for the capability, not the vendor
   ([decoupling](../cantos-engineering/references/decoupling.md)).
7. **Test** per the table below, then report with the publication fields.

## Verification: what each claim needs

| Claim | Cheapest adequate oracle | Honest label |
|---|---|---|
| each blocker is reachable with an exact variant | baseline-publishable fixture with exactly one defect per row | `example-tested` |
| the gate never short-circuits | one row with every defect, asserting the full sorted list | `example-tested` |
| the gate equals an independent truth table over all gate-dimension combinations | bounded exhaustive loop against a hand-written oracle | `differentially-tested` (state the domain and bound) |
| any change to a bound input makes an approval stale | generated candidate × generated mutation | `property-tested` |
| illegal lifecycle transitions are rejected | exhaustive state × event table, no wildcard arm | `example-tested`; `statically-checked` for the exhaustive match |
| the witness cannot be forged | compile-fail test constructing it outside its module | `type-enforced` |
| crash between commit and outbox dispatch, duplicate delivery, interrupted upload | named failpoints and restart ([fault injection](../cantos-production-pipeline/references/fault-injection-testing.md)) | `fault-injected` |
| upload, conditional write, checksum and private access on a real S3-compatible store | local MinIO or the chosen vendor's emulator, really running | `integration-tested` — never for an in-memory fake |
| byte-range seek, CORS and caching through the CDN | probe through the actual delivery host | `integration-tested` only on a real CDN; otherwise a stated gap |
| no listener-visible incomplete release | listener API response plus a delivery GET of every asset, under each fault | `fault-injected` + `integration-tested` when both are real |
| assertions kill defects in the gate and staleness code | mutation run on the pure modules | `mutation-tested` |

The oracle for listener visibility is the **listener API and the delivery path**, never a
`SELECT` on the release table. Mocked and live evidence are reported separately, as the
[work plan](../../../docs/work-plan/README.md#shared-completion-evidence) requires.

## Smells that fail review

| Smell | Why it is wrong | Do instead |
|---|---|---|
| `rights_status: Option<String>`; `None` treated as fine | unknown silently passes | closed enum with `Unrecorded`, evaluated to a blocker |
| `approval.is_stale = true` set by edit handlers | the next new edit path forgets it | compute with `approval_is_current` from immutable values |
| approvals keyed by `episode_id` alone | a new render inherits an old approval | bind the full candidate identity |
| gate checked in the handler, commit in a later transaction | revocation or edit lands in between | re-read and re-decide inside the commit transaction |
| `?` on the first failing check | UI shows one reason per click | collect every blocker, sort, return all |
| hiding "Publish episode" is the permission check | any client can POST | backend authorization producing `PublishPermit` |
| `publish(latest_candidate)` | publishes something nobody reviewed | the request names candidate ID, manifest digest and expected active release |
| republish overwrites `episode-01.m4a` | in-progress streams and offline checksums break | new immutable keys per content |
| setting public-read on the production bucket | every draft becomes reachable | copy approved renditions to the publication class |
| asset marked ready when PUT returned 200 | truncated or wrong bytes are published | ready only after size + checksum verification |
| verifying with an origin `HEAD` only | CDN path, ranges and CORS stay unproven | probe the intended delivery path |
| presigned URL in a tracing span, row or manifest | the secret leaks and expires | `SignedUrl` newtype with redacted `Debug`, minted per request |
| deleting the prior release's objects at republish | in-progress playback and downloads break | retention decision, two-phase garbage collection |
| `UPDATE release SET manifest = …` | audit and approvals lose meaning | a new release; manifests are written once |

## Completion report

Use the [foundation report](../cantos-engineering/SKILL.md#9-completion-report) with every field,
then add:

```text
Release ID:               <id, or "none: no release produced">
Manifest digest:          <algorithm:hex over canonical bytes, manifest schema version>
Gates evaluated:          rights · QC · approvals · assets · attribution → pass | blocker codes
Storage path classes:     <private-staging / publication + key pattern; never a signed URL>
Prior release / rollback: <active before → after; evidence the prior stayed playable via the
                           listener API and a delivery GET, or "not exercised">
Delivery verification:    <host class probed, range/CORS/content-type results, or the gap>
Fault points exercised:   <named crash points and outcomes, or "none">
```

"Publication verified" is not an evidence level. Name the label from the vocabulary for each
line, and keep MinIO, a local proxy and a real CDN distinct.

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| rights records, asset classes, fail-closed evaluation, commit-time recheck, provenance, restriction inheritance, creator scoping, secrets in records | [`rights-and-provenance.md`](references/rights-and-provenance.md) |
| approval records, candidate identity, staleness, the publication gate, blockers, witness, permissions, readiness UI contract | [`approvals-and-gates.md`](references/approvals-and-gates.md) |
| release lifecycle, staged protocol, manifest, commit transaction, outbox, republish, rollback, retraction, reconciliation, crash points | [`staged-publication.md`](references/staged-publication.md) |
| buckets and key classes, asset readiness, uploads, delivery-path verification, signed URLs, CDN ranges and caching, retention, MinIO | [`storage-and-delivery.md`](references/storage-and-delivery.md) |

## Compose with

- [`cantos-engineering`](../cantos-engineering/SKILL.md) for the order and report, with
  [`types-as-proofs.md`](../cantos-engineering/references/types-as-proofs.md) (witnesses),
  [`functional-core.md`](../cantos-engineering/references/functional-core.md) (transactional
  rejection), [`immutability.md`](../cantos-engineering/references/immutability.md),
  [`boundary-hardening.md`](../cantos-engineering/references/boundary-hardening.md) (manifest and
  rights serde), [`persistence.md`](../cantos-engineering/references/persistence.md) and
  [`http-api-boundary.md`](../cantos-engineering/references/http-api-boundary.md).
- [`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md) for accepted artifacts,
  QC records, fingerprints, leases and the outbox mechanism this skill's events ride on.
- [`cantos-script-ir`](../cantos-script-ir/SKILL.md) for the script revision and digest an
  approval binds.
- [`cantos-listening`](../cantos-listening/SKILL.md), which consumes the listener projection of a
  release and the visibility predicates defined here.
- [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) and
  [`cantos-ui-design`](../cantos-ui-design/SKILL.md) for the Studio publish surface that renders
  blockers as "disabled with reason".
- Reviewing a publication change: [`cantos-code-review`](../cantos-code-review/SKILL.md), whose
  security lens covers fail-closed gates, storage exposure and secrets.
