# Live client capability boundary

Reviewed 2026-10-16 against develop `25b1c701`, with the source-scoped native step-2 update below
on 2026-10-18 over #247 `cc1e56b`. This matrix connects implemented contracts to
consumers and outstanding acceptance; it is not proof that the deployed environment is ready.
[#179](https://github.com/loveoverflowcom/cantos/issues/179) owns real acceptance and
[#180](https://github.com/loveoverflowcom/cantos/issues/180) cross-client parity.

## Studio and producer capabilities

| Capability / contract | Consumer and authorization | Implementation / remaining acceptance |
| --- | --- | --- |
| [Studio identity](../architecture/studio-oidc.md) | Studio-only session; opt-in OIDC with one/two provisioned administrator subjects; development token mode separate | Code exists; live issuer/session journey remains separate. No public Studio signup |
| [Catalog](../../packages/contracts/catalog-v1.md), [Content](../../packages/contracts/content-v1.md) | Studio current cookie/role; Origin on writes; exact revision/operation | API and bounded real Studio consumer exist; #66 completed. Remaining responsive/accessibility/live QA is under #131/#179/#180 |
| [Brief](../../packages/contracts/production-brief-v1.md) | Owner prepares exact snapshot; separate producer read bearer with cookies omitted | Implemented instructions, stale scope and revocable reads; not upload authority |
| [Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md) | Owner prepares/reviews; exact producer upload bearer for status and bounded binary PUT | Actual bytes inspected and admitted/finalized with immutable receipt; no separate S3 finalize route inferred |
| [Private content reads](../../packages/contracts/private-content-reads-v1.md) | Current owner and exact original target; GET/HEAD/Range and refreshed access | Private preview code exists; real chosen-provider isolation remains separate, never listener authority |
| [Work artwork](../../packages/contracts/artwork-v1.md) | Active owner; exact PNG/JPEG intent/asset; revision-CAS select/replace/clear | API and Studio UI exist; #156 completed. Public Work cover remains absent |
| [Catalog delegation](../../packages/contracts/delegation-v1.md) | Owner lists/issues/revokes; separate `cantos_dlg_` bearer for closed Work actions | API and Studio owner panel exist. Catalog grants authorize neither Brief nor audio/review/publication |
| [Publication](../../packages/contracts/publication-v1.md) | Current owner distribution clearance, exact review/approval/readiness | Immutable release/active-pointer core exists; configured real delivery and retained-release acceptance remain gated |

## Theatre capabilities

| Capability / contract | Web / CMP consumer | Boundary and next owner |
| --- | --- | --- |
| [Discovery](../../packages/contracts/theatre-discovery-v1.md) | Web Home, Work detail and unit walk, and the production Android/iOS catalog hosts, read the actual contract; explicit `?mode=mock` is a separate preview | Work/search/units/manifest APIs exist. `cargo xtask seed theatre` publishes a synthetic local catalog ([runbook](../deployment/theatre-seeded-catalog.md), [record](../evidence/theatre-seeded-catalog-integration.md)). #172 owns state/pagination gaps; #158/#159 own the W01/M01–M02 design and remaining states; Android runtime is not yet observed |
| [Theatre identity](../../packages/contracts/theatre-identity-v1.md) | Separate Web BFF and native resource credentials; no Studio-cookie reuse | Backend/Web/native adapters and enrollment state exist. #174/#191 own real login/signup; [current handoff](theatre-identity-handoff.md) |
| [Library/Progress](../../packages/contracts/listener-library-v1.md) | Separate authenticated listener; Web account/readonly Library boundary; CMP full Library client and account SQLDelight/outbox composition | Backend and native source exist; full Web/native sync/offline acceptance remains #12/#17/#35 |
| Public audio bytes | Theatre player only from publication-bound authorization, never private Studio preview | Explicit filesystem promotion + origin-mode server composition under [ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md); [step-1 API/storage evidence](../evidence/theatre-live-catalog-step1.md), #170 deployed delivery, #13 Web and #14 native listening |
| Public Work cover / P1 metadata | Web W01–W03 and CMP M01–M06 | #157 → #171 and #177/#178; no public fields inferred from private artwork or bundled mock images |
| Background/offline/native media | Platform adapters plus durable local state | #15/#16/#17; no native audio or durable production sync from preview reducers or unsigned host builds |

Theatre Web defaults to anonymous live discovery and opens a live Work detail by `work_id`. An API
outage remains an explicit error/unavailable state, not an automatic mock fallback. The production
CMP hosts browse the same catalog without sandbox mapping. Android release accepts a configured
HTTPS origin and iOS has a separate production catalog target. Search/filter, refresh and cursor
recovery use the actual server contract; unconfigured hosts show unavailable. Step-2 native
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

The [source and layer ledger](../evidence/theatre-native-step2.md) pins unmerged #247 (`cc1e56b`)
and separates the native streaming/account/Library/SQLDelight production composition from real
Android/iOS and cross-client acceptance. Default absent native issuer is unavailable; native
registration, bookmarks and offline downloads are not asserted. The production QA APK has its
own packaging/distribution channel and is never the demo artifact. #179/#180 own acceptance
coordination; #14/#17/#35/#191/#174/#184 retain their independent gates.
