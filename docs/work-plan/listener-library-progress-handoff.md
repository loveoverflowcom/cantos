# Listener Library / Progress handoff

Backend slice for #12 / #17 / #35, additive to [PR93](https://github.com/loveoverflowcom/cantos/pull/93)
Discovery/Playback and its [handoff](../archive/handoffs/discovery-library-handoff.md). The MVP stays **Khám phá | Của tôi**,
with **Nghe tiếp**, **Đã lưu**, **Lịch sử gần đây**. Current Web/CMP reducers remain mock/session-only;
this slice does not turn their stores into account-sync acceptance. See [contract](../../packages/contracts/listener-library-v1.md)
and [evidence](../evidence/listener-library-progress.md).

The final develop recheck includes PR96 Web Library/Account visual changes. Its current
`TheatreContext` still calls mock `toggle_saved` and session-only progress/history reducers;
the account view is not a verified identity or sync adapter. Those inherited visual screens do
not broaden this backend MVP contract. Web consumers still need dynamic UUID/source binding
and intent-based transport rather than using enum/demo IDs or toggles as server commands.

## Implemented server boundary

| Capability | Versioned API | Remaining production acceptance |
| --- | --- | --- |
| Continue / resume facts | GET `/api/v1/theatre/library/continue` | actual retained-release access, explicit client resolution; no autoplay |
| Saved Work state | GET `/library/saved`, GET/PUT `/library/saved/{work}` under Theatre | shared cards/details, stable desired intent and response fencing |
| History | GET `/library/history`, POST `/library/history/playback-starts`, POST `/library/history/clear` | actual-engine acknowledgement, confirmation UI, epoch-aware outbox |
| Exact progress | GET/PUT `/progress/{publication}` under Theatre | real player seek/checkpoint/completion/restart integration |
| Unknown outcomes | GET `/operations/{operation}` under Theatre | retry recovery without new keys or changed bodies |
| Account authentication | separate typed bearer port; default deny | verified Listener issuer/session/actor mapping adapter |

No bookmarks/downloads/analytics endpoint, automatic guest merge, reference-v2 navigation, OpenAPI
or generated Kotlin client is delivered. Do not close the broad issues for this backend slice.
Publication/delivery defaults still fail closed; private Storage PR90 does not supply public bytes.

## Identity campaign coordination

`contexts::listening::auth::ListenerAuthenticator` receives redacted `ListenerCredentials`, and
returns sealed `AuthenticatedListener` or a typed unauthenticated/unavailable outcome.
Only a trusted crate-local issuer adapter can call `AuthenticatedListener::from_verified_actor`.
`adapters::inbound::http::router_with_listener_identity` is the explicit composition seam; the normal host
uses `DenyListenerAuthentication`. No raw actor ID, Studio cookie or private development token
is accepted by that adapter. Mapping issuer/subject to actors is not authentication.

The Theatre identity successor must verify issuer/signature/audience/time and revocation, then use
the existing exact issuer/subject mapping from identity PR92 rather than create another registry.
Its production endpoint/service composition must retain deny-by-default behavior for absent,
unsupported and unverifiable providers. Library has no concrete PR92 dependency. Migration0019
belongs here; Discovery0017, identity mapping0018 and Studio OIDC0020 remain independently owned.

Issue #34 has no accepted guest/account merge decision. This backend therefore persists account
state only. Guest storage/import, account-linking and automatic merges require a separate decision.
Logout/login alone never creates history or playback. Client A outbox entries are account/session
bound and cannot be relabeled and sent as B. The server always derives account scope from the
verified principal and checks the actor remains active.

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

## Ordered residual acceptance queue

- **Identity owner:** implement real Theatre issuer verifier and registry adapter, malicious
  issuer/audience/expiry/revocation cases and account-switch lifecycle. No production secret setup
  is part of this campaign.
- **PostgreSQL owner:** execute the seven registered ignored regressions via `cargo xtask verify
  postgres` in an authorized disposable environment. Also add/run representative multi-Work
  keyset traversal and controlled expiry-while-Catalog-fence-waits schedules; compare SQL to the
  pure projection at one committed snapshot. Capture application-role plans and performance below.
- **Web owner:** replace only the existing MVP mock reducers with typed authenticated transport,
  deterministic outbox/reconciliation and actual-engine events; run two-device Save/Unsave races,
  reversed responses, explicit rewind versus delayed write, clear/preservation, A→logout→B→restart,
  unavailable/withdrawn/replaced audio and no-autoplay journeys.
- **CMP owner:** generate/drift-check the same schema version, use stable platform device identity
  and actual native/background engine acknowledgements; run the same cross-client account
  schedules with Web and actual playback, without treating in-memory tests as sync acceptance.
- **Publication/delivery owner:** real fail-closed public object delivery, current access revocation
  and retained exact-release policy. No live provider, paid service or signed-URL fallback here.
- **Contract owner:** explicit OpenAPI/client generation acceptance, optional future bookmark domain,
  and approved receipt/session retention plus any guest import decision. Existing operations are
  retained indefinitely until a safe retry horizon is decided.

## PostgreSQL read/workload inventory

All account reads use one read-only REPEATABLE READ transaction and current account authorization.
Writes use READ COMMITTED with active-actor SHARE lock, one Listener state UPDATE lock, conditional
register writes and atomic receipt/sequence commit. This coarse account lock is deliberately the
first-write/dedup authority; contention must be measured before changing its granularity.

| Read / consumer | Authoritative query and scale | Index / dependencies / consistency |
| --- | --- | --- |
| progress / player/detail | Listener + publication point read | progress PK; actor active; exact scalar/snapshot validation; own state survives access loss |
| Saved current / card/detail | Listener + Work point read, synthetic false revision0 when absent | Saved PK; accepted false records remain; no other library deletion |
| receipt / outbox recovery | Listener + operation point read | operations PK; immutable body/digest; no receipt expiry |
| Saved page / Của tôi | saved=true, order DESC / Work C ASC, limit≤50+1 and frozen frontier | partial recent index; Save/Unsave changes eligibility/order |
| Continue / Của tôi | newest register per Work **before** completion/frontier filtering | Work/recent index; may scan account release history; no precomputed second authority |
| History / Của tôi | latest started entry per Work, newest first, keyset≤50+1 | recent index; playback starts/clear epoch; completed progress retained |
| resume projection / Continue/history | one batch of ≤50 current unit releases; history progress batched by publication | owning Media eligibility path, fresh access clock, Catalog/rights/approval/pointer/actor changes; no per-item SQL |

Expected request frequency, plans, rows/scans/buffers/spills, index/write cost and measured latency
are **unknown**. No DBSP or cache is introduced. **DBSP: defer**, pending a representative authorized
PostgreSQL baseline; account point writes/read-after-write and current access decisions remain
PostgreSQL-authoritative even if an eventual view is evaluated later.

Proposed disposable acceptance workload (targets, not measurements): 100 accounts, 100 Works each,
20 exact releases per Work, 50-item pages, skewed hot accounts, 20 clients / 10 simultaneous active
accounts, 80% reads / 20% writes. Warm API targets p50≤30ms, p95≤100ms, p99≤250ms, 100requests/s;
write p95≤150ms, zero stale/account-crossing results, immediate committed visibility, 2vCPU/2GiB,
no spills. Baseline budget 2minutes and ≤1GiB disposable data; inventory storage/WAL/index overhead
and concurrent-write wait time. Record cold/warm runs separately and capture SELECT
EXPLAIN(ANALYZE,BUFFERS) with the actual app role, parameters and transaction. No speedup is claimed.
No external incremental serving path exists, so CDC lag/rebuild/fallback claims are NOT_APPLICABLE.
