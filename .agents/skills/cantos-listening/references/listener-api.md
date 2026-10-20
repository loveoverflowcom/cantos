# Listener API

> **Scope.** Design, version, implement and verify the HTTP/JSON contracts that Theatre Web and
> the CMP app consume: catalog, work and episode views, immutable publication manifests,
> playback authorization, progress, bookmarks and download authorization. Covers identifiers,
> compatibility, the schema as source of truth and generated types, operation IDs and
> idempotency, error codes, server revisions, pagination, listener visibility, backend
> authorization and the "listening is free" guarantee. Use before adding or changing a listener
> endpoint, wire type, error code or contract fixture.

Product rules: [mobile § API contracts](../../../../docs/architecture/mobile.md#api-contracts),
[business rule 25](../../../../docs/product/business-rules.md#listening-and-mobile) and the
[040 acceptance criteria](../../../../docs/work-plan/040-theatre-web-listening.md#acceptance-criteria).
Generic Axum handler, DTO and error-mapping technique lives in
[`http-api-boundary.md`](../../cantos-engineering/references/http-api-boundary.md); this file adds
only what is specific to listeners. Listener routes are JSON; adding MessagePack to one follows
the foundation's [wire codec procedure](../../cantos-engineering/references/wire-codec.md#5-extending-to-theatre-and-cmp)
and needs a measured listener payload first. Progress and bookmark semantics are in
[`progress-sync.md`](progress-sync.md); download grants in [`downloads-contract.md`](downloads-contract.md).

## Resource map

Every listener route lives under `/api/v1/theatre` (`cantos_contracts::routes::THEATRE_BASE`) and
never reuses a Studio route, session or editor `Store` method; a Studio session grants nothing
here. Two routers implement it: `adapters/inbound/http/theatre.rs` (anonymous Discovery/Playback, DTOs
in `cantos_contracts::theatre`) and `adapters/inbound/http/theatre_library.rs` (account-owned Library,
DTOs in `cantos_contracts::listener_library`). The actual contracts own names and behavior:
[Discovery v1](../../../../packages/contracts/theatre-discovery-v1.md) and
[Library v1](../../../../packages/contracts/listener-library-v1.md). Actual public delivery is
fail-closed unless the host composes `router_with_theatre_delivery`; OpenAPI and drift-checked
Kotlin generation remain required client acceptance.

| Resource | Operation | Status | Mutability | Caching |
|---|---|---|---|---|
| Catalog and search | `GET /works?q&kind&language&limit&cursor`, `GET /search` | implemented | changes when a release activates or is retracted | keyset cursor |
| Work and units | `GET /works/{work}`, `GET /works/{work}/units` | implemented | changes on (re)publish | keyset cursor |
| Publication manifest | `GET /publications/{publication}` | implemented | immutable | per Discovery v1 |
| Playback authorization | `POST /publications/{publication}/playback-authorizations` | implemented, fail-closed without delivery | none durable; mints a short-lived URL | `no-store` |
| Continue, Saved, History pages | `GET /library/continue`, `GET /library/saved`, `GET /library/history` (`limit`, `cursor`) | implemented, account-only | projections of registers | `no-store` |
| Saved register | `GET`, `PUT /library/saved/{work}` | implemented | revisioned desired state | `no-store` |
| Playback start, clear | `POST /library/history/playback-starts`, `POST /library/history/clear` | implemented | history revision and epoch | `no-store` |
| Progress | `GET`, `PUT /progress/{publication}` | implemented | revisioned; conditional | `no-store` |
| Operation receipt | `GET /operations/{operation}` | implemented | immutable | `no-store` |
| Bookmarks | `GET /library/bookmarks` (`limit`, `cursor`), `PUT /library/bookmarks/{bookmark}`, `POST /library/bookmarks/{bookmark}/delete` | implemented (Library v1.2, #299), account-only | immutable value; final tombstone; metadata always present | `no-store` |
| Download manifest, grants, authorizations | see [`downloads-contract.md`](downloads-contract.md) | proposed | manifest immutable; grant revisioned; URL disposable | `no-store` except the manifest |

Account routes (`/library/*`, `/progress/*`, `/operations/*`) pass through
`theatre_auth::bff_identity`, which admits either the Theatre Web cookie or the native bearer —
never both, never a Studio cookie — and refuses everything with the default
`DenyListenerAuthentication` when neither identity purpose is configured.

Playback authorization is a `POST` because each call makes an access decision and mints a new
credential; it is not a safe, cacheable read. The immutable manifest never contains a URL with a
signature, so it stays cacheable and loggable.

## Read performance

For the database-backed reads in the resource map, use the foundation's
[PostgreSQL/DBSP workflow](../../cantos-engineering/references/postgresql-read-performance.md).
Keep the visibility matrix, cursor law, no-production-work test and backend access checks below
as acceptance oracles for every serving path, including fallback.

## Identity

**A1 — Identifiers are opaque, typed and immutable.**

- *Why.* Progress, bookmarks, downloads and caches outlive titles, slugs and storage layouts. An
  identifier derived from any of them breaks on the first rename or storage migration.
- *Good.* `{"work_id": "wrk_8d2f…", "title": "Ánh đèn cuối sân khấu"}`; the title is display
  data that may change without touching any reference.
- *Counterexample.* `/works/anh-den-cuoi-san-khau/episodes/1`, or progress keyed by the object
  key `releases/ep01/master.m4a`. Episode *position in a list* is not identity either.
- *Oracle.* A fixture pair where only titles changed decodes to identical IDs; newtype barriers
  make a `PublicationId` impossible to pass where a `ContentUnitId` is expected (`type-enforced`).
- *Status.* implemented server-side: `listening::domain` parses `OperationId`, `DeviceId`,
  `PublicationId`, `RenditionId` and `EngineSessionId` from canonical UUID text
  (`platform::ids::canonical_uuid`) and reuses Catalog's `WorkId` and `ContentUnitId`; client
  domain types are proposed. *Exception.* A human-readable slug may exist as a presentation alias
  that the server resolves to an ID; clients never store or send it as a key.

`publication_id` identifies one immutable release of an episode — the release ID of
[`cantos-publication`](../../cantos-publication/references/staged-publication.md), exposed under
the name the [mobile contract](../../../../docs/architecture/mobile.md#api-contracts) uses. "The
episode's current audio" is a pointer the server resolves, never an identifier a client persists.
Timing identity is a different fact: two releases (a rollback reuses an earlier manifest) can
carry byte-identical audio. Expose the playable assets' checksums in the projection so a client
can tell "audio changed" from "release changed" without comparing titles or IDs.

```rust
// Actual shape (listening/domain/mod.rs): private field, fallible parse, no serde.
pub struct PublicationId(String);
impl PublicationId {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        canonical_uuid(raw).map(Self).ok_or(DomainError::InvalidId)
    }
}
```

```kotlin
// Illustrative, proposed. A value class keeps IDs from mixing without runtime cost.
@JvmInline
value class PublicationId private constructor(val value: String) {
    companion object {
        fun parse(raw: String): PublicationId? = raw.takeIf(ID_FORMAT::matches)?.let(::PublicationId)
    }
}
```

Content, publication, rendition, operation, device and engine-session IDs are canonical
lowercase UUID text. Exact audio timing is the value `AudioIdentity` (work, unit, publication,
rendition, SHA-256, duration); it is an identity, never an access grant.

### Account identity

The account scope is `ListenerId`, a private newtype over the registry
`platform::identity::domain::ActorId`, constructed only by `ListenerId::from_actor` inside the
crate and reached only through `AuthenticatedListener::listener()`. The issuer adapters
(`TheatreListenerAuthenticator`, `NativeListenerAuthenticator`) validate the verified actor with
`ActorId::new` and fail closed as `Unavailable` when a stored actor ID is malformed. No request
body, path or query carries the account; PostgreSQL stores it as `listener_id` referencing
`actors(id)`. A listener is not a Catalog `OwnerId`: ownership of Studio content and listening
as an account are different facts that happen to share the actor registry grammar
(`listener_scope_admits_exactly_the_registry_actor_grammar_studio_owners_already_use`).

## The schema is the source of truth

**A2 — The committed OpenAPI document is the reviewed contract; wire types are derived from it.**

- *Why.* Hand-maintained Kotlin or Leptos DTOs drift from the server silently; the
  [contracts guide](../../../../packages/contracts/README.md) forbids divergent definitions.
- *Good.* `packages/contracts/listener-api/openapi.yaml` (proposed path) reviewed in the PR that changes
  behavior; Kotlin wire types generated from it; Rust either generated from it or emitting it,
  with CI regenerating and failing on any diff.
- *Counterexample.* A `data class ProgressDto` written by hand "until generation is set up", or a
  Leptos struct copied from the server crate with one field renamed.
- *Oracle.* Regenerate-and-diff (`statically-checked`); every fixture decodes in every
  generated type set (`example-tested`); server responses validate against the schema in handler
  tests.
- *Status.* partial. Implemented: the committed JSON Schemas
  `packages/contracts/schema/theatre/{discovery,library,playback}/v1.schema.json`, strict serde DTOs
  in the `cantos-contracts` crate (`theatre`, `listener_library`) shared by the API and Theatre
  Web, and fixture conformance tests (`packages/contracts/api/tests/listener_library_wire.rs`,
  `theatre_discovery_wire.rs`). Proposed: OpenAPI and generated or drift-checked Kotlin; CMP
  has no Library wire types yet. *Exception.* None for wire types. Hand-written **domain** types
  are expected; they are mapped from wire types at the boundary, as `listening::domain` does.

Choose and record (ADR) one production path: **schema-first** (generate Rust and Kotlin from the
committed file) or **code-first** (Rust annotations emit the schema; CI fails when the emitted
file differs from the committed one). Candidates to evaluate: utoipa or aide for code-first;
typify, progenitor or openapi-generator for schema-first; kotlinx.serialization targets for
Kotlin. Every generated file names its source, schema version and generation command.

Generated wire types are raw. Map them into domain values once, at the edge, with the exact
error — see [`boundary-hardening.md`](../../cantos-engineering/references/boundary-hardening.md):

```kotlin
// Illustrative, proposed. Generated DTO in; a domain value or the exact contract error out.
fun ProgressRecordDto.toDomain(): Decoded<ProgressRecord> {
    val publication = PublicationId.parse(publicationId)
        ?: return Decoded.Rejected(ContractError.MalformedId(field = "publication_id"))
    val position = Position.ofMillis(positionMs)
        ?: return Decoded.Rejected(ContractError.OutOfRange(field = "position_ms"))
    return Decoded.Accepted(ProgressRecord(publication, position, ProgressRevision(revision)))
}
```

Theatre Web is Rust, so it depends on the shared `cantos-contracts` wire crate (today
`live/library.rs` decodes `ContinuePage`, `SavedPage` and `HistoryPage`); it never depends on
server domain, application or SQL modules. Add TypeScript types only when a JavaScript consumer exists
(a service worker, for example), generated from the same schema.

### Contract fixtures

Library fixtures live in `packages/contracts/fixtures/theatre/library/v1/` (22 files today: each
operation's request and response, every closed error code, both conflict kinds and an empty
page); Discovery and the mock player have sibling directories. Keep for every operation:
a normal case, each error code the operation can return, a conflict carrying the current record,
an empty page, the last page and long Vietnamese titles such as `Ánh đèn cuối sân khấu — Tập 1:
Một lời hẹn`. Each fixture must validate against the schema, decode in Rust and Kotlin wire
types, map to a domain value or the exact `ContractError`, and re-encode stably. Fixtures are
original sample content; never a private manuscript or a real signed URL.

## Compatibility

Positions and durations are integer milliseconds (`position_ms`, `duration_ms`); sizes are
integer bytes. Library server times are integer Unix milliseconds from the database clock
(`accepted_at_ms`); device times travel only as `device_reported_at_ms` context. Never float seconds: Rust and Kotlin round
and print floats differently, and the drift appears exactly at seek boundaries.

| Change | Class | Requirement |
|---|---|---|
| New optional response field; new endpoint; new optional request field with a server default | additive (minor) | old fixtures still decode; clients ignore unknown response fields |
| New value in an enum the schema declares **open** (error `code`) | additive | clients map unknown values to a documented fallback by HTTP status and `retryable` |
| New value in a **closed** enum (playback status, write kind) | breaking (major) | new path version or a negotiated capability, plus an ADR |
| Rename, removal, type or unit change, optional → required, changed meaning of a code | breaking | new major; keep the previous major for the documented support window |

Server enums are always closed and exhaustively matched. "Open" is a property of the wire
mapping only: the client's boundary has one explicit `Unrecognized` arm, and its domain enum
stays closed. Mobile clients in the field lag releases, so the support window for the previous
major is a product decision to record before the first breaking change. A schema-diff tool
(oasdiff is a candidate) can classify changes; its verdict is `statically-checked`, not a
replacement for review.

## Operation IDs and idempotency

**A3 — Every retriable write carries an operation ID created when the listener's intent happens.**

- *Why.* Mobile networks drop responses after the server committed. Without a stable key, the
  retry creates a second bookmark or advances progress twice.
- *Good.* The client writes `{operation_id, body}` to its durable outbox, then sends; every retry
  resends the identical body with the same key.
- *Counterexample.* The HTTP client's retry interceptor generating a fresh UUID per attempt, or a
  key derived from `now()`.
- *Oracle.* Duplicate delivery returns the original response and leaves the store unchanged;
  reusing a key with a different body returns `idempotency_key_reused` and has no effect.
- *Status.* implemented server-side for every Library mutation; client outboxes are proposed.
  *Exception.* Reads, and the playback/download authorization `POST`s, which have no durable
  effect beyond an audit row; if they gain one, they gain a key.

Server mechanics as implemented: `listener_operations` has primary key
`(listener_id, operation_id)` and stores the SHA-256 of the canonical JSON
`("library-v1", route, body)` plus the exact receipt bytes and digest; update, delete and truncate
are rejected by trigger. Every mutation first locks the account row (`listener_state … FOR
UPDATE`), so a concurrent duplicate waits and then replays instead of racing. Same key and
digest → the stored receipt, byte for byte. Same key, different digest → `idempotency_key_reused`,
no effect. Semantic conflicts are receipts too. `GET /operations/{operation}` returns the original
receipt after a lost response; [`progress-sync.md`](progress-sync.md) shows the full order. No
expiry exists: receipts are retained indefinitely until a retry-horizon decision is recorded, so
`operation_expired` is still a proposal.

## Errors

**Implemented (Library).** An error body is exactly `{"code": …, "retryable": …}`
(`dto::LibraryError`); no message, no SQL or private field. A semantic conflict is **not** an
error body: it is a `409` receipt carrying the original operation ID, the outcome, the current
revisioned state and an explicit resolution
(`fixtures/theatre/library/v1/progress_conflict.json`):

```json
{
  "operation_id": "00000000-0000-4000-8000-00000000000a",
  "outcome": "revision_conflict",
  "resolution": "listener_chooses",
  "state": { "resource": "progress", "current": { "position_ms": 15000, "revision": 1, "…": "…" } }
}
```

`LibraryError` is closed and mapped to `(status, code, retryable)` by one exhaustive function,
`theatre_library::error`, whose test asserts every row and the exact body:

| Code | HTTP | Retryable | Client behavior |
|---|---|---|---|
| `invalid_request` | 400 | no | client defect: malformed ID, body, query or binding |
| `invalid_cursor` | 400 | no | restart pagination |
| `unauthenticated` | 401 | no | re-authenticate; never resubmit as another account |
| `not_found` | 404 | no | absent, other account's, or not current-public for a new key |
| `invalid_position` | 422 | no | client defect; position outside `[0, duration_ms]` or kind/position mismatch |
| `idempotency_key_reused` | 422 | no | quarantine the outbox entry and report it |
| `stale_device_sequence` | 409 | no | outbox or device identity corrupted; quarantine |
| `revision_exhausted` | 409 | no | revision reached the signed bigint maximum |
| `unavailable` | 503 | yes | resend the identical operation, or read its receipt |
| `corrupt_state` | 500 | no | server invariant failure; report |
| outcome `revision_conflict` | 409 | — | receipt: reconcile from `state` and `resolution` |
| outcome `history_epoch_conflict` | 409 | — | receipt: the acknowledgement predates a clear; drop it |
| outcome `bookmark_deleted` | 409 | — | receipt: the ID's tombstone is final; the create is refused, never retried |

Discovery/Playback errors are owned by [Discovery v1](../../../../packages/contracts/theatre-discovery-v1.md).

**Proposed, not implemented:** `publication_withdrawn` (410), `access_denied` (403),
`access_expired` (401 after one refresh), `rendition_unavailable` (503, never a trigger for
generation or transcoding), `offline_not_permitted` (403),
`operation_expired` (422) and `contract_version_unsupported` (400). Each one needs a fixture and
an exact-code test before it ships. Tests assert the exact variant and code per case — never
`is_err()` or a bare status. Returning `403` for a draft would confirm that it exists; drafts and
absent releases must return the same status and the same body.

## Server revisions and time

Revisioned registers (progress, Saved, history state) carry `revision`, a server-assigned integer
that increases by exactly one per accepted write and is bounded by the signed bigint maximum
(`revision_exhausted`). `base_revision = 0` means "no register yet". Each account also has one
monotonic `listener_order`, the account-wide order of accepted changes used by pages.
`accepted_at_ms` comes from the database clock. Every write response is a receipt carrying the
resulting or current state, so a client never needs a follow-up read to learn its revision.
`device_reported_at_ms` is context and is never used for ordering.

## Pagination

- **Implemented (Library).** `limit` is `1..=50` (default 20). The cursor is a random 256-bit hex
  key into a process-memory `CursorVault`, bound to the listener, route, limit, history epoch and
  an immutable frontier (`upper_order`, last order and Work), expiring after 15 minutes; a process
  restart invalidates every cursor (`invalid_cursor`). No internal ID or payload is in the
  token. Order is descending `listener_order`, then C-collated Work UUID.
- Items present for the whole iteration appear exactly once; items changed during iteration may
  or may not appear, but never cause a duplicate or a skip. That is a property: generate
  registers and concurrent changes, page through, and assert the law.
- Visibility and ownership are re-applied on every page; a cursor is not an authorization token.
- Discovery pagination is owned by its contract.

## Listener visibility

**A4 — Only the active, complete release is listener-visible.**

- *Why.* [Business rule 23](../../../../docs/product/business-rules.md#review-and-publication)
  forbids listener discovery of partial uploads and unapproved previews.
- *Good.* Handlers take a `ListenerPublication` witness that only the active-release resolver can
  construct, from the pointer and readiness facts owned by
  [`cantos-publication`](../../cantos-publication/references/staged-publication.md).
- *Counterexample.* `WHERE status != 'draft'`, which admits staged, failed and unapproved rows
  the next time someone adds a state.
- *Oracle.* A visibility matrix against real PostgreSQL: seed an episode with no release (script
  draft only) and one release in each state of the
  [release lifecycle](../../cantos-publication/references/staged-publication.md#2-the-release-lifecycle-is-a-closed-state-machine)
  — `Draft`, `Staging`, `Verified`, `Ready` (scheduled, not yet active), `Active`, `Superseded`,
  `Retracted`, `Abandoned` — and assert every endpoint's result (catalog, work, manifest, playback
  authorization, download manifest, download authorization) as listed, `404` or `410`.
  `integration-tested`. Add a row whenever the lifecycle gains a state.
- *Status.* implemented for anonymous current-public access: `ListenerPublication::resolve`
  admits only an active, complete, approved, rights-cleared release of a Public work with an
  active owner, and every other state answers the same `not_found`
  ([Discovery v1](../../../../packages/contracts/theatre-discovery-v1.md)). Library re-admits a
  *new* progress key, Saved desire or history start against the current publications before
  commit; an already-owned register stays readable after access disappears. The retained-release
  exception below is **proposed**: superseded releases are not resumable today and no `410` exists.
  *Exception.* A `Superseded` release stays fetchable by ID for an account
  with progress, bookmarks or a grant on it, as far as retention and access allow
  ([mobile § Durable listening progress](../../../../docs/architecture/mobile.md#durable-listening-progress));
  it is never listed in the catalog. A `Retracted` release answers `publication_withdrawn` to such
  an account and `not_found` to everyone else.

The listener manifest is an explicit projection of the release manifest. Provider, voice and
model identifiers, costs, QC findings, internal storage keys and script source never cross it.
Assert the field set with the schema in tests (`additionalProperties: false` on the server-side
response validator), so a new release-manifest field cannot leak by default.

## Listening is free

**A5 — No listener request creates production work.**

- *Why.* [Business rule 25](../../../../docs/product/business-rules.md#listening-and-mobile) and
  [AGENTS.md](../../../../AGENTS.md): playback streams stored renditions; it never invokes AI.
- *Good.* A missing rendition returns `rendition_unavailable` and raises an operational alert.
- *Counterexample.* "Transcode on demand if the format is missing", or a manifest handler that
  enqueues a "warm the cache" job through the production queue.
- *Oracle.* Both halves, because each catches what the other misses:
  structural — the listening module imports no production orchestration, provider port or job
  enqueue API (`statically-checked` once a dependency or import check exists); behavioral — the
  test below (`integration-tested`).
- *Status.* structural half implemented: `scripts/check_repository.py` allows `listening` to
  import only `catalog` and `media`, and Discovery v1 states that playback never invokes
  production (`statically-checked`, `documented`). The behavioral row-count test below is
  **proposed**; no such test exists yet. *Exception.* None.

```rust
// Illustrative, proposed. Table names come from cantos-production-pipeline, not guesses.
#[tokio::test]
async fn repeated_play_creates_no_production_work() {
    let world = TestWorld::with_real_postgres().await;
    let release = world.seed_active_release("Ánh đèn cuối sân khấu", "Một lời hẹn").await;
    let before = world.production_row_counts().await; // runs, steps, attempts, reservations, outbox

    for _ in 0..5 {
        world.theatre().manifest(release.publication_id).await.expect("manifest");
        world.theatre().authorize_playback(release.publication_id).await.expect("authorization");
        world.theatre().put_progress(release.checkpoint_at_ms(60_000)).await.expect("progress");
    }
    world.storage().delete_rendition(release.asset_id).await;
    let missing = world.theatre().authorize_playback(release.publication_id).await;

    assert_eq!(missing.unwrap_err().code(), ErrorCode::RenditionUnavailable);
    assert_eq!(world.production_row_counts().await, before);
    assert_eq!(world.fake_providers().total_calls(), 0);
}
```

A code comment saying "this never triggers TTS" is `documented`; only the test is evidence.

## Authorization

**A6 — The backend decides access; signed URLs are a consequence, never a credential to keep.**

- Implemented: Discovery/Playback is anonymous and limited to current public releases.
  Every Library handler first resolves `AuthenticatedListener` from exactly one credential
  (Theatre Web cookie or native bearer, a single `Authorization` header, bounded and redacted
  `ListenerCredentials`); there is no request field naming the account. Entitlements and a
  `ListenerAccess` value beyond "account + current public" are proposed.
- The decision is a pure function over access and publication policy returning the exact
  denial. Hiding a button is presentation; tests call the API directly as a non-entitled account.
- Signed URLs are short-lived and minted after authorization. The implemented delivery value in
  `media::listener_delivery` redacts its URL in `Debug`, and the Library stores no URL. The
  log-capture test that asserts no captured line contains the signature query parameter is
  proposed.
- Cross-account access to an account-scoped resource (another account's progress, receipt,
  bookmark or grant ID) returns `404`, not `403`; Library receipts and registers are looked up
  only inside the authenticated `listener_id`.

## Testing ladder for a listener endpoint

1. Pure decisions with exact error variants: access, visibility projection, cursor decoding
   (`cargo test -p cantos-server --lib contexts::listening`, plus the `--doc` compile-fail barriers).
2. Fixtures decoded and re-encoded by Rust wire types (`cargo xtask verify contracts`); Kotlin
   and regenerate-and-diff are proposed.
3. Handler tests against real PostgreSQL: visibility matrix, denials, exact code and status,
   duplicate replay, key reuse, pagination law. The Library's real-database tests are
   `#[ignore]`d in `listening/postgres/tests.rs` and run only in `cargo xtask verify postgres`;
   default-deny Router tests run in `tests/listener_library_contract.rs`.
4. Response conformance: actual handler output validated against the schema.
5. The no-production-work test.
6. Real storage/CDN delivery for playback authorization, range requests and expiry, per the
   [040 review boundary](../../../../docs/work-plan/040-theatre-web-listening.md#review-boundary).

## Common mistakes

- Persisting "latest episode audio" on the client instead of the `publication_id` it played.
- Returning a signed URL inside the immutable manifest, which makes it uncacheable and leaks it.
- One generic `error: String`; clients then branch on English text.
- Float seconds for positions; offset pagination over a live catalog.
- A "temporary" hand-written DTO in CMP that becomes permanent.
- Treating a mocked handler response as evidence of CDN playback.

## Report additions

```text
Contract version:     listener API <x.y.z> (/v1); schema diff: additive | breaking (+ADR link)
Fixtures:             <paths>; decoded by Rust wire types | Kotlin wire types (<target>) — commands
Visibility matrix:    <states × endpoints run>; real PostgreSQL: yes | no
Error codes covered:  <codes with an exact-code test> / <codes in the schema>
No-production-work:   <test name> → pass | fail | not run
```
