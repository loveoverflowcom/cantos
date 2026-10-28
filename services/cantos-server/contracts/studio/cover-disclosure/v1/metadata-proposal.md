# Proposed metadata-cover scope (#574)

This is an unmounted application contract, pending policy acceptance in
[ADR0033](../../../../../../docs/decisions/0033-release-pinned-public-cover.md#proposed-574-metadata-only-extension--not-accepted-or-activated).
It defines no new live HTTP route. The existing v1 release-cover requests remain unchanged.

`CoverDisclosureService::record_metadata` accepts:

```json
{
  "expected_work_revision": 2,
  "evidence_reference": "private reference to explicit metadata-image permission",
  "disclosure": {
    "operation_id": "00000000-0000-4000-8000-000000000001",
    "image_asset_digest": "ia-r1:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "decision": "cleared",
    "valid_until_ms": 1900000000000
  }
}
```

All objects reject unknown fields. A current authenticated active Work owner, exact current Work
revision, admitted image of that Work, nonblank private evidence reference and future exclusive
expiry are required. The existing immutable clearance response is returned. A new binding and
clearance commit atomically; no response exposes the evidence reference or image provenance.

Replay preserves the original result after later edits. Reusing its operation with a different
revision, evidence reference or legacy unscoped request is a conflict. The existing revocation
request remains sufficient to remove public display. Renewal with metadata scope is a new call.

This method alone grants neither public metadata nor host delivery. Metadata projection also
requires the source/rights-verified disclosed snapshot at precisely this revision, the latest
same-image clearance and the separately enabled host policy. A newer decision with no metadata
binding cannot inherit it. No existing consent is backfilled. The non-login fictional principal
cannot use this authenticated method; its operator intake/consent path remains deferred until
actual image authority and policy are selected.
