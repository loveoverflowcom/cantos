# MVP Work Queue — External Audio Handoff

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/README.md`.
> The original text below preserves its session-specific status and instructions; it is
> not the active queue. Read [current priorities](../../work-plan/current-priorities.md) and GitHub before implementation.

> Scope reset 2026-10-12: [ADR 0018](../../decisions/0018-external-audio-production.md).
> Studio is Web-only and manages novel/story/podcast content and their audio; Theatre is
> the listener Web/CMP client. Backend TTS, provider workers and in-server mixing are removed.
> Older TTS implementation evidence and GitHub issue history remain archival.

## Executable behavior review boundary

[Issue 106](https://github.com/loveoverflowcom/cantos/issues/106) introduces the bounded
[Quint foundation](../../../specs/behavior/README.md) before its separate
[implementation-observation handoff](../handoffs/quint-conformance-handoff.md). Start with the existing
private AudioSubmission core; keep publishing/listening and wider UI/CMP scenarios deferred
until their exact requirements/observations are reviewed. A model PASS does not accept a
product feature or replace the backend sequence below. The
[foundation evidence](../../evidence/quint-behavior-foundation.md) separates model stages;
[PR B evidence](../../evidence/quint-code-conformance.md) records independently observed pure-core
conformance. Application/IO, provider/SQL and shared API/Leptos/CMP runtime remain later
review boundaries, not implied by either gate. [PR 123](https://github.com/loveoverflowcom/cantos/pull/123)'s
pure-core conformance is now included in [PR 117](https://github.com/loveoverflowcom/cantos/pull/117),
which reconciles both slices with latest develop `1c1d2107f1ca1aacac8c66b7b418c1070b35160c`
and preserves existing identity/storage ADRs by assigning Quint ADR 0027. Earlier results
remain historical; fresh rebased gates are recorded separately. Issue 106 remains open.

## Ordered implementation

For [#169](https://github.com/loveoverflowcom/cantos/issues/169), the
[CMP comparison](../../evidence/cmp-localization-spike.md) and
[locale repair ledger](../../evidence/issue169-cmp-locale-acceptance.md) add bounded candidate and
application evidence. Next: review #167's renderer-neutral ADR and target-specific library
compatibility before wide catalog conversion, then complete the explicitly deferred iOS,
input/account, accessibility and formatting acceptance. The saved UI preference is separate
from domain storage; no listener/playback dependency is reordered. GitHub owns execution and
review status, and #169 remains open.

The [15 October 25-commit assurance review](../../evidence/develop-25-assurance-review.md) pins
the reviewed develop window, bounded local-note repair, mocked launcher coverage and a
strengthened but unexecuted listener-order Kani proposition. The sequential second session
repairs Studio artwork recovery/validation/Blob-lifetime and edge SPA cache classification,
and adds Theatre control regressions in the same PR. Next: execute the changed native/Web
tests and exact Kani harness on the pinned toolchain, then the named browser/Caddy acceptance
matrix; source and Python configuration checks do not establish those results. This does not
accept draft Quint models, alter product policy or replace
the backend and runtime acceptance sequence below; GitHub owns PR execution status.

For the Theatre local-development handoff, prioritize
[#183](https://github.com/loveoverflowcom/cantos/issues/183)'s bounded launch/readiness repair
before the real-capability acceptance matrix/runbook in
[#179](https://github.com/loveoverflowcom/cantos/issues/179) and the cross-client mapping in
[#180](https://github.com/loveoverflowcom/cantos/issues/180). This makes a failed API dependency
observable before consumers assess their integration. Then continue the independently gated
Web listening and real public-delivery work in
[#13](https://github.com/loveoverflowcom/cantos/issues/13) and
[#170](https://github.com/loveoverflowcom/cantos/issues/170), retaining #12's listener contract
and #10/#11's storage/publication prerequisites. Startup and an empty anonymous Work page do
not accept those capabilities. The [local-run compatibility/evidence ledger](../../evidence/theatre-local-run.md)
records option A as proposed pending review, the frontend-only migration path and separately
unrun acceptance; GitHub owns implementation and review status. This DX sequence does not
authorize a provider, schema, publication or native playback change.

For [#175](https://github.com/loveoverflowcom/cantos/issues/175), review the
[proposed interest-receipt ADR](../../decisions/0031-membership-interest-receipts.md) and
[source/recovery handoff](../../work-plan/membership-interest-handoff.md) before adding production contract
or persistence. Recommended order is named submitter/contact/privacy/retention/abuse policy
acceptance, shared Web/CMP contract and pure core, real SQL/HTTP recovery/ownership tests, then
client adoption. Existing preview diagnostic privacy hardening is independent; it accepts
neither the recommended signed-in policy nor durable submission. Keep #12 identity and #129/#130
client acceptance separate; this does not reorder media delivery or authorize guest linking.

The optional [Membership plan publication proposal](../../product/membership-plan-publication.md)
for [#176](https://github.com/loveoverflowcom/cantos/issues/176) has a separate recommended
order: product/source decision under #34, accepted versioned contract with Web/CMP consumers,
pure publication rules and source/API acceptance, then each client adapter. Keep plan display
independent of interest receipt delivery and payment/entitlement. No plan DTO, database or
commercial source should precede that decision gate; this does not reorder media/listener work.

The [Work-scoped delegation review](../handoffs/work-scoped-delegation-handoff.md) follows
[issue #113](https://github.com/loveoverflowcom/cantos/issues/113). The
[source-bound RFC](../../architecture/work-scoped-delegation-rfc.md) compared the options; the
owner selected central opaque grants for a first Catalog slice
([ADR 0028](../../decisions/0028-work-scoped-delegation.md), [contract](../../../packages/contracts/delegation-v1.md),
[evidence](../../evidence/work-scoped-delegation-implementation.md)). It does not replace producer
grants. The Studio Web owner panel for listing, issuing and revoking grants is implemented
([evidence](../../evidence/work-scoped-delegation-studio-ui.md)). Next, in order: the Quint
capability for the accepted contract, then one bounded review per further action. None reorders the backend dependencies below.

1. **Catalog / editorial identity:** work types `novel | story | podcast`, stable
   chapter/episode records, optional manuscript text or Script IR, ownership and rights.
   Do not force a script/cast workflow on plain narration or uploaded recordings.
2. **Instructions API:** versioned immutable production brief/skills bundle read by an
   external AI producer, with caller scope, text/rules/target profile and provenance digests.
3. **External audio intake:** scoped producer credentials; private prepare/PUT/finalize;
   idempotency, SHA-256/size/format verification, immutable audio rendition and review record.
4. **Review & release:** creator QC/rights decisions, atomic immutable release manifest, CDN
   and auditable replacements (old release remains playable on failure).
5. **Theatre Web/CMP:** discovery, streaming, background controls, offline downloads and
   synchronized playback progress based only on released media.

The next Catalog acceptance is the [Studio Web slice](../handoffs/011-studio-catalog-api.md) using the
implemented [Catalog JSON API](../../../packages/contracts/catalog-v1.md). The internal foundation
and HTTP evidence are recorded separately in [session 1](../../evidence/studio-catalog-foundation.md)
and [session 2](../../evidence/studio-catalog-api.md). Keep #66 open for responsive and keyboard
Studio Web acceptance; GitHub owns execution status.

The [Content Identity slice](../../decisions/0019-content-identity.md) adds immutable content beneath
Catalog metadata. Its [handoff to Production Brief](../handoffs/content-identity-production-brief-handoff.md)
keeps the next backend boundary narrow: exact content/guidance revision pinning and independent
producer access, before linking intake/publication. Catalog Web acceptance remains separately
scoped; this sequence does not close #66 or #64. The [Brief v1 backend](../../../packages/contracts/production-brief-v1.md)
provides exact snapshots and read grants; [session3 handoff](../handoffs/production-brief-audio-lifecycle-handoff.md)
keeps intake linkage, review/release and later live client integration in separate boundaries.
## Private audio admission and next review boundary

[Audio Lifecycle v1](../../../packages/contracts/audio-lifecycle-v1.md) reuses attributed PR69
intake and binds verified private bytes to exact Content/Brief/profile/rights declarations.
Named append-only reviews and private candidates expose current blockers; public publication
always remains blocked. [Local evidence](../../evidence/audio-lifecycle.md) and the
[frontend handoff](../handoffs/audio-lifecycle-frontend-handoff.md) describe tested boundaries.

[Provider-independent private storage](../../architecture/object-storage.md) adds durable original
target metadata, filesystem/S3 adapters and exact owner-authorized audio preview while retaining
Catalog/Editorial/Ingestion readers. Its [implementation evidence](../../evidence/provider-independent-storage.md)
separates local and mocked results from real-provider acceptance. It does not complete #10's
deployment/delivery acceptance, #11's publication or #12's listener boundary. Changing the
write target moves no existing objects.

The [Long Vân follow-up](../../evidence/longvan-s3-acceptance.md#offline-harness-hardening-and-live-blocker)
keeps provider acceptance blocked after an anonymous cache-HIT read exposed a private-ACL
probe. Offline harness diagnostics and ownership guards do not resolve that deployment
boundary. The next storage-operator step is to correct and reverify private access after
signed reads warm the exact URL; full provider acceptance and the three prepared speech
uploads through Cantos API follow only after that gate. Those audio uploads remain NOT_RUN.

The [latest-develop consistency review](../../evidence/develop-consistency-review-20261015.md)
keeps exact historical-target readiness ahead of conditional private reads and preserves the
transport cache policy through HTTP composition. The independent second review also repairs
Leptos nested supporting-text role boundaries and the PostgreSQL preview helper's stale MIME
cache oracle. Browser/computed-color and real SQL acceptance remain pending; this does not
claim renderer parity, change the provider/publication acceptance order or accept either
proposed ADR.

The private Work artwork slice ([#105](https://github.com/loveoverflowcom/cantos/pull/105))
extends this verified-payload/MIME boundary inside Media and reuses the same storage registry,
retaining the audio envelope and original-target reads. Its additive migration0023 stays
separate from storage migration0015. The [artwork evidence](../../evidence/private-work-artwork.md)
keeps current core/filesystem/controlled-peer checks separate from pending real SQL,
authorized HTTP/restart and provider acceptance. No second S3 adapter or public artwork
projection is needed for that owner-only outcome.

Recommended backend follow-ups preserve that order:

| Order | Capability owner | Outcome and dependency |
| --- | --- | --- |
| 1 | Media/storage; operator deployment boundary (#10) | Accept private policies, conditional writes, version-pinned grants, Range/CORS/expiry and measured capacity on the chosen provider; declarations/mocks cannot establish production behavior |
| 2 | Media review and rights owners (#9) | Bind named listening/rights/QC findings to exact admitted bytes and revisions; format admission is not approval |
| 3 | Publication (#11) | Implement approved immutable manifests and atomic replacement/retention, consuming exact verified locations and current approvals; keep private intake isolated |
| 4 | Listening (#12), then Theatre clients (#13/#14) | Define separate listener identity/access and publication-bound reads before real discovery/playback; never reuse private Studio handlers |

These owners identify architectural responsibility, not assignees. GitHub owns assignment and
execution status; provider/migration/publication acceptance remains separate from this slice.

The [live client boundary](../../work-plan/live-client-api-matrix.md) connects the bounded Studio forms and
keeps Theatre capabilities unsupported until a reviewed listener/publication contract exists.
Remaining Studio controls and responsive/keyboard/user acceptance stay with #66; transport
checks do not complete #51/#64. Next: user acceptance of the bounded Studio flow, production
identity/storage acceptance and stronger rights/QC policy,
then atomic publication/delivery before Theatre playback. Keep broader #51/#64/#66 acceptance
separate. No listening/interoperability is settled by admitting an Opus envelope.

The [PR73 browser follow-up](../../evidence/live-client-browser-acceptance.md) exercised the real
bounded Studio flow and retains its historical partial/FAIL acceptance. The
[bounded correction/recheck record](../../evidence/live-client-finding-fixes.md) owns the later
default-kind, existing-profile session and chip-contrast evidence. Next within #66, prioritize
pending-write/concurrent identity cases, then full accessibility acceptance; keep missing
browser headers and expiry interactions explicit. GitHub retains execution status. The recommended publication/Theatre
backend sequence above is unchanged.

## Publication core and next listener boundary

For proposed public genres, attribution and podcast grouping, review the
[#177 decision matrix](../../work-plan/public-metadata-decision-handoff.md) and
[proposed ADR 0030](../../decisions/0030-release-safe-public-metadata.md) as inputs to
[#157](https://github.com/loveoverflowcom/cantos/issues/157)'s applicable P1 decisions.
Record accepted source/release/rights/consumer and compatibility semantics before adding
fields or migrations. Prefer one accepted genre/filter slice over implementing all proposal
families together; retain existing q/kind/language search, public eligibility and flat chapter
ordering. This is dependency guidance, not acceptance of P1 or a change to publication order.
Keep the independent [existing Discovery v1 continuation repair](../../work-plan/public-metadata-decision-handoff.md#existing-discovery-v1-continuation-repair)
and its mocked application evidence separate from later real PostgreSQL expiry/pagination
and Web/CMP consumer acceptance. An existing-contract correction does not open the P1 gate.

[Publication v1](../../../packages/contracts/publication-v1.md) implements owner-authored distribution
clearance, exact approval, all readiness blockers, immutable releases, atomic active pointers and
minimal withdrawal over the existing candidate/intake path. [Verification](../../evidence/publication-core.md)
separates passing core/Router/proof tests from failed PostgreSQL startup and unrun delivery.
Unconfigured delivery cannot activate. [Discovery/Playback then Library/Progress handoff](../handoffs/publication-discovery-handoff.md)
defines the next two separately reviewed slices and the independent
[private storage PR90](https://github.com/loveoverflowcom/cantos/pull/90) integration prerequisite.
Keep #9/#10/#11 open through real storage, SQL/concurrency, listening/rights and public acceptance;
Studio audit manifests do not themselves authorize Theatre playback.

The bounded [Range transport prototype](../../evidence/playback-range-transport.md) links #120's
contract-fixture groundwork to that same sequence. Before connecting live Theatre audio, accept
#10/#11's publication-class object isolation, version pinning and per-fetch expiry/withdrawal
checks under the [proposed delivery decision gates](../../decisions/0038-publication-streaming-evidence-gates.md).
Shared Studio transport and mocked publication bytes do not replace those dependencies.

For [#170](https://github.com/loveoverflowcom/cantos/issues/170), review
[proposed ADR 0032](../../decisions/0032-theatre-origin-playback-delivery.md) (origin-mode Range
with sealed 120 s grants). Its pure grant kernel, origin adapter, `/theatre/media/{grant}` route
and mock-integrated fixture exist behind trusted composition that no host performs; see the
[core evidence ledger](../../evidence/public-media-delivery-core.md). Recommended order: owner
acceptance of ADR 0032, then #10/#11's promoted publication-class `PublicationObjects` adapter,
then real-bytes/expiry/withdrawal/key-rotation/access-log acceptance through the deployed proxy,
then #13/#14 player integration of the documented refresh contract. For
[#171](https://github.com/loveoverflowcom/cantos/issues/171), the release-pinned cover kernel and
[proposed ADR 0033](../../decisions/0033-release-pinned-public-cover.md) are inputs to #157's P0
decision only; no DTO, storage or delivery work starts before #157 accepts a contract.
The [cover implementation handoff](../../work-plan/public-cover-implementation-handoff.md) maps the existing
seams, required owner decision receipt and subsequent SQL/storage/Web/CMP acceptance. Use that
receipt before adding the public projection; private cover admission and audio delivery policy
cannot supply it. This artwork gate does not require accepting unrelated P1 metadata families.
Both draft Quint capabilities are model-only and the three new Kani kernels are registered but
not yet run. GitHub owns execution status.

## Compiler compatibility review boundary

The [production Rust pin](../../architecture/rust-toolchain.md) keeps edition 2021 and locked
dependencies stable. Complete platform/tooling acceptance independently of the optional stable-API
candidates in [issue 125](https://github.com/loveoverflowcom/cantos/issues/125). Prioritize a
measured consumer and its semantic oracle before Range formatting, Ogg parsing or syntax changes;
retain independent Kani/Quint results and the existing publication/storage gates. This does not
reorder the backend capabilities above or revive DBSP production jobs.

## Auth/account shared-foundation sequencing

[Issue 127](https://github.com/loveoverflowcom/cantos/issues/127) adds the scoped
[auth/account v0.4.0 contract](../../../packages/ui-system/shared/README.md#scoped-authaccount-mapping-v040)
and official local Logo Kit to both shared renderers, before app-level adoption. Its
[evidence ledger](../../evidence/ui-auth-account-v2.md) keeps implementation tests separate from
missing runtime/design admission. The six Membership APIs now have the bounded admission
record below; other primitives/variants and native acceptance remain pending.
PR 132's 15 October follow-up repairs Web auth press interpolation and narrow large-text
layout; real Chrome interaction, both themes and 320 px/200% text now have local evidence.
CMP desktop motion tests and the SDK 35 preview build also pass. Prioritize the remaining
Android host/device accessibility and both-renderer qualification before consumer adoption;
the shared-foundation ordering is unchanged.

Recommended order stays small and reviewable:

1. Complete fresh Leptos and CMP runtime/accessibility/visual admission for the exact shared
   token/source revision. Do not accept a screenshot from the HTML prototype or a build instead.
2. #128's [Membership interest boundary](../../product/membership-interest.md) permits pure
   form/receipt/plan state and explicitly labeled opt-in Web/CMP previews while production
   submission remains unavailable. [Preview evidence](../../evidence/theatre-membership-preview.md)
   separates fixtures and renderer observations from actual receipt/API acceptance. Shared
   variants remain preview-only until admission; no prices, entitlements or checkout are invented.
   PR 137's resumed review repairs reopened plan-selector state and adds repeated IME Done
   coverage. Fresh Rust/Web/CMP and browser observations are recorded separately from recovered
   disk/Python failures, the unavailable media-tool contract gate and native/production acceptance.
   Its [CMP design follow-up](../../evidence/pr137-cmp-design-v2.md) replaces local visual wrappers
   with six admitted shared APIs, adds scoped token v0.5.0 and external tonal-field labels,
   and retains the unavailable/fixture boundary. Fresh real Compose images are delivered on
   the owner's Desktop. Android APK compilation is separate from the unavailable IDE capture
   and unrun native/reader acceptance. Complete the recorded production/native gaps; earlier
   PASS results never accept different follow-up source.
3. #129's [Web Home/Auth/Account boundary](../../evidence/theatre-web-auth-account.md) applies the
   admitted shared variants to anonymous Home and explicit contextual auth, while retaining
   #35's two primary destinations and avatar Account entry. [ADR 0034](../../decisions/0034-theatre-signup-and-provisioned-studio.md)
   accepts listener self-registration; keep current unsupported provider signup explicitly
   unavailable until its exact mechanism and return contract are implemented and accepted.
   A prototype form or route is no registration receipt. Prioritize the recorded provider-return,
   cancellation, browser history and focus evidence before broadening this Web slice. #130 native adoption remains independent
   of this browser lifecycle and sandbox #122; its first [guest-first CMP slice](../../evidence/theatre-cmp-auth-account.md)
   composes the same contextual Back and fenced completion semantics over the existing native
   account owner, with provider, device and assistive-technology acceptance still unrun.
4. #131's [Studio shell slice](../../evidence/studio-login-dashboard.md) adopts login-only
   entry and a bounded Bàn viết projection over the existing authorized workspace. Apply
   ADR 0034's one or two securely provisioned Studio administrator accounts through server-side
   admission; Theatre signup cannot grant this authority. Keep live
   issuer/group denial, full browser/assistive-technology acceptance and exact artwork fidelity
   separate from synthetic-HTTP UI evidence. The next review should finish those acceptance
   gaps before expanding dashboard summaries or changing the editor/data owners.

This does not reorder publication/storage acceptance, change auth grants, or close consumer
issues. GitHub owns execution status; the queue records only dependency order and rationale.

After the bounded #128 preview, prioritize #12/#34 decisions on submitter/contact ownership,
receipt persistence and reconciliation, and accepted plan data before a production Membership
adapter. The #129 Web boundary preserves unavailable Membership information without importing
preview receipt/plan fixtures into production; live-provider acceptance, fresh component admission
and #130 native return handling retain their separate review boundaries. Independent review of
the preview remains required before relying on its implementation evidence.

## Existing foundations to retain

The [Leptos v3 workbench](../../evidence/leptos-preview-v3.md) and
[complete design inventory](../../evidence/ui-design-inventory-v3.md) retain the existing API/state
baseline and distinguish proposed anatomy from renderer admission. For #141, prioritize bounded
shared controls/layout batches with a concrete consumer, then Theatre cover/episode/player
primitives, then Web-owned Studio tables/editor/upload/QC/access compositions. Each batch follows
lookup → reuse/extend → declare and preview → state/theme/motion evidence → admission → product
adoption. The [#142 scenario workbench](../superseded-plans/015-leptos-ui-system.md#reproducible-leptos-preview-scenarios)
adds exact replay for existing controls; next, use its
[recipe contract](../../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay) and
[browser evidence](../../evidence/leptos-preview-scenarios.md) to close the affected runtime gaps
before expanding proposal batches. Keep #129/#131 product rollout and #140 pixel comparison
separate; local/JVM evidence does not complete native or screen-reader acceptance.
The CMP counterpart, [#143](https://github.com/loveoverflowcom/cantos/issues/143), reuses that
inventory and token reconciliation: its [first slice](../../evidence/issue143-cmp-preview.md) adds the
gallery's Design v3 tab, root-injected Light/Dark/System, named dp evidence constraints and capture
records; its [order for the remaining batches](../superseded-plans/016-cmp-ui-system.md#design-v3-cmp-slice-issue-143)
starts with Theatre compositions over registered primitives.
The [#144 CMP recipe workbench](../../../packages/ui-system/cmp/README.md#scenario-recipes-and-replay)
and [Theatre desktop scenarios](../../../apps/cantos-theatre/mobile/README.md#scenario-workbench)
make selected component/state or mock composition, synthetic session seed and ordered intents
reproducible through reset/recreation and versioned snapshots. Use those recipes to close the
affected renderer/state/lifecycle gaps before expanding #143's proposal batches. Preserve #109's
memory-session semantics; replay does not implement durable storage, native audio or OS process
death. The [#122 module extraction](../../evidence/issue122-cmp-sandbox-isolation.md) keeps these
diagnostics in `:preview-sandbox`, which consumes the neutral UI and owners in `:shared`.
The production Android host now composes `:shared` only; `cargo xtask theatre-host
android-release` inspects its release compile/runtime graphs and APK/AAB classes, resources and
manifest for sandbox exclusion. Rerun it when #14 adds production adapters, permissions, signing
or R8. The demo host cannot supply that acceptance. Preserve #109's storage ordering/fault/reset contracts,
#144's replay/lifecycle semantics and #130's guest/account boundary while composing production
adapters; add storage libraries only with #16/#17's consumers and reviewed account policy.
This orchestration slice qualifies no shared API and does not reorder the backend queue;
GitHub owns implementation and acceptance status.

The bounded [Web Back presentation comparison](../../evidence/shared-back-v3-alignment.md) keeps
blocked Back labels quiet while retaining readable disabled semantics. Its before/after pixel
regions use unreviewed regression snapshots. Next, approve an exact Web disabled-state design
baseline and complete its attributed recheck before expanding visual-fidelity claims; this does
not admit a new shared component or change CMP/product rollout order.

The [local visual-comparison slice](../../evidence/visual-compare.md) supplies advisory pixel
metrics/regions through the [existing UI method](../../../.agents/skills/cantos-ui-design/references/visual-review.md#10-reference-pairing-and-local-comparison).
Recommended next review order: version/approve exact platform design baselines, calibrate real
cross-renderer fonts/noise on those pairs, then consider optional capture integration or a
component-specific gate. The short calibration and metric PASS do not qualify a shared component
or reorder publication/native acceptance. Keep stale preview qualification separate from QA
tooling; GitHub #140 owns execution status and ADR 0029 remains a proposed local tooling choice.

The [internal actor registry](../../architecture/identity-registry.md) keeps legacy ownership intact
and adds exact external keys with explicit operator registration/linking. The bounded
[Studio OIDC](../../architecture/studio-oidc.md) slice retains its signed-mock/local-SQL evidence and
adds actual-cookie confirmation and mode-specific login/logout controls to Studio Web. The independent identity review fixed lock-wait expiry, OAuth Basic encoding and shared-cookie
account/cancellation races. Studio now has [current integrated evidence](../../evidence/studio-oidc.md#independent-integrated-acceptance-13-october-2026):
all17 local preflight gates PASS and11 actual browser observations PASS, with remaining runtime
accessibility/provider gates explicit.

[ADR 0025](../../decisions/0025-actor-identity-root.md) (proposed) roots owner and listener identity
in the registry `ActorId`. The listener slice is implemented: `ListenerId` wraps `ActorId`, not
Catalog's `OwnerId`, with no schema or wire change. The two Studio slices (owner as
`OwnerId(ActorId)`, acting-actor fields as `ActorId`) are recommended as separate cohesive
refactors before new Studio authorization work, because they only change Rust signatures.

The following Theatre ledger describes the **13 October snapshot**, superseded by the
[resumed native appendix](#resumed-native-identity-source-and-next-acceptance--14-october-2026).
The [separate Theatre identity handoff](../handoffs/theatre-identity-2026-10-16.md#independent-integrated-acceptance-handoff--13-october-2026)
now records the mounted readonly Web account/Library consumer and native exchange/resource/
registry/session implementation. Its [13 October integrated evidence](../../evidence/theatre-identity.md#independent-integrated-acceptance--13-october-2026)
records fourth full preflight 16 PASS/PostgreSQL FAIL, corrected full PostgreSQL PASS and final
8 selected preflight PASS with no skips; this is not one 17-green invocation. Actual Web runtime
is 14 PASS/0 FAIL/2 INCONCLUSIVE/7 NOT_RUN with 12 inspected PNGs. Android mock compile PASS
did not activate the then-held manifest/host source; PR101 stayed draft pending its direct human
exception in that snapshot. iOS is UNSUPPORTED without required Kotlin/Native 2.2.20. The Android
source step is now resolved as recorded below; named
OS/provider/secure-store/accessibility gates remain required before production activation
or broader playback/synchronization acceptance. Registry lookup grants no surface authority;
explicit client linking and operator-prepared live provider/client/TLS acceptance remain separate.
GitHub owns execution status; that historical evidence manifest is SHA256
`e507afad65f29c01f9959b36f52de564171cce26ea0de27bda5e80fa2c216591`.

The [domain assurance hardening](../../assurance/domain-invariants.md) strengthens
existing six-crate boundaries. [PR75](https://github.com/loveoverflowcom/cantos/pull/75)
merged ordered steps 1–4; the user-approved follow-on
[PR81](https://github.com/loveoverflowcom/cantos/pull/81) delivers 5–11: actual
editorial/production/media/incremental consumers, typed client state and the
opt-in three-kernel verifier workflow. The [proof record](../../assurance/kani.md)
binds actual execution to its versions/inputs/bounds; current skills describe only
implemented rules. Adequate audited boundaries were retained without empty commits
or speculative wrappers. GitHub owns publication/review status.

Implementation delivery and named core evidence do not complete broader Studio
acceptance, rights/QC, publication or listener implementation. Real PostgreSQL
acceptance in that earlier assurance record remains unrun after denied disposable startup;
its full Rust gate fails in upstream service fixtures, with the original port flake retained
separately. Later storage/read verification belongs to its own
[evidence record](../../evidence/provider-independent-storage.md).
Next owners must resolve/recheck those environments without weakening private-input
guards, then perform the existing browser/IME/cookie/conflict and listener acceptance.
No broader product feature or CMP refactor is introduced by this assurance work.

Script IR validation/revision persistence, manuscript import, host-generated adaptation
proposal review, production metadata/rights, pure WAV inspector, shared UI system,
MessagePack-capable editorial routes, tests and `cargo xtask`. The earlier editorial endpoints
are separate from the implemented Catalog API and general external-producer handoff proposal.
The narrow development intake is described above. Prior worker/Google/eSpeak/VieNeu samples and
DBSP job views are historical experiments, not prerequisites for the new roadmap.

## Producer-side Cantos Forge

[Issue #82](https://github.com/loveoverflowcom/cantos/issues/82) and the proposed
[ADR 0022](../../decisions/0022-cantos-forge-producer-cli.md) add a producer-side CLI outside the
API runtime. Its [foundation](../../../services/cantos-forge-functions/README.md) holds the crate, xtask build, the
tested pure core and a host-assisted local workflow (workspace, register, validate, export a
local bundle). Recommended next: instruction bundles and AI/human review evidence commands, then
provider orchestration and the authorized handoff as separate boundaries. None of this changes the
API sequence above. GitHub owns status.

## Canonical work plans

- [010 — source/script import and editor](010-import-and-edit-script.md) — completed portions retained.
- [020 — external production instructions and audio handoff](../../work-plan/020-cast-and-generate-dialogue.md).
- [030 — external master intake, QC and release](../../work-plan/030-mix-review-and-publish.md).
- [040 — Theatre Web](../../work-plan/040-theatre-web-listening.md). The session-only
  [Web mock](../../evidence/theatre-web-mock.md) and [PR 02 local/browser validation](../../evidence/theatre-web-pr02.md)
  establish the mock foundation only, including shared Web control-label alignment. Retain it while implementing publication and the authorized
  listener API first; neither completes 040 nor its issues.
  The [#35 Web library MVP mock](../../evidence/theatre-web-library-mvp.md) provides a bounded UX
  review step: Khám phá/Của tôi navigation, direct Continue/Saved/History sections and persistent
  mock mini-player. The [library/account v2 visual pass](../../evidence/theatre-web-library-account-v2.md)
  reworks those two screens (cluster, status rows, bundled fonts/mock artwork) without new product
  scope; its Drive reference comparison and Avatar qualification remain open. Retain publication,
  listener identity/API and real playback/synchronization prerequisites for production acceptance.
  The mock does not settle the broader product/backend decisions tracked by
  [#34](https://github.com/loveoverflowcom/cantos/issues/34) or close #35.
- [050 — native Theatre Mobile](../../work-plan/050-cmp-native-listening.md). The memory-only
  [CMP mock](../../../apps/cantos-theatre/mobile/README.md) completes neither 050 nor its issues.
  The [#35 CMP library MVP mock](../../evidence/theatre-cmp-library-mvp.md) follows the Web
  presentation with two primary destinations and direct Continue/Saved/History sections.
  The [reference UI repair](../../evidence/theatre-cmp-ui-repair.md) restores artwork/font anatomy
  and keeps large-text controls and preference dialogs within that same isolated mock boundary.
  The [radius normalization](../../evidence/theatre-cmp-shapes.md) keeps the same mock anatomy
  while sharing shape roles with the existing design system.
  The [mock storage simulator](../../../apps/cantos-theatre/mobile/README.md#mock-storage-simulator)
  ([#109](https://github.com/loveoverflowcom/cantos/issues/109)) exercises shared-session
  load/store/recreate, latency, faults and reset fencing before the local metadata store ADR;
  it is memory-only and supplies no durable persistence or process-death evidence. Its database
  and presets form one [mock app sandbox](../../../apps/cantos-theatre/mobile/docs/architecture.md#mock-app-sandbox)
  in `:preview-sandbox`, replacing the separate memory progress adapter. Its composition
  factories are explicit; the production-safe `:shared` module has no mock fallback.
  [ADR 0026](../../decisions/0026-theatre-mobile-local-storage-sandbox.md) (proposed) records the
  separate KMP build boundary, per-layer storage test doubles and adoption gates. The
  adapter-agnostic `ProgressPort` contract and its mock bindings stay together in sandbox tests;
  core-only tests stay in `:shared` to avoid a reverse test dependency. Okio/SQLDelight-style
  dependencies wait for their #16/#17 consumers. The production Android host and its
  release graph/APK/AAB inspection establish #122's exclusion for the unsigned, unshrunk
  release; the host deliberately renders an unavailable state until #14's adapters exist.
  Review common/JVM behavior separately from Android/iOS runtime; implement the authorized
  listener/publication and account contracts before durable sync or production acceptance.

The [CMP engineering skills audit](../../evidence/cmp-engineering-skills.md) strengthens typed
functional Kotlin, layer/ownership review and reproducible verification guidance. This is
documentation maintenance; it changes neither the recommended implementation order nor 050's
native acceptance status.
- [GitHub execution map](../../work-plan/project-planning.md) and [audio drama craft](../../work-plan/audio-drama-quality.md)
  support these, without authorizing TTS inside Cantos.

## Shared completion evidence

Every PR records the exact baseline, files and contracts changed, commands actually run,
test outputs with `PASS`, `FAIL`, `NOT_RUN` or `UNSUPPORTED`, mock/live distinctions,
unresolved blockers and compatibility impact. Do **not** require GitHub CI to finish before
local verification-based handoff; do not claim local tests ran if this environment cannot run
them. For audio, `audio-measured`, `audio-listened` and `uploaded-and-object-verified`
are separate evidence levels.

A missing upload/storage/identity backend is **not** satisfied by a schema-only proposal.
Acceptance requires real, authorized upload, retry, wrong-scope, tampering and publication
failure tests.

## Local service runtime support

[Draft PR77](https://github.com/loveoverflowcom/cantos/pull/77) owns the Podman-first runtime
change ([ADR 0021](../../decisions/0021-podman-service-runtime.md),
[evidence](../../evidence/podman-service-runtime.md)); GitHub owns execution status. It adds a
bounded local infrastructure option while preserving native verification. It does not move the
product queue: finish live client acceptance, then reviewed identity/storage and stronger QC,
then publication before Theatre playback. Next runtime acceptance needs an operator-prepared
rootless Podman/Machine environment, compatible `nocreate`, private TLS and disposable fixture
lifecycle/persistence evidence. Production backups/upgrades and live Kanidm OIDC acceptance remain separately
scoped work; unavailable runtime evidence is not a blocker to reviewing this draft.

## Public discovery before account-owned listening state

The [Discovery/Playback backend contract](../../../packages/contracts/theatre-discovery-v1.md)
adds a current-public projection, stable opaque keysets and explicit unavailable media grants.
Keep Publication/delivery/SQL acceptance and the isolated Web/CMP mock-to-live adapter work
separate. The [Library/Progress handoff](../handoffs/discovery-library-handoff.md) requires the coordinated
typed listener identity boundary, conditional/idempotent writes and removal-safe progress.
No listener registry or native playback is inferred from anonymous catalog reads.

## Listener Library / Progress successor

The [account Library/Progress v1](../../../packages/contracts/listener-library-v1.md) and
[identity/Web/CMP handoff](../../work-plan/listener-library-progress-handoff.md) follow inherited draft PR93.
The [evidence record](../../evidence/listener-library-progress.md) distinguishes pure/use-case/Router
checks from unrun real PostgreSQL and issuer/client playback acceptance. Keep MVP Khám phá | Của tôi;
no automatic guest merge or reference-v2 expansion. Independent identity and public delivery
remain fail-closed dependencies, and this draft does not complete the broad listening issues.

## Web performance measurement

The independent [Web performance pattern audit](../../evidence/web-performance-patterns.md)
([#146](https://github.com/loveoverflowcom/cantos/issues/146)) adds a release asset inventory and
bounded mock-player/library reactive projections. Next, establish the actual static-host topology
and cold/warm browser baseline before cache-header activation; retain old assets through deploy
and rollback. Profile the Studio editor before changing its projection strategy, and keep
SSR/Islands/lazy splitting as compatibility experiments with their own adoption evidence.
This work does not reorder identity, storage, publication or listener acceptance.

For [#154](https://github.com/loveoverflowcom/cantos/issues/154), keep the
[Web player interaction boundary](../../evidence/theatre-web-player-interaction.md) independent
of live delivery: prioritize the native-range draft/commit contract and root-session continuity,
then paired browser interaction and accessibility observations. Measure transport/timeline
consumers before further scheduling or throttling changes. Complete Chromium/Firefox/Safari
input, zoom, focus and resize-observer cleanup evidence before accepting the UI; real audio,
Range, expiry recovery and preload comparisons still follow #13/#120. Host work counts and
asset inventories do not satisfy either runtime gate or reorder the publication dependencies.

## Versioned binary transport pilot

[Issue #97](https://github.com/loveoverflowcom/cantos/issues/97) delivers the bounded Studio
Protobuf pilot and portable CMP fixture harness through one branch/PR.
[The migration ledger](../../evidence/protobuf-wire-migration.md) separates pure-core/router/JVM
evidence from deferred real PostgreSQL, browser save/reload/rollback, Android/iOS and performance
acceptance. Keep JSON default, preserve legacy MessagePack compatibility, and finish those
activation gates before expanding the wire contract. This does not reorder publication/listener
work or change identity, rights, audio or storage scope. GitHub owns execution status.


## Identity integration order

Consume the [exact identity registry](../../architecture/identity-registry.md) before credential
issuance, then integrate Studio OIDC, Theatre Web/native identity and Admin authorization in that
order. Preserve the [handoff](../handoffs/identity-authentication-handoff.md#integrated-registry-consumption)
and additive migration checksums. Current local/mock acceptance belongs in evidence records;
production provider/client preparation, native runtime acceptance and explicit cross-client
identity linking remain separate prerequisites. Equal email or subject text across different
issuers never establishes a shared actor. GitHub owns execution status.

## Private Work cover API/DB acceptance

The user-requested [issue104 MVP plan](012-private-work-artwork.md) selects one durable,
owner-only Work cover and exact private preview from the
[checked standards reference](catalog-media-standards-reference.md). Its
[new-session implementation handoff](../handoffs/catalog-media-mvp-ultra-handoff.md) permits core rules and
new opt-in metadata contracts first; real storage integration reuses the final independently
reviewed PR90 boundary and coordinated additive migration 0023. No product implementation is
claimed by these planning docs. The scope is API/DB, not Studio UI or public artwork delivery;
P1 metadata, thumbnails and transcripts are explicitly deferred. This is an authorized narrow
extension of the wire pilot, preserving JSON default and its outstanding acceptance gates,
without reordering identity/storage or broader publication/listener acceptance.

The [Studio private artwork control](../../evidence/studio-private-artwork-ui.md) consumes that
contract for owner prepare/upload/admission, CAS selection and explicit clear. Prioritize
actual owner/rights/storage/restart acceptance next; browser HTTP doubles and admitted private
images do not satisfy public artwork or publication delivery. The Studio UI introduces no
new backend dependency and does not reorder the broader queue; GitHub owns execution status.

The [implemented private artwork contract](../../../packages/contracts/artwork-v1.md) now reuses
the reviewed private-storage boundary. The next scoped acceptance is disposable PostgreSQL
upgrade/constraints/grants, actual owner HTTP/rights/session behavior, read plans and host/PG
restart faults. The [dated evidence ledger](../../evidence/private-work-artwork.md) separates those
remaining gates from current core, decoder, filesystem and controlled-peer checks.


The identity campaign's PR3 [Theatre identity handoff](../../work-plan/theatre-identity-handoff.md) follows the
reviewed registry/Studio dependencies and the merged Listener Library seam. Preserve the latest
[Studio integrated acceptance](../../evidence/studio-oidc.md#independent-integrated-acceptance-13-october-2026)
and its HTTP negotiation/session fences while integrating Theatre's separate Web BFF, account
fencing and native public-client exchange/resource IO in migration0024. In the 13 October
snapshot, combined Theatre
verification was pending and Android INTERNET/callback registration and host mounting were
unapplied after automatic approval review rejected the source activation. The direct human
exception and those source steps are now resolved in the resumed appendix below.
Provider/device/secure-store and native playback acceptance remain separate.
The [Admin0022 slice and handoff](../handoffs/admin-authorization-handoff.md) adds dedicated resource-token
verification, current role decisions, Admin-only access revocation and append-only audit in PR4.
Its fresh combined verification follows the reviewed develop98f247e/Theatre/storage/IDE merge;
this queue does not claim campaign completion, live-provider/native playback acceptance or a
guest-import policy. GitHub owns execution/merge status.

## Resumed native identity source and next acceptance — 14 October 2026

The [resumed Theatre handoff](../handoffs/theatre-identity-2026-10-16.md#resumed-native-identity-handoff--14-october-2026)
and [fresh evidence appendix](../../evidence/theatre-identity.md#resumed-native-identity-acceptance--14-october-2026)
supersede the previously held Android source exception. The user authorized the exact
INTERNET/callback/Application/MainActivity source patch; it is applied and the actual opt-in
host builds. Common/JVM verification passed 207 tests (28 identity) with zero failures/errors/skips;
the same new
regressions failed against the preserved old cleanup controller before hash-restored repair.
The source manifest contract passed 2 tests and packaged-manifest inspection confirms the
permission, callback and `singleTask` mount. Android is **PASS — Theatre mock Android build**;
iOS remains **UNSUPPORTED** for unavailable canonical Kotlin/Native 2.2.20. Final resumed local
preflight is **PASS: all 17 gates, no skips**, including 331 repository tests and mutation
9 killed/0 survived. PostgreSQL is **PASS**: 16 Theatre Web, 7 native and 18 Studio OIDC tests plus
all inherited HTTP/process-kill/PostgreSQL-restart/migration/replay/codec/storage/publication
suites, on source
`1e2adae537d6b94ee824a5eb8f290a9dbdfa170a`/tree `ab556046cc5922d933c8792c6e7a037b2d595ec2`; retained manifest SHA256
`ab3f1270317a1db8b6f7640a6df6db048070c8dc92d03135554392b38bb87f67`. The previous 13 October Web/runtime counts and manifest
remain historical, not a new native acceptance record.

The reviewed Theatre identity snapshot inherited storage migration0015 from develop and retained
its then-sorted 23-entry registry 1–21,24,25 without editing historical bytes. This private Work
artwork branch additionally preserves coordinated migration0023 and the final Admin merge's
migration0022; the combined registry is now 25 entries, 1–25. Failed persistence/unknown revoke
retains the exact native credential for acknowledged retry; pending/unavailable denial with
lost/corrupt credentials remains closed and controlled recovery is still unassigned. No live
provider, app install or OS/TLS/credential configuration was performed. Next recommended order
is named device/system-browser/Keystore/Keychain/process-lifecycle/accessibility acceptance,
then authorized provider/public-client/current-rights acceptance and reviewed recovery/at-rest/
backup/retention. Native playback, durable Library/outbox, guest import, explicit cross-client
linking and broader synchronization keep their separate owners. Production stays disabled;
GitHub owns PR execution/merge status and broader issues remain open.

## Theatre mobile architecture dependency guard

`check_theatre_architecture` in [check_repository.py](../../../scripts/check_repository.py) runs in
the `repository` gate and retains the inward layer/package checks. The sandbox extraction
extends the production-safe shared boundary to `app` and `adapters`, with main-source,
declared Gradle dependency/source-routing and resource guards. Its scope and actual negative
checks are recorded in [#122 evidence](../../evidence/issue122-cmp-sandbox-isolation.md).
The guard is lexical: it does not establish compiler/Gradle resolution, runtime behavior or
production DEX/resource exclusion. Retain it while adding production adapters; the production
host's real release graphs and artifacts are inspected by `cargo xtask theatre-host
android-release`, whose inspector is unit-tested and sensitivity-checked. This tooling
change does not reorder the product queue; GitHub owns implementation and acceptance status.

## Editor workspace hygiene

[VS Code workspace settings](../../../.vscode/settings.json) now exclude generated outputs
and caches from Explorer, search and recursive watching when the repository root is open.
This tooling change does not reorder the product queue. Verify Explorer/Search behavior
on a VS Code host before editor acceptance, including separately opened Theatre Mobile,
UI-system CMP and Android preview Gradle roots; GitHub owns PR execution status.

## Historical Admin integration checkpoint — 13 October 2026

This historical checkpoint superseded provisional descriptions for its exact source only.
Fresh verification after the resumed develop merge is pending; these prior results are not
promoted to current integrated acceptance.
The [Admin handoff](../handoffs/admin-authorization-handoff.md#current-independent-integration-handoff) and
[evidence](../../evidence/admin-authorization.md#independent-integrated-review-and-local-acceptance)
pin the normally integrated implementation `97efec8b358bcc7e7b79249fe55687933af6e67f`, tree
`82fba427a0bd4af02f4a48e78a69dd8bbcb1281f`. Full `cargo xtask preflight --changed-only` reports
PASS for all 17 gates with no skips; mutation killed 9 with 0 survivors. Its 23-entry registry
preserves original identity/Admin migrations; artwork0023 and storage0015 stay with their owners.
Inherited Studio/Theatre HTTP negotiation, private session fences and separately scoped credentials
remain intact. Admin revocation affects only its local platform access, with current admission,
operation replay and required append-only audit; it supplies no resource ACL or global logout.

In that snapshot, PR101 remained draft while the Android INTERNET/callback/host source
exception was unresolved and PR103 kept that dependency boundary. The authorized source step
is now resolved and PR101 merged at develop `98f247e`; the remaining native
system-browser/secure-store/device acceptance precedes authorized live-provider lifecycle and
production protection/recovery/retention gates. Admin's live custom current-group contract is
unaccepted; the five-second observation witness does not make remote membership SQL-atomic.
Keep activation closed while those facts are unproved. The source and local/mock results do not
complete native playback, durable outbox, guest-import/linking policy, broad listening issues or
the four-PR merge campaign. GitHub owns execution/merge status; this queue records the next work.

Current Admin integration normally consumes reviewed develop `98f247e`, including storage0015,
editor workspace hygiene and the authorized Theatre Android source. Its combined registry is
1–22,24,25 (24 entries) with historical bytes unchanged. Tested source `eccf31684524572eef3048e75a9abd08d52c6581`,
tree `d3f3b31ba7ab28969b47a1008d2200bf8b7bed92`, passed all 17 local gates without skips: 332 repository
Python tests, 9 killed/0 survived Script IR mutants, 207 common/JVM tests with 0 failures/errors/skips,
and actual PostgreSQL 14 Admin+1 restart, 16 Theatre Web/7 native/18 Studio plus all inherited
HTTP/process-kill/restart/storage/migration/replay/codec/publication suites. The
[resumed Admin evidence](../../evidence/admin-authorization.md#resumed-integrated-review-and-acceptance--14-october-2026)
pins the 10-record/12-source/67-raw-proof manifest at SHA256
`1a0336cf7b598ee416fc91dc5f70936bdd6c8a56447fa0cfafb0a3d179e507dc`. The 10 issued-Admin rendition
401 cases are isolation assurance, not a reproduced production defect or storage IO claim.
GitHub owns PR103 push/merge status; local evidence does not close the separate acceptance gates.
Later develop PR108 CMP extraction `648c5b76` was normally integrated at `980fca66`, tree
`fdb22b5a`. Its actual proportional preflight passed repository/rust-format/theatre-cmp-demo,
including207 common tests; the canonical mock Android build passed. The independent source review
preserves25 existing function bodies/signatures, apart from required visibility. The exact
`eccf316` full17 record remains separate from this later supplement, SHA256
`527507c28217e36eb05a6cd2e8c839a38827faab2b4c9a71f80da1b678319551`.
Production stays closed until live current-group/client/credential/rotation and named native
OS/secure-store/provider acceptance, audit protection/retention and recovery gates are accepted.

## Theatre native launch handoff

The [Mac Web auth and spacing slice](../../evidence/theatre-web-auth-mac.md) repairs the returned
Guest callback retry and the Library Account-entry gap, with browser fixture/visual evidence.
Next accept existing-account Web login against an operator-prepared API/issuer/test account and
apply [ADR 0034](../../decisions/0034-theatre-signup-and-provisioned-studio.md): Theatre listeners
self-register; Studio has no public signup and admits only one or two securely provisioned
administrators. Verify Studio admission at callback and every protected request, including
refusal of a valid Theatre account and removal from the configured Studio subjects. These local
checks can progress independently of the missing live issuer/client/test account. Local fixtures
do not accept real signup, session reload or provider logout. Keep the existing Draft PR #196 and
native acceptance gaps; no provisioning is implied by this order.

For [#191](https://github.com/loveoverflowcom/cantos/issues/191), finish configured-artifact
inspection and separately authorized Web/native provider, callback, secure-store and lifecycle
acceptance after the [nonsecret build-input slice](../../evidence/theatre-auth-activation.md).
The existing Web BFF/proxy and native controller remain the owners; do not rebuild their
protocol or infer live login from source tests or Android preview. [#174](https://github.com/loveoverflowcom/cantos/issues/174)
continues to own email-verified self-registration and its Web/native contract. After ADR0034's
product choice, the owner approved code and isolated tests in the same Draft PR #196 for a
Theatre module that verifies email before creating a new Kanidm person, then sends an issuer
credential-setup link. Kanidm1.10.5 has those authenticated APIs although its public UI lacks
integrated signup; keep the issuer and existing OIDC/session owner. First finish pure/contract,
deterministic adapter/HTTP/Web and disposable-PostgreSQL evidence. Then review external
existing-email policy, naming/canonicalization, TTL/quota/retention/recovery and effective
issuer protection before requesting specific service-principal/grant/mail authorization.
The built-in onboarding group can reset existing people; do not infer least privilege from its
name or call an application-only new-person guard issuer protection. Missing configuration
remains disabled, and SMTP acceptance alone is not inbox or account completion. Real accounts,
credentials, standing grants, SMTP and security changes still need separate explicit authority.
Native and live acceptance gaps stay explicit; this does not reorder publication.
The [second independent review](../../evidence/theatre-auth-activation.md#second-independent-review--16-october-2026)
and authorized tool-update follow-up distinguish a passed Android host preview from pending
configured-artifact and live acceptance. CLI 1.0.16500706 / Studio Rabbit 1 now render the
existing guest discovery wrapper after the sandbox's debug-only tooling correction; source
compilation and inspected static pixels/text do not accept OS callbacks or live identity.

The bounded [issue133 launch slice](../../evidence/theatre-cmp-launch.md) retains the existing
synchronous host lifetimes and adds platform launch resources plus an optional app-owned CMP
presentation. Do not introduce a timer or a remote readiness gate to make that frame visible.
Next, prioritize shared logo/component admission in [#127](https://github.com/loveoverflowcom/cantos/issues/127),
then the configured identity/guest-shell composition in [#130](https://github.com/loveoverflowcom/cantos/issues/130),
coordinating [#122](https://github.com/loveoverflowcom/cantos/issues/122)'s release isolation.
Native launch acceptance still needs the pinned iOS toolchain, API23 fallback, OS callbacks,
restoration and screen-reader observations. The evidence record distinguishes the diagnostic
Android runtime from JVM captures; it does not accept production native listening or change
the backend sequence above. GitHub owns implementation and acceptance status.

For [#190](https://github.com/loveoverflowcom/cantos/issues/190), keep native launcher assets
independent of that splash and of shared CMP component admission. The
[launcher handoff](../../evidence/theatre-native-launcher-icons.md) binds generated resources to the
official Logo Kit. Next accept the installed Android mask/theme/cache matrix and the iOS
AppIcon bundle/Home/App Library/Settings observations with a separately supplied pinned native
environment. Source hashes and APK compilation cannot accept those runtime claims. This native
branding review does not reorder backend delivery, authorize toolchain installation or expand
Studio beyond Web.
