# Current priorities

This is recommended sequencing from the owning contracts and implemented boundaries, not a
live status mirror. Re-read linked issues and current source before work;
open PRs and remaining acceptance can change independently.

## Reuse the implemented foundation

Catalog Work/ContentUnit CRUD, immutable Content, Brief read grants, scoped private Audio
Lifecycle, owner artwork and the bounded Studio consumers exist. [#66](https://github.com/loveoverflowcom/cantos/issues/66)
is closed for Catalog + Studio consumer; remaining QA belongs to #131/#172/#179/#180.
Publication, Discovery, Library/Progress, identity adapters and Firebase Hosting tooling also
have implemented slices. Their presence does not establish provider, playback or deployment acceptance.
See [product status](../product/brief.md#implementation-status) and the [contract index](../../packages/contracts/README.md).

## Database and Hosting transition prerequisites

[#238](https://github.com/loveoverflowcom/cantos/issues/238) removes retired provider runtime
before [#239](https://github.com/loveoverflowcom/cantos/issues/239)'s current fresh schema is used.
Review source cleanup and fresh/refusal SQL evidence together, then qualify the explicit narrow
`--cutover-epoch2` lane against its exact legacy-883e188/28-migration and empty-business guards,
sibling restore/import, frozen-row comparison, off-host recovery and paired rollback boundaries.
Reconcile the six browser inputs and existing OIDC clients through the same operation before
direct Hosting/API acceptance. The [cutover runbook](../deployment/postgres-schema-cutover.md)
preserves PostgreSQL authority and keeps PostgreSQL 17 → 18.6 independent. Populated business/session
data still requires a reviewed general transformer, which remains unimplemented; a fresh source
build does not authorize rewriting migration checksums or bypassing the guards.

[#237](https://github.com/loveoverflowcom/cantos/issues/237) is an independent Hosting track:
first establish project ownership/availability for the requested short Site IDs, then approve
per-app site allocation/deploy and exact-origin acceptance through the
[Hosting runbook](../deployment/firebase-hosting.md) and accepted
[ADR 0041](../decisions/0041-direct-hosting-api-boundary.md). Preserve the direct-Hosting/API
boundary, origin/callback drift safeguards and Studio/Theatre isolation. Short Site IDs alone
establish neither Bearer authentication nor issuer acceptance; #253 removes same-site domains
as an authentication prerequisite. Qualify the minimal identity gateway and exact client/origin
contracts separately. Keep the schema cutover independent of
that rollout.

## Recommended next boundaries

| Order | Issue owner | One next outcome | Dependency / review input |
| --- | --- | --- | --- |
| 1 | [#179](https://github.com/loveoverflowcom/cantos/issues/179), [#180](https://github.com/loveoverflowcom/cantos/issues/180) | Establish the real API/PG/storage/issuer acceptance matrix for the existing Studio → release → Theatre path | [Client matrix](live-client-api-matrix.md); run/readiness code from #183 already exists, integration acceptance remains separate |
| 2 | [#10](https://github.com/loveoverflowcom/cantos/issues/10), [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#11](https://github.com/loveoverflowcom/cantos/issues/11) | Verify private-provider isolation and exact rights/review/publication bindings; preserve old releases on failure | [030](030-mix-review-and-publish.md); [Long Vân blocker](../evidence/longvan-s3-acceptance.md#offline-harness-hardening-and-live-blocker) is evidence to reconcile, not a successful provider gate |
| 3 | [#170](https://github.com/loveoverflowcom/cantos/issues/170), [#120](https://github.com/loveoverflowcom/cantos/issues/120) | Qualify configured filesystem promotion and origin Range/expiry/withdrawal, then hand exact releases to native playback | [ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md), [step-1 handoff](../evidence/theatre-live-catalog-step1.md); unconfigured delivery stays fail-closed and deployment/native acceptance remain separate |
| 4 | [#172](https://github.com/loveoverflowcom/cantos/issues/172) | Review the multi-Work/unit PostgreSQL walk regressions, then accept client recovery against the existing public contract | [State matrix](theatre-catalog-state-handoff.md#postgresql-walk-follow-up), accepted [ADR 0035](../decisions/0035-theatre-catalog-unit-availability.md); local SQL results belong to the PR, Web/CMP behavior and duration-schema tightening remain separate |
| 5 | [#13](https://github.com/loveoverflowcom/cantos/issues/13), [#14](https://github.com/loveoverflowcom/cantos/issues/14), [#158](https://github.com/loveoverflowcom/cantos/issues/158), [#159](https://github.com/loveoverflowcom/cantos/issues/159) | Connect one real discovery → episode → playback slice per client with typed unavailable/error states | #170/#172; [040](040-theatre-web-listening.md), [050](050-cmp-native-listening.md). The discovery → detail → unit-list part exists on Web and the CMP debug host against a [seeded local catalog](../deployment/theatre-seeded-catalog.md); the [step-2 runtime](../evidence/theatre-native-step2.md) adds native/account/Library/SQLDelight integration while preserving the independent [detail-identity guard](../evidence/issue159-cmp-live-catalog.md); next are exact-artifact device/codec/identity/two-client acceptance; follow with #15/#16/#17/#35 background/offline/sync |

For the Web implementation chain ([#269](https://github.com/loveoverflowcom/cantos/issues/269)
player/media origin, [#270](https://github.com/loveoverflowcom/cantos/issues/270)
route/catalog paging, [#271](https://github.com/loveoverflowcom/cantos/issues/271)
identity readiness/presented Library reads, and [#272](https://github.com/loveoverflowcom/cantos/issues/272)
Saved/progress/history writes and paused exact-release resume), the Web core, reducer/outbox
reconciliation and transport slices are implemented; qualify real issuer/signup, PostgreSQL
multi-device races and deployed media/CORS acceptance separately.
Reuse the [Web boundary](../../apps/cantos-theatre/web/README.md#scope-and-honest-behavior) and
[existing enrollment lane](../deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing)
rather than rebuilding authentication or accepting fixtures as live evidence.

Order expresses capability dependencies. The following tracks can proceed independently within
their own decision/acceptance gates; none requires rebuilding the completed foundation.

| Track | Next outcome / gate |
| --- | --- |
| [#242](https://github.com/loveoverflowcom/cantos/issues/242) | Reuse the existing Firebase enrollment owners for isolated local HTTP/PostgreSQL/Auth Emulator coverage and recipient tooling; [local test boundary](../deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing). Complete the separate disposable Kanidm + actual Web setup/OIDC/session/reload/logout lane and the two-browser initiator/recipient regression before accepting the whole issue. Emulator or mock-issuer results do not accept #174/#191/#206 or deployed Hosting cookies |
| [#174](https://github.com/loveoverflowcom/cantos/issues/174), [#191](https://github.com/loveoverflowcom/cantos/issues/191), [#206](https://github.com/loveoverflowcom/cantos/issues/206) | Accept real Theatre login/onboarding while preserving Kanidm and provisioned-only Studio; [identity handoff](theatre-identity-handoff.md). Accepted [ADR 0036](../decisions/0036-firebase-enrollment-proof.md) selects Firebase mailbox proof and one-shot RAM setup for the confirming browser. The [runtime slice](../evidence/firebase-enrollment-runtime.md) provides default-disabled adapters, durable fences and Web continuation; the [live record](../evidence/theatre-live-auth-206.md) observed, on a local Kanidm and the real Firebase project `cantos-a348b`, real mail → Cantos handler → one-shot setup → OIDC login → logout and a refused code replay (interaction-tested; not activation). Activation stays blocked on: a custom action handler (a Spark project refused to set the action URL, so the default hosted handler still receives the mail link and the first code was invalid for an undetermined reason), a least-privilege Firebase signing/creation credential (the API now mints its own bearer from the service-account key, fixture-tested only; the dedicated one-permission account in the [service-account runbook](../deployment/firebase-enrollment-service-account.md) is designed, not created, and the broad default Admin SDK key was used), retirement of that key and remaining test artifacts (the original Firebase test user was deleted; see the enrollment audit), exact project/key association, effective bootstrap/issuer protection, recipient/policy/retention and an operator-accepted Kanidm scope map. [ADR 0040](../decisions/0040-firebase-email-proof-alternatives.md) (proposed) rejects polling/default `VERIFY_EMAIL` as proof; its controlled fresh-mailbox follow-up found `ADMIN_ONLY_OPERATION` for an absent user, then two successful forwarded-code consumptions and refused replays with a dedicated admin-precreated UID and client signup disabled. Next: review an explicit email-link design amendment and qualify its implementation; production registration remains off; the [enrollment audit](../evidence/theatre-enrollment-audit-20261017.md) records the edge hardening (trusted-proxy quota identity, optional registration grants, Kanidm management-API filter, action-code headers). Keep SQL/emulator/browser and live evidence separate; retained-email conflicts or lost handoffs require reconciliation. Preserve #199 origin and UI qualification gates |
| [#203](https://github.com/loveoverflowcom/cantos/issues/203), vision [#82](https://github.com/loveoverflowcom/cantos/issues/82) | Establish trusted owner invocation/access grants and cross-runtime paid-dispatch fencing before enabling the first bounded Gemini/Drive segment. Reuse the relocated Rust producer, local durable attempt adapter and finite worker boundary; [ADR 0022](../decisions/0022-cantos-forge-producer-cli.md) records the accepted Studio tab/appData/service direction. The [worker acceptance boundary](../deployment/forge-worker.md) separates local source from deployed packaging, OAuth, app-data, Gemini/encoder, structured Studio and explicit API handoff. Preserve the [existing attempt assurance](../evidence/forge-attempt-reconciliation.md); a local CAS store or closed issue is not cloud acceptance |
| [#212](https://github.com/loveoverflowcom/cantos/issues/212) | Keep the mechanical infrastructure boundary separate: edge and static Hosting assets live in `infra/`, while `xtask` owns commands; [layout](../architecture/repository-layout.md). Forge source is in `services/cantos-forge-functions`; future `infra/firebase/functions` deploy policy and real runtime acceptance remain distinct from the source relocation under #203 |
| [#199](https://github.com/loveoverflowcom/cantos/issues/199), [#184](https://github.com/loveoverflowcom/cantos/issues/184) | Reconcile actual deployed revisions and qualify direct Hosting → explicit API-origin/CORS/routing under [ADR 0041](../decisions/0041-direct-hosting-api-boundary.md) before real browser authentication. Source safeguards require Hosting's HTTPS API origin and consistent per-surface VPS inputs; qualify deployed security headers separately. Same-site domain/API alias and a default-deny private-Kanidm gateway precede session acceptance; interim management-path blocking is insufficient. Registration activation and Android distribution remain independent. [Current runbook](../deployment/vps-production.md) |
| [#157](https://github.com/loveoverflowcom/cantos/issues/157) → [#171](https://github.com/loveoverflowcom/cantos/issues/171) | Accept exact public-cover policy before projection/delivery; [cover handoff](public-cover-implementation-handoff.md). Private cover and its Studio UI are implemented; #156 is closed |
| [#177](https://github.com/loveoverflowcom/cantos/issues/177), [#178](https://github.com/loveoverflowcom/cantos/issues/178) | Choose a concrete metadata/transcript consumer and accept its release/rights/compatibility rules before schema expansion |
| [#106](https://github.com/loveoverflowcom/cantos/issues/106) | Extend only reviewed [behavior catalog](../../specs/behavior/README.md) capabilities and their implementation observations; preserve draft vs accepted lifecycle |
| [#141](https://github.com/loveoverflowcom/cantos/issues/141), [#142](https://github.com/loveoverflowcom/cantos/issues/142), [#143](https://github.com/loveoverflowcom/cantos/issues/143), [#145](https://github.com/loveoverflowcom/cantos/issues/145), [#167](https://github.com/loveoverflowcom/cantos/issues/167) | Finish bounded preview/runtime/localization decisions using [Leptos](015-leptos-ui-system.md) and [CMP](016-cmp-ui-system.md) inputs; completed #168/#169/#122 slices are foundations, not new tasks |
| [#221](https://github.com/loveoverflowcom/cantos/issues/221) → [#222](https://github.com/loveoverflowcom/cantos/issues/222) → [#223](https://github.com/loveoverflowcom/cantos/issues/223) | Complete the remaining shared design/accessibility/renderer admission. PR #225 is merged; the [Select density continuation](../evidence/web-select-density-221.md) adds Web-only Regular/Compact and a task-owner-authorized locale-topbar migration exception, with acceptance pending and no merge/deploy. Keep Account/Settings theme, [BackButton](../evidence/web-back-button-222.md) and [AuthTonal](../evidence/web-auth-tonal-223.md) adoption gated; inspect actual preference, Back and signup flows independently |
| [#175](https://github.com/loveoverflowcom/cantos/issues/175), [#176](https://github.com/loveoverflowcom/cantos/issues/176) | Decide interest-receipt/privacy and read-only plan publication separately; no inferred payment or entitlement implementation |

For [#251](https://github.com/loveoverflowcom/cantos/issues/251), review the opt-in
`presentation=1` [Library](../../packages/contracts/listener-library-v1.md#presentation-metadata-v110-explicit-opt-in)
and [discovery](../../packages/contracts/theatre-discovery-v1.md#presentation-metadata-v110-explicit-opt-in)
contracts before the separate CMP projection follow-up. Keep default strict v1 consumers on
their unchanged representation; qualify release-pinned row names and current-public counts/ranks
against the PostgreSQL boundary before client adoption. Authored episode labels and seasons stay
with #177; a list rank supplies neither.

For the action-icon and Forge UI track, review the shared registry, Button/submit adapters and
existing previews in [#228](https://github.com/loveoverflowcom/cantos/issues/228) before the
Studio Forge mock [#227](https://github.com/loveoverflowcom/cantos/issues/227), Theatre Web
auth/navigation [#224](https://github.com/loveoverflowcom/cantos/issues/224) and native CMP
actions [#229](https://github.com/loveoverflowcom/cantos/issues/229). Reuse the #225 Select,
BackButton and AuthTonal candidates; their renderer/admission gates still precede adoption.
This track changes presentation and explicit mock composition, not live auth/producer authority.
The [Studio Forge debug mock](../evidence/studio-forge-mock-ui.md) now supplies the isolated
controller/composition review surface. Inspect its real renderer and reference, including
raw-text recovery, exact sync identity and dialog focus, before live-client implementation;
mock receipts do not satisfy the worker's invocation or API-admission gates.
For Web auth, inspect the [phase/action hierarchy](../evidence/theatre-web-auth-actions-224.md)
in the existing no-network auth diagnostic and actual consumer flow. Preserve the distinction
between contextual Back and explicit Firebase cancellation; qualify shared action icons before
moving the diagnostic icon mapping into live consumers.
The [CMP action/Select candidate](../evidence/theatre-cmp-action-icons-229.md) retains native
Material popup ownership and isolates new auth glyphs in the existing sandbox. Complete
both-renderer admission and named Android/iOS accessibility observations before product adoption;
JVM screenshots/tests and a launched Java process do not supply native acceptance.

For localization, review the [baseline-addressed ownership audit](../evidence/i18n-audit-20261017/README.md)
and its bounded guard before extending copy. Reuse the current resource owners and keep
preview tooling separate from Vietnamese specimens. The next architectural outcome remains
the reviewed #167 catalog/runtime decision; renderer builds and ownership repairs do not
accept persisted-locale precedence, arbitrary formatting or native screen-reader/IME behavior.

Do not append completed sessions here. Move their dated handoff to [archive](../archive/README.md)
and leave a short current pointer. Existing source-bound evidence remains at its original path.
