# Live client capability boundary

Original reviews: 2026-10-16 at `25b1c701` and 2026-10-18 at #247 `cc1e56b`.
The current source boundary below supersedes their provider/PG/mock-only descriptions and connects contracts to
consumers and outstanding acceptance; it is not proof that the deployed environment is ready.
[#179](https://github.com/loveoverflowcom/cantos/issues/179) owns real acceptance and
[#180](https://github.com/loveoverflowcom/cantos/issues/180) cross-client parity.

## Studio and producer capabilities

| Capability / contract | Consumer and authorization | Implementation / remaining acceptance |
| --- | --- | --- |
| [Studio identity](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) | Native Identity email/password with a Studio-only cookie, roster and exact origin; development token mode separate | Source and bounded real loopback integration exist; HTTPS/deployed and broader browser acceptance separate. No public Studio signup |
| [Catalog](../../services/cantos-server/contracts/studio/catalog/v1/README.md), [Content](../../services/cantos-server/contracts/studio/content/v1/README.md) | Studio current cookie/role; Origin on writes; exact revision/operation | API and bounded real Studio consumer exist; #66 completed. Remaining responsive/accessibility/live QA is under #131/#179/#180 |
| [Brief](../../services/cantos-server/contracts/studio/briefs/v1/README.md) | Owner prepares exact snapshot; separate producer read bearer with cookies omitted | Implemented instructions, stale scope and revocable reads; not upload authority |
| [Audio Lifecycle](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md) | Owner prepares/reviews; exact producer upload bearer for status and bounded binary PUT | Actual bytes inspected and admitted/finalized with immutable receipt; no separate S3 finalize route inferred |
| [Private content reads](../../services/cantos-server/contracts/studio/private-content-reads/v1/README.md) | Current owner and exact original target; GET/HEAD/Range and refreshed access | Private preview code exists; real chosen-provider isolation remains separate, never listener authority |
| [Work artwork](../../services/cantos-server/contracts/studio/artwork/v1/README.md) | Active owner; exact PNG/JPEG intent/asset; revision-CAS select/replace/clear | API and Studio UI exist; #156 completed. Public display has its separate release/disclosure contract; private artwork alone grants none |
| [Catalog delegation](../../services/cantos-server/contracts/studio/delegation/v1/README.md) | Owner lists/issues/revokes; separate `cantos_dlg_` bearer for closed Work actions | API and Studio owner panel exist. Catalog grants authorize neither Brief nor audio/review/publication |
| [Publication](../../services/cantos-server/contracts/studio/publication/v1/README.md) | Current owner distribution clearance, exact review/approval/readiness | Immutable release/active-pointer core exists; configured real delivery and retained-release acceptance remain gated |

## Theatre capabilities

| Capability / contract | Web / CMP consumer | Boundary and next owner |
| --- | --- | --- |
| [Discovery](../../services/cantos-server/contracts/theatre/discovery/v1/README.md) | Web Khám phá (search by `q`/`kind`; public API content-language support is separate from the retired Web language filter, cursor pagination), Work detail and unit walk read the actual contract: Work reads use `presentation=2` (`unit_count`, public `cover`), unit reads `presentation=1` (`ordinal`); the production Android/iOS catalog hosts read it too. `?mode=mock` is a separate preview compiled only by Theatre Web's opt-in `preview` feature | Work/search/units/manifest APIs exist. Plain `seed theatre` refuses while its old target is absent; the explicit [prepared-sample lane](../deployment/theatre-seeded-catalog.md) imports private Works/chapters/covers and does not publish mock MP3s. The [earlier public seed record](../evidence/theatre-seeded-catalog-integration.md) is historical. #172 owns state/pagination gaps; #158/#159 own the W01/M01–M02 design and remaining states; Android runtime is not yet observed |
| [Identity v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) | Separate Theatre Web cookie and native Bearer; no Studio-cookie reuse | Native signup/password/RPC/session consumers exist; provider login/setup is retired. Browser/device/deployed qualification is separate; [current handoff](theatre-identity-handoff.md) |
| [Library/Progress](../../services/cantos-server/contracts/theatre/library/v1/README.md) | Separate authenticated listener; Web account/live Library synchronization; CMP full Library client and account SQLDelight/outbox composition | Backend and native source exist; full Web/native sync/offline acceptance remains #12/#17/#35 |
| [Release-bound bookmarks](../../services/cantos-server/contracts/theatre/library/v1/README.md#release-bound-bookmarks-v120) | Same listener boundary; CMP player control, player/Library list and open/delete over the existing SQLDelight outbox. Web has no bookmark UI | #299: API, native Listening persistence, sync and CMP consumer exist; old PG migration 0003 is history. Web cookie/native Bearer use Identity v1. Native device/audio and production data conversion/rollout remain separately qualified |
| Public audio bytes | Theatre player only from publication-bound authorization, never private Studio preview | Explicit filesystem promotion + origin-mode server composition under [ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md); [step-1 API/storage evidence](../evidence/theatre-live-catalog-step1.md), #170 deployed delivery, #13 Web and #14 native listening |
| Public Work cover / P1 metadata | Web W01–W03 and CMP M01–M06 | Public cover backend and Web/CMP `presentation=2` consumers exist under ADR 0033; exact release pins and owner disclosure are required. P1 metadata remains #177/#178; bundled mock artwork grants no public field |
| Background/offline/native media | Platform adapters plus durable local state | #15/#16/#17; no native audio or durable production sync from preview reducers or unsigned host builds |

Theatre Web defaults to anonymous live discovery and opens a live Work detail by `work_id`. An API
outage remains an explicit error/unavailable state, not an automatic mock fallback. The production
CMP hosts browse the same catalog without sandbox mapping. Android release accepts a configured
HTTPS origin and iOS has a separate production catalog target. Search/filter, refresh and cursor
recovery use the actual server contract; invalid configuration shows unavailable (CMP defaults to `https://api.cantos.cc`). Step-2 native
streaming, account and Library source is composed into production roots; device playback,
configured login and live Library synchronization remain acceptance gaps. A seeded catalog,
a successful empty Work page or a mounted mock player is not native audio acceptance.

## Local and deployed acceptance

Use the current [Studio runbook](../../apps/cantos-studio/web/README.md),
[Theatre runbook](../../apps/cantos-theatre/web/README.md#build-run-and-checks) and
[xtask contract](../../xtask/README.md) for prerequisites and command semantics. Theatre
`run --web` starts/checks API + frontend; `app --web` reuses an existing API and reports
frontend-only readiness. Launch never provisions identity/storage or creates playable releases.

For a synthetic disposable environment, observe Catalog → Content/Brief → bounded upload →
private preview/review → readiness, with exact retries, stale/expired/revoked/wrong-owner and
lost-response cases. Preserve explicit producer-vs-owner credentials. Public playback requires
its separately accepted configured release-delivery path before Web/native observations.

[Historical client evidence](../evidence/live-client-integration.md),
[browser follow-up](../evidence/live-client-browser-acceptance.md) and
[finding recheck](../evidence/live-client-finding-fixes.md) retain their original scope/outcomes.
Do not copy an early “no preview/no listener API” limitation forward as current implementation.

Static Firebase Hosting is independent from the VPS API. Its [runbook](../deployment/firebase-hosting.md)
keeps deployed-origin/session/playback acceptance separate from bundle deployment. Planned Forge
Functions are a producer execution boundary under #203, not a second authoritative metadata
backend, Hosting or listener capability. The #212 `infra/` asset split does not implement that
consumer or establish deployment acceptance.


## Native step-2 integration

The historical [source and layer ledger](../evidence/theatre-native-step2.md) pins the original #247 (`cc1e56b`)
and separates the native streaming/account/Library/SQLDelight production composition from real
Android/iOS and cross-client acceptance. Current native registration/password/session adapters are described in the CMP README;
offline downloads are not implemented; bookmarks (#299) have source/JVM/loopback evidence, not device evidence. The production QA APK has its
own packaging/distribution channel and is never the demo artifact. #179/#180 own acceptance
coordination; #14/#17/#35/#191/#174/#184 retain their independent gates.
