# MVP Work Queue — External Audio Handoff

> Scope reset 2026-10-12: [ADR 0018](../decisions/0018-external-audio-production.md).
> Studio is Web-only and manages novel/story/podcast content and their audio; Theatre is
> the listener Web/CMP client. Backend TTS, provider workers and in-server mixing are removed.
> Older TTS implementation evidence and GitHub issue history remain archival.

## Executable behavior review boundary

[Issue 106](https://github.com/loveoverflowcom/cantos/issues/106) introduces the bounded
[Quint foundation](../../specs/behavior/README.md) before its separate
[implementation-observation handoff](quint-conformance-handoff.md). Start with the existing
private AudioSubmission core; keep publishing/listening and wider UI/CMP scenarios deferred
until their exact requirements/observations are reviewed. A model PASS does not accept a
product feature or replace the backend sequence below. The
[foundation evidence](../evidence/quint-behavior-foundation.md) separates model stages;
[PR B evidence](../evidence/quint-code-conformance.md) records independently observed pure-core
conformance. Application/IO, provider/SQL and shared API/Leptos/CMP runtime remain later
review boundaries, not implied by either gate. PR B is stacked on unmerged PR A and must
be rebased/retargeted with fresh evidence after the foundation is accepted.

## Ordered implementation

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

The next Catalog acceptance is the [Studio Web slice](011-studio-catalog-api.md) using the
implemented [Catalog JSON API](../../packages/contracts/catalog-v1.md). The internal foundation
and HTTP evidence are recorded separately in [session 1](../evidence/studio-catalog-foundation.md)
and [session 2](../evidence/studio-catalog-api.md). Keep #66 open for responsive and keyboard
Studio Web acceptance; GitHub owns execution status.

The [Content Identity slice](../decisions/0019-content-identity.md) adds immutable content beneath
Catalog metadata. Its [handoff to Production Brief](content-identity-production-brief-handoff.md)
keeps the next backend boundary narrow: exact content/guidance revision pinning and independent
producer access, before linking intake/publication. Catalog Web acceptance remains separately
scoped; this sequence does not close #66 or #64. The [Brief v1 backend](../../packages/contracts/production-brief-v1.md)
provides exact snapshots and read grants; [session3 handoff](production-brief-audio-lifecycle-handoff.md)
keeps intake linkage, review/release and later live client integration in separate boundaries.
## Private audio admission and next review boundary

[Audio Lifecycle v1](../../packages/contracts/audio-lifecycle-v1.md) reuses attributed PR69
intake and binds verified private bytes to exact Content/Brief/profile/rights declarations.
Named append-only reviews and private candidates expose current blockers; public publication
always remains blocked. [Local evidence](../evidence/audio-lifecycle.md) and the
[frontend handoff](audio-lifecycle-frontend-handoff.md) describe tested boundaries.

[Provider-independent private storage](../architecture/object-storage.md) adds durable original
target metadata, filesystem/S3 adapters and exact owner-authorized audio preview while retaining
Catalog/Editorial/Ingestion readers. Its [implementation evidence](../evidence/provider-independent-storage.md)
separates local and mocked results from real-provider acceptance. It does not complete #10's
deployment/delivery acceptance, #11's publication or #12's listener boundary. Changing the
write target moves no existing objects.

The private Work artwork slice ([#105](https://github.com/loveoverflowcom/cantos/pull/105))
extends this verified-payload/MIME boundary inside Media and reuses the same storage registry,
retaining the audio envelope and original-target reads. Its additive migration0023 stays
separate from storage migration0015. The [artwork evidence](../evidence/private-work-artwork.md)
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

The [live client boundary](live-client-api-matrix.md) connects the bounded Studio forms and
keeps Theatre capabilities unsupported until a reviewed listener/publication contract exists.
Remaining Studio controls and responsive/keyboard/user acceptance stay with #66; transport
checks do not complete #51/#64. Next: user acceptance of the bounded Studio flow, production
identity/storage acceptance and stronger rights/QC policy,
then atomic publication/delivery before Theatre playback. Keep broader #51/#64/#66 acceptance
separate. No listening/interoperability is settled by admitting an Opus envelope.

The [PR73 browser follow-up](../evidence/live-client-browser-acceptance.md) exercised the real
bounded Studio flow and retains its historical partial/FAIL acceptance. The
[bounded correction/recheck record](../evidence/live-client-finding-fixes.md) owns the later
default-kind, existing-profile session and chip-contrast evidence. Next within #66, prioritize
pending-write/concurrent identity cases, then full accessibility acceptance; keep missing
browser headers and expiry interactions explicit. GitHub retains execution status. The recommended publication/Theatre
backend sequence above is unchanged.

## Publication core and next listener boundary

[Publication v1](../../packages/contracts/publication-v1.md) implements owner-authored distribution
clearance, exact approval, all readiness blockers, immutable releases, atomic active pointers and
minimal withdrawal over the existing candidate/intake path. [Verification](../evidence/publication-core.md)
separates passing core/Router/proof tests from failed PostgreSQL startup and unrun delivery.
Unconfigured delivery cannot activate. [Discovery/Playback then Library/Progress handoff](publication-discovery-handoff.md)
defines the next two separately reviewed slices and the independent
[private storage PR90](https://github.com/loveoverflowcom/cantos/pull/90) integration prerequisite.
Keep #9/#10/#11 open through real storage, SQL/concurrency, listening/rights and public acceptance;
Studio audit manifests do not themselves authorize Theatre playback.

## Existing foundations to retain

The [internal actor registry](../architecture/identity-registry.md) keeps legacy ownership intact
and adds exact external keys with explicit operator registration/linking. The bounded
[Studio OIDC](../architecture/studio-oidc.md) slice retains its signed-mock/local-SQL evidence and
adds actual-cookie confirmation and mode-specific login/logout controls to Studio Web. The independent identity review fixed lock-wait expiry, OAuth Basic encoding and shared-cookie
account/cancellation races. Studio now has [current integrated evidence](../evidence/studio-oidc.md#independent-integrated-acceptance-13-october-2026):
all17 local preflight gates PASS and11 actual browser observations PASS, with remaining runtime
accessibility/provider gates explicit.

[ADR 0025](../decisions/0025-actor-identity-root.md) (proposed) roots owner and listener identity
in the registry `ActorId`. The listener slice is implemented: `ListenerId` wraps `ActorId`, not
Catalog's `OwnerId`, with no schema or wire change. The two Studio slices (owner as
`OwnerId(ActorId)`, acting-actor fields as `ActorId`) are recommended as separate cohesive
refactors before new Studio authorization work, because they only change Rust signatures.

The following Theatre ledger describes the **13 October snapshot**, superseded by the
[resumed native appendix](#resumed-native-identity-source-and-next-acceptance--14-october-2026).
The [separate Theatre identity handoff](theatre-identity-handoff.md#independent-integrated-acceptance-handoff--13-october-2026)
now records the mounted readonly Web account/Library consumer and native exchange/resource/
registry/session implementation. Its [13 October integrated evidence](../evidence/theatre-identity.md#independent-integrated-acceptance--13-october-2026)
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

The [domain assurance hardening](../assurance/domain-invariants.md) strengthens
existing six-crate boundaries. [PR75](https://github.com/loveoverflowcom/cantos/pull/75)
merged ordered steps 1–4; the user-approved follow-on
[PR81](https://github.com/loveoverflowcom/cantos/pull/81) delivers 5–11: actual
editorial/production/media/incremental consumers, typed client state and the
opt-in three-kernel verifier workflow. The [proof record](../assurance/kani.md)
binds actual execution to its versions/inputs/bounds; current skills describe only
implemented rules. Adequate audited boundaries were retained without empty commits
or speculative wrappers. GitHub owns publication/review status.

Implementation delivery and named core evidence do not complete broader Studio
acceptance, rights/QC, publication or listener implementation. Real PostgreSQL
acceptance in that earlier assurance record remains unrun after denied disposable startup;
its full Rust gate fails in upstream service fixtures, with the original port flake retained
separately. Later storage/read verification belongs to its own
[evidence record](../evidence/provider-independent-storage.md).
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
[ADR 0022](../decisions/0022-cantos-forge-producer-cli.md) add a producer-side CLI outside the
API runtime. Its [foundation](../../apps/cantos-forge/README.md) holds the crate, xtask build, the
tested pure core and a host-assisted local workflow (workspace, register, validate, export a
local bundle). Recommended next: instruction bundles and AI/human review evidence commands, then
provider orchestration and the authorized handoff as separate boundaries. None of this changes the
API sequence above. GitHub owns status.

## Canonical work plans

- [010 — source/script import and editor](010-import-and-edit-script.md) — completed portions retained.
- [020 — external production instructions and audio handoff](020-cast-and-generate-dialogue.md).
- [030 — external master intake, QC and release](030-mix-review-and-publish.md).
- [040 — Theatre Web](040-theatre-web-listening.md). The session-only
  [Web mock](../evidence/theatre-web-mock.md) and [PR 02 local/browser validation](../evidence/theatre-web-pr02.md)
  establish the mock foundation only, including shared Web control-label alignment. Retain it while implementing publication and the authorized
  listener API first; neither completes 040 nor its issues.
  The [#35 Web library MVP mock](../evidence/theatre-web-library-mvp.md) provides a bounded UX
  review step: Khám phá/Của tôi navigation, direct Continue/Saved/History sections and persistent
  mock mini-player. The [library/account v2 visual pass](../evidence/theatre-web-library-account-v2.md)
  reworks those two screens (cluster, status rows, bundled fonts/mock artwork) without new product
  scope; its Drive reference comparison and Avatar qualification remain open. Retain publication,
  listener identity/API and real playback/synchronization prerequisites for production acceptance.
  The mock does not settle the broader product/backend decisions tracked by
  [#34](https://github.com/loveoverflowcom/cantos/issues/34) or close #35.
- [050 — native Theatre Mobile](050-cmp-native-listening.md). The memory-only
  [CMP mock](../../apps/cantos-theatre/mobile/README.md) completes neither 050 nor its issues.
  The [#35 CMP library MVP mock](../evidence/theatre-cmp-library-mvp.md) follows the Web
  presentation with two primary destinations and direct Continue/Saved/History sections.
  The [reference UI repair](../evidence/theatre-cmp-ui-repair.md) restores artwork/font anatomy
  and keeps large-text controls and preference dialogs within that same isolated mock boundary.
  The [radius normalization](../evidence/theatre-cmp-shapes.md) keeps the same mock anatomy
  while sharing shape roles with the existing design system.
  The [fake storage simulator](../../apps/cantos-theatre/mobile/README.md#fake-storage-simulator)
  ([#109](https://github.com/loveoverflowcom/cantos/issues/109)) exercises shared-session
  load/store/recreate, latency, faults and reset fencing before the local metadata store ADR;
  it is memory-only and supplies no durable persistence or process-death evidence. Its database
  and presets form one [mock app sandbox](../../apps/cantos-theatre/mobile/docs/architecture.md#mock-app-sandbox)
  that also backs `AppContainer.mock`, replacing the separate memory progress adapter.
  [ADR 0026](../decisions/0026-theatre-mobile-local-storage-sandbox.md) (proposed) records the
  per-layer storage test doubles and adoption gates and adds the shared `ProgressPort` contract
  suite; Okio/SQLDelight-style dependencies wait for their #16/#17 consumers.
  Review common/JVM behavior separately from Android/iOS runtime; implement the authorized
  listener/publication and account contracts before durable sync or production acceptance.

The [CMP engineering skills audit](../evidence/cmp-engineering-skills.md) strengthens typed
functional Kotlin, layer/ownership review and reproducible verification guidance. This is
documentation maintenance; it changes neither the recommended implementation order nor 050's
native acceptance status.
- [GitHub execution map](project-planning.md) and [audio drama craft](audio-drama-quality.md)
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
change ([ADR 0021](../decisions/0021-podman-service-runtime.md),
[evidence](../evidence/podman-service-runtime.md)); GitHub owns execution status. It adds a
bounded local infrastructure option while preserving native verification. It does not move the
product queue: finish live client acceptance, then reviewed identity/storage and stronger QC,
then publication before Theatre playback. Next runtime acceptance needs an operator-prepared
rootless Podman/Machine environment, compatible `nocreate`, private TLS and disposable fixture
lifecycle/persistence evidence. Production backups/upgrades and live Kanidm OIDC acceptance remain separately
scoped work; unavailable runtime evidence is not a blocker to reviewing this draft.

## Public discovery before account-owned listening state

The [Discovery/Playback backend contract](../../packages/contracts/theatre-discovery-v1.md)
adds a current-public projection, stable opaque keysets and explicit unavailable media grants.
Keep Publication/delivery/SQL acceptance and the isolated Web/CMP mock-to-live adapter work
separate. The [Library/Progress handoff](discovery-library-handoff.md) requires the coordinated
typed listener identity boundary, conditional/idempotent writes and removal-safe progress.
No listener registry or native playback is inferred from anonymous catalog reads.

## Listener Library / Progress successor

The [account Library/Progress v1](../../packages/contracts/listener-library-v1.md) and
[identity/Web/CMP handoff](listener-library-progress-handoff.md) follow inherited draft PR93.
The [evidence record](../evidence/listener-library-progress.md) distinguishes pure/use-case/Router
checks from unrun real PostgreSQL and issuer/client playback acceptance. Keep MVP Khám phá | Của tôi;
no automatic guest merge or reference-v2 expansion. Independent identity and public delivery
remain fail-closed dependencies, and this draft does not complete the broad listening issues.

## Versioned binary transport pilot

[Issue #97](https://github.com/loveoverflowcom/cantos/issues/97) delivers the bounded Studio
Protobuf pilot and portable CMP fixture harness through one branch/PR.
[The migration ledger](../evidence/protobuf-wire-migration.md) separates pure-core/router/JVM
evidence from deferred real PostgreSQL, browser save/reload/rollback, Android/iOS and performance
acceptance. Keep JSON default, preserve legacy MessagePack compatibility, and finish those
activation gates before expanding the wire contract. This does not reorder publication/listener
work or change identity, rights, audio or storage scope. GitHub owns execution status.


## Identity integration order

Consume the [exact identity registry](../architecture/identity-registry.md) before credential
issuance, then integrate Studio OIDC, Theatre Web/native identity and Admin authorization in that
order. Preserve the [handoff](identity-authentication-handoff.md#integrated-registry-consumption)
and additive migration checksums. Current local/mock acceptance belongs in evidence records;
production provider/client preparation, native runtime acceptance and explicit cross-client
identity linking remain separate prerequisites. Equal email or subject text across different
issuers never establishes a shared actor. GitHub owns execution status.

## Private Work cover API/DB acceptance

The user-requested [issue104 MVP plan](012-private-work-artwork.md) selects one durable,
owner-only Work cover and exact private preview from the
[checked standards reference](catalog-media-standards-reference.md). Its
[new-session implementation handoff](catalog-media-mvp-ultra-handoff.md) permits core rules and
new opt-in metadata contracts first; real storage integration reuses the final independently
reviewed PR90 boundary and coordinated additive migration 0023. No product implementation is
claimed by these planning docs. The scope is API/DB, not Studio UI or public artwork delivery;
P1 metadata, thumbnails and transcripts are explicitly deferred. This is an authorized narrow
extension of the wire pilot, preserving JSON default and its outstanding acceptance gates,
without reordering identity/storage or broader publication/listener acceptance.

The [implemented private artwork contract](../../packages/contracts/artwork-v1.md) now reuses
the reviewed private-storage boundary. The next scoped acceptance is disposable PostgreSQL
upgrade/constraints/grants, actual owner HTTP/rights/session behavior, read plans and host/PG
restart faults. The [dated evidence ledger](../evidence/private-work-artwork.md) separates those
remaining gates from current core, decoder, filesystem and controlled-peer checks.


The identity campaign's PR3 [Theatre identity handoff](theatre-identity-handoff.md) follows the
reviewed registry/Studio dependencies and the merged Listener Library seam. Preserve the latest
[Studio integrated acceptance](../evidence/studio-oidc.md#independent-integrated-acceptance-13-october-2026)
and its HTTP negotiation/session fences while integrating Theatre's separate Web BFF, account
fencing and native public-client exchange/resource IO in migration0024. In the 13 October
snapshot, combined Theatre
verification was pending and Android INTERNET/callback registration and host mounting were
unapplied after automatic approval review rejected the source activation. The direct human
exception and those source steps are now resolved in the resumed appendix below.
Provider/device/secure-store and native playback acceptance remain separate.
The [Admin0022 slice and handoff](admin-authorization-handoff.md) adds dedicated resource-token
verification, current role decisions, Admin-only access revocation and append-only audit in PR4.
Its fresh combined verification follows the reviewed develop98f247e/Theatre/storage/IDE merge;
this queue does not claim campaign completion, live-provider/native playback acceptance or a
guest-import policy. GitHub owns execution/merge status.

## Resumed native identity source and next acceptance — 14 October 2026

The [resumed Theatre handoff](theatre-identity-handoff.md#resumed-native-identity-handoff--14-october-2026)
and [fresh evidence appendix](../evidence/theatre-identity.md#resumed-native-identity-acceptance--14-october-2026)
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

`check_theatre_architecture` in [check_repository.py](../../scripts/check_repository.py) runs in
the `repository` gate (and CI) and rejects import, fully qualified-name and package/directory
violations for `domain`, `ports`, `application`, `features` and `demo` under
`apps/cantos-theatre/mobile/shared/src/*Main`; `adapters` and `app` are explicitly unguarded.
`DemoCopy` moved to the leaf `demo` package to remove a `features` → `app` import cycle. The
guard is lexical (no overload/alias resolution) and does not prove behavior such as clocks or
random sources in `domain`. Evidence: `statically-checked, example-tested`
(negative fixtures plus real-tree mutants). This tooling change does not reorder the product
queue; GitHub owns PR execution status.

## Editor workspace hygiene

[VS Code workspace settings](../../.vscode/settings.json) now exclude generated outputs
and caches from Explorer, search and recursive watching when the repository root is open.
This tooling change does not reorder the product queue. Verify Explorer/Search behavior
on a VS Code host before editor acceptance, including separately opened Theatre Mobile,
UI-system CMP and Android preview Gradle roots; GitHub owns PR execution status.

## Historical Admin integration checkpoint — 13 October 2026

This historical checkpoint superseded provisional descriptions for its exact source only.
Fresh verification after the resumed develop merge is pending; these prior results are not
promoted to current integrated acceptance.
The [Admin handoff](admin-authorization-handoff.md#current-independent-integration-handoff) and
[evidence](../evidence/admin-authorization.md#independent-integrated-review-and-local-acceptance)
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
[resumed Admin evidence](../evidence/admin-authorization.md#resumed-integrated-review-and-acceptance--14-october-2026)
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
