# Admin v1

Dedicated `/api/v1/admin` backend capability. Default unavailable without a separately configured
Admin verifier and operator-applied function grants; no Studio/Theatre/producer credential grants
access. The [authorization policy](../../../../../../docs/architecture/admin-authorization.md) owns issuer,
role, freshness, local revocation and audit semantics.

- GET `/capabilities`: `CapabilityResponse` with exact internal `actor_id`, closed `role`, explicit
  `source`, seconds-since-Unix `valid_until`, and committed `audit_id`.
- POST `/actors/{actor}/revoke-access`: `RevokeAccessRequest` contains only non-nil UUID
  `operation_id`. `RevokeAccessResponse` includes that canonical UUID, target ActorId,
  `revoked=true` and original committed `audit_id`. No actor disable or resource ACL change.
- Runtime DTOs are the Server's `contract::admin` (no client consumer); [schema](access.schema.json) and
  [fixtures](fixtures/capability.json) define the bounded response shape. No frontend
  consumer, Kotlin contract or UI is added.

Errors use the existing JSON `ApiError`: credential/unmapped actor401 `unauthenticated`;
missing/unknown/stale role, inactive/blocked actor or denied action403 `forbidden`;
invalid operation/target400 `invalid_request`; target missing404 `not_found`;
operation mismatch409 `operation_reused`; provider/audit/storage/corruption503 `unavailable`.
HTTP parsing errors precede application decisions and are not capability evidence. No-store and
normal origin policy apply. The deprecated Studio alias contains no Admin endpoint.

Replay requires the same operation/body and current authority. On an ambiguous acknowledgement,
retry that operation; never mint a new key to conceal an unknown result. Bearers remain private;
responses/audit never expose provider credentials or subject/email/username. Additive future
response changes require schema/DTO compatibility review; no generic permission framework exists.
