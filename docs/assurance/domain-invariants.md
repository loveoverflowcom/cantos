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
(1–4, 5–7, 8–11). Keep implementation commits ordered and unsquashed; do not
merge the PR into `develop`. The user's later completion instruction queues a
merge of latest `develop` into the feature branch after all three phases, then
one separate Sol 6.1 Ultra review workchat with fixes pushed to this same PR.
A no-op audit is recorded rather than
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

## Phase 1: shared UI checkpoint

U1/U2 now use exhaustive matches for action/input, disabled, busy and read-only
permissions. Adding a control state requires handling each production projection;
text fields and textareas use the same read-only projection. Public component
parameters, consumer call sites, raw editing values and composition callbacks are
unchanged. No domain rule, rendering proof, token/style or localization was added.

`cargo test --locked -p cantos-ui` passed 53 tests (including two new full-state
matrix tests and two compile-fail checks for private/non-default nonempty text).
Package clippy with warnings denied passed. Existing native string-render tests
and the new text-state matrix cover HTML associations/state projections; they
are not live DOM, browser interaction, pixels or screen-reader evidence. No
rendered behavior was intentionally changed. `cargo xtask verify web` passed all
four gates: both apps' WASM lint, Studio build, Theatre build and UI preview.
The first run failed because Trunk's default cache directory was read-only;
the retry used an isolated task cache through `XDG_CACHE_HOME`. These successful
builds still do not establish live interaction, visual or assistive-technology
behavior.

## Phase 1: Catalog, ingestion and Script IR checkpoint

C1/C2 retain their existing distinct ID types and immutable snapshot fields.
Accepted Work/ContentUnit/ContentRevision/ScriptRef sequences now use a private
`Revision` representation with a fallible `1..=i64::MAX` admission and checked
successor. It has no Deserialize, Default or unchecked From path. Raw New/Stored
facts, public numeric projections, conflict/diagnostic order and the intentional
zero pre-first-content head remain unchanged. Serialization is transparent.
This is `TYPE_ENFORCED` plus `RUNTIME_VALIDATED`, not an authority witness or proof.

I1 now admits immutable metadata and original bytes together in `AdmittedImport`.
Both the existing application and stored-row reconstruction consume the same
fallible boundary, and insertion derives stored metadata from that admitted
value instead of taking a second metadata snapshot. The original diagnostics
and their ordering, bytes, extraction behavior and replay comparisons are
preserved. The witness states intrinsic metadata/size validity only: empty or
malformed source bytes may still produce an extraction failure. It does not
certify receipt integrity, parser success, rights or transaction authorization.
I2 continues to check stored byte hash, outcome digest and text projection.

S1 already had sealed validated ScriptContent and read-only identity; its runtime
implementation needed no justified replacement. No schema, canonical digest,
wire shape, SQL migration or persisted-byte rewrite was introduced. Seven API
compile-fail examples now check distinct IDs, immutable Work identity, the
absence of Deserialize on Work, and the absence of Deserialize/Default on
admitted ManuscriptText/ScriptContent.
The mutation runner's exact baseline count increases from 15 to 16 to include
the new Script IR property; its zero-test, failure and mutant guards remain intact.

The following generated laws actually ran (`PROPERTY_TESTED`; proptest defaults
otherwise unchanged). Bounded samples are not exhaustive or model-checked:

| Production boundary / law | Generated domain; cases and fixed RNG seed |
| --- | --- |
| Revision admission/successor equals an independent u128 oracle; transparent JSON | arbitrary u64; 256 cases; 20261013, plus explicit zero/i64/u64 edges |
| Manuscript NFC/LF canonicalization is idempotent and NFD-equivalent; wire restoration reproduces content/digest | 0–63 chunks from 11 Vietnamese/decomposed/whitespace/emoji choices, after a valid prefix; 128 cases; 20261014 |
| Script canonical export re-admits with identical bytes/content digest/speech; NFD-equivalence | accepted two-scene fixture with one dialogue varied by 0–63 chunks from 11 choices; 128 cases; 20261015 |
| Admitted import raw projection preserves original bytes and metadata | arbitrary byte vectors of length 0–256; 128 cases; 20261016, plus exact maximum-size and diagnostic-order cases |

`cargo xtask verify rust` passed formatting, warnings-denied workspace clippy and
all workspace Rust tests, including the seven API and two shared UI compile-fail
examples. API library tests passed 131 cases. Ignored PostgreSQL tests remain
uncredited. Targeted Catalog tests and the complete API suite also passed during
implementation; final workspace checks include the final source changes.

`cargo xtask verify postgres` failed before test execution. Official PostgreSQL
17.11 tools initialize the disposable cluster, but this environment denies
creating `/tmp/.s.PGSQL.<port>` with `Operation not permitted`. An exact reviewed
escalation retry failed at the same boundary. No runner, socket/network policy,
database or integration expectation was weakened to obtain a green result.
PostgreSQL behavior remains `NOT_VERIFIED` / FAIL; no integration pass is claimed.
Kani remains NOT_RUN pending the phase 2 kernels and phase 3 opt-in workflow.

Final changed-only preflight (`cargo xtask preflight --changed-only --base
origin/develop`) ran all 13 selected gates: repository (221 Python cases), Rust
format/lint/tests for the five affected crates, experimental DBSP lint/tests,
WAV reference, actual-byte audio inspection, both apps' WASM lint/builds, UI
preview and mutation all passed (12 gates). PostgreSQL alone failed at the
environment boundary above, so preflight's overall result is **FAIL**, not PASS.
The nine actual source mutations were all killed, with zero survivors, timeouts
or unviable mutants and an exact 16-test passing baseline. This is targeted
mutation evidence, not a correctness proof or DBSP production adoption.

The first expanded preflight stopped at architecture validation because a domain
property referenced its wire adapter. The unchanged reconstruction law now lives
at the application boundary. Visibility declarations on the narrow import value
were adjusted to crate visibility while preserving private fields and fallible
construction. No architecture checker or failure expectation was weakened.

## End-of-phase fresh-develop rebase

Fresh `develop` advanced to `3a78e77a2190cd443401ed95d58c5933e302724e`,
including PR73 live client/finding fixes and PR76 engineering guidance. The
task-only rebase applied all four ordered commits without source conflicts;
the original published head was retained locally before rewriting task history.
The initial inventory and its evidence above remain attributed to the original
`4f33a90` baseline, rather than being relabeled as fresh-base evidence.

The current baseline now includes bounded Studio Catalog/Brief/private-audio
forms and an explicitly unsupported Theatre transport boundary. Phase 3 must
audit their actual async state and preserve PR73's work-selection, session/cookie
and draft-preservation fixes. No production listener API or real playback engine
was implemented by this rebase or hardening work. The merged app, HTTP shell,
repository instructions and existing xtask changes were retained unchanged.

Against this fresh baseline, full workspace Rust verification passed 727 tests
with 136 explicitly ignored cases, and contracts verification passed all four
gates again. Expanded changed-only preflight passed the same 12 gates, including
225 Python repository cases, affected-crate tests, DBSP, both WASM builds, UI
preview and all nine killed mutants. PostgreSQL alone failed before tests with
the same Unix-socket denial, so the rebased preflight result remains **FAIL**.
Ignored cases, Kani, live browser/assistive-technology and audio listening earn no
new verification credit from the rebase.

## Phase 2: editorial, adaptation and production checkpoint

E1 stored accepted sequences now pass through a private `AcceptedRevision`: positive signed
PostgreSQL counters and the exact predecessor are admitted before save replay or revision
reconstruction. Review-operation reconstruction admits a positive counter too; unchecked signed
casts and a zero accepted revision are removed. Raw pre-first-save heads still permit zero.
SQL queries, stored/wire fields, canonical exports and replay-before-staleness behavior remain
unchanged. The pure constructor has fixed edge examples and a 256-case fixed-seed independent
`i128` sequence oracle. This is `TYPE_ENFORCED`, `RUNTIME_VALIDATED` and `PROPERTY_TESTED`;
real row corruption/concurrency evidence is still blocked by the recorded PostgreSQL startup
failure. It is not a formal proof or a replacement for current access checks.

A1 retains raw serialized `FrozenAdaptation`, `FrozenContext` and public `TrustedBinding` facts.
The materializer and edited-proposal kernel now consume a private borrowed `BoundSource` built
only by intrinsic source/binding admission. It pins the exact source and binding while used;
both public entry points re-admit every reuse, including changed source ID/hash, malformed
record IDs and namespaces. This witness proves neither authentic provenance nor source/base
registry integrity, ownership, rights, human coverage review or current authorization. Existing
SQL reconstruction, pinned source/base comparisons, actor/run locks and repeated authentication
remain in place. All transport and frozen digest shapes are unchanged.

P1's planning authorization witness now requires the current owner and calls the same
`evaluate_for_owner` gate used by the application. An old-owner frozen approval cannot mint the
witness after transfer. The witness remains a point-in-time decision over supplied facts; no
paid dispatcher exists, and any future consumer must reload/recheck owner, rights, review,
head, settings and time under its transaction fence. Compile-fail examples reject forged fields
and direct Deserialize. No claim of perpetual authority is made.

The production rights successor now refuses a value above `i64::MAX`, matching the actual
storage domain. Both this limit bug and the owner-transfer disagreement were reproduced by
failing tests before implementation; the same regressions pass after the fixes. Existing
settings/budget estimation, historical frozen-document re-admission and approval precedence
were audited and retained. Producer-grant maximum lifetime still depends on issuance and SQL:
the public read-scope snapshot contains expiry but no issuance timestamp, so no invented field
or guessed standalone lifetime rule was added.

Execution on the phase-1 fresh base, Rust 1.87.0:

- `cargo test --locked -p cantos-api --lib --test production --test adaptation --test brief_domain`:
  134 unit, 22 production, 15 adaptation and 8 brief-domain tests PASS.
- `cargo test --locked -p cantos-api --doc`: all 9 compile-fail checks PASS.
- `cargo clippy --locked -p cantos-api --all-targets -- -D warnings`: PASS.
- `cargo fmt --all -- --check`, `git diff --check`: PASS.
- `cargo xtask verify repository`: PASS, including 225 Python tests.
- Real PostgreSQL integration remains NOT_RUN after the earlier verified startup/socket denial;
  no ignored SQL test is counted as a pass. Kani harness execution remains NOT_RUN here.

Official Kani 0.68.0 and CBMC 6.11.0 were successfully installed/setup in isolated verification
tooling. Its required nightly-2026-08-21 (rustc 1.100.0-nightly) bootstraps the installer because
its pinned `home` dependency needs Rust 1.88; the production/default Rust selection remains
1.87.0. Installing the verifier is not execution evidence. Commit 7 will record actual harnesses,
commands, bounds, coverage results and limitations.

## Phase 2: media and numeric-kernel checkpoint

M1 retains a private immutable `SubmissionWindow` with nonnegative creation, a positive lifetime
of at most 86,400,000 ms and checked subtraction. Submission creation and SQL/raw restoration
use its one admission boundary; rendition restoration and admission consume its inclusive-start,
exclusive-expiry membership rule. Negative/pre-created admission clocks and replay before the
settled admission now fail closed. A supplied settled rendition must bind the same submission
ID/digest, exact brief/content binding and declarations before it can authorize replay. These
lower-boundary defects were reproduced before fixes. Existing revocation/expiry/stale/digest
precedence for previously valid facts is retained; transaction-time SQL clock, authorization,
revocation, exact content/brief and stored-object checks remain required.

M2's real WAV payload-access kernel checks header addition, payload addition, conversion to
platform-sized indices and the actual buffer length before slicing. The previous raw-offset
`u64::MAX` panic was reproduced directly; the normal RIFF walker already bounds its offsets,
so this is defense-in-depth rather than a newly demonstrated hostile-file exploit. Empty,
exact-end, one-byte overrun, maximal offsets/sizes and a 256-case fixed-seed independent `u128`
oracle are covered. The PCM sample-width/channel products already use bounded widths and
widened byte-rate arithmetic; no justified behavior replacement was needed there.

Actual-byte `VerifiedAudio` already has private original bytes/metadata and no raw/serde
constructor, and storage consumes it before committing a receipt. Scheme-specific SHA-256
admission, stored canonical payload/digest comparisons and byte/hash rechecks were audited and
retained. Raw metadata or a restored rendition alone does not certify the current private object
or audio/listening quality. No checksum version, wire shape, stored format or migration changed.

D1's raw flag counts are now validated at the engine-independent CountView staging boundary,
with the same predicate consumed by the optional DBSP adapter. An impossible insertion or
replacement rejects the entire step without changing the view. `Page` has private immutable
fields admitted only by `plan_page`, retains its starting frontier, rejects zero page limits,
and checked position advancement rejects a mismatched starting frontier or advancing-page replay.
A compile-fail check rejects fabricated page fields. Examples and a fixed-seed sorted-set paging
oracle cover idle/exhausted/full windows, row count, monotone frontier, retained bootstrap and
replay refusal. A page certifies neither bootstrap identity, complete SQL/whole-transaction
retrieval nor successful engine application. There is still no live domain consumer of this
experimental paging/count library, and PostgreSQL remains authoritative; no feed, worker or
production DBSP adoption was introduced.

Executed on the same fresh phase-1 base, Rust 1.87.0:

- API library tests: PASS, 146 tests after the final membership/paging property edits.
- API doc tests: PASS, all 10 compile-fail checks.
- API clippy with all targets and warnings denied: PASS.
- `cargo xtask verify contracts`: all four gates PASS, including independent WAV/ffprobe and
  two explicitly executed actual synthetic Opus inspection cases.
- `cargo xtask verify dbsp`: PASS, 325 package/doc tests and 136 ignored cases. It exercises the
  real feature-gated in-memory circuit but does not establish PostgreSQL/CDC integration.
- Final membership generation additionally samples created/expires/expires-minus-one clocks,
  rather than relying on independent random clocks to hit a short admitted window.
- Real PostgreSQL integration remains blocked/NOT_RUN; no public delivery or listening result
  is implied. Bounded Kani propositions and attributable execution will be recorded in commit 7.

Final commit-6 incremental/property retest: all 20 focused incremental tests PASS with the real
`dbsp-experimental` feature, including the final paging law and the circuit/recount oracle.
Repository verification PASS with 225 Python cases; final Rust format/diff checks PASS. Independent
source review found no actionable defect and explicitly limited Page evidence to the starting
frontier. PostgreSQL integration remains the separate blocked acceptance obligation.

## Phase 2: bounded proof checkpoint

C2 / M2 / M1 gain only the specific `KANI_VERIFIED` propositions in
[the executed Kani record](kani.md): production Catalog revision admission/successor,
WAV payload-range arithmetic and submission-window admission/membership.
Kani 0.68.0 / CBMC 6.11.0 ran all three actual production kernels on full-width
symbolic inputs, no assumptions or stubs, unwind 2 with checks enabled.
All 21 reachability covers were satisfied; no assertion or unwinding check failed.
Source identities, exact commands, reachable edge classes and intentional
unreachable error branches are recorded there. This is `bounded-model-checked`,
not a proof of transaction authority or the entire lifecycle/parser.

The verifier is isolated from normal Rust 1.87.0 and introduces no runtime
proof dependency or lock change. xtask adoption remains ordered commit 10;
existing Theatre Kani harnesses are not executed by this checkpoint.
SQL integration is still NOT_RUN because socket startup was denied; current
authorization/time/rights/revocation/storage acceptance remains a shell obligation.

### Delivery boundary update

During phase 2, PR #75 was externally merged with only ordered commits 1–4
(merge `a321a3fdc9dd291a8e45b44df33bc9536e991319`). A merged PR cannot accept
additional commits. The user explicitly approved one follow-on Draft PR for
ordered commits 5–11 and the later separate Ultra review. This supersedes the
original single-PR delivery plan above; it changes no assurance or merge gate.
The phase-2 handoff requires rebasing onto fresh `develop`, retaining the already
merged phase-1 work and updated skills/service-runtime changes. The task does not merge
the follow-on PR into `develop`.

### Phase-end independent gate record

The fresh-develop rebase preserves merged phase-1 work, service-runtime changes
and the updated layered-verification / diagnostics skills. The three exact Kani
kernels were rerun after the first rebase; the subsequent diagnostics-only base
advance changes no Rust source, Cargo manifest/lock or proof configuration.
[The proof record](kani.md) binds both the executed model and that comparison.

`cargo xtask verify rust` actually ran after the rebase: workspace formatting and
warnings-denied clippy **PASS**, workspace tests **FAIL** (742 passed, 18 failed,
136 ignored; Cargo stops before later doctests). All API test executables completed
successfully before the upstream xtask failures. Seventeen service-fixture tests
reject the existing read-only `/tmp/.git` ancestor. One representative test on
clean detached `develop` `782793dcee86b10211e6090b419a8e6fcba967fb` reproduces
that exact rejection before runtime effects; the private-input guard and test
are unchanged by this phase. This is pre-existing / environment-triggered,
not a passing gate and not permission to weaken the private-input guard.
The eighteenth failure is the unchanged port-ownership test's post-drop
`port_is_free` assertion, not an observed initial-bind EPERM. Its precise cause
is not established by that output. Focused diagnostic results remain separate;
no later pass erases the original workspace failure.

Disposable PostgreSQL integration remains NOT_RUN after the earlier denied
socket startup. Full-workspace / upstream xtask acceptance and real SQL
acceptance remain explicit obligations; only the named passed core/proof scopes
are handed off as verified. Owner: unassigned — assignment required before
acceptance. No temporary instrumentation, permission change, cache cleanup,
container start or security-guard bypass is part of this diagnostic handoff.

After the final docs-only base advance, `cargo xtask verify contracts` passed all
four gates. `cargo xtask preflight --changed-only --base origin/develop --skip postgres`
passed seven applicable gates: repository (225 Python tests), Rust format,
API lint/tests (321 passed, 136 ignored including unrun SQL), optional DBSP
(326 passed, 136 ignored), independent WAV reference and actual-byte audio
inspection (two explicitly executed Opus cases). PostgreSQL stays **NOT_RUN**,
explicitly skipped for the earlier denial. These independent passed scopes do
not supersede the separately recorded full-workspace **FAIL** above.
Two focused isolated port-test repetitions passed; its original full-suite
failure and unconfirmed cause are still recorded. Diagnostic worktrees/processes
were cleaned up, and no temporary instrumentation remained.

## Phase 3: Studio application-state checkpoint

F1/F2 now retain one private `Exchange` algebraic state (Idle, Pending, Ready,
Failed or Ambiguous) with read-only projections, rather than independently
writable phase/pending/result/error/uncertainty flags. Immutable command intent
and request tickets bind completion and reconciliation. The actual production
`Command::check_reply` admits only the identity, version, operation, byte and
aggregate bindings that the existing DTOs expose. Wrong-target acknowledgements
fail as typed protocol/reply mismatches; no backend authorization is duplicated.
The Production projection retains the exact unknown operation after a later
Forbidden or NotFound retry. Editor save findings require the pending actor,
script and current bytes to equal the submitted snapshot before the shell
attaches them. Newer native editing bytes are not replaced or normalized.

Five named regressions failed before repair: another Work response, another
Prepare operation receipt, unknown Production outcome followed by Forbidden,
unknown outcome followed by NotFound, and findings arriving after newer draft
text. All now pass. Ordinary Rust 1.87 checks passed 166 unit tests and one
compile-fail construction barrier; focused Live 23, Editor 25 and Production 26
also passed. Native/WASM warnings-denied Clippy, formatting, repository225 and
all four Web gates passed. These are `TYPE_ENFORCED`, `RUNTIME_VALIDATED`, exact
example/adapter tests and compilation evidence; no Studio Kani proof is claimed.

The existing Editor/Import/Adaptation/Inspector snapshots and adequate stale
response/actor/resource fences were audited and retained. PR73 default selection,
owner-session versus producer-token cookie behavior and chip/raw-buffer fixes
remain intact. Client route strings still use the existing fallible canonical
path admission; not every DTO identifier became a statically distinct newtype.
ContentHistory has no echoed unit, Candidate no echoed submission, Review no
unit/submission binding, mutations no operation echo and pages no cursor echo.
Those gaps remain request-ticket/backend obligations; no field was invented.

Real out-of-order browser dispatch, cancellation/unmount, unknown-write receipt
reconciliation, revision conflict, cookie races and live Vietnamese Telex/VNI IME
acceptance remain NOT_RUN. Check newer native typing with delayed findings and
unchanged submitted text with valid findings; build success is no runtime or
accessibility result. Acknowledged Editor revision progression and malformed
accepted-export projections remain narrower hardening opportunities under an
unverified corrupt/misbound upstream-response assumption, not a demonstrated
normal-path failure or new product rule.

## Phase 3: Theatre listener-state checkpoint

T1/T2 now retain a private immutable `ValidatedResumeSnapshot` instead of raw
resume facts in progress. Fallible admission checks exact source, bounded
position and supported speed, with unchanged OutOfBounds → UnsupportedSpeed →
generation-exhaustion precedence. Reducer loading, PlayerSession exports,
restoration, feedback and mock callers consume its read-only projections. Raw
input events remain raw and invalid events reject atomically. Both position 60,001
for duration 60,000 and speed 999 previously replaced valid progress; both
regressions failed before the repair and now pass.

Probe generation/phase fields are private. Its existing checked begin/cancel/
finish methods remain the transition authority. Check/Cancel now admit and
commit generation advancement before the shell aborts transport; exhaustion
leaves state and transport unchanged. The external struct-literal compile-fail
regression and source-order regression failed before repair, then passed. Source
ordering does not establish actual browser AbortController execution.

The initial checkpoint passed 66 unit tests and three compile-fail doctests.
After integrating merged PR83, the package passes 86 unit + three compile-fail
tests. Its two-destination navigation and combined Continue/Saved/History remain
intact; existing position recency projection and single root player survive.
Finite tests cover all u16 speeds × six position partitions (393,216 pairs),
625 four-step retention sequences (2,500 transitions) against a naive
last-occurrence oracle, and 1,600 probe completion cells. Existing 19-event
reducer traces up to depth 3, seven playback vectors and six probe cases passed.
These are deterministic tables/bounded-exhaustive tests, not a universal formal
claim or random-property campaign. Native/WASM Clippy, formatting, repository225
and four Web gates passed. DeferredFake/RecordingMedia doubles exercise existing mock
adapter effects; no genuine engine/network/audio boundary was crossed.

Listener intent remains distinct from observed fake-engine state; stale source/
generation callbacks cannot replace newer intent. Root player construction stays
above route leaves. Synthetic catalog/episode keys, session-only progress,
queue/bookmark/saved-work/history separation and explicit mock versus unsupported-live
roots remain intact. No publication identity, persistent synchronization, Theatre
API, real browser/native audio or completed-listen feature was invented. Existing
Theatre Kani harnesses remain unregistered/NOT_RUN; their bodies are unchanged.

## Phase 3: targeted verifier checkpoint

`cargo xtask verify kani` now owns three registered exact API-library kernels,
with repeated typed `--harness` subset selection; omission selects all three.
Only explicit `preflight --kani` opts in. Normal builds, `verify rust`, ordinary
full preflight, CI and merged PR77 service lifecycle remain unchanged. Pure
changed-input selection narrows kernel owners and conservatively broadens
shared/uncertain inputs. Missing/incompatible tools are UNSUPPORTED; skips or
stopped later rows are NOT_RUN; incomplete/zero/wrong harness evidence, failed
checks/covers, unexpected unreachability or input/lock changes FAIL.

The installed absolute Kani bundle driver is used directly in cargo-plugin mode,
with fixed versions/model and no installer/setup, download or default toolchain
change. Rust binds the same exact bounded configuration buffer it parses and
hashes, including absent Cargo-config candidates. Python checks that approved
candidate identity before probing/proof and on every outcome. Configuration
consumption stops at 1 MiB + 1; child output is capped at 4 MiB and proof execution
at 900 seconds. Cleanup signals only the owned process group before reaping its
leader. Model-source hashing streams bounded-memory chunks but has no total
source-size cap; endpoint identities assume frozen inputs and do not isolate
against concurrent mutate-and-restore edits.

Fifty-six focused xtask tests and 73 Python synthetic parser/configuration/process
tests passed; repository verification passed 298 tests and all 18 skills. Those
fixtures are ordinary tests, not proofs. Actual `cargo xtask verify kani` reran
all three production harnesses after review repairs and after fresh-develop
integration: Catalog 100 checks/6 covers, WAV 20/7, submission window 10/8, all
PASS with lock/config/model identities unchanged. The [proof record](kani.md)
binds raw logs, metadata, exact invocation and model/version limits. Targeted
review findings in approval freshness, relative bundle paths and Rust read
allocation were fixed/reviewed; paired Python total configuration consumption
was capped too. The final targeted source review has no actionable finding.

## Final six-crate invariant ledger

This is the current enforcement/evidence projection. The initial inventory and
all dated execution records above remain historical, with their original bases
and outcomes. `TYPE_ENFORCED` means safe public construction barriers, not
formal authority; `RUNTIME_VALIDATED` means actual production admission. Example,
compile-fail and bounded-exhaustive tests remain distinct from random properties
and actual model-checking. No crate receives a whole-program proof label.

| Crate / owning invariants | Implemented enforcement and independently executed evidence | Residual runtime obligation / bounds |
| --- | --- | --- |
| `cantos-contracts` — W1–W4 | Existing strict DTO/codec decoder retained; all Content variants, integer bounds, unknown/nested fields, exact prefix/trailing-byte and PATCH distinctions tested in 61 package cases and the contract gates. Raw serde values are deliberately not domain witnesses. | HTTP size/admission/current authority remains API-owned; no widened MessagePack route scope, fuzz/formal or network claim. |
| `cantos-ui` — U1–U5 | Private nonblank reason/name types and closed control-state enums retained; exhaustive action/field projection with complete state matrices; 53 tests including compile-fail barriers, native rendering and Web compilation. No justified new rendering abstraction. | Live DOM ID uniqueness, focus/keyboard, contrast/pixels, screen reader/reduced motion and genuine IME remain separate acceptance; no rendering Kani. |
| `cantos-api` — C1–C5, I1/I2, S1–S3, E1/A1/P1/M1/M2/D1 | Sealed storage-bounded revisions, admitted immutable import, existing sealed Script IR, positive editorial reconstruction, consumed bound adaptation source, current-owner planning witness, storage-bounded rights successor, submission lifetime/clock/replay bindings, checked WAV range and atomic experimental count/page transitions. Compile-fail/example/fixed-seed independent properties and actual three-kernel Kani results recorded above and in [proof record](kani.md). | SQL/current ownership/rights/head/time/revocation/constraints/byte object authenticity and transactions remain rechecked at actual shells. PostgreSQL NOT_RUN after denied startup. DBSP opt-in only, no live consumer; publication always unimplemented. Only three 64-bit local kernels model-checked. |
| `cantos-studio` — F1/F2 | Private single exchange and immutable retry provenance; real DTO-available reply binding; ambiguous operation retention through later denial; exact save-finding snapshot guard. 166 unit + one compile-fail test, five fail-before regressions, adapter fixture checks and native/WASM/Web checks. Adequate existing client validation/projections retained. | Raw projection/echo gaps and current backend auth remain explicit. Browser lifetime/cookies/receipt reconciliation/revision conflicts, Vietnamese IME/native buffer and accessibility NOT_RUN. No client authorization witness or whole-UI proof. |
| `cantos-theatre` — T1/T2 | Private admitted resume checkpoints, private probe state, immutable checked transitions before transport abort; existing intent/engine, stale-event and root lifetime boundaries retained. 86 unit + three compile-fail tests after PR83 integration; exact fail-before checks, independent finite retention oracle, mock adapters and unchanged shared vectors. | Synthetic/session-only mock model; unsupported live boundary. Browser abort/unmount, release identity/storage/CDN/auth/audio/native/persistence NOT_RUN or unimplemented. Two existing Theatre Kani sources NOT_RUN. |
| `xtask` — X1/X2 | Registered typed opt-in Kani gate selection and fixed model/tool compatibility, strict evidence validation, lock/input identities and owned bounded subprocess cleanup. Ordinary default full/build/CI and merged service lifecycle commands retained. 56 focused xtask + 73 Python tests PASS; repository 298 PASS. The separate full-workspace Rust gate still fails in 17 unchanged service fixtures. | Ordinary parser/pipeline tests are not proofs. Missing/incompatible prerequisites are UNSUPPORTED; skips/stopped later gates NOT_RUN. Actual workspace service-fixture failures remain unresolved; no security guard weakened or mandatory paid/proof CI lane. |

## Final execution and remaining acceptance

On the integrated Rust/Web source, `cargo xtask verify rust` with
`RUST_TEST_THREADS=1` passes formatting and warnings-denied workspace Clippy,
then **FAILS** workspace tests: 799 passed, 17 failed, 136 ignored; later doctests
are not reached by that command. All 17 fail in unchanged upstream service
fixtures at the same private-input/Git-ancestor guard observed and reproduced
against clean develop in phase2. No guard, fixture root or assertion was weakened.
The earlier phase2 full result (742 passed/18 failed/136 ignored) remains actual
historical FAIL. Its port post-drop assertion passes this serial run and two
focused repeats; the cause remains unconfirmed, not a proven socket denial.

Independent `cargo xtask verify contracts` passes all four gates, including actual
synthetic-byte audio cases. `cargo xtask verify dbsp` passes its optional feature
suite and ten compile-fail checks; DBSP remains unadopted by live domain consumers.
The integrated Web four gates, repository 298, Theatre 86+3 and actual Kani three gates pass independently.
A separate `cargo test --workspace --doc --locked` passes all 16 compile-fail
checks (API 10, UI 2, Studio 1, Theatre 3), after the full Rust command stopped before
doctests. The default API scope therefore has 311 passed executable tests plus
10 separately passed construction barriers; optional DBSP has 326 total passed,
136 ignored. These counts are scoped, not one aggregate all-green workspace.

The required final changed-only preflight explicitly opts into Kani and skips
only PostgreSQL and the two excluded/missing-Gradle CMP gates:

```sh
RUST_TEST_THREADS=1 cargo xtask preflight --changed-only --base origin/develop \
  --kani --skip postgres --skip cmp-ui-preview --skip theatre-cmp-demo
```

It **FAILS** at the same 17 workspace service fixtures: repository 298, rust-format,
Script IR reference and workspace lint PASS; rust-tests FAIL. DBSP, WAV/audio,
Web/preview, mutation and the three Kani rows are NOT_RUN in that preflight because
execution stopped. CMP and PostgreSQL are NOT_RUN by explicit skip. The separately
executed contracts/DBSP/Web/Kani results above remain independent PASS, never a
rewritten preflight status. Mutation was not rerun in phase3; the unchanged Script
IR validator retains its separately recorded phase1 campaign, not a new claim.

PostgreSQL is NOT_RUN after the previously denied Unix-socket startup, including
reviewed escalation. No alternate startup/guard bypass was attempted. CMP is
outside this hardening scope; merged PR85 is retained upstream, with no added
CMP implementation or runtime acceptance. Builds and mock/native string tests
are compilation/test evidence, not new browser, pixels, IME or audio acceptance.

All eleven planned implementation/audit steps are delivered in their ordered
history; adequate audited boundaries were retained rather than given empty
commits, phantom wrappers or new runtime rules. PR75 contains merged 1–4 and the
user-approved follow-on [PR81](https://github.com/loveoverflowcom/cantos/pull/81)
contains 5–11. Implementation completion does not mean all acceptance passed.
Fresh-develop integration and exact final identities are recorded separately in
the PR/handoff; the feature PR is not merged into develop by this task. One new
Ultra review is queued after this phase.

Only the named core/test/proof scopes earn `CORE_VERIFIED`; real SQL/API/browser/
device acceptance remains `IO_ACCEPTANCE_PENDING`. There is no overall PASS
while the recorded Rust gate fails or required integration is unrun. Owners for
blocked acceptance are unassigned and must be assigned before accepting it:

1. Run disposable `cargo xtask verify postgres` in an environment permitting its
   documented socket startup; require actual migrations/constraints/current
   authorization/transaction/replay/corruption results. Do not bypass the denial.
2. Resolve the upstream service-fixture environment and intermittent post-drop
   port assertion without weakening private-input guards, then rerun full Rust.
3. Exercise actual Studio/browser async/cookie/conflict/IME scenarios and Theatre
   probe cancellation/unmount. Real listener implementation/publication/audio/
   native acceptance remains future product work, not this mock hardening.

Remote CI was neither inspected, dispatched nor polled. No screenshot,
production/provider acceptance, real listening, persistent access, default
production toolchain change or backend authorization duplication is claimed.

## Ultra review checkpoint

The queued independent Ultra review is now complete. Its
[committed report](ultra-review.md) covers the cumulative obligations and fresh
develop integration. One confirmed P2 Kani configuration-home defect was fixed
and published in `3367980818be17caaffe45b30eedfd8dc3ffe76e`, tree
`e32fb3de5347a98dd32ee4ab75665029aa94a6d6`. Empty `CARGO_HOME` now resolves actual
`HOME/.cargo`; unresolved homes refuse before probing. Independent review of the
minimal four-file repair has no actionable finding. Prior dated counts/results
above remain historical rather than silently overwritten.

Fresh workflow checks pass 59 focused xtask tests and 77 Python verifier tests;
repository verification after the fix passes 302 tests and all 18 skills.
All three actual Kani proofs ran again on a clean equal-tree local commit:
Catalog 100 checks / 6 covers, WAV 20 / 7, window 10 / 8, all PASS, with exact
[raw/metadata/model/config/lock identities](kani.md#ultra-review-reexecution).
Fresh API/client/contracts/DBSP/doc scopes and four Web gates also pass separately.

Full serial `cargo xtask verify rust` on the published repair still **FAILS**:
802 passed, 17 failed, 136 ignored. Format and workspace Clippy pass; the same
unchanged service-fixture private-input/Git-ancestor guard prevents those 17
tests. No guard or assertion was weakened. Later doctests are not reached by
that command; their independent 16-case run passes. Implementation review
completion does not establish an overall PASS or resolve required IO acceptance.

Final report-only repository verification passes 302 tests / 476 text files /
18 skills. Required changed preflight against fresh `origin/develop`, with
`--kani --skip postgres --skip cmp-ui-preview --skip theatre-cmp-demo`, still
FAILS at the same 17 fixtures (802 passed / 17 failed / 136 ignored). Repository,
format, Script IR reference and workspace lint PASS; later DBSP/WAV/audio/Web/
preview/mutation and all three Kani rows are NOT_RUN in that preflight. The
independent successful runs above do not rewrite its result. PostgreSQL/CMP
skips remain NOT_RUN; browser/device/audio acceptance and blocked owners remain
as listed above.
PR81 stays Draft/open and is not merged into develop by this review.

## Domain/wire boundary and Studio request-phase checkpoint (14 October 2026)

Media listener eligibility no longer builds Theatre wire DTOs. `media::domain::listening`
keeps `DiscoveryQuery` (now over Catalog `WorkKind`), the `ListenerPublication`
witness with read-only `work`/`unit`/`release`/`discoverable` facts, and the unchanged
keyset/access-window kernels. The new application-layer `media::listener_projection`
alone maps those facts to `cantos_contracts::theatre`; the listener HTTP shapes and
the Library manifest adapter are unchanged. Persisted canonical documents in
`domain::{publication,lifecycle,artwork,image}` intentionally keep their contracts
types. `check_repository.py` now rejects any pure-domain path into
`cantos_contracts::theatre`; this lexical repository check reports 22 violations on the
pre-change file and none now. A discovery application test passed unchanged
against the pre-change code before the move, and projected DTOs validate against the
published discovery v1 JSON Schema.

F1's `Editor` and `Production` no longer expose independently writable request flags.
Each keeps one private closed phase: Editor `Idle | Reading | Saving |
ReconciliationRequired | CheckingSession(held)`, Production `Idle | Reading | Sending |
Unresolved`; an ambiguous/unknown outcome lives only inside an unresolved save or
command. Editor fields are private behind read-only accessors, `ReadTicket` replaces
the raw read counter, and the shell's two direct `busy`/`status` writes became the
`checking_session`/`release_session_check` transitions (`TYPE_ENFORCED`). "Dirty" stays
derived from bytes rather than stored.

A temporary, uncommitted differential harness compared the old reducers (oracle) with
the new ones. Production: no divergence in 209,790 transitions to depth 9. Editor:
1,376,368 transitions to depth 9, with every divergence in two intended classes:
`release_session_check` is a no-op outside a check and shows the settled notice instead
of a stale `Loading`; and a read abandoned when a session settled is never re-accepted by
a later check. The second class is a latent defect in the old code: a stale read failure
cleared an in-flight save's retry identity, and a stale load changed its base. It is
now pinned by `an_abandoned_read_cannot_settle_a_later_save_during_a_session_check`.
The committed bounded-exhaustive Editor test checks phase/status, retry identity,
draft-preservation and stale-response invariants over 483,280 transitions (48,157
worlds, 28 events, depth 8). A one-off run of eight injected Editor mutants killed all
eight. These are example/bounded-exhaustive and mutation evidence, not `KANI_VERIFIED`.

Rust 1.87 changed-only preflight PASSED rust-format, rust-lint, rust-tests (810 passed,
0 failed), dbsp, wav-reference, Studio web-lint and web-build. PostgreSQL and
audio-inspection were `UNSUPPORTED` here (no ffmpeg/ffprobe) and stay `NOT_VERIFIED`;
the unchanged `listener-access` Kani harness was not rerun (no Kani bundle). The
repository gate under Python 3.9 fails with the same 51 entries as clean develop;
Python 3.14 passes 355 tests. No browser, IME, cookie-race or listener HTTP/PostgreSQL
runtime acceptance ran.
