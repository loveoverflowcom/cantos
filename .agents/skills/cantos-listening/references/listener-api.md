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
only what is specific to listeners. Progress and bookmark semantics are in
[`progress-sync.md`](progress-sync.md); download grants in [`downloads-contract.md`](downloads-contract.md).

## Resource map (proposed)

Nothing below exists. Names are placeholders for the first listener PR (040.1) to confirm in the
schema review; the shape — which resources exist and how each one may change — is the point.

| Resource | Illustrative operation | Mutability | Caching |
|---|---|---|---|
| Catalog | `GET /v1/theatre/works?cursor=…&limit=…` | changes when a release activates or is retracted | short TTL; keyset cursor |
| Work and episodes | `GET /v1/theatre/works/{work_id}` | changes on (re)publish | short TTL |
| Publication manifest | `GET /v1/theatre/publications/{publication_id}` | immutable | cacheable indefinitely; `ETag` = manifest digest |
| Playback authorization | `POST /v1/theatre/publications/{publication_id}/playback-authorizations` | none durable; mints a short-lived URL | `Cache-Control: no-store` |
| Progress | `GET`, `PUT /v1/theatre/progress/{publication_id}` | revisioned | `no-store`; conditional writes |
| Bookmarks | `GET /v1/theatre/bookmarks?since=…`, `PUT`, `DELETE /v1/theatre/bookmarks/{bookmark_id}` | revisioned; tombstoned | `no-store` |
| Download manifest, grants, authorizations | see [`downloads-contract.md`](downloads-contract.md) | manifest immutable; grant revisioned; URL disposable | `no-store` except the manifest |

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
  make a `PublicationId` impossible to pass where an `EpisodeId` is expected (`type-enforced`).
- *Status.* proposed. *Exception.* A human-readable slug may exist as a presentation alias that
  the server resolves to an ID; clients never store or send it as a key.

`publication_id` identifies one immutable release of an episode — the release ID of
[`cantos-publication`](../../cantos-publication/references/staged-publication.md), exposed under
the name the [mobile contract](../../../../docs/architecture/mobile.md#api-contracts) uses. "The
episode's current audio" is a pointer the server resolves, never an identifier a client persists.
Timing identity is a different fact: two releases (a rollback reuses an earlier manifest) can
carry byte-identical audio. Expose the playable assets' checksums in the projection so a client
can tell "audio changed" from "release changed" without comparing titles or IDs.

```rust
// Illustrative, proposed. Opaque IDs parsed once at the boundary; no public field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicationId(Uuid);

impl TryFrom<&str> for PublicationId {
    type Error = ContractError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        raw.parse().map(Self).map_err(|_| ContractError::MalformedId { kind: IdKind::Publication })
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

The ID format (UUID, ULID, prefixed) is a decision for the first schema; the barrier is not.

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
- *Status.* proposed. *Exception.* None for wire types. Hand-written **domain** types are
  expected; they are mapped from generated wire types at the boundary.

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

Theatre Web is Rust, so it may depend on a generated wire-type crate; it never depends on server
domain, application or SQL modules. Add TypeScript types only when a JavaScript consumer exists
(a service worker, for example), generated from the same schema.

### Contract fixtures

Keep request/response fixtures per operation (proposed `packages/contracts/listener-api/fixtures/<operation>/<case>.json`):
a normal case, each error code the operation can return, a conflict carrying the current record,
an empty page, the last page and long Vietnamese titles such as `Ánh đèn cuối sân khấu — Tập 1:
Một lời hẹn`. Each fixture must validate against the schema, decode in Rust and Kotlin wire
types, map to a domain value or the exact `ContractError`, and re-encode stably. Fixtures are
original sample content; never a private manuscript or a real signed URL.

## Compatibility

Positions and durations are integer milliseconds (`position_ms`, `duration_ms`); sizes are
integer bytes; server timestamps are RFC 3339 UTC. Never float seconds: Rust and Kotlin round
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
- *Status.* proposed. *Exception.* Reads, and the playback/download authorization `POST`s, which
  have no durable effect beyond an audit row; if they gain one, they gain a key.

Server mechanics: `(account_id, operation_id)` is unique; the operation log row stores a digest
of the canonical request body and the outcome, written in the **same transaction** as the effect.
Same key and digest → replay the stored outcome. Same key, different digest → exact error, no
effect. Claim the key first (insert the log row) so a concurrent duplicate blocks on the unique
index and then replays, instead of racing to a false conflict; [`progress-sync.md`](progress-sync.md)
shows the ordering. Retain operation records at least as long as the longest client retry
horizon, which is itself a recorded decision; an older key returns `operation_expired`, never a
silent re-apply.

## Errors

Every error body carries a stable machine `code`, a `retryable` flag and, for conflicts, the
current server state. `message` is developer text; clients localize from `code`, and the copy
belongs to [`cantos-ui-design`](../../cantos-ui-design/references/localization.md).

```json
{
  "code": "progress_revision_conflict",
  "retryable": false,
  "message": "Progress for this publication changed after base revision 4.",
  "current": {
    "publication_id": "pub_7f3a…",
    "position_ms": 900000,
    "revision": 5,
    "accepted_at": "2026-10-09T08:15:02Z"
  },
  "resolution": "listener_chooses"
}
```

Proposed codes (the schema review fixes the final set; each one needs a fixture and a test):

| Code | HTTP | Retryable | Client behavior |
|---|---|---|---|
| `not_found` | 404 | no | unavailable state; also returned for drafts, staged and unapproved releases |
| `publication_withdrawn` | 410 | no | explain the change; only for a publication this account was previously authorized for |
| `access_denied` | 403 | no | explain; never loop |
| `access_expired` | 401 | after one refresh | refresh session or authorization once, preserve position, then fail |
| `rendition_unavailable` | 503 | bounded | operational fault; never a trigger for generation or transcoding |
| `invalid_position` | 422 | no | client defect; position outside `[0, duration_ms]` |
| `progress_revision_conflict` | 409 | no | reconcile from `current` and `resolution` |
| `idempotency_key_reused` | 422 | no | quarantine the outbox entry and report it |
| `stale_device_sequence` | 409 | no | outbox or device identity corrupted; quarantine |
| `bookmark_deleted` | 409 | no | drop the local operation; the tombstone wins |
| `offline_not_permitted` | 403 | no | disable download with the stated reason |
| `operation_expired` | 422 | no | discard the operation; resync from the server |
| `invalid_cursor` | 400 | no | restart pagination |
| `contract_version_unsupported` | 400 | no | prompt to update the app |

The server's `ListenerError` enum is closed and mapped to `(status, code)` by one exhaustive
function. Tests assert the exact variant and the exact code per case — never `is_err()` or a bare
status. Returning `403` for a draft would confirm that it exists; drafts and absent releases must
return the same status and the same body.

## Server revisions and time

Revisioned resources (progress, bookmarks, grants) carry `revision`, a server-assigned integer
that increases by exactly one per accepted write, and `accepted_at` from the server clock.
Clients echo `base_revision` on writes; a write response returns the resulting record so a
client never needs a follow-up read to learn its own revision. A device timestamp, when sent,
is stored as `device_reported_at` context and is never used for ordering.

## Pagination

- Keyset pagination over an immutable ordering tuple with a unique tie-breaker (for example
  `(activated_at, publication_id)`); an opaque, tamper-evident cursor; a bounded `limit` with a
  server maximum; `next_cursor: null` on the last page.
- Items present for the whole iteration appear exactly once; items activated or retracted during
  iteration may or may not appear, but never cause a duplicate or a skip. That is a property:
  generate a catalog and a concurrent sequence of activations and retractions, page through, and
  assert the law.
- Visibility is re-applied on every page; a cursor is not an authorization token.

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
- *Status.* proposed. *Exception.* A `Superseded` release stays fetchable by ID for an account
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
- *Status.* proposed. *Exception.* None.

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

- Every listener handler resolves a `ListenerAccess` value (account, entitlements) or the
  anonymous access the publication's policy allows. Progress and bookmarks always require an
  account; whether published episodes may be heard anonymously is an open product decision.
- The decision is a pure function over access and publication policy returning the exact
  denial. Hiding a button is presentation; tests call the API directly as a non-entitled account.
- Signed URLs are short-lived and minted after authorization. Model them as a `SignedUrl` type
  with a redacting `Debug`/`Display` and no persistence mapping, so a log line or a database row
  cannot hold one by accident. A log-capture test performs a playback authorization and asserts
  that no captured line contains the signature query parameter.
- Cross-account access to an account-scoped resource (another account's progress, bookmark or
  grant ID) returns `404`, not `403`.

## Testing ladder for a listener endpoint

1. Pure decisions with exact error variants: access, visibility projection, cursor decoding.
2. Fixtures decoded and re-encoded by Rust and Kotlin wire types; regenerate-and-diff.
3. Handler tests against real PostgreSQL: visibility matrix, denials, exact code and status,
   duplicate replay, key reuse, pagination law.
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
