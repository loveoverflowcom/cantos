# Audio Publication v1

Implemented Studio metadata core over [Audio Lifecycle v1](../../audio-lifecycle/v1/README.md),
[Content Identity](../../content/v1/README.md) and [Production Brief v1](../../briefs/v1/README.md).
[Schema](publication.schema.json), Server-owned DTOs in `src/contract/studio/publication.rs`,
and synthetic [request fixtures](fixtures/) own JSON transport.
[Evidence and limits](../../../../../../docs/evidence/publication-core.md) separate core verification
from its original unrun PG/delivery acceptance. Native persistence and HTTP test roots are
now recorded in the [replacement ledger](../../../../../../docs/architecture/server-turso/remaining-pg-retirement.md). This does not close #9/#10/#11.

## Route inventory

All paths begin `/api/v1/studio`, require the current owner Studio cookie and inherit
JSON-only 32 KiB request bounds, same-origin mutation checks and `Cache-Control: no-store`.
There are no deprecated, Theatre or Admin aliases.

| Method / relative path | Request | Success |
| --- | --- | --- |
| GET `/content-units/{unit}/release-candidates/{candidate}/readiness` | none |200 `PublicationReadinessResponse` |
| POST same candidate path `/distribution-rights` | `RecordDistributionRightsRequest` |201 `DistributionClearanceDocument` |
| POST same candidate path `/publication-approvals` | `RecordPublicationApprovalRequest` |201 `PublicationApprovalDocument` |
| POST same candidate path `/publish` | `PublishAudioReleaseRequest` |201 `ActiveAudioReleaseResponse` |
| GET `/content-units/{unit}/publication/active` | none |200 `ActiveAudioReleaseResponse` |
| POST `/content-units/{unit}/publication/withdraw` | `WithdrawAudioReleaseRequest` |200 `AudioReleaseWithdrawalResponse` |

A readiness response is advisory. It returns every accumulated typed blocker, exact candidate
digest, current rights/approval records and pointer version/ID. `passed=true`, producer QC
assertions and unknown JSON fields cannot authorize publication. Mutable facts are checked
again under the Catalog mutation fence at publish time. Publish rejects all blockers with
existing `production_blocked`/409 and `issues[].path="publication"`, where `rule` is the
exhaustively mapped snake-case blocker. Pointer disagreement uses `stale_revision`/409;
dependency errors use503; corrupt persisted records use500. Unauthorized subjects are concealed
through the existing owning-context error mapping.

## Distribution authority and exact approval

A distribution clearance is an append-only, authenticated current-owner decision for one
exact candidate digest. `cleared` requires all claimed rights and supporting evidence references
to exist in that owner's existing rights evidence registry. Coverage includes Work, Content
and rendition rights references. The decision has a positive sequence, DB-recorded millisecond
clock, exclusive `valid_until`, named actor and notes. `revoked` is a new decision, never an
update. Registered references and owner attestation are not proof of license legality or terms.
No producer, reference string, private object or prior QC decision becomes a distribution authority.

Publication approval is a separate append-only decision pinning candidate and clearance digests.
A new `approved` decision checks current candidate/owner/brief/metadata/upload/profile/integrity,
latest QC/rights/editorial reviews and valid complete distribution clearance. It excludes only
approval and delivery blockers, allowing an owner to approve before delivery exists. `withdrawn`
remains available for a correctly bound candidate/clearance. Sampling time after byte verification
prevents a slow verification from authorizing an already-expired clearance.

Current rights/approval are latest by server sequence, not caller timestamps. Stale approval,
future evidence, expired/revoked clearance, changed candidate reviews or wrong Work audio fail
closed. Existing named lifecycle reviews retain their original meaning; no listening quality or
legal verification is inferred from an Opus envelope.

## Immutable release and replacement

The sealed domain publication witness can be created only after pure readiness succeeds. The
release snapshots exact Work/ContentUnit IDs and metadata revisions/titles; rendition ID/digest,
SHA-256/length/profile/duration; admitted Content language and full revision; exact Brief
revision; three accepted named review records; distribution clearance and approval; and an
opaque, publication-class delivery object identity. Manifest and release have domain-separated
`rm-r1` and `rl-r1` SHA-256 fingerprints over canonical versioned JSON. Restoration admits
embedded evidence and checks real candidate/rendition/Content bindings, not hashes alone.

`ReleaseManifestDocument` is an owner/audit DTO, not a public Theatre projection. It includes
review/evidence details. No staging key, private pathname, bearer or public/signed URL is accepted
or returned. The separate [Discovery contract](../../../theatre/discovery/v1/README.md)
projects public metadata and applies current rights/delivery checks through Media; the owner
audit DTO is never returned to an anonymous listener.

Publish requires both expected pointer version and expected active release ID. A single
Turso writer unit writes release, guarded active pointer, durable audit event and operation
receipt under the owning Work/clock fence. Pointer identity is fixed, version increments are
forward-only and active pointers reference the same unit/owner. Settled rows reject update,
delete and replacement/upsert attempts. Uncommitted failure leaves no new release, pointer/event or receipt.

Republish creates a new UUID and immutable record with `supersedes`; it never edits the old
manifest. The explicit policy `retain_until_rights_expiry_or_withdrawal` retains old snapshots
and bytes without an implicit fallback switch. Later QC/content/metadata changes can block a
replacement without invalidating a previously approved release. Current clearance/approval
must still match the old snapshot; revocation, expiry, changed rights decision/approval or an
explicit release tombstone stop its resolution. Renewed rights therefore require explicit
new approval/publication. No request deletes shared bytes. Reference-aware garbage collection
and delivery retention enforcement remain acceptance dependencies.

Withdrawal appends a tombstone and operation/audit receipt. Withdrawing the active release
clears it and increments the pointer; withdrawing a superseded release leaves the newer pointer
unchanged. The receipt reports `withdrawn_release_id`, not a false empty active lookup. No old
release is resurrected automatically. Restore service through a new valid publish operation.

UUID operation IDs are scoped to the authenticated actor and operation. Identical replay returns
the original durable receipt, including after restart or later withdrawal; it does not reactivate
anything. Reusing an ID with a different subject/request fails. Clearances canonicalize rights
and evidence sets before comparing retry input. Concurrent publishers are serialized by the
serialized writer and checked pointer. Native race/restart tests are mapped in the replacement ledger;
production contention and deployed delivery remain separate acceptance.

## Storage and migration boundary

Media-owned `PublicationService` composes metadata authority and
`PublicationDelivery::observe(PublicationObject)` containing rendition ID,
refined SHA-256, length and explicit profile. The result must attest a distinct publication-class
object, exact integrity and a finite verification window valid at the sampled clock. It cannot return a staging
object reference or URL. `UnavailableDelivery` is the production default: no release can activate
until a trusted host explicitly provides a real verified delivery adapter via
`router_with_publication_delivery`. Generic persistence plumbing owns neither the port nor a
business service. The named integration
fake is test-only and establishes no public storage/CDN capability.

Private byte verification reuses current `AudioStorage`; publish rechecks it under the final
metadata fence. Filesystem/remote object state and SQL cannot be one transaction. A real immutable
delivery/retention implementation must establish stable object existence through commit and
resolution. The private target registry is not a public promotion API. Configured native filesystem
publication/origin adapters have a separate [Server boundary](../../../../README.md#origin-public-media-storage).

Native migrations 14 and 23 own releases, pointers, events and operation receipts. Historical
PG registry/grant details do not configure native serving. A configured delivery adapter,
actual byte integrity and independent production delivery/retention acceptance remain required.
