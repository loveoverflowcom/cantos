# Live client capability boundary

Baseline: `origin/develop@7396ede88466ef56dc9e7759956a4072d6672a5b`, inspected routes in
`services/cantos-api/src/interfaces/http/studio/` and raw DTOs in
`packages/contracts/api/src/`. This is an implementation boundary, not publication or user
acceptance. Follow the [audio handoff](audio-lifecycle-frontend-handoff.md).

| Capability / actual method and path (under `/api/v1`) | Consumer / authorization | Wire and media | Boundary |
| --- | --- | --- | --- |
| POST `studio/session` | Studio Web development token exchanged for HttpOnly cookie | SessionRequest/Response JSON | Implemented development identity; no Kanidm |
| GET/POST `studio/works`; GET/PATCH/DELETE `studio/works/{id}`; GET history | Studio Web current cookie and permission; Origin on writes | Catalog DTOs, JSON, private no-store | Actual metadata; visibility never publishes |
| GET/POST `studio/works/{id}/content-units`; POST reorder; GET/PATCH/DELETE `studio/content-units/{id}` | Studio Web cookie and role | Catalog DTOs, expected revisions and operation IDs | Stable chapter/episode identity; bounded opaque cursors |
| GET content head/history/exact revision; POST content revisions under `studio/content-units/{id}` | Studio Web cookie and role | Content DTOs; text/recording/source/Script IR | Immutable ContentRevision; metadata links are independent |
| POST production-briefs; GET production-briefs/{version} under Studio unit | Studio Web owner cookie | Brief DTOs, JSON | Exact copied Content/guidance/profile and freshness |
| POST brief producer-grants; DELETE unit producer-grants/{id} | Studio Web owner cookie | Grant DTOs | Scoped read credential; never upload authority |
| GET `studio/producer/content-units/{id}/production-briefs/{version}` | External producer read bearer; browser credentials omit | ProductionBriefDocument JSON | Separate from owner sessions/upload bearer |
| POST audio-submissions; GET exact submission; DELETE upload-permission under Studio unit | Studio Web owner cookie | AudioLifecycle DTOs JSON | Private, scoped, expiring, revocable admission |
| GET `studio/producer/audio-submissions/{id}`; PUT same path + `/bytes` | Exact producer upload bearer; browser credentials omit | JSON status; bounded octet-stream PUT → JSON | Actual-byte inspection; explicit Opus mono20/24/32/40 profiles |
| POST submission reviews; POST/GET release-candidates | Studio Web owner cookie | Exact digest/checksum and named review DTOs | Private candidates always block public delivery |
| Legacy scripts/revisions/imports/adaptation/production; script audio-intents | Studio Web existing cookie adapters | Existing contracts | Retained; not listener interfaces |
| `/theatre/*`, `/admin/*` | No implemented listener/admin identity | JSON404 not_found | Reserved; no invented endpoints or DTOs |
| Publication/manifest/media GET/range/offline/progress | Theatre Web/CMP | No available contract or delivery URL | Deferred; cannot connect playback to private Studio audio |

## Reviewable client slice

Connect Studio Web to the actual Catalog/Content/Brief/private-audio owner APIs, preserving
typed DTOs, exact replay snapshots, failed-save/error states and cancellation/late-response
fences. Producer upload uses its own bearer with cookies omitted. Do not persist credentials
or introduce backend synthesis, mixing, transcoding, publication or CI.

Theatre Web/CMP gain explicit configuration, transport/error/cancellation seams and tested
unsupported capabilities. The mock remains a separately named preview mode. No listener
endpoint means **no live Theatre integration**; transport checks are not playback evidence.

## Acceptance still owned by the user and future backend slices

User end-to-end acceptance, production identity/credential delivery, object storage,
publication/visibility, delivery range/seek, native background playback and listening quality
remain separate. Contract tests, mocked adapters and real disposable PostgreSQL are agent
checks. Kani may check only named bounded pure Rust invariants, never Kotlin/network/full apps.

## User end-to-end walkthrough

The bounded implementation/evidence is in the [record](../evidence/live-client-integration.md).
On an unlocked Mac with connected browser control, use installed toolchains and the existing
local command surface. Do not substitute a production or inherited database URL.

1. Run `cargo xtask verify postgres --keep`, then
   `CANTOS_TEST_CONTEXT=audio cargo xtask run cantos-studio --web --test-cluster`.
   This selects the runner's fresh retained AudioLifecycle fixture with narrow app-role grants
   and private storage. Use only its synthetic development sign-in token described by the runner.
2. At `http://127.0.0.1:8081`, sign in, choose **Content & audio**, list works and create an
   original synthetic story/podcast plus chapter/episode. Test Vietnamese text, keyboard/focus,
   narrow/wide widths, light/dark and large text. Use synthetic declared rights already present
   in that fixture (`catalog-rights`); declarations establish no production clearance.
3. Read content head to distinguish absent404from a stored revision. Append text/recording,
   read exact/history content; create a brief with an explicit20/24/32/40profile and real
   permitted-use/attribution/QA declarations. Change content afterward and observe stale scope.
4. Select an externally encoded synthetic Ogg Opus file≤32MiB. Prepare and upload the exact
   bytes; inspect Network to confirm producer PUT has Authorization bearer and no Cookie, while
   owner requests retain the Studio cookie. No audio preview is available on this backend.
5. Refresh status; compare producer declarations to verified server media facts. Record named
   QC/rights/editorial decisions only with actual evidence. Change the selected rendition or
   decision/evidence and verify consent is invalidated. Create/read a private candidate and
   confirm `public_delivery_not_implemented` remains a blocker.
6. Test failed/stale/concurrent edits without losing drafts. Interrupt a write, retry its exact
   operation, and reconcile prepared/uploaded receipts after a lost response; mismatches remain
   unknown. Verify navigation/cancellation/newer-response and same/different-actor boundaries.
7. Start Theatre Web separately with `cargo xtask app cantos-theatre --web` while the API runs.
   Default mode checks the real reserved group and stays unsupported. `?mode=mock` is the
   explicitly isolated preview. Native CMP hosts remain mock; no private Studio asset is playable.
8. Stop the xtask-owned processes with Ctrl-C and stop the exact retained cluster using the
   runner's printed command. Record user acceptance separately; do not merge this draft PR.

Next backend prerequisites remain secure listener identity and immutable publication/delivery
before catalog/manifest/playback/progress/download interfaces. iOS concrete transport and native
runtime acceptance then follow that reviewed contract. No backend endpoint is fabricated here.
