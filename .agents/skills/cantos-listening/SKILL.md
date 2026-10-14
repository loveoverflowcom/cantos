---
name: cantos-listening
description: >-
  Rule owner for Cantos listener contracts shared by Theatre Web and the CMP mobile app: the
  versioned listener HTTP/JSON API (catalog, immutable publication manifests, progress,
  bookmarks, download authorization), one playback state vocabulary implemented as a pure
  reducer per platform and agreed through shared JSON test vectors, conflict-safe progress and
  bookmark sync with server revisions, idempotency keys and tombstones, and the offline download
  contract. Use when adding or changing a listener endpoint, player state, resume, sleep timer,
  interruption, progress outbox, bookmark, conflict prompt, signed URL or download entitlement.
---

# Cantos listening

This skill is a **behavioral contract and router** for everything a listener's client and the
backend must agree on. It composes [`cantos-engineering`](../cantos-engineering/SKILL.md) — its
required order, evidence vocabulary and completion report — and owns how listener contracts are
modeled, implemented and verified. It does not own what Theatre does; the documents do.

## Product rules live in the docs

| Product question | Owning section |
|---|---|
| Listener identifiers, versioning, operation IDs, errors, pagination | [mobile § API contracts](../../../docs/architecture/mobile.md#api-contracts) |
| Player states, interruptions, resume, sleep timer, signed URL refresh | [mobile § Playback behavior](../../../docs/architecture/mobile.md#playback-behavior) |
| Download manifest, entitlement, revocation, account scope | [mobile § Downloads and access rights](../../../docs/architecture/mobile.md#downloads-and-access-rights) |
| Progress scope, outbox, reconciliation, bookmarks, release changes | [mobile § Durable listening progress](../../../docs/architecture/mobile.md#durable-listening-progress) |
| Streaming without generation, downloads, progress context (rules 25–29) | [business rules § Listening and mobile](../../../docs/product/business-rules.md#listening-and-mobile) |
| Theatre Web scope, non-goals and acceptance | [040 — Theatre Web listening](../../../docs/work-plan/040-theatre-web-listening.md) |
| Native scope, non-goals and acceptance | [050 — CMP native listening](../../../docs/work-plan/050-cmp-native-listening.md) |
| Player presentation, resume prompts, state labels | [UI system § Theatre](../../../docs/design/ui-system.md#theatre-compact-listening) |

Link these; never paraphrase a rule into a competing copy. Where the docs leave a rule open —
the completion rule, the offline validity window, automatic resolution of competing sessions,
tombstone and idempotency retention, anonymous listening — record a product decision (a doc
update or an [ADR](../../../templates/adr.md)) **before** encoding it. A reducer arm, reconcile
row or denial variant with no owning sentence is a gap in the plan, not a free choice.

## Repository reality

Inspect the code before citing it; this table is the status at the last sync, not proof.

| Surface | Implemented | Owning contract / code |
|---|---|---|
| Discovery and playback authorization | `/api/v1/theatre/{works,search,works/{work},works/{work}/units,publications/{publication},publications/{publication}/playback-authorizations}`; `ListenerPublication` witness | [Discovery v1](../../../packages/contracts/theatre-discovery-v1.md); `contexts/media` (`ListenerService`, pure `domain/listening.rs`, wire mapping `listener_projection.rs`); `interfaces/http/theatre.rs` |
| Library and progress | Continue, Saved, History, playback starts, clear, progress, operation receipts | [Library v1](../../../packages/contracts/listener-library-v1.md); `contexts/listening`; `interfaces/http/theatre_library.rs`; migration `0019_listener_library_progress.sql` |
| Listener admission | sealed `AuthenticatedListener`, default `DenyListenerAuthentication`, Theatre Web cookie and native bearer adapters | [Theatre identity v1](../../../packages/contracts/theatre-identity-v1.md); `listening/{auth,theatre_identity}.rs`; `theatre_auth::bff_identity` |
| Wire contract | JSON Schema `schema/theatre/library/v1.schema.json`, strict Rust DTOs `cantos_contracts::listener_library`, 22 fixtures in `fixtures/theatre/library/v1/` | `packages/contracts/api/tests/listener_library_wire.rs` |
| Mock player | Rust `core/playback/reducer.rs` (Theatre Web) and Kotlin `reducePlayback` (CMP) both run the `theatre-mock-playback@1` corpus | `packages/contracts/fixtures/theatre/playback/v1/` |
| Clients | Theatre Web reads Continue/Saved/History (`live/library.rs`); no client sends Library mutations or keeps an outbox | — |

**Not implemented (still proposals here):** bookmarks and their tombstones, downloads (manifest,
grant, authorization), receipt expiry (`operation_expired`), the production player and native
engine adapters, OpenAPI with generated or drift-checked Kotlin, and the behavioral
"listening is free" row-count test. Real PostgreSQL and account-sync acceptance follow the
[Library evidence](../../../docs/evidence/listener-library-progress.md); never infer them from this
table. Libraries named here (utoipa, openapi-generator, Media3, AVFoundation…) are candidates that
need a recorded decision and a pinned, verified version; proptest and Kani are already in use.

## Identity vocabulary

| Term | Meaning | Code |
|---|---|---|
| Actor | one row of the platform registry `actors(id)`: 1–64 lowercase ASCII letters, digits, `_`, `-` | `platform::identity::domain::ActorId` |
| Owner | the actor who owns a Studio work, unit or asset; a Studio authorization fact | `catalog::domain::OwnerId` (same grammar, Studio-side) |
| Listener | an actor acting as a Theatre account; the scope of all listener state | `listening::domain::ListenerId(ActorId)` |
| Authenticated listener | sealed proof that an issuer adapter verified the credential | `listening::auth::AuthenticatedListener`, crate-only constructor |
| Device, operation, engine session | client-minted canonical UUIDs; context, never authority | `DeviceId`, `OperationId`, `EngineSessionId` |

The account is never a request field: handlers derive `ListenerId` only from
`AuthenticatedListener`, and the database stores it as `listener_id` referencing `actors(id)`.
A listener is never typed as an `OwnerId`, and a Studio session or cookie never yields a listener.
[ADR 0025](../../../docs/decisions/0025-actor-identity-root.md) (proposed) records the touchpoint
inventory and the remaining Studio slices that root `OwnerId` in `ActorId`.

## The required order — listening overlay

The foundation's [required order](../cantos-engineering/SKILL.md#the-required-order) applies.
Listening work adds two obligations: the contract changes **before** the code, and every
implementation that claims a contract runs that contract's fixtures.

For database-backed catalog, manifest, progress and bookmark reads, apply the foundation's
[read-performance workflow](../cantos-engineering/references/postgresql-read-performance.md).
It owns the DBSP fit decision and evidence; listener visibility and authorization remain owned
by this skill and its documents.

```text
read the owning doc section, the work item and any existing contract or vector
        ↓
name the listener-visible invariant (seeds below) and its failure mode
        ↓
change the contract first: schema, fixtures, player vectors, conflict table — reviewed as spec
        ↓
encode identity and state as types; generated wire DTOs stay at the edge
        ↓
decide in a pure core: reduce · reconcile · decide_download · ListenerPublication
        ↓
run the shared fixtures and vectors on every implementation that claims them
        ↓
cross the real boundary: PostgreSQL conditional write, storage/CDN delivery, device adapter
        ↓
report contract version, vectors, conflict scenarios and the platforms that ran them
```

## Durable decisions, pure cores

Each durable listening decision has exactly one owner and is callable from a plain unit test with
no runtime, database, media engine or network ([functional core](../cantos-engineering/references/functional-core.md)).

| Decision | Pure core | Status | Shell around it |
|---|---|---|---|
| What the player does next | `reduce(PlayerState, Event) -> Transition` (Rust), `reducePlayback(PlaybackState, PlaybackEvent)` (Kotlin) | implemented for the mock corpus; production player proposed | media element or native engine adapter, timers, local persistence |
| Whether a progress write is accepted | `listening::domain::reconcile_progress(Option<&ProgressRecord>, &ProgressCandidate, now, order) -> Result<ProgressOutcome, DomainError>` over the Kani-checked kernel `reconcile::classify` | implemented, Rust backend only | `PgLibrary::mutate`: account lock, receipt, sequence guard, conditional `UPDATE` |
| Whether a Saved desire is accepted | `reconcile_saved(&SavedRecord, &SavedCandidate, now, order) -> Result<SavedOutcome, DomainError>` | implemented | same transaction |
| Whether a playback start or clear changes history | `HistoryState::start(&HistoryStart, seen_session)` and `HistoryState::clear(&HistoryClear) -> Result<HistoryOutcome, DomainError>` | implemented | session tombstones, history rows |
| What a stored progress may resume | `resume_status(stored, exact, current_unit_audio) -> ResumeStatus`; Continue via `projection::continue_records` | implemented, read-only | page queries over current publications |
| Whether a listener may see a publication | `media::domain::listening::ListenerPublication::resolve(ListenerFacts)` | implemented | discovery, manifest and Library admission queries |
| Whether a download may be authorized | `decide_download(&Entitlement, &ListenerPublication, &OfflinePolicy, ServerInstant) -> Result<DownloadGrant, DownloadDenial>` | proposed | entitlement lookup, URL signer, grant row |

The playback reducer is the deliberate exception to "one implementation per policy": it is
presentation-adjacent state that must run offline inside each client. Its meaning is shared
through **data** (vectors), never through shared runtime code, Rust FFI into CMP or a WebView.
Progress reconciliation and download authorization are policy and stay server-side.

## Rules this skill owns

Status values: **manual** (review checklist only), **proposed** (automated check designed here,
not implemented), **implemented** (exists in the repository, with the scope stated). Full rule
cards with a good example, a counterexample and the exception live in the linked reference.

| ID | Rule | Failure it prevents | Oracle | Status |
|---|---|---|---|---|
| L1 | Listener identifiers (`work_id`, `unit_id`, `publication_id`, `rendition_id`) are opaque, typed and immutable; never titles, slugs or storage paths | a rename or storage migration orphans progress, bookmarks and downloads | newtype barriers; fixture with a renamed title keeps every ID | implemented server-side (canonical UUID newtypes, `AudioIdentity`); client domain types proposed |
| L2 | The committed OpenAPI/JSON Schema is the reviewed contract; Rust and Kotlin wire types are generated or drift-checked; no hand-copied DTOs | silent client/server divergence | regenerate-and-diff; every fixture decodes in every client | implemented for JSON Schema + strict Rust DTO fixtures; OpenAPI and Kotlin proposed |
| L3 | Every retriable write carries an operation ID created at intent time and persisted before the first send | a retry after a crash double-applies a write | duplicate delivery replays the original response; key reuse with a new body → `idempotency_key_reused` | implemented server-side (`listener_operations`); client outbox proposed |
| L4 | Only the active, complete release is listable, fetchable or authorizable; drafts, staged and unapproved releases are indistinguishable from absent | unapproved audio reaches listeners | visibility matrix against real PostgreSQL | implemented (`ListenerPublication`, Library re-admission); evidence per contract |
| L5 | A play, manifest, authorization or progress request creates no production job, attempt, cost reservation or provider call | listening silently costs money | row-count test plus zero fake-provider calls — a test, not a comment | structural half statically-checked (`listening` may import only `catalog`, `media`); row-count test proposed |
| L6 | Authorization is decided in the backend; signed URLs are minted after it, short-lived, redacted, never stored as identity | hidden buttons treated as security; leaked credentials | denial-variant handler tests; log-capture test | implemented (sealed principal, default deny, redacted delivery URL `Debug`); log-capture test proposed |
| L7 | One closed status vocabulary with a separate pause cause and failure reason; listener intent is a separate fact from engine state | an interruption shown as a user pause; buffering shown as paused | exhaustive matches; shared vectors | implemented for the mock player |
| L8 | Rust and Kotlin reducers agree through a reviewed vector corpus; unknown event kinds fail the harness | the two players mean different things by "paused" | vector run on each platform, named in the report | implemented for `theatre-mock-playback@1` |
| L9 | Cold launch, sleep-timer expiry, explicit pause and remote progress never start or move playback | unexpected audio; the playhead jumps under the listener | vectors plus a property over random event sequences | partial: mock `restore-session` vectors; property and sleep timer proposed |
| L10 | A session plays one immutable publication; a new release is an offer, never a silent swap | the listener lands in mismatched timing | vector `active-release-changed-while-playing` | proposed (server side: `resume_status` never substitutes) |
| L11 | A progress write is accepted only against the current server revision; otherwise the response carries the current record. No max-position merge; device clocks are context | a late offline device erases newer progress, or rewinds are lost | conflict table, interleaving property, PostgreSQL concurrency test | implemented server-side; see [`progress-sync.md`](references/progress-sync.md) |
| L12 | Bookmark deletion is a tombstone; replayed creates cannot resurrect it | deleted bookmarks reappear after offline replay | property over replays; tombstone table | proposed |
| L13 | A download is identified by immutable publication and asset checksums; signed URLs are disposable; revocation limits are stated honestly, with no DRM claim | corrupt or mismatched offline files; promises the product cannot keep | manifest immutability test; denial table; copy review | proposed / manual |

## Ledger seeds

Start the foundation's [working ledger](../cantos-engineering/SKILL.md#1-name-the-invariant-before-writing-code)
from these owning sentences, then add the rows your change touches.

| Claim | Owning sentence | Cheapest oracle | Target evidence |
|---|---|---|---|
| Repeated play incurs no synthesis | [invariants: "Listener presses play repeatedly"](../../../docs/product/business-rules.md#product-invariants-to-verify) | production row counts before/after N plays | `integration-tested` |
| Late offline progress never blindly overwrites | [invariants: "Offline progress arrives late"](../../../docs/product/business-rules.md#product-invariants-to-verify) | conflict table; two-device interleaving property; real conditional update | `example-tested`, `property-tested`, `integration-tested` |
| Drafts and unapproved episodes are not playable | [040 acceptance](../../../docs/work-plan/040-theatre-web-listening.md#acceptance-criteria) | visibility matrix per endpoint | `integration-tested` |
| Rewind and completion follow explicit rules | [040 acceptance](../../../docs/work-plan/040-theatre-web-listening.md#acceptance-criteria) | conflict rows C3, C11; completion vectors | `example-tested` |
| No auto-play after cold launch or sleep timer | [mobile § Playback behavior](../../../docs/architecture/mobile.md#playback-behavior) | vectors on Rust and Kotlin; device lifecycle flow | `differentially-tested`, then `device-tested` |
| Republishing never shifts active timing | [040 acceptance](../../../docs/work-plan/040-theatre-web-listening.md#acceptance-criteria) | pinned-release vectors; resume prompt test | `differentially-tested`, `interaction-tested` |
| Incomplete downloads are never offered as ready | [mobile § Downloads and access rights](../../../docs/architecture/mobile.md#downloads-and-access-rights) | server: manifest checksums and sizes; client: owned by CMP | `example-tested` server-side |

## Evidence for listening work

Use the foundation's [evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
verbatim. The listening-specific mapping:

- A reducer passing the reviewed vector corpus is `differentially-tested` **on the named
  platform** (`Rust native`, `Rust wasm32`, `Kotlin/JVM`, `Kotlin/Native iOS simulator`). It is
  never `device-tested`, because no native engine or OS callback was involved.
- Vectors produced by running one reducer and replayed on the other are a self-differential
  divergence check, not a correctness oracle; say which kind you ran.
- The two-device interleaving property over the pure server model is `property-tested`; the
  same generator against PostgreSQL is `integration-tested` and differential against the model.
- Fixture decoding with generated types is `example-tested`; regenerate-and-diff is
  `statically-checked`. A schema file existing is `documented`.
- Playback through the real storage/CDN path is `integration-tested`; lock-screen, interruption
  and background behavior need `device-tested` with the named device or simulator.
- A fake provider asserting zero calls is `example-tested` against a double; pair it with the
  real-database row count before claiming the "listening is free" invariant.

## Completion report additions

Extend the foundation's [completion report](../cantos-engineering/SKILL.md#9-completion-report);
every foundation field stays required. Add:

```text
Contract version:          listener API <x.y.z> (/v1) · player vectors <contract@version> ·
                           download manifest <n>; schema diff: none | additive | breaking (+ADR)
Fixture vectors:           <paths and counts>; hand-authored | generated-for-divergence; changed here?
Conflict scenarios:        <conflict/tombstone table rows run by ID>; property cases and seed;
                           PostgreSQL concurrency run: yes | no
Shared vectors ran on:     Rust native | Rust wasm32 | Kotlin/JVM | Kotlin iOS | Kotlin Android —
                           exact command per platform; "not run" stated per platform
No-production-work check:  <test name> → pass | fail | not run
Open product decisions:    <rules encoded as needs-decision, with the owning doc to update>
```

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| endpoints, identifiers, versioning, schema and generated types, idempotency, error codes, revisions, pagination, visibility, authorization, "listening is free" | [`listener-api.md`](references/listener-api.md) |
| player states, intent vs engine, the reducer, interruptions, sleep timer, cold restore, completion, release pinning, access refresh, shared vectors | [`playback-semantics.md`](references/playback-semantics.md) |
| progress outbox, revisions, operation log, `reconcile`, competing sessions, bookmarks and tombstones, conflict tables, interleaving properties | [`progress-sync.md`](references/progress-sync.md) |
| download manifest and grants, entitlement and offline policy, revocation limits, replacement and withdrawal, account scoping, server-side delivery | [`downloads-contract.md`](references/downloads-contract.md) |

Foundation techniques these references build on:
[`http-api-boundary.md`](../cantos-engineering/references/http-api-boundary.md),
[`persistence.md`](../cantos-engineering/references/persistence.md),
[`postgresql-read-performance.md`](../cantos-engineering/references/postgresql-read-performance.md),
[`types-as-proofs.md`](../cantos-engineering/references/types-as-proofs.md),
[`boundary-hardening.md`](../cantos-engineering/references/boundary-hardening.md),
[`immutability.md`](../cantos-engineering/references/immutability.md),
[`property-and-differential-testing.md`](../cantos-engineering/references/property-and-differential-testing.md).

## Decoupling map

```text
packages/contracts/ (schema, fixtures, player vectors)   ← reviewed spec; Rust DTOs in api/
        ↑ strict Rust DTOs today; generated or drift-checked Kotlin proposed
contexts/listening            domain: catalog::domain IDs + platform::identity::domain::ActorId;
        ↑                     adapter: media's current-publication reads (library_public_work,
        ↑                     library_publications) and catalog's lock; never production
interfaces/http/theatre_library.rs ← theatre_auth::bff_identity admits the sealed principal
Theatre Web player core (Rust, no Leptos/web-sys import) ← Leptos shell, media element adapter
CMP player core (Kotlin common, no Compose/engine import) ← native Android/iOS media adapters
```

- `contexts/listening` may depend only on `catalog` and `media`
  (`CONTEXT_UPSTREAM` in `scripts/check_repository.py`), never the reverse, and never on
  production. That import edge is the statically-checked structural half of L5.
- Client player cores import neither their UI framework nor their media engine; adapters
  translate engine and OS callbacks into `PlayerEvent`s and execute `PlayerEffect`s.
- Clients depend on the contract, never on server domain modules, SQL types or Rust layouts.
  Theatre Web may depend on a generated wire-type crate; it never depends on backend internals.

## Compose with

- [`cantos-publication`](../cantos-publication/SKILL.md) owns the release manifest, the active
  release pointer, rights (including offline rights) and storage/CDN delivery. This skill owns the
  listener-facing projection and its visibility filter, never the release transition.
- [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) implements the Theatre Web shell:
  [media element interop](../cantos-leptos-web/references/dom-and-media-interop.md) and
  [web testing](../cantos-leptos-web/references/web-testing.md).
- [`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md) implements the CMP shell:
  [native playback](../cantos-cmp-mobile/references/native-playback.md),
  [downloads and local state](../cantos-cmp-mobile/references/downloads-and-local-state.md) and
  [CMP testing](../cantos-cmp-mobile/references/cmp-testing.md).
- [`cantos-ui-design`](../cantos-ui-design/SKILL.md) owns how states, conflict prompts and
  revocation copy look and read, including Vietnamese copy and accessibility.
- [`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md) owns the job tables
  that the "listening is free" test counts; reuse its names instead of guessing.

## Stop conditions

Stop and report instead of proceeding when a change would:

- start adaptation, TTS, mixing or on-demand transcoding from any listener request;
- reconcile progress by maximum position, last arrival or device clock;
- move the playhead, swap the publication or start audio without a listener gesture;
- hand-maintain a Kotlin or Leptos DTO beside the schema, or share runtime code via FFI;
- store or log a signed URL, or use it as a file or cache identity;
- claim DRM, guaranteed remote erasure or immediate offline revocation;
- encode an undecided product rule (completion, offline validity, automatic conflict resolution)
  without a recorded decision.
