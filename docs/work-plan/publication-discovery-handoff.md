# Publication core → Discovery/Playback → Library/Progress

Publication is the first independently reviewed slice in this sequence. Its
[Studio contract](../../packages/contracts/publication-v1.md) and [evidence](../evidence/publication-core.md)
are implemented, while PostgreSQL and real public delivery acceptance remain outstanding.
No listener, library or progress endpoint is introduced by this slice.

## Existing dependencies to consume

- Media owns private candidates, immutable rendition verification and publication/retention
  rules. Reuse those values; never accept producer `passed=true` or read staging files directly.
- `AudioRelease` admits exact candidate/rendition/Content snapshots. `retained_release_eligible`
  evaluates latest rights/approval, withdrawal and current expiry; `delivery_matches_manifest`
  checks object identity/integrity/window. Metadata/content/QC changes can reject a replacement
  while retaining a prior valid release; revoked/expired rights cannot retain public access.
- Media-owned `PublicationService` composes the existing Store and `PublicationDelivery`;
  the platform never owns this business port. `PublicationDelivery` is a narrow metadata consumer of trusted publication-object evidence.
  Default `UnavailableDelivery` keeps production activation blocked. Complete real delivery
  semantics after [private storage PR90](https://github.com/loveoverflowcom/cantos/pull/90)
  is actually merged (reported checkpoint `9306d22954b44aa9be390495aa0b2581c0738d22`); do not treat
  `PrivateStorage` or private target registration as public promotion.
- `active_audio_release` is a current-owner Studio use case, not a public access port.
  Add a media-owned current-eligibility resolver for the real Theatre consumer rather than
  impersonating an owner or exposing the Studio audit DTO. Define listener entitlement/access
  policy before public discovery, signed delivery, range playback or offline grants.
- PostgreSQL is publication authority. Pointer, release, operation and audit event commit
  atomically; CAS uses both pointer version and active release ID. Replays return receipts
  without reactivating withdrawn media.
- `audio_publication_events` is a durable atomic audit feed, not an outbox dispatcher or ordered
  CDC checkpoint. Its UUID/time/version are not global commit order. Discovery must define an
  actual snapshot/change frontier before using it for an incremental projection. Begin with
  authorized PostgreSQL reads; separately assess DBSP when measured aggregation earns it.
- Migration0016 is standalone;0015 is reserved for the private target registry. Reconcile the
  real merged registry by adding0015 in numeric order, preserving all checksums. No0015 table
  or adapter dependency is assumed here.

## Discovery/Playback acceptance

1. Fetch latest develop and inspect the actual storage merge/ports; no unpublished integration.
2. Establish listener/public metadata, current rights/withdrawal/owner-state decisions and
   title/language/profile filters. Keep audit evidence private and paging stable.
3. Implement real delivery existence, exact SHA/length verification, publication-class object
   lifetime/retention, bounded signed links and byte-range/seek behavior. Never route a staging
   key into public delivery. If the prerequisite is absent, return a typed unavailable result.
4. Execute migration/replay/app-role permissions and current compiled publication Router/PG
   tests. Resolve the denied disposable PostgreSQL startup through an authorized environment
   owner, without weakening private storage guards or bypassing denial.
5. Add publisher races, expiry during I/O, rights revocation after signing, missing delivery,
   reconnect/crash acknowledgement, public cache isolation and retained-release tests. The
   current fake proves composition only; core Kani proves pointer/clock rules only.
6. Inventory listener reads, set workload budgets and capture the real authorized PG baseline
   before choosing indexes/cache/DBSP. No measured publication performance is claimed yet.

## Library/Progress dependencies

Consume stable immutable release IDs and ContentUnit identity from the reviewed listener
contract. Define whether history/progress follows the unit or release; pin exact release for
offline grants and retained history. Withdrawal/expired rights remove playable access without
silently deleting user progress. Library saves, history, conditional progress, idempotency,
optimistic versions and multi-device sync are a later separately scoped PR. Nothing in this
core establishes those APIs or their authorization policy.

## Open acceptance owned by Publication/Storage

Real public storage/retention, production identity/session issuance and application grants;
legal distribution terms and named subjective listening quality; actual SQL/concurrency/rollback
and process/cluster crash tests; private-input baseline failures; PostgreSQL plans/performance;
Studio Web integration and browser/accessibility acceptance. Keep #9/#10/#11 and wider product
issues open. Publication is CORE_VERIFIED / IO_ACCEPTANCE_PENDING, not production-ready.
