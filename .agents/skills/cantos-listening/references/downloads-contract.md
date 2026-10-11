# Downloads contract

> **Scope.** The server-side and contract half of offline listening: what a download is (an
> immutable manifest identified by publication and asset checksums), the pure
> `decide_download` over entitlement, offline rights and release visibility, a revisioned grant
> with an offline validity window, short-lived authorization kept separate from identity,
> revalidation and revocation stated honestly, replacement and withdrawal, account scoping,
> server-side delivery verification and fixtures. Use when adding or changing a download
> manifest, grant, entitlement rule, authorization endpoint, offline policy or the copy that
> describes revocation. The client lifecycle, staging and storage live in
> [`downloads-and-local-state.md`](../../cantos-cmp-mobile/references/downloads-and-local-state.md).

The product rules live in [mobile § Downloads and access rights](../../../../docs/architecture/mobile.md#downloads-and-access-rights),
[business rule 27](../../../../docs/product/business-rules.md#listening-and-mobile) (opt-in,
resumable, integrity-checked, subject to access rules; document what happens after replacement,
expiry or withdrawal) and the [050 acceptance criteria](../../../../docs/work-plan/050-cmp-native-listening.md#acceptance-criteria).
Streaming rights and offline rights are different: permission to publish does not grant offline
redistribution ([`rights-and-provenance.md`](../../cantos-publication/references/rights-and-provenance.md)).
Nothing here exists; every route, type and field is a **proposal**. Open decisions are listed in
§ 10 and must be recorded before they are encoded.

## 1. What a download is

A download is **bytes whose identity is the immutable publication and the asset checksums** — never
a URL, a title or an object key. Three records, three lifetimes:

| Record | Meaning | Mutability | Caching |
|---|---|---|---|
| download manifest | the exact assets (IDs, sizes, checksums, formats, durations) of one immutable publication | immutable | cacheable; `ETag` = manifest digest |
| download grant | "this account on this device may hold this publication offline until…" | revisioned; never edited in place | `no-store` |
| download authorization | short-lived signed URLs to fetch the manifest's assets now | none durable | `no-store`; disposable |

The manifest is an explicit **projection** of the release manifest (L4 in
[`../SKILL.md`](../SKILL.md#rules-this-skill-owns)): provider, voice and model identifiers, costs,
QC findings, storage keys and script source never cross it.

```rust
// Illustrative and proposed. Server-side values; the Kotlin wire types are generated from the schema.
pub struct DownloadManifest {
    publication: PublicationId,
    manifest_digest: ManifestDigest,
    assets: NonEmpty<DownloadAsset>,
}

pub struct DownloadAsset {
    asset: AssetId,
    role: AssetRole,                // e.g. episode audio rendition
    format: AudioFormat,
    byte_size: ByteCount,
    checksum: Checksum,             // algorithm + hex, from the verified publication objects
    duration: DurationMs,
}
```

Checksums and sizes come from the **verified** objects the publication recorded
([staged publication](../../cantos-publication/references/staged-publication.md)), not from a fresh
read of storage at request time. A manifest with a signed URL inside is a defect: it is
uncacheable and leaks a credential.

## 2. `decide_download` is pure

Authorization to download is a decision over facts, with the exact denial named. It lives in the
backend core; the client renders the outcome.

```rust
// Illustrative and proposed. No I/O: the shell resolves the facts and one server instant.
pub fn decide_download(
    entitlement: &Entitlement,
    publication: &ListenerPublication,      // witness: only the active, complete release
    offline: &OfflineTerms,                 // rights verdict for Use::OfflineDownload and policy
    now: ServerInstant,
) -> Result<DownloadGrantTerms, DownloadDenial>;

pub enum DownloadDenial {
    NotEntitled,
    OfflineNotPermitted,                    // rights do not cover Use::OfflineDownload
    PublicationUnavailable,                 // not active, complete and visible
    PublicationWithdrawn,
    AccountRestricted,
    GrantLimitReached,                      // only if a limit is decided (§ 10)
}
```

- **No wildcard arm.** A new denial or entitlement variant must force a decision.
- **Rights are evaluated by the owner.** The offline verdict comes from the same evaluator as every
  other use; this skill does not re-implement it
  ([P1–P3](../../cantos-publication/SKILL.md#rules-this-skill-owns)).
- **Server time only.** Device clocks are context; the validity window is computed from
  `ServerInstant`.
- **Exact errors on the wire.** Each denial maps to a stable code in the listener error
  vocabulary ([`listener-api.md`](listener-api.md#errors)); clients branch on the code, never on
  prose, and localize it ([`localization.md`](../../cantos-ui-design/references/localization.md#5-errors-are-typed-then-mapped)).

## 3. The grant is a revisioned record

```rust
// Illustrative and proposed.
pub struct DownloadGrant {
    id: GrantId,
    account: AccountId,
    device: DeviceId,
    publication: PublicationId,
    state: GrantState,
    issued_at: ServerInstant,
    valid_until: ServerInstant,         // the offline validity window (decision pending, § 10)
    revalidate_after: ServerInstant,
    revision: GrantRevision,
}

pub enum GrantState { Active, Expired, Revoked(RevocationReason), Superseded { by: PublicationId } }
```

- **Created idempotently.** The request carries an operation ID created at intent time; a
  duplicate returns the original grant, and key reuse with a different body is
  `idempotency_key_reused` ([L3](../SKILL.md#rules-this-skill-owns)). One active grant per
  account × device × publication.
- **Never edited.** Renewal, revocation and supersession append a new revision; history stays for
  audit and for explaining state to the listener.
- **Revocation carries a reason** (`RightsRevoked`, `PublicationWithdrawn`, `AccountClosed`,
  `UserRemoved`) so the client can show an honest message.
- **Grants sync like any listener resource:** `GET /v1/theatre/download-grants?since=<revision>`
  with a keyset cursor, so a client learns about revocations on reconnect.

## 4. Authorization is disposable and separate

`POST …/download-authorizations` (illustrative) requires a valid grant and mints a short-lived
signed URL **per asset**, with range support.

- **Short lived, refreshable.** Refresh separately from the file's identity; a refresh never changes
  what is being downloaded. A transfer that outlives its URL resumes with a new authorization and
  the same validator ([client protocol](../../cantos-cmp-mobile/references/downloads-and-local-state.md)).
- **A secret.** A `SignedUrl` newtype with a redacting `Debug`/`Display` and no persistence mapping;
  a log-capture test asserts no signature appears
  ([`storage-and-delivery.md`](../../cantos-publication/references/storage-and-delivery.md#5-signed-urls-are-secrets-with-a-lifetime)).
- **`Cache-Control: no-store`.** Never part of the manifest, a grant row, a file name or a cache key.
- **Deciding again.** Authorization re-evaluates the grant, not only its existence: a revoked or
  expired grant yields a denial, not a URL.

## 5. Revalidation and revocation — be honest

The product states that strict immediate revocation cannot be guaranteed while a device is offline
and forbids invented DRM guarantees
([mobile](../../../../docs/architecture/mobile.md#downloads-and-access-rights)). The contract
therefore does not promise more than this:

| Client situation | Server contract | Client result (owned by the CMP reducer) |
|---|---|---|
| online, grant valid | grant sync returns it `Active` | play |
| online, revoked | grant sync returns `Revoked(reason)` | mark unavailable, clean up per policy, show the reason |
| offline, within `valid_until` | nothing to ask | play; revalidate on reconnect |
| offline, past `valid_until` | nothing to ask | unavailable until revalidated; explain |
| online, expired | renewal allowed if still entitled and rights hold | renew or explain the denial |

- **Disclose it.** User-facing copy states that a removed episode may remain playable offline until
  the validity window ends, and that Cantos cannot erase a copy from a device it cannot reach.
  Never claim DRM, remote erasure or immediate revocation.
- **Cleanup is local.** Revocation, expiry or removal deletes local files only; it never deletes
  server publication assets.
- **The window is a product decision** ([§ 10](#10-open-decisions-to-record-first)); until it is
  recorded, code returns a configured window and tests prove the mechanism, not the number.

## 6. Replacement and withdrawal

| Event | Server behavior | Client expectation |
|---|---|---|
| republish (new publication) | the old grant stays valid for its window; sync marks it `Superseded { by }` and exposes the new publication | keep the old download playable; offer the update; download the new one into its own path and switch only after verification |
| failed replacement download | nothing to undo on the server | the previous completed download stays playable |
| withdrawal / retraction | stop authorizing; revoke affected grants with `PublicationWithdrawn` per the rights policy; answer `publication_withdrawn` to accounts with a grant | explain the withdrawal; remove or keep per the recorded policy |
| rights revocation | the same, with `RightsRevoked` | as withdrawal |

Retention couples to the publication skill: prior objects must stay fetchable while grants and
in-progress downloads reference them, and garbage collection is two-phase
([`storage-and-delivery.md`](../../cantos-publication/references/storage-and-delivery.md)). A
rights-forced deletion is the one exception and must make the affected grants fail loudly.

## 7. Account scope

- Grants and authorizations are keyed by account; a request for another account's grant ID
  returns `404`, not `403` ([A6](listener-api.md#authorization)).
- A device session switch or logout invalidates in-flight authorizations for that account
  generation; the client hides the other account's files immediately.
- Download rows never store an email, a signed URL or a mutable title; identifiers are opaque.
- Anonymous downloading is a pending product decision (§ 10); until decided the endpoints require
  an account.

## 8. Server-side delivery verification

"Incomplete downloads are never offered as ready" has a server half: the manifest must describe
bytes that actually deliver.

- Fetch **every manifest asset through the delivery path** in an integration test and compare size
  and checksum with the manifest, under range requests and with an expired URL
  (`integration-tested`; a local file is not delivery).
- `Content-Length` and `ETag` agree with the manifest; the delivery host honors `Range` and
  returns `206` with correct `Content-Range`.
- A published release with a missing or mismatched object never yields a manifest: the
  `ListenerPublication` witness already requires a ready release.

## 9. Fixtures and vectors

Commit under `packages/contracts/` (proposed): a manifest with two assets, a grant per `GrantState`, the
denial table (one row per `DownloadDenial` with its error code) and the revalidation table of § 5.
Rust and Kotlin decode and re-encode every fixture; regenerate-and-diff covers the schema. The
client reducer's transition vectors (`reduce(entry, event)`) belong to the CMP skill and consume
these fixtures.

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Identity is publication plus asset checksums | a re-signed URL or renamed title orphans a file | manifest keyed by IDs and checksums | URL or object key as file identity | fixture with changed URL and title decodes to the same identity | proposed · none |
| The manifest is an immutable projection | provider and storage details leak; cache poisoned | explicit field set, `additionalProperties: false` | serializing the release manifest | schema conformance test | proposed · none |
| `decide_download` is pure and total | a new denial silently allows | exhaustive match, exact codes | `unwrap_or(true)` on rights | denial table; no wildcard arm | proposed · none |
| Offline rights are a separate verdict | streaming permission enables redistribution | `Use::OfflineDownload` evaluated by the owner | reusing the streaming verdict | `streaming_grant_does_not_cover_offline_download` | proposed · none |
| Grants are revisioned and created idempotently | duplicate grants; lost revocation history | operation ID; append a revision | updating `valid_until` in place | duplicate-request and revision tests | proposed · none |
| Authorization is separate and disposable | a leaked or stale URL becomes identity | `no-store`, redacted `SignedUrl` | URL stored in a row or the manifest | log-capture and schema tests | proposed · none |
| Revocation is described honestly | a promise the product cannot keep | validity window and disclosure copy | "your copy is erased" | copy review; revalidation table | manual · none |
| Cross-account ids answer `404` | enumeration of other accounts' grants | account-scoped lookup | `403` with detail | direct-request test as another account | proposed · none |

## 10. Open decisions to record first

Record each as a document update or an [ADR](../../../../templates/adr.md) before encoding it; a
reducer arm or server rule with no owning sentence is a gap in the plan.

- the offline validity window and revalidation cadence;
- whether anonymous listeners may download, and any per-account or per-device grant limit;
- whether grants are per publication or per asset, and the maximum concurrent downloads;
- the retention period for superseded and withdrawn objects while grants reference them;
- signed versus public delivery for download assets, and the rights wording for offline use.

## 11. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| each denial is reachable with its exact code | one row per `DownloadDenial` | `example-tested` |
| the manifest is immutable and a faithful projection | schema and digest test; changed storage details do not change it | `example-tested` |
| duplicate requests create one grant | replay test on real PostgreSQL | `integration-tested` |
| revisions never rewrite history | property over generated grant events | `property-tested` |
| no signed URL reaches logs or rows | log capture and row scan | `example-tested` |
| every manifest asset delivers with the stated size, checksum and ranges | fetch through the delivery path | `integration-tested`; only real delivery counts |
| the offline-revocation copy is accurate | a named reviewer | recorded review; not an automated claim |
