# MVP Work Queue — External Audio Handoff

> Scope reset 2026-10-12: [ADR 0018](../decisions/0018-external-audio-production.md).
> Studio is Web-only and manages novel/story/podcast content and their audio; Theatre is
> the listener Web/CMP client. Backend TTS, provider workers and in-server mixing are removed.
> Older TTS implementation evidence and GitHub issue history remain archival.

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

The [live client boundary](live-client-api-matrix.md) connects the bounded Studio forms and
keeps Theatre capabilities unsupported until a reviewed listener/publication contract exists.
Remaining Studio controls and responsive/keyboard/user acceptance stay with #66; transport
checks do not complete #51/#64. Next: user acceptance of the bounded Studio flow, secure
identity/storage and stronger rights/QC policy,
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
acceptance remains unrun after denied disposable startup; the full Rust gate still
fails in upstream service fixtures, with the original port flake retained separately.
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
  mock mini-player. Retain publication,
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
lifecycle/persistence evidence. Production backups/upgrades and Kanidm OIDC remain separately
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
