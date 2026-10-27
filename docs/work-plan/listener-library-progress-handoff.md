# Listener Library / Progress handoff

The [Library v1 contract](../../services/cantos-server/contracts/theatre/library/v1/README.md)
owns current account-scoped Save, progress, history, resume, operation receipts and release-bound
bookmarks. Server persistence is Turso. Web has live cookie-session/Library synchronization;
CMP composes native Bearer sessions, SQLDelight/outbox and the native player. Explicit previews
remain isolated and do not supply live acceptance. Read each current consumer:
[Web](../../apps/cantos-theatre/web/README.md), [CMP](../../apps/cantos-theatre/mobile/README.md).

## Implemented server boundary

Continue, Saved Work, history/start/clear, exact progress, bookmarks and operation reconciliation
are implemented behind separate Theatre authority. Web cookies never authorize Studio; native
Bearer uses its own purpose and current Identity security facts. The normal configured host
consumes [Identity v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md),
not the retired provider issuer or an always-denied Library composition.

The Server derives account scope from the authenticated principal and checks active actor/current
resource eligibility in the owning unit. Registry lookup is not authentication. No guest/account
merge, offline-download authorization or analytics endpoint follows from this contract. Logout
never relabels account A's outbox as B; history starts only after actual engine acknowledgement.
Private media success does not authorize public bytes or native playback.

## Required Web/CMP consumer behavior

1. Mint an operation UUID and stable device UUID at **intent**, retain the immutable body and
   increasing device/resource sequence through timeout/restart retries. A conflict is a recorded
   outcome; resolving it is a new intent against the current revision. Do not change the old key.
2. Keep exact publication/rendition/hash/duration with Work/ContentUnit continuity. On replacement
   offer a new release only by explicit choice. No silent chapter/audio substitution, clamp or
   automatic seek. Resume state does not authorize bytes; use the existing Playback boundary.
3. Do not use max position. Preserve rewind, checkpoint-to-end versus explicit completion, and
   explicit restart. Render `keep_server` versus `listener_chooses` conflicts separately.
4. Save is a Work-only desired boolean. Unsave preserves progress, history, bookmarks and downloads.
   Cards and detail use the same account/revision register, with false tombstones retained.
5. Emit history only after the actual engine acknowledges start for the current account, source
   and engine session. Detail views, grant requests, failed Play intents and progress reads/writes
   do not count. Deduplicate session acknowledgement and include the observed history epoch.
6. Require a fresh confirmation for clear using observed revision/epoch. Clear history alone;
   delayed old epochs conflict, and a replayed old start cannot restore cleared history.
7. Fence asynchronous responses by account/session and local intent generation. An old receipt
   is historical, not current state. Ignore older register revisions and reread after outbox
   recovery. Clear projections/credentials immediately on logout before account B becomes visible.
8. Restart paging after invalid/expired cursor or process restart. Tokens bind account, route and
   page size. A scan freezes an upper accepted-order frontier; refresh to see later updates.

## Remaining acceptance

Qualify actual Web/native account switch and restart, cross-client Save/progress conflicts,
unknown acknowledgement/reconciliation, history epochs and exact-release resume against the
current implementations. Keep Android/iOS playback, background lifecycle, secure storage,
HTTPS cookie/CORS and deployed delivery evidence separate from pure/JVM/loopback results.
No original mock-only consumer needs to be rebuilt to repeat the historical campaign.

Native tests and former PG assertions are mapped in the
[Listening port](../architecture/server-turso/listening.md),
[business closure](../architecture/server-turso/cmp-business-api.md) and
[retirement ledger](../architecture/server-turso/remaining-pg-retirement.md).
Use affected native Rust/Turso/contract and Web/CMP checks through [xtask](../../xtask/README.md);
`verify postgres` is retired. Measure current Turso SQL/index first under #384, then consider
ArcSwap/Quick Cache if justified; old PG plans do not establish native performance.

The [original backend handoff](https://github.com/loveoverflowcom/cantos/blob/2ee9360817a207eb89a8f05ee9ed42102d714229/docs/work-plan/listener-library-progress-handoff.md)
and [original evidence](../evidence/listener-library-progress.md) preserve their source-bound
mock-only/PG observations. They are historical comparison inputs, not current implementation
or executable next-step instructions. GitHub owns live issue/acceptance status.
