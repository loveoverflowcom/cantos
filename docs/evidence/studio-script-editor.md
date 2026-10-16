# Studio Script Editor evidence

Date: 2026-10-10. Review boundary: [issue #4](https://github.com/loveoverflowcom/cantos/issues/4)
and [010 — Import and edit a versioned script](../assurance/script-editor-acceptance.md).
Base: `297f6fdb42568819d48c507ec09713667b3a6659` on `develop`; branch
`feat/studio-script-editor`. Implementation milestone: `8d1c858`. Upstream `develop` at
`4b2f786` was integrated by `8deb25c`, preserving its WAV work. The later upstream read-only
Script IR diff CLI at `efa5ae1` was integrated by `30fff50`; it remains separate from this Studio
delivery. The Studio source fingerprint and final Safari assets did not change in either merge.
Final implementation/PR/merge
SHAs and required exact-SHA CI belong to the GitHub execution record for issue #4; this document owns the local behavior and checks.

This is a local implementation and verification record. Native and real PostgreSQL evidence is available below.
The corrected fixture journey has now run in native Safari: import, proposal editing/acceptance,
validation focus, pending-input/navigation guards, real timeout/exact retry, stale-write recovery,
history, owner review and reopening after host restart. The bounded UI gate is **PASS** for
the recorded desktop, narrow and 200% narrow scenarios in light and dark schemes, with opened
captures and native keyboard/focus observations below. Final integrated native, WASM, repository
and fresh PostgreSQL checks passed. Independent code/visual review found no actionable findings.
Task-owned host, retained/final disposable clusters and Safari window were cleaned up; no user
process was stopped. This local record does not infer CI, a merge or issue closure from builds.

## Invariant, owner and boundary

A creator can edit a complete Script IR draft through structured controls and explicitly accept
or save a new immutable revision without altering its original source, frozen proposal or prior
accepted revisions. Denied, stale, repeated or interrupted actions preserve local text and the
exact unresolved operation. Navigation, pane changes and late responses must not substitute
another actor's, script's or editor's content.

`authoring.rs` is a pure Studio boundary module. Its read models are projections for rendering;
mutations patch known leaves of the complete JSON value. Stable IDs, provenance, rights, cue
assets and unsupported fields survive unrelated edits. Duplicate JSON keys, unsupported schema
versions and numbers that cannot round-trip without precision loss are rejected before a
structured edit. Unsupported data remains available in the advanced JSON draft; preserving an
unknown field does not make it admissible to the current backend contract.

Losslessness here means preserving the complete decoded JSON value except the requested leaf
edit or explicit entity insertion/removal. JSON whitespace, key order and equivalent numeric
notation can change when serialized; original source-byte identity is a separate backend
invariant. The focused oracle adds unknown nested metadata, edits one Vietnamese line, and
compares the entire document against an independently patched expected value. Separate checks
retain the cue's asset/rights record, every other pronunciation entry, original character/line
IDs and provenance while editing delivery or cue descriptions. A domain read/save can still
canonicalize supported text under the unchanged Script IR rules; save acknowledgements do not
rewrite an active browser text field.

The core supports metadata, characters, acts/scenes, dialogue/narration, delivery/emotion,
pronunciation and typed ambience/music/SFX cues. Narration remains dialogue referencing the
explicit narrator character. New entities receive permanent IDs from the shell; reordering
preserves complete values and references. Referenced characters and cue-anchored dialogue cannot
be deleted without an explicit preceding correction. Bounded structural undo/redo retains 40
immutable snapshots. Local checks protect safe editing structure; server admission remains the
only Script IR validation authority.

`editor.rs` owns actor-bound local drafts, dirty/clean baselines, immutable save intents,
optimistic bases, read tickets and acknowledgement/error transitions. `InputActivity` tracks
each buffered or composing field separately, so resolving one input cannot hide another.
`inspector.rs` owns script/head-scoped history/source reads and exact saved-revision owner-review
intents. Proposal acceptance continues through the existing adaptation reducer and transaction.
Leptos renders and dispatches; `api.rs` owns URLs/transport and shared wire types remain in
`contracts/api`. The server/domain crate does not enter the browser dependency graph.

Each structured editor now owns a scoped Leptos `Provider` for its `AuthoringContext`. Proposal
and accepted-revision editors stay mounted with separate document, selection, history, pending
fields and namespace ownership. Dynamic select choices set `option.selected` from the model
after constructing their options, including an explicit unresolved current value. Opening a
document must therefore show its actual speaker/role/emotion/cue choice without silently
selecting the browser's first option or editing the model.

The shell passes current buffered-input activity into revision/proposal/accepted-revision read
completion. A response arriving before blur or composition commit can update compared remote
facts but cannot replace local text/baseline/base while those bytes are pending. Account/script
read tickets and saved-head inspector contexts fence late responses; review/acceptance
acknowledgement is cleared when text, buffered activity or the compared base changes. Native
state regressions cover these transitions; the recorded browser walkthrough separately drove
invalid-buffer, cancellation, timeout and stale-head event sequences.

The additive authenticated `POST /api/v1/validation` previews the existing bounded reader's
shape and semantic findings. It requires the shared session/Origin checks, creates no database
records, and proves neither target permission nor source/rights eligibility. Save and acceptance
still revalidate current authority, evidence and optimistic base in the existing PostgreSQL
transaction. Diagnostic focus maps both current JSON positions and semantic paths containing
stable act/scene/dialogue/cue IDs; reordering does not change the semantic identity.

`api.rs` applies a nominal 15-second browser `AbortController` deadline to each request through
receipt of headers and decoding of the complete JSON body. Completion or future cleanup clears
the timer and aborts the owned fetch. Expiry/transport/decode failure is `unavailable`; a possible
write keeps its existing immutable actor/operation/base/body intent for explicit reconciliation.
The transport neither retries automatically nor interprets abort as proof of server rollback.
The browser deadline is distinct from the existing server middleware deadline and is not a
production timeout/SLA measurement.

No migration, persistence authority, inference runtime, AI provider client or paid call is added.
The user-controlled Gemini/ChatGPT/Codex host creates proposals through the earlier tools; this
slice opens and edits those proposals. Synthetic proposals below establish integration behavior,
not model quality.

## Claim and oracle ledger

| Claim / failure mode | Owner and cheapest adequate oracle | Evidence held / limit |
| --- | --- | --- |
| Structured edits cannot discard unrelated fields or rewrite IDs | Pure document patches; exact complete-value comparison against committed Vietnamese fixtures | PASS, example-tested; unsupported unknown fields still fail backend admission |
| Reordering and structural undo preserve entity identity and content | ID-based move comparison; delete/undo exact snapshot regression | PASS, example-tested; real focus/IME behavior still requires browser input |
| A pending field cannot disappear when another field commits/cancels, or be overwritten by a completed read | Native registry/read regressions; blank intensity plus another edited field; Escape restoration; stale-head read preserving local draft | PASS for the driven cases, example-tested and interaction-tested; named OS IME behavior remains unrun |
| Validation identifies the current entity and focuses its native field | Current contract semantic/pointer regressions; missing pronunciation target, activated summary, corrected exact field and revalidation | PASS, example-tested, interaction-tested and accessibility-checked for that field; reorder focus remains native-core evidence |
| Preview validates without persistence or approval | Real authenticated HTTP, hostile/invalid input, Origin/session/bounds checks and unchanged database counts | PASS, integration-tested; preview conveys no target ownership |
| Dirty text, stale responses and changed actors cannot replace the current draft | State regressions; native reload/close Stay cancellation; pending-input script-switch guard; stale-head read and host reauthentication | PASS, example-tested and interaction-tested for recorded cases; reload/close/script-switch captures opened and inspected |
| Stale writes and repeated acceptance/save/review preserve history | Fresh PG replay cases; live external revision 3, rejected base-2 save, explicit rebase and fresh acknowledgement producing revision 4 | PASS, integration-tested and interaction-tested; no automatic merge is claimed |
| Reader/editor/owner permissions remain backend decisions | Existing real authorization suites plus reader save/review rejection in editor journey | PASS, integration-tested; every permission role was not separately exercised in native Safari |
| Source, proposal and old accepted revisions remain immutable | Exact HTTP/PG oracle: revisions 1/2/3 pinned, full source/proposal unchanged, final head/review 4 and history 1–4 | PASS, integration-tested; original synthetic fixture only |
| Process loss and PostgreSQL restart preserve settled facts | Earlier PG restart/migration replay; live host PID 50386 replaced by 65410 on same cluster, relogin and exact head 4 reopen | PASS, fault-injected, integration-tested and interaction-tested; not backup restore/failover evidence |
| The three-pane editor works on desktop/narrow/zoom with Vietnamese text and keyboard input | Opened final desktop/narrow captures at 100% and narrow captures at 200%; native keyboard/focus observations in light/dark schemes | PASS, screenshot-captured, screenshot-inspected and interaction-tested for recorded scenarios; no reference-fidelity, VoiceOver or named OS IME claim |
| Multiple structured editors have isolated context ownership | Scoped `Provider` correction followed by the driven proposal/revision editing and reopen flow | PASS for recorded corrected flow, interaction-tested; original collision capture retained as history |
| Native select state displays/edits the intended model | Explicit selected-option projection; actual corrected speaker/emotion/delivery editing retained in immutable export; long cue-anchor selection inspected after overflow correction | PASS for recorded fixture choices, interaction-tested; full text remains in the native option popup |
| A stopped HTTP host releases pending transport while preserving write identity | Actual `SIGSTOP` on PID 50386, visible unavailable/pending draft, `SIGCONT`, explicit exact retry and revision/count oracle | PASS, fault-injected and interaction-tested; partial-body corruption and production timeout behavior unrun |

## Acceptance matrix

| Acceptance step | Implemented route/behavior | Core / real backend | Final browser result |
| --- | --- | --- | --- |
| Import an authorized Vietnamese source | Existing immutable manuscript import and private original/source comparison | PASS: import suite, fresh HTTP journey and exact source oracle | PASS: live 118-byte import, three blocks, CRLF warning and raw source/hash read; rights remain fixture claims |
| Open a host-created proposal | Existing owner-scoped adaptation review; source/findings/caller declarations visible | PASS: fixture context/proposal read, zero inference attempts | PASS: opened synthetic caller context and proposal; no real host model-quality result |
| Edit scene, character and spoken lines | Structured navigation and native narration/dialogue/delivery/pronunciation/cue controls | PASS: lossless core and real accepted JSON; scoped context/selected-choice corrections | PASS for driven scene, Mai name/personality/dialogue, hopeful 525 delivery and pronunciation edits; long text/cue controls inspected in final layouts |
| Compare source and changes; inspect validation | Preserved source/warnings, keyed changes, backend preview and field-path focus | PASS: exact source/hash, validator HTTP and diagnostic mapping | PASS: pronunciation-target error summary focused exact field; correction revalidated successfully; source read inspected |
| Explicitly review and accept/save | Current-text acknowledgement; immutable acceptance, saved-head owner review and CAS save | PASS: backend replays/authorization/history plus actual head 4 oracle | PASS: proposal accepted as revision 1; exact timeout retry revision 2; stale recovery with reset review/fresh acknowledgement saved revision 4; owner review pinned to 4 |
| Reopen exact accepted and historical versions | Exact revision/current head/history; read-only comparison preserves draft | PASS: pinned revisions 1–3, source/proposal and PG rows | PASS: history 1–4, revision 1 opened read-only; relogin after host restart reopened exact updated line/head 4 clean; multi-page history not separately driven |
| Preserve unsaved/pending text during navigation and failure | Field buffers, unload/discard guard, exact unresolved retry and explicit stale rebase | PASS: reducers, stale/replay suites and live fault injection | PASS for recorded invalid-buffer/Escape, reload/close Stay, cancelled script switch, delete/cancel/focus, timeout/retry and stale-read/rebase; confirmed-close loss remains documented |
| Desktop, narrow, zoom and accessibility | Existing tokens, Vietnamese/English copy, semantic fields and visible focus | Final native/WASM/assets checks passed; real captures/focus recorded separately | PASS for 1322×963 desktop at 100% light/dark, 720×863 narrow at 100% light, and 721×865 narrow at 200% light/dark capture sizes; no horizontal scrollbar in final native accessibility observations; CSS viewport unmeasured; no VoiceOver/OS IME claim |

## Executed checks and provenance

Local environment: Mac mini M4, macOS 15.6, Rust 1.87.0, Trunk 0.21.14, Safari 18.6 and
Homebrew PostgreSQL 17. The backend tests create a fresh disposable cluster; an inherited or
production database URL is never substituted. The runtime uses the restricted application
role and original synthetic Vietnamese source/proposal content. No real manuscript, model,
external identity deployment, media store or generated audio is involved.

| Result | Exact command / artifact | Scope and remaining limit |
| --- | --- | --- |
| PASS | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Final integrated native workspace; log `native-clippy.log` |
| PASS | `cargo test --workspace --locked` | Final integrated workspace, including 100 Studio tests; ordinary tests do not run ignored PG cases; log `native-tests.log` |
| PASS | `cargo test -p cantos-studio --locked authoring::tests` | 13 authoring tests + 2 matching localization tests; focused core agent run |
| PASS | `rustfmt --edition 2021 --check apps/web/src/authoring.rs` | Focused core formatting, also covered by final workspace formatter |
| PASS | `PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" python3 scripts/test_postgres.py` | Fresh post-integration cluster: 12 revision + 9 import + 21 adaptation/editor = 42 real PG cases; log `postgres-final.log` |
| PASS | `cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings` | Final integrated WASM check; log `wasm-clippy.log` |
| PASS | `NO_COLOR=true trunk build --locked` from `apps/web` | Final CSR/assets build after the select/closed-details CSS correction; log `trunk.log`; compilation only |
| PASS | Actual HTTP process kill and PostgreSQL restart with migration/intent replay | Inspected `target/studio-editor-evidence/server-restart.json`; settled counts and exact observations retained |
| PASS | `cargo fmt --all -- --check` | Final integrated workspace; log `fmt.log` |
| PASS | `python3 scripts/check_repository.py` | Final integrated documents/contracts/links; log `repository-check.log` |
| PASS | `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | 70 repository-script tests on the integrated tree; log `python-tests.log` |
| PASS | `python3 scripts/reference_script_ir.py` | Unchanged independent Script IR golden oracle; log `reference.log` |
| PASS, earlier bounded run | `TMPDIR=/private/tmp python3 scripts/mutate_script_ir.py` | 9 killed, 0 survivors for the unchanged validator/encoder core; log `mutations.log`; not rerun after the upstream WAV/diff integrations, which did not change that core |
| PASS | `cargo build --locked --bin inspect-wav`, then `python3 scripts/reference_wav_inspection.py` | Preserved upstream WAV regression: 5 PCM files agree with Python wave/hashlib without ffprobe; logs `wav-build.log` / `reference-wav.log`; no production audio-quality claim |
| PASS | `git diff --check` and `git diff origin/develop --check` | Final integrated patch whitespace |
| PASS for recorded interactions and layout scenarios | Native Safari fixture journey and opened captures below | Real timeout/CAS/validation/cancel/restart flows; final 100% desktop/narrow and 200% narrow light/dark inspection |
| NOT_RUN | VoiceOver, named OS Telex/VNI composition, device/mobile, provider/audio/publication/deployment | No evidence claimed in this slice |

Raw local logs/receipts are ignored under `target/studio-editor-evidence/` and
`target/revision-evidence/`. The current successful PG suite logs are
`postgres-tests.log`, `import-postgres-tests.log` and `adaptation-postgres-tests.log` in the
latter directory. An earlier focused editor log expected HTTP 201 from an existing route that
returns 200; that test-oracle assertion was corrected and is superseded by the successful
21-case adaptation/editor suite. Earlier failures are superseded by these successful gates, not reported as passes. Credentials and database files remain outside Git.

The real backend journey changed the unresolved character to “Minh — người gác cửa”, scene title
to “Bên cánh cửa — đã đối chiếu”, and Mai's line to “Chúng mình sẽ chờ đến bình minh.” with hopeful
delivery at 525 permille. Explicit acceptance created revision 1; owner review referenced its
exact c1/e1 digests. A later save changed the same stable dialogue to “Chúng mình sẽ chờ đến sáng.”
as revision 2. Reader save/review attempts were forbidden, a stale base conflicted, and repeated
accept/save/review intents returned their original receipts without extra records.

The restart oracle records identical before/after counts: one source, one proposal/submission,
one acceptance, two revisions, one review and zero generation attempts. Original source SHA-256
was `b242e7ce5c677751d5bcd7384ddb0698b740c30294978e4aa5a1a46b929a24f2`. Exact source/proposal/accepted
revision/review/history responses and committed-intent replay were compared. This is a real
HTTP/PostgreSQL integration journey with fixture content, not a Leptos interaction result or
provider-generation result.

## Browser observation and final local UI gate

The executor drove native Safari 18.6 against the task-owned loopback host at port 38081 and
the fresh local PostgreSQL cluster. Observations below are executor-driven real input; the
independent API/PostgreSQL oracle checked durable results separately. Original context-owner
and choice-initialization findings were corrected before this journey; the initial local
`screenshots/context-collision-before.jpg` remains historical finding evidence.

1. Imported the synthetic Vietnamese source as `src_d690d1bdcf07479bb53402afebae24be`. The
   receipt showed 118 original bytes, three ordered extracted blocks and the CRLF normalization
   warning. Raw source and SHA-256 read matched the original fixture. Opened synthetic caller
   context `f4d87735-1d7b-44ca-9011-40c89b2c0ed5` targeting script
   `79f8695e-ca7b-45fe-bd9d-e27a60090235`; no model was invoked.
2. Used the structured controls to edit a long Vietnamese scene title, Mai's name/personality
   and dialogue, hopeful delivery at 525 permille and pronunciation `Vọng Đài` → `vọng đài`.
   Explicit acknowledgement/acceptance created revision 1 with export digest
   `sir-e1:sha256:f0316cc2b214e00a9868d388c769762aed68f8b75eda14baa18e910dc1da754c`.
3. Entered a pronunciation surface absent from the line. Backend preview reported
   `pronunciation_target_missing`; activating the summary focused the exact native pronunciation
   field. Corrected the surface and revalidated successfully. The focus capture was opened and
   inspected. Blank intensity failed commit; editing a different text field kept the original
   unfinished buffer visible and navigation blocked. Escape restored that intensity to 300
   while retaining the other edit.
4. Triggered native reload while dirty. The native Stay/Escape interaction retained the draft;
   its capture was opened and inspected. Opened Mai's inline delete confirmation and cancelled
   with Escape; native focus returned to the delete button. A later temporary scene title,
   “Bản nháp giữ riêng khi đóng cửa sổ — không lưu”, survived Cmd-W followed by native
   alert Escape/Stay, retaining script `79f8695e-ca7b-45fe-bd9d-e27a60090235` and base 4. Changing
   the script-ID candidate to the other fixture and pressing Read opened the native discard
   confirmation; Escape/Cancel retained the same script/base/title. Both guard captures were
   opened and inspected. Restoring the original script-ID candidate and using structural Undo
   restored the exact original title and clean state; these guard checks made no database write.
5. Stopped task HTTP PID 50386 with `SIGSTOP`, then clicked Save. The nominal 15-second browser
   deadline produced unavailable while retaining text and the exact pending intent. Its capture
   was opened and inspected. Resumed the same host with `SIGCONT`; explicit exact-operation
   Retry produced revision 2, with no extra retry revision in the independent oracle. This is a
   real stalled-host fault injection, not partial-body corruption or provider-timeout evidence.
6. An independent authenticated CAS write changed only `/episode/title`, producing revision 3.
   The UI's Save from base 2 returned stale conflict and kept the local draft. Reading head 3
   kept that draft and cleared review acknowledgement. Explicitly adopted head 3 as base,
   acknowledged the freshly compared text and saved revision 4. Historical revisions 1 and 2,
   source and full proposal context remained exact after the competing write.
7. Opened history containing revisions 1–4 and revision 1 in read-only comparison. Recorded
   owner review against head 4. Replaced HTTP PID 50386 with PID 65410 on the same disposable
   database; after relogin, the UI reopened the exact updated Vietnamese line/head 4 clean.
   The final oracle agrees between HTTP and PostgreSQL, pins owner review to 4 and reports zero
   generation attempts.
8. Opened and inspected final desktop captures at 1322×963 / 100% zoom in light and dark
   schemes, a narrow 720×863 / 100% light capture, and narrow 721×865 / 200% light/dark
   captures. These are JPEG capture dimensions verified from image metadata, superseding
   earlier approximate window coordinates; they include browser chrome, and CSS viewport
   dimensions were not measured. The initial 200% overflow was traced to the cue-dialogue native select: client
   width 604 with scroll width 639 propagated to a canvas width 640 and body client/scroll
   widths 654/664. The targeted structured-select overflow/ellipsis correction was rebuilt and
   the same long fixture reobserved. Its native popup retained the complete selected Mai text;
   final native accessibility observations exposed only a vertical scrollbar, with no horizontal
   scrollbar. A desktop select measured approximately 44 screen pixels at 100% and 88 at 200%,
   consistent with its 44 CSS-pixel control height.
9. Pronunciation removal controls fit the desktop row and stacked in the narrow layout.
   Alt-Tab from Mai's text moved to the disclosure summary; Return expanded it. The exact
   pronunciation field's purple focus outline and contrast were observed in both schemes.
   Native Escape from delete confirmation returned focus to Delete. The closed-details rule
   explicitly preserves native closed semantics while retaining mounted draft state. These
   observations close the recorded UI gate; they do not establish a screen-reader or OS IME gate.

The select/body measurements were real runtime observations through a temporary app diagnostic,
not automated DOM assertions. That diagnostic was fully removed from the final view before the
final build. Native accessibility state, driven keyboard input and opened pixels provide the
recorded evidence. All listed screenshot files are JPEGs with matching `.jpg` extensions.
`target/studio-editor-evidence/screenshots-final.json` records each file's format, exact capture
dimensions and SHA-256. An independent reviewer also opened the five final layout/theme
captures and reported no actionable clipping or focus findings; this image review does not
substitute for the executor's interactions.

The independent records are `target/studio-editor-evidence/ui-baseline.json`,
`ui-cas-verification.json` and `ui-final-verification.json`. The final quiescent local oracle
holds exact pinned revisions 1/2/3, source and full proposal context; history `[1,2,3,4]`; owner
review pinned to 4; HTTP/PG equality; reopened Vietnamese dialogue equality; and zero server
generation attempts. Script-specific durable rows are four revisions with head 4. Final head
export digest is `sir-e1:sha256:ad2373835bfb757ca5fcd7392e669c061478aa1fded8e6f6726ae97a9a910152`.
This supplements the separate earlier two-revision PG journey; their fixture identities/counts
are not conflated.

| Local capture under `target/studio-editor-evidence/screenshots/` | Evidence / limit |
| --- | --- |
| `desktop-editor-final.jpg` | screenshot-captured, screenshot-inspected, 1322×963 / 100%, light; final editor |
| `desktop-dark-final.jpg` | screenshot-captured, screenshot-inspected, 1322×963 / 100%, dark; final editor |
| `narrow-editor-final.jpg` | screenshot-captured, screenshot-inspected, 720×863 / 100%, light; final narrow editor |
| `narrow-200-cue-final.jpg` | screenshot-captured, screenshot-inspected, 721×865 / 200%, light; corrected long cue-anchor select |
| `narrow-200-dark-focus-final.jpg` | screenshot-captured, screenshot-inspected, 721×865 / 200%, dark; exact field focus and stacked controls |
| `validation-pronunciation-focus.jpg` | screenshot-captured, screenshot-inspected, 1324×967; exact invalid pronunciation field focus |
| `timeout-pending.jpg` | screenshot-captured, screenshot-inspected, 1324×967; unavailable status and retained pending text/intent |
| `unsaved-reload-guard.jpg` | screenshot-captured, screenshot-inspected, 260×224 native alert; reload Stay/Escape retained draft |
| `unsaved-close-guard.jpg` | screenshot-captured, screenshot-inspected, 260×224 native alert; close Escape/Stay retained title/script/base |
| `unsaved-script-switch-guard.jpg` | screenshot-captured, screenshot-inspected, 720×863; discard Escape/Cancel retained title/script/base |
| `desktop-reopened.jpg`, `narrow-reopened.jpg` | Earlier screenshot-captured, screenshot-inspected reopened views, 1324×967 / 722×867 at 100%; superseded for final layout by captures above |
| `narrow-200-pronunciation.jpg` | Historical 722×867 initial 200% overflow finding; corrected and superseded by final cue/focus captures |
| `context-collision-before.jpg` | Historical 1324×967 pre-correction finding, not final successful editor proof |

Independent review completed with no actionable findings. Final feature/PR/merge identities,
required exact-SHA CI and postmerge status are recorded by the GitHub delivery record.
The recorded interactions establish only the stated scenarios, not VoiceOver, named OS IME,
production identity/TLS or every viewport/accessibility policy scenario.

## Read inventory and DBSP decision

| Read / consumer | Existing authority and request shape | Freshness / change in this slice |
| --- | --- | --- |
| Current accepted head | `GET /api/v1/scripts/{script}/head`; current actor/script access and immutable complete revision | Existing point read; explicit editor read ticket and actor/script guard |
| Exact historical version | `GET /api/v1/scripts/{script}/revisions/{revision}`; current actor/script access and fixed immutable revision | Existing point read; read-only comparison with its own ticket |
| Revision history | `GET /api/v1/scripts/{script}/history?after_revision=…&limit=…`; existing bounded ascending revision/review range | Existing paged endpoint; Studio requests 20 rows and exposes next-page action |
| Preserved source text | `GET /api/v1/scripts/{script}/sources/{source}`; current access and linked immutable source/hash | Explicit reads cached only in current actor/script/head inspector state; no request per keystroke |
| Imported/binary source fallback | `GET /api/v1/imports/{id}` and authenticated original route; existing owner-scoped import authority | Existing fallback for supported imported source IDs; access failure remains visible |
| Proposal/source comparison | `GET /api/v1/adaptations/{id}/review`; existing owner/context/proposal/input/source facts | Existing coherent point reads; generation is never dispatched |
| Draft validation preview | `POST /api/v1/validation`; current session plus bounded pure Script IR reader | New nonpersisted validation; no target metadata, history query or serving read model |

No underlying history/source/current/proposal query or database hot read path changes. The UI
reuses bounded pagination and explicit cached source reads rather than introducing per-line or
per-input SQL, an unbounded list or a repeated aggregate. Current authorization remains checked
on each request; changing actor/script/head invalidates visible cached inspector facts. Reads
between pages can observe later commits and are not claimed to form a frozen history snapshot.

**DBSP: DEFER.** No measured hot aggregate consumer or query change justifies a new engine,
CDC boundary, materialized cache or dependency. Existing PostgreSQL point/range reads remain
authoritative; prior bounded benchmarks do not prove this new UI's production throughput.
No new latency/cold-cache/concurrency/write-overhead benchmark or speedup is claimed here.

## Compatibility and residual risk

Script IR reads/writes remain `{0.1.0}`. No Script IR converter, schema/golden/digest scheme change or stored
revision rewrite is introduced. The validation wire addition is additive and creates no durable
record. Editing effective spoken fields changes a later revision's SpokenContent; reorder-only
changes preserve line identity/content. This slice performs no TTS or mix/QC/publication
invalidation execution, so no audio regeneration result is claimed.

Buffered drafts and exact retry intents live in the browser session's memory. Native unload
confirmation is a guard, not durable crash recovery: a confirmed reload/close discards unsaved
text. Structural history is bounded; pronunciation entries have no stable IDs in the current
schema and remain explicitly indexed. Conflicts require comparison and an explicit new base;
there is no automatic collaborative merge.

No supplied design-reference image is available for this request. The existing versioned Studio
tokens and [UI system](../design/ui-system.md) govern the compact Material 3 Expressive direction;
no pixel-faithful reference match is claimed. Native labels, keyboard/focus observation and
opened captures can establish bounded accessibility evidence, but no VoiceOver/screen-reader
walkthrough or named OS Telex/VNI IME run has been supplied. Composition guard reducer tests and Vietnamese paste/fixture text are not OS input-method evidence.

Production identity/TLS, legal adjudication of source/content rights, real Vietnamese AI quality,
ChatGPT/Gemini web connector registration, casting, TTS, mixing, publication, Theatre, native CMP
playback and mobile runtime remain outside this boundary. No CI expansion, package/toolchain
upgrade, provider configuration, secrets setup or deployment is part of this task. The recorded
local browser gate and independent review are complete. GitHub owns the final PR/merge identity
and exact-SHA CI execution; these are not inferred from local verification.

## Independent review and cleanup

The read-only review compared all 34 Studio paths against `develop`
`4b2f78688048cf73e65d7b9fd11e9ae4fe42bdd4`, including the integrated snapshot and final CSS.
Assessment: **no-actionable-findings**; coverage: **complete-for-declared-scope**. The reviewer
independently opened all five final JPEG captures and read the executor logs; heavy gates were
not repeated in the reviewer lane. WAV PR #22 was preserved and excluded from the Studio review.
After the later diff CLI integration, the reviewer pinned `30fff505ce6b5d9da2381fd0e48966cb213349bb`
against `efa5ae1ca1fb8258e1167159d908bb405452c3de`: the same 34 Studio paths remained, all 27
source paths were byte-for-byte unchanged, and README/work-plan entries retained both upstream
features. The integration audit found no new actionable finding and excluded the upstream CLI
from Studio ownership. Its local note is `target/studio-editor-evidence/review-integration.md`.

The final 27 non-Markdown source/contract/script files have the sorted path/content fingerprint
`6685b8a82f4d40233006a236f16570cb55dfe5bba24e0a249f09b1343c53cd51`; CSS SHA-256 is
`69457e314005d3b98e6cfa26547e25d7bcf3170f0b4b176c75bb88f6c4615942`. Administrative
completion metadata in this evidence file is separate from that source fingerprint.

`target/studio-editor-evidence/cleanup.json` records stopped task host PID 65410, the stopped
retained walkthrough cluster, the automatically stopped final fresh cluster and closed task
Safari window. The original user window and unrelated processes were left untouched.
