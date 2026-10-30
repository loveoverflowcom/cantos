# Draft: `cantos.media.public-playback-delivery` (#170)

Module-owned inventory of [`cantos.media.public-playback-delivery`](../../../../specs/behavior/catalog.json). The [root inventory](../../../../specs/behavior/inventory.md) keeps the shared authority framing; the registry owns the stable ID and lifecycle.

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Accepted for step-1 origin integration only | [ADR 0032](../../../../docs/decisions/0032-theatre-origin-playback-delivery.md#decision) selects origin-mode Range delivery with sealed 120 s grants and per-fetch rechecks; deployed delivery/performance and native playback acceptance remain pending, and [ADR 0038](../../../../docs/decisions/0038-publication-streaming-evidence-gates.md) keeps its evidence gates. | Lifecycle stays `draft` until a capability review accepts it; a PASS is model-only and accepts nothing. |
| Implemented, independently of this model | [Pure kernel](../../src/contexts/media/domain/playback_grant.rs) `grant_expiry`/`decide_media_fetch`; [application](../../src/contexts/media/listener_media.rs) admits before planning and again after object open; [authorization](../../src/contexts/media/listener_application.rs) re-resolves after sealing. Native example/property/tamper tests and the route mock integration stay authoritative for their boundaries. | `admit` and `issuedExpiry` mirror those seams; no Rust observer is registered (`--conformance` does not apply). |
| Model assumption | One immutable publication (identity 1); another unit's release is identity 2. Time classes 0–6 with publishedAt=1, rightsUntil=5, TTL=2, including rollback. `authentic` abstracts the HMAC result; tokens, keys, URLs and bytes are not modeled. Eligibility loss (withdraw/revoke/supersede/owner disable) is one irreversible fact. | No hash, Range, header, SQL isolation, restart or concurrency claim; HTTP transport is native-tested. |
| Model assumption | `Opening` is shell history between admission and the post-open recheck; `completeFetch` abstracts verified object bytes. | Covers withdrawal/expiry while the object opens; not lane bounds or body streaming. |


S05 (#651), under accepted ADR 0054 D06/D07, resolves the model's `eligible` input only after
current metadata generation membership and its exact associated release pass their live gates.
No unassociated/latest release is a fallback. Grant/fetch, time, identity and post-object-open
rechecks are unchanged. Class: approved requirement change + missing generation-admission
scenario; no `.qnt` change or observer claim. Real-file and HTTP tests cover that shell separately.
