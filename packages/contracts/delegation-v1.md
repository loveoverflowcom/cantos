# Work-scoped delegation v1

Status: **implemented** for the [ADR 0028](../../docs/decisions/0028-work-scoped-delegation.md)
first slice (Catalog read and content write on one Work). The Studio Web owner panel
(list, issue with a one-time reveal, revoke) consumes the owner routes; see the
[Studio delegation UI evidence](../../docs/evidence/work-scoped-delegation-studio-ui.md).
Production role provisioning is an operator step. Raw DTOs are in
[`api/src/delegation.rs`](api/src/delegation.rs), the [schema](schema/delegation/v1.schema.json)
and [synthetic fixtures](fixtures/delegation/v1/README.md) are shared with the tests.

A grant lets one registered delegate actor perform closed actions on exactly one Work. It is
not a Studio session, a Work membership, an Admin capability or a producer grant, and none of
those can be presented in its place.

## Credential

`cantos_dlg_` followed by 64 lowercase hexadecimal characters (32 random bytes minted by the
server). It is shown once, in the response that created the grant. The prefix supports secret
scanning; it is not a checksum. Send it only as `Authorization: Bearer <token>`; never in a URL,
cookie, log or manuscript. Lost tokens cannot be recovered: revoke and issue a new grant.

## Actions

| Label | Allows |
| --- | --- |
| `work:read` | the granted Work, its content-unit page and any of its units |
| `content:read` | content head, an exact content revision and content history of its units |
| `content:write` | appending a content revision to one of its units: `text` (with `source: null`) or `recording` only, rights references unchanged |

Unknown labels are rejected by the transport. Review, publication, rights, membership, archive,
grant issuance, Script/source links, briefs, audio and artwork are not delegable in v1.

## Owner routes (Studio session cookie and Origin)

| Method and path | Request → response |
| --- | --- |
| `POST /api/v1/studio/works/{work_id}/delegations` | `IssueDelegationRequest` → `201 IssueDelegationResponse` with `token`; same operation and request again → `200` with `token: null` |
| `GET /api/v1/studio/works/{work_id}/delegations?limit=1..50&after={grant_id}` | → `DelegationGrantPage`, ordered by grant ID, including revoked and expired grants |
| `DELETE /api/v1/studio/works/{work_id}/delegations/{grant_id}` | → `204`; idempotent; the first revocation's actor and time are kept |

Only the Work owner may call them: a member receives `403 forbidden`, anyone else
`404 not_found`. `expires_at` is Unix milliseconds, after the server's issuance time and at most
30 days later. The grantor is the authenticated owner; the request cannot name one.

| Refusal | Status, code and issue |
| --- | --- |
| malformed body, unknown field or action label | `400 invalid_request` |
| bad `id`/`work_id`, actor syntax, empty or duplicate actions | `422 invalid_catalog` with `id`/`work_id` `invalid_uuid`, `delegate_actor_id` `invalid_actor`, `actions` `empty`/`duplicate_action` |
| delegate is the owner, unknown or inactive | `422 invalid_catalog`, `delegate_actor_id` `self_delegation` or `unavailable_actor` |
| lifetime outside `(0, 30 days]` | `422 invalid_catalog`, `expires_at` `invalid_expiry` |
| operation reused with another request | `409 operation_reused` |
| grant ID already used by this owner | `409 catalog_conflict`, `id` `duplicate_id` |
| archived Work | `409 catalog_conflict`, `lifecycle` `archived` |

## Delegate routes (bearer only)

Base `/api/v1/studio/delegated`; responses reuse [Catalog v1](catalog-v1.md) and
[Content v1](content-v1.md) shapes.

| Method and path | Action | Response |
| --- | --- | --- |
| `GET /grant` | any valid grant | `DelegationGrantResponse` for the presented grant |
| `GET /works/{work_id}` | `work:read` | `CatalogWork` |
| `GET /works/{work_id}/content-units` | `work:read` | `CatalogContentUnitPage` (cursor bound to the grant) |
| `GET /content-units/{unit_id}` | `work:read` | `CatalogContentUnit` |
| `GET /content-units/{unit_id}/content/head` | `content:read` | `ContentRevisionResponse` |
| `GET /content-units/{unit_id}/content/revisions/{revision_id}` | `content:read` | `ContentRevisionResponse` |
| `GET /content-units/{unit_id}/content/history` | `content:read` | `ContentHistoryResponse` |
| `POST /content-units/{unit_id}/content/revisions` | `content:write` | `201 AppendContentResponse`; same operation retried → same receipt |

A delegated append is recorded by the delegate actor and linked to its grant. Its replay
identity includes the grant, so another grant cannot replay it. No browser Origin is required
for this one route because it never accepts a cookie.

| Refusal | Status and code |
| --- | --- |
| missing, malformed, duplicated or mixed credential (cookie or Studio session header present) | `401 unauthenticated` |
| unknown, revoked, not yet valid or expired grant; grantor or delegate inactive; grantor no longer owner; Work archived | `401 unauthenticated` |
| resource outside the granted Work, or missing | `404 not_found` |
| action not granted; Source/Script link; rights change | `403 forbidden` |

Lifecycle refusals take precedence, so a revoked token learns nothing about any Work.

## Consistency

Every delegated request decides on current facts inside one transaction behind the Catalog
clock and rechecks them before commit. Revocation takes the same clock exclusively: a request
admitted before the revocation commits may finish; every later request is refused. Revocation
cannot recall a committed receipt or a response already sent.
