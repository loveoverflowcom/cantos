# Domain assurance ledger

## Scope and evidence vocabulary

Initial inventory, recorded before implementation on 2026-10-13 against fresh
`origin/develop` `4f33a901b87be770be7cf5bbdac909408570240d`. All six Cargo workspace
members are in scope. CMP/Kotlin is outside this Rust audit. This ledger is an
inventory of implemented code, not a claim that proposed publication, listener
integration or external production exists.

The owning architecture remains [ADR 0018](../decisions/0018-external-audio-production.md),
[Content identity](../decisions/0019-content-identity.md),
[API surfaces](../decisions/0015-api-surface-boundaries.md) and
[wire protocol](../architecture/wire-codec.md). Types express local propositions;
PostgreSQL/current authorization/time still determine transaction authority.

| Category | Meaning here |
| --- | --- |
| `TYPE_ENFORCED` | Private representation or exhaustive enum prevents the stated alternative through the public safe API. No formal proof is implied. |
| `RUNTIME_VALIDATED` | A production admission/transition/reconstruction function checks the proposition. |
| `PROPERTY_TESTED` | A named generated-input law actually ran; record its domain and configuration. |
| `KANI_VERIFIED` | A named harness calling the production function ran successfully over stated symbolic inputs, assumptions and bounds. |
| `INTEGRATION_TESTED` | A named test crossed real local PostgreSQL or another actual boundary; fakes and source inspection do not qualify. |
| `NOT_VERIFIED` | Required evidence has not run, is unsupported, or does not establish the proposition. |

A row may carry several categories. Existing source tests are listed as **available
oracles**, not automatically credited as executed evidence in this audit. Example
tests, compile-fail checks, mutation results and mock integration are recorded
separately below. No row initially earns `PROPERTY_TESTED`, `KANI_VERIFIED` or
`INTEGRATION_TESTED` merely because a test/harness/document exists.

## Initial invariant and entry-path inventory

The code owner is the module, rather than an invented person or new service. Paths
below are relative to the repository root. Raw DTO fields, raw stored facts and
request defaults are intentionally not domain witnesses.

### cantos-contracts (`packages/contracts/api`)

| ID / proposition | Owner and admission boundary | Failure / bypass audit | Initial enforcement; available oracle; residual obligation |
| --- | --- | --- | --- |
| W1: Exactly one DTO value decodes, with bounded MessagePack nesting and UTF-8 intact | `src/wire.rs::decode` | Trailing bytes, truncated length prefixes, deeply recursive skipped fields, malformed markers | `RUNTIME_VALIDATED`; `tests/msgpack_wire.rs` goldens, truncation, depth and UTF-8 tables. No fuzz or model-checking result. HTTP body limits live in the API. |
| W2: Strict object fields and closed wire enums are identical across supported codecs | DTO serde declarations | `Deserialize` intentionally creates raw values; it must not be presented as ownership/rights/revision admission | `RUNTIME_VALIDATED`; `tests/*_wire.rs` fixture/schema parity and independent MessagePack goldens. Add coverage for current Content DTOs and full integer edges; do not widen route codec support. |
| W3: PATCH absence, clear and replacement remain distinguishable | `src/catalog.rs::PatchField` | `Default` means Missing only; standalone Missing serializes null, while DTO fields skip it | `TYPE_ENFORCED` enum, `RUNTIME_VALIDATED` serde; `tests/catalog_wire.rs`. Semantic clear restrictions stay in Catalog, not DTOs. |
| W4: Wire numeric representability is distinct from a domain sequence bound | raw `u64`/`i64` DTO fields | A successfully decoded zero or `u64::MAX` is not an accepted database revision | `RUNTIME_VALIDATED` serde shape only; downstream fallible conversion required. Record rejection of negative/fractional/out-of-range values without changing raw DTO ranges. |

### cantos-ui (`packages/ui-system/leptos`)

| ID / proposition | Owner and admission boundary | Failure / bypass audit | Initial enforcement; available oracle; residual obligation |
| --- | --- | --- | --- |
| U1: Disabled/loading actions require a nonblank reason; icon-only controls require a nonblank accessible name | `src/button.rs::{DisabledReason,AccessibleName,ButtonState}` | Private tuple fields; no serde/Default/From bypass on nonempty strings. `ButtonState::Enabled` default is valid | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; `tests/actions_navigation.rs`. Current predicate `matches!` falls through if variants are added; strengthen exhaustive state projection. No screen-reader claim. |
| U2: Text read-only is distinct from disabled; choices have no fictitious read-only state | `src/fields.rs::{FieldState,ChoiceState}` | Synthetic callbacks must use the same permission as HTML state; raw values/labels/errors remain caller-owned | `TYPE_ENFORCED` enums, `RUNTIME_VALIDATED` callback guards; `tests/fields.rs`. Add exhaustive tables for every state and preserve IME/raw editing values. |
| U3: Only visible messages appear in description/error references | `src/fields.rs::{descriptions,error_reference}` | Blank `Option<String>` is raw presentation input, not an error witness; caller supplies unique stable DOM IDs | `RUNTIME_VALIDATED`; native string-render tests. DOM ID uniqueness, live focus and assistive technology still require app/runtime evidence. |
| U4: Links retain native navigation and reject unsupported executable schemes | `src/navigation.rs::LinkHref` | Private fields and fallible constructor; no unchecked serde/default. Controls do not decide routes or unsaved business state | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; `tests/actions_navigation.rs`. Browser URL interpretation and focus remain runtime obligations. |
| U5: Dialog close reasons and feedback tones remain explicit | `src/dialog.rs`, `src/feedback.rs` | Render framework/browser effects are not a suitable Kani target | `TYPE_ENFORCED` closed enums; native dialog/feedback tests available. No rendering formal proof; keyboard/focus/reduced motion are not established by SSR. |

### cantos-api: Catalog, ingestion and Script IR

| ID / proposition | Owner and admission boundary | Failure / bypass audit | Initial enforcement; available oracle; residual obligation |
| --- | --- | --- | --- |
| C1: Work, unit and content revision identities cannot be interchanged or edited | `contexts/catalog/domain/{ids,work,unit,content}.rs` | Private ID/snapshot fields, fallible parsers, Serialize only; raw New/Stored facts admitted by constructors | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; domain tests and `catalog_postgres.rs` available. Actor token validity is not actor existence/access. |
| C2: Metadata/accepted revision counters are positive, PostgreSQL-bounded and never wrap | `domain/metadata.rs::{validate_revision,next_revision}`, `content.rs::next_content_revision` | Accepted snapshots currently retain raw `u64` internally after validation; content head zero is an intentional pre-first-append state | `RUNTIME_VALIDATED`; boundary/overflow/conflict tests. Refine accepted counters without changing DTO/DB representation. Valuable small Kani candidate after refinement. |
| C3: Unit category agrees with immutable parent; archived records change only through explicit transitions | `domain/unit.rs`, `reorder.rs` | Restore checks parent kind; transition checks exact parent ID, draft status, sequence and timestamps | `TYPE_ENFORCED` category/lifecycle enums, `RUNTIME_VALIDATED`; all kind pairs, wrong parent, archived/reorder/permutation tests. Relationship and currentness still require SQL transaction checks. |
| C4: Content is immutable, NFC/LF canonical and exact-pinned; content and full revision digests differ | `domain/content.rs`, `application/content_wire.rs`, `postgres/content.rs::decode` | No Deserialize/Default on admitted text/content; SQL restores raw DTO through constructors and compares identity/sequence/digest. Script/source links are references, never clearance | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; `content_tests.rs` literal digests and NFC cases. Add generated canonical/idempotence and constructor-bypass checks; don't renormalize persisted snapshots silently. |
| C5: Rights declarations are bounded unique token references, with owner-only set changes | `domain/metadata.rs`, `content.rs::rights_change_requires_owner` | Raw WorkDetails serde can exist invalid but Work admission validates; declarations are not verified rights | `RUNTIME_VALIDATED`; multi-error/set-order tests. Current access and referenced owner evidence remain shell checks. |
| I1: Import metadata and byte size are admitted together before extraction or durable writes | `contexts/ingestion/application.rs`, `domain/receipt.rs` | Application checks a raw DTO then continues using it; lower storage insert facts are strings/bytes without an admission witness | `RUNTIME_VALIDATED`; metadata/extraction tests. Introduce a narrow immutable validated input consumed by the existing application/storage boundary, preserving all diagnostics and original bytes. |
| I2: Stored import receipt binds original bytes, metadata, outcome and text projection | `ingestion/postgres.rs::decode_import` | Stored serde metadata is revalidated; byte hash/receipt digest/projection compared. Some `Row::get` paths panic on wrong SQL types | `RUNTIME_VALIDATED`; `imports_postgres.rs` corruption/replay tests available. SQL type correctness and role constraints still need real PostgreSQL; do not claim extraction authenticity from a digest alone. |
| S1: Script admission checks bounded structure, global unique IDs, narrator/speaker/cue/provenance resolution | `script_ir/{wire,validation,model}.rs` | Public ScriptContent has only module-private fields, no Deserialize/Default or setters; raw DTO defaults only optional collections | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; `tests/script_ir.rs` accepted/rejected schema corpus, exact diagnostics and malformed input. Internal sibling modules are a trusted construction boundary. |
| S2: Canonical NFC/whitespace/export and c1/e1/speech bytes preserve domain meaning | `script_ir/canonical.rs`, `read_canonical_script` | Stored exports reject noncanonical bytes; original ingestion bytes remain unchanged; exact decimal intensity avoids float-rounding admission | `RUNTIME_VALIDATED`; independent Python/golden digest, normalized-size/intensity/pronunciation tests. Add generated re-admission laws/compile-fail barriers; don't rewrite existing digests or schema. |
| S3: Positional edits don't become identity changes or speech edits | `script_ir/diff`, `SpokenLine` | IDs and speech projections are read-only; placement stays separate | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; `tests/script_diff.rs` independent/property oracles available. No producer cache or backend TTS exists. |

### Remaining cantos-api contexts

| ID / proposition | Owner and admission boundary | Failure / bypass audit | Initial enforcement; available oracle; residual obligation |
| --- | --- | --- | --- |
| E1: Save/review accepts exact replay or checks current revision/access; revisions never wrap | `contexts/editorial/domain/{access,revisions,evidence}.rs`, application/PostgreSQL | Raw request/SQL facts must not confer permanent authorization; replay is immutable historical fact, not new approval | `TYPE_ENFORCED` decision enums, `RUNTIME_VALIDATED`; editorial/revision PostgreSQL tests available. Transaction-time reauthentication/current evidence remain required. |
| A1: External proposals bind exact source, request, script/revision and reviewed coverage | `contexts/adaptation/domain`, application | Raw proposal/generation DTOs are declarations; FrozenAdaptation/FrozenContext/TrustedBinding expose fields (the first two also Deserialize), so they are not sealed provenance witnesses | `TYPE_ENFORCED` closed decision shapes, `RUNTIME_VALIDATED` application/SQL re-admission; `adaptation.rs`, `adaptations_postgres.rs`. Inspect raw frozen-context reuse at lower boundaries in phase 2; no inference backend. |
| P1: Frozen production/brief inputs pin content, guidance, rights/profile and budget facts | `contexts/production/domain`, `application/briefs.rs::restore` | Private Guidance/ProductionBrief fields, no Deserialize/Default; restore re-admits and compares full raw snapshot/digest | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; `production.rs`, `brief_domain.rs`, PostgreSQL tests. A frozen candidate is not eternal rights/access permission or generated audio. |
| M1: Private submission/rendition/review/candidate lifecycles require exact bindings and byte integrity | `contexts/media/domain`, lifecycle application/PostgreSQL | Sealed admitted objects restore from raw snapshots; receipt/storage/current grant facts still need transaction/byte rechecks | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; lifecycle/intake tests. Candidate remains private; `PublicDeliveryNotImplemented` always blocks publication. |
| M2: WAV/PCM/Ogg offsets, chunk counts/durations and digests reject overflow/truncation | `contexts/media/domain/wav_inspection`, `audio_intake.rs`, domain profiles | Numeric byte positions are raw at parser edge; external metadata cannot be trusted as verification | `RUNTIME_VALIDATED`; WAV/Opus hostile bytes and checksum tests. Select pure arithmetic kernel for bounded proof; proof does not establish audio listening/quality. |
| D1: Incremental deltas maintain count/state without changing PostgreSQL authority | `platform/incremental`, optional `dbsp-experimental` | Generic experimental engine only; retired job consumer remains removed | `RUNTIME_VALIDATED`; generic differential engine tests available. No production adoption or DBSP proof implied; assess bounded arithmetic independently. |

### cantos-studio, cantos-theatre and xtask

| ID / proposition | Owner and admission boundary | Failure / bypass audit | Initial enforcement; available oracle; residual obligation |
| --- | --- | --- | --- |
| F1: Studio preserves raw edits/IME, exact save snapshot and ambiguous-outcome reconciliation | `apps/cantos-studio/web/src/{editor,authoring,api,wire}.rs` | Client state is a projection, never backend authorization; raw DTO IDs/revisions may need typed boundary values | `TYPE_ENFORCED` state/error enums, `RUNTIME_VALIDATED`; editor/authoring tests. Live Catalog/audio flows on independent draft PR #73 are not on this baseline. |
| F2: Stale async response, logout/unmount or repeated write cannot replace newer client intent | Studio import/adaptation/production/browser modules | Operation/request identity and reactive lifetime need explicit audit; DOM guards alone cannot prove backend outcomes | `RUNTIME_VALIDATED`; app tests available. Phase 3 refines existing application state without importing backend rules. |
| T1: Playback intent and observed mock engine state are distinct; stale session events are ignored | `apps/cantos-theatre/web/src/core/playback`, `application/player_session.rs` | Private state/validated positions; public raw events are not engine evidence | `TYPE_ENFORCED`, `RUNTIME_VALIDATED`; reducer/vector tests. This is session-only fake media, not native/browser production playback. |
| T2: Catalog/resource identity, queue and progress remain stable across navigation/interruption | Theatre core/application, shell context | Publication/catalog strings are mock values; local session memory is not backend release authority | `RUNTIME_VALIDATED`; catalog/listening/route tests. Phase 3 audits actual code only; no invented Theatre API. |
| X1: Changed-path gate selection is deterministic and unknown impact broadens coverage | `xtask/src/preflight/{changes,graph,select}.rs`, `gate.rs` | Raw paths map into typed gates; no Kani gate currently registered | `TYPE_ENFORCED` enums, `RUNTIME_VALIDATED`; selector/unit tests. Add an opt-in real Kani workflow in phase 3; ordinary tests must never be reported as proofs. |
| X2: Reports distinguish PASS, FAIL, NOT_RUN and UNSUPPORTED and preserve owned-process lifetimes | `xtask/src/{report,pipeline,process,tools}.rs` | Missing tools/failures/skipped gates are reported distinctly; a successful process with zero selected tests still needs evidence review | `TYPE_ENFORCED` result enums, `RUNTIME_VALIDATED`; xtask tests available. No CI polling, paid resources, or new mandatory proof lane. |

## Concrete residuals to revisit in phases 2 and 3

These are source-audited obligations, not reproduced end-to-end defects:

- `production/domain/commands.rs::decide_rights_version` caps `u64` overflow,
  while persistence `checked_number` rejects values beyond `i64::MAX`. Bring the
  pure decision into agreement with its real storage domain.
- `AuthorizedProductionInputs` evaluates a snapshot without a current-owner
  argument; the application uses `evaluate_for_owner`. No dispatcher consumes
  this planning witness. Any added consumer must revalidate owner/currentness.
- Generic incremental `Page`/`GroupCounts` expose raw fields, and frontier/count
  application admits regressing or inconsistent facts. The DBSP adapter adds
  checks and there is no live domain consumer; do not invent one in this audit.
- Media admission assumes the SQL clock is authoritative; the pure kernel needs
  explicit evidence for negative or pre-created timestamps.
- Editorial row reconstruction rejects negative revisions but relies on SQL for
  positive (nonzero) sequences; some replay casts similarly rely on constraints.
- Production `estimate_dialogues` with no lines may return known zero before
  validating a present malformed rate. `prepare_inputs` validates settings first.
- Producer scope restoration relies on issuance and SQL for maximum grant
  lifetime; scoped read/currentness must still be checked at each transaction.
- Generic incremental snapshot parsing and public paging facts have weaker
  standalone contracts than DBSP adapters. Theatre resume snapshots are raw;
  the playback reducer validates them, but local memory can retain invalid facts.
- Strong initial Kani candidates are editorial save append/replay, checked
  revision/brief arithmetic, cost ceiling arithmetic, or bounded byte offsets.
  A successful bounded proof would still not establish storage or authorization.

## Ordered implementation / proof obligations

1. Record this inventory before code changes.
2. Contracts: strengthen current DTO numeric/unknown/malformed/truncation/round-trip
   evidence while keeping raw values and route codec scope unchanged.
3. Shared UI: make every state projection exhaustive, retain component consumers
   and raw editing values; test the complete state matrix. No Kani rendering.
4. Catalog/ingestion/Script IR: seal admitted revision/import boundaries, cover
   constructor/reconstruction and canonicalization laws. Preserve immutable IDs,
   all persisted bytes, existing wire shapes/migrations and domain authority.
5. Editorial/adaptation/production: audit frozen/current witness reuse at lower
   operation and transaction boundaries, including exact replay versus new authority.
6. Media/incremental: strengthen lifecycle and checked byte/arithmetic kernels
   without widening publication or the experimental DBSP boundary.
7. Kani: pick two or three real production kernels, record symbolic domain,
   nonvacuous coverage, assumptions/unwind bounds, version and actual verifier result.
8. Studio state; 9. Theatre state; 10. opt-in xtask Kani selector; 11. final evidence.

The work is delivered in one sequenced Draft PR, in three sequential phases
(1–4, 5–7, 8–11). No merge or squash. A no-op audit is recorded rather than
creating an empty commit or an abstraction without a current consumer.

## Verification record (initial)

- Rust/Cargo were initially absent on the dot cloud computer. Official Rust
  1.87.0 (`rustc 1.87.0 (17067e9ac 2025-05-09)`), rustfmt, clippy and the WASM
  target were installed into isolated task tooling; no repository or production
  default version was changed.
- `cargo test --locked -p cantos-contracts -p cantos-ui -p cantos-api --no-fail-fast`
  passed 388 tests at the unchanged baseline; 135 tests were explicitly ignored,
  including real PostgreSQL cases. Ignored cases earn no integration evidence.
- `cargo xtask verify repository` passed repository checks and 221 Python tests
  for this inventory. `cargo xtask preflight --changed-only --base origin/develop`
  passed repository and Rust-format gates. These checks do not establish
  application behavior.
- Kani is not yet implemented or executed: `NOT_VERIFIED` / NOT_RUN.
- PostgreSQL integration, browser interaction, screen-reader, audio listening and
  remote CI: NOT_RUN in this audit. Existing evidence is historical provenance.

Subsequent commits append attributable execution results and remaining obligations
rather than replacing this initial inventory with retrospective claims.

## Phase 1: contracts checkpoint

W2/W4 coverage now includes every ContentBody variant, current Content fixtures,
raw unsigned/signed limits, negative/fractional/overflow numbers, nested unknown
authority fields, every strict request prefix (including UTF-8 splits), trailing
bytes and PATCH missing/clear/value across both codecs. This is codec regression
evidence, not MessagePack route enablement or semantic admission.

The existing production wire decoder/DTO shapes needed no justified behavioral
refactor after audit; they remain unchanged. No framework/newtype was added to
raw transport. `cargo test --locked -p cantos-contracts` passed 61 tests, including
9 added Content cases; package clippy with warnings denied passed. `cargo xtask verify contracts` passed all four gates: Script IR reference,
contract tests, independent WAV reference and actual-byte audio inspection
(two ignored real codec/profile tests explicitly executed). Repository verification
also passed. No PostgreSQL or browser integration is claimed at this checkpoint.

An initial test expectation classified a JSON prefix cut inside a Vietnamese
UTF-8 sequence as Truncated. The executed decoder correctly reports InvalidUtf8
before JSON parsing. The final test distinguishes these exact error classes;
no decoder behavior was changed to satisfy the test.
