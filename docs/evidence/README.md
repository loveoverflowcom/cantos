# Evidence records

This directory contains source-bound observations, test reports, fixtures and assets. It is
**not the work queue or an implementation specification**. Keep existing paths and binary/
JSON fixture bytes stable; age alone is not a reason to delete evidence.

Read the [current product/architecture map](../README.md), [decision lifecycle](../decisions/README.md)
and owning contract first. Then choose the relevant record and check its source revision,
inputs, tool/environment, live-vs-mocked boundary and deferred acceptance. An older FAIL or
NOT_RUN is retained history; an older PASS does not establish current-code or production status.

## Reading paths

| Capability | Records and limits |
| --- | --- |
| Localization integration | [PRs #511/#512/#513 review](localization-review-511-513.md); generator regressions, stacked fix dependency and separate Web/CMP/native acceptance |
| Theatre expressive foundation | [#452 Drive comparison and Web/CMP checks](theatre-expressive-foundation-452.md); opt-in tokens/components, browser and JVM observations; qualification, mobile and downstream app acceptance remain separate |
| Post-merge UI review | [#461/#459/#457 and Select repair](ui-review-20261025-select.md); fresh base, reproduced CMP invalid-perimeter defect, current JVM tests/captures and bounded requalification; product/device acceptance remains separate |
| Sixteen-commit Theatre UI review | [26 October manifest and repairs](ui-sixteen-commit-review-20261026.md); public-cover revalidation/lifetime and Login keyboard regressions, scoped model/mock evidence, with stale admission and runtime acceptance explicit |
| Catalog/Studio consumers | [API](studio-catalog-api.md), [browser follow-up](live-client-browser-acceptance.md), [finding recheck](live-client-finding-fixes.md), [combined CMS](studio-cms-editorial-20261023.md), [bounded navigation follow-on](studio-cms-navigation-20261023.md); #66 completion and remaining QA live in GitHub |
| Brief/private audio | [Brief](production-brief.md), [Audio Lifecycle](audio-lifecycle.md); technical admission is not rights/listening approval |
| Private storage/artwork | [Storage](provider-independent-storage.md), [provider blocker](longvan-s3-acceptance.md), [artwork](private-work-artwork.md), [Studio UI](studio-private-artwork-ui.md); keep real-provider and fixture/browser acceptance separate |
| Publication/delivery | [Core](publication-core.md), [Range transport](playback-range-transport.md), [origin delivery](public-media-delivery-core.md); composition-gated cores are not live playback |
| Theatre catalog integration | [Step-1 production catalog/API/storage](theatre-live-catalog-step1.md); [earlier seeded Web/CMP evidence](theatre-seeded-catalog-integration.md) retains its original scope. Native runtime/playback require separate evidence |
| Identity | [Studio OIDC](studio-oidc.md), [Theatre activation](theatre-auth-activation.md), [Theatre identity](theatre-identity.md), [Firebase runtime](firebase-enrollment-runtime.md), [Theatre live sign-in](theatre-live-auth-206.md); live mail/provider/device gates stay explicit |
| CMP Desktop Preview live Library | [Desktop live acceptance](cmp-desktop-live-acceptance.md); loopback-fixture and real Server/Identity lanes are separate rows, no GUI window content, audio or device is observed |
| CMP registration preview | [#296 shared enrollment](issue296-cmp-enrollment.md); JVM protocol fixtures and native build evidence remain separate from the isolated complete signup journey and device acceptance |
| Behavior | [Quint foundation](quint-behavior-foundation.md), [pure-core observations](quint-code-conformance.md); draft model PASS is not accepted behavior or DB/API/client conformance |
| Historical incremental platform | [DBSP review](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/evidence/dbsp-platform-review.md); retired grouped-count/capture experiment, retained by pinned history for provenance; #384 defers incremental engines and current read work follows SQL/index first |
| Turso query composition | [#414 review](turso-query-composition-review.md); static-shape inventory of Discovery/Catalog SQL, composer options and the kept raw-SQL decision. No planner or performance claim |
| Embedded Turso engine | [Capability report](turso-capability-report.md); probes of the pinned engine on named hosts and filesystems, recorded hazards with their mitigations, dependency review and what remains `NOT_RUN` (macOS, true `ENOSPC`, footprint). Not a schema or a durability claim |
| Develop consistency | [Eight-commit review](develop-eight-commit-review-20261016.md); pinned merge-aware scope, receipt-wait expiry repair and independently reported local gates |
| Native test-VPS cutover | [23 October cutover and Pages record](native-test-vps-cutover-20261023.md); owner-authorized test of the unadmitted image pair, native units, Cloudflare Pages and DNS; no browser/sign-in acceptance and no legacy data migration |
| Post-merge integration | [23 October vertical journeys](post-merge-integration-20261023.md); Studio Identity → Editorial/Catalog write → restart, Theatre/CMP live re-runs, loopback browser only; roster provisioning remains a contract gap |
| Theatre Web keyed rows | [#359 membership/content separation](theatre-keyed-rows-359.md); scoped host, browser identity/focus and separate reactive/DOM counters, with browser reorder and performance acceptance explicit |
| Engineering method | [FCIS/reactivity skills audit](fcis-reactivity-skills-audit.md); baseline-pinned skill findings and runtime follow-up checklist, with no runtime, browser or Kani evidence |
| Localization ownership | [17 October audit](i18n-audit-20261017/README.md); baseline-addressed five-scope inventory, existing-owner repairs, bounded changed-source guard and separate renderer/runtime acceptance |
| Runtime cleanup | [Current/retired SQL caller inventory](legacy-runtime-inventory.md); `a1` routes/DTOs are removed together with their Studio branch, while controlled schema cutover and live acceptance remain separate |
| Deployment tooling | [Historical Hosting tool evidence](issue199-firebase-hosting.md); current topology/operations live only in the [production runbook](../deployment/vps-production.md), never a dated deployment diary |

## Retired backend experiments

[ADR 0018](../decisions/0018-external-audio-production.md) supersedes backend TTS/provider/job
execution. Retained [eSpeak](tts-receipt-inspection.md), [provider prerequisites](tts-provider-prerequisites.md),
[Google adapter](google-tts-adapter.md), [neural research](local-neural-tts-research.md),
[neural samples](local-neural-tts.md) and [DBSP job summary](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/evidence/dbsp-job-summary.md) describe
the original experiments. They do not authorize restoring a worker or claim a live provider.
Forge's producer-side work is independently scoped by #203.

Session-specific planning instructions are in the [archive](../archive/README.md). Record a new
result only for a concrete verification claim with provenance; put routine docs-only check
results in the PR instead of adding another session ledger.

- [Theatre native integration step2](theatre-native-step2.md): production native adapters, account/Library/store/outbox and artifact path; per-layer checks and explicit native/device/identity gaps.
- [Theatre CMP unified application](theatre-cmp-unified-application.md): the demo experience restored on `com.cantos.theatre` over real owners; component-to-port map, deliberate gaps, per-layer checks and screenshots beside the demo.
- [Native test-VPS Resend mail](native-test-vps-resend-mail-20261023.md): Firebase action-URL refusal, SMTP selection, Identity network finding and sending-domain DNS state on the test host; delivery not observed.
