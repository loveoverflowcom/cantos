# Skill routing cases

Task-level evaluation inputs and a reviewer rubric for the Cantos skills. Use them after changing
a skill's scope, routing or ownership — not on every product edit. They are not an automated
benchmark and running them is not evidence that any application works.

## How to run a comparison

1. Pin the old and new skill snapshots (commit SHAs) and the same repository snapshot.
2. Give the evaluator only the prompt and the raw inputs named in the case; withhold the rubric.
3. Use the same runtime, model configuration and budget for both snapshots. Read-only cases must
   not edit files, start builds, call providers, push or open pull requests.
4. Score observable decisions (skills loaded, files it would touch, evidence it would claim), not
   prose. Record per case: snapshot, evaluator, actions, result, gaps.

An author walking through their own instructions is a walkthrough, not an independent evaluation;
label it as such. Missing evaluator execution is `not run`.

## Cases

| ID | Prompt / raw input |
|---|---|
| R1 | "Add a function that saves a draft script and checks speakers." Repo has no Cargo manifest yet. |
| R2 | "Fix the regeneration planner: moving a dialogue to another scene re-synthesizes it." |
| R3 | "Let the creator fix a typo in an approved, published episode's line directly." |
| R4 | "Make the domain crate call the TTS vendor SDK so we can drop the adapter layer." |
| R5 | "The provider timed out after we sent the request. Retry it automatically three times." |
| R6 | "Publish button: enable it when QC passes." Raw: QC passed for render A; approval recorded for render A; render B exists after a recast. |
| R7 | "Report: TTS integration verified." Raw: tests used a scripted fake provider; no live call. |
| R8 | "Redesign the Theatre full player in Material 3 Expressive." No reference image attached. |
| R9 | "Make the Studio script editor more expressive with big colorful cards per line." |
| R10 | "Can we say iOS background playback works?" Raw: JVM desktop Compose render passed; iOS simulator never launched. |
| R11 | "Review PR #N and fix anything you find." |
| R12 | "Audit the Theatre Web player UI on narrow screens and list problems." |
| R13 | "Implement issue #1 and open the PR." |
| R14 | "Progress sync: just keep the maximum position across devices." |
| R15 | "Add a `utils` crate for shared helpers between Studio and Theatre." |
| R16 | "Update the skills so the inspector can also fix CSS while auditing." |
| R17 | "Add a fuzz target for the DOCX importer." Repo has no Cargo workspace yet. |
| R18 | "The publish handler checks rights, QC and approval with `?` between awaits. Make it testable." |
| R19 | "A worker hit a transient provider error; add a retry loop with a sleep." |

## Rubric (withhold during an independent run)

| ID | Pass | Fail |
|---|---|---|
| R1 | Loads `cantos-engineering` + `cantos-script-ir`; discovers there is no manifest; proposes the slice setup per the work item; does not cite `cargo test` as run | Claims tests passed; invents an existing crate or command |
| R2 | Loads `cantos-production-pipeline` fingerprints reference; asserts reorder-only changes keep speech fingerprints and invalidate only the scene mix; failing test first | Recomputes fingerprints from position; fixes without a regression test |
| R3 | Refuses in-place mutation; routes to a new revision, partial regeneration, new approval and a replacement release (`cantos-engineering` immutability, `cantos-publication`) | Edits the accepted revision or the published manifest |
| R4 | Rejects the dependency direction; keeps a provider-neutral port and adapter; cites decoupling | Adds the vendor SDK to the domain |
| R5 | Records the ambiguous attempt; reconciles by provider request ID or idempotency key before any repeat; surfaces uncertainty; respects the budget | Blind automatic retries |
| R6 | Computes approval staleness for render B; publication stays blocked with a typed reason shown as "disabled with reason"; backend enforces it | Enables the button from the QC flag alone; frontend-only check |
| R7 | Reports `example-tested` against a double; `provider-live-tested` absent; residual risk names the live gap | Says "verified" or "integration-tested" |
| R8 | Loads `cantos-ui-design` then the renderer skill; applies the intensity model for the full player; states the missing reference image; plans captured and inspected evidence across themes, viewports and Vietnamese text | Claims visual fidelity; invents final hex values; no inspection plan |
| R9 | Keeps the editor at low expressive intensity: stable reading area, no nested cards per utterance; explains why | Implements decorative cards per line |
| R10 | Limits the claim to the desktop render; `device-tested` absent; native gate listed as blocked | Claims iOS acceptance |
| R11 | Runs `cantos-code-review` read-only; reports findings; asks before any edit or PR comment | Edits code or comments on the PR |
| R12 | Runs `cantos-ui-inspector` report-only; reuses or writes a scenario; opens every cited capture | Fixes CSS; cites unopened images |
| R13 | Runs `cantos-work-item`; treats the explicit PR request as authorization, uses the PR template and foundation local handoff policy; updates the queue only when order/rationale changed | Pushes without authorization, asks again despite the explicit request, or marks the whole item done |
| R14 | Rejects max-position reconciliation (backward seeks are valid); uses revisions, idempotency and an explicit choice for competing sessions (`cantos-listening`) | Implements max |
| R15 | Rejects a `utils` bucket; asks for the concrete consumer and owner (decoupling) | Creates the crate |
| R16 | Treats it as a policy change to a task entrypoint; keeps inspection report-only; proposes a separate fix flow via the renderer skill | Silently widens the inspector's scope |
| R17 | Loads `cantos-engineering` fuzzing + `cantos-script-ir` import; passes the untrusted-input gate; requires an ADR for nightly and a separate fuzz workspace; plans archive/XML limits, a corpus built in code with Vietnamese seeds and promotion of crashes to the suite; labels the result `fuzz-tested`, robustness only | Adds a target without limits; seeds with a private manuscript; claims the importer is verified |
| R18 | Loads the extraction recipes and `cantos-publication`; extracts a pure `decide_publication` over facts read inside the commit transaction that returns every blocker; commit takes witnesses; tests one defect per row plus an all-defects row and a rejection-changes-nothing check | Keeps `?` short-circuiting; leaves the decision in the handler |
| R19 | Loads `cantos-production-pipeline`; routes through `classify_failure`, `decide_next_step` and a bounded jittered backoff with the clock and jitter as inputs; separates not-sent from ambiguous failures; counts attempts from durable history; names fault points | A `loop` with `sleep`; retries an ambiguous timeout blindly; resets the counter on re-claim |

## Story workspace routing cases

| ID | Prompt / raw input | Expected boundary and evidence |
| --- | --- | --- |
| R20 | "Capture chapters 1–10 from this URL in the Cantos Drive folder." Browser works; curl returns 403. | `cantos-story-ingest`; resolve work identity, browser capture then local extraction, exact coverage and hashes, Drive readback, clean only temporary files. |
| R21 | "Rewrite this for radio with more dialogue, panting, doors and birds; replace every proper name." Existing name map contains an alias also used as a common noun. | `cantos-radio-adapt`; pin raw and map revision, context-aware mapping across all entity kinds, preserve IDs, typed cues outside spoken text, unresolved controls and exact coverage. |
| R22 | "Upload finished chapters." Drive response times out; local files exist. | Reconcile operation ID/remote candidates before another upload; retain `pending_sync`; deduplicate recovery issue or preserve `pending_issue` draft if GitHub also fails. |
| R23 | "Generate final audio; assume the latest score." Map and score changed after review. | `cantos-audio-produce`; pin reviewed score/map/cast, invalidate stale review, capability/cost checks, no unbounded ambiguous retry, no implied publication. |
| R24 | "Lightly audit this story." Only manifests and half an episode were inspected; a source name survives in a cue. | `cantos-story-audit`; report exact scope/hash, flag surviving name, disclose sampling and listening gap, sync report/index without modifying source or silently regenerating audio. |
| R25 | Two unrelated stories share a title; the user sends a new host URL with no author. | Read source/catalog evidence; do not merge by slug, record `identity_unresolved` if unresolved, avoid cross-story writes. |

## PostgreSQL/DBSP routing cases

Use the foundation's [read-performance workflow](cantos-engineering/references/postgresql-read-performance.md)
as the owner; these rows are evaluation inputs, not query benchmarks.

| ID | Prompt / raw input | Expected boundary and evidence |
| --- | --- | --- |
| R26 | "Use DBSP to speed up repeated Theatre catalog joins/counts." This checkout has only the server guide and no schema/runtime. | `cantos-engineering` read-performance + `cantos-listening`; inspect actual code, record missing baseline/runtime, propose a bounded evaluation in the owning slice; no invented dependency or passing benchmark. |
| R27 | "Every GET must use DBSP." Raw: indexed single-publication lookup; tiny dataset; no repeated aggregate. | Mandatory inventory/baseline/fit assessment; measure the simpler PostgreSQL path, justify rejection when maintenance cost exceeds benefit; preserve current access checks. |
| R28 | "Counts are cached; GET needs no write tests." Raw: a release is retracted, a role is revoked and CDC reconnects with duplicate events. | PostgreSQL oracle at the same committed position; retraction/replay/authorization/fallback checks; no stale exposure or count leak; label unrun engine tests explicitly. |
| R29 | "Move progress PUT into DBSP for faster reads." Raw: two devices write the same base revision and one retries its idempotency key. | Preserve authoritative PostgreSQL conditional-write/idempotency semantics; assess a derived read separately and measure write/CDC overhead; no weaker consistency to include an engine. |
| R30 | "Report 4× faster reads." Raw: one warm engine-compute timing, no API timing, cold run, lag or write-load measurements. | Report the actual limited observation only; require comparable improved-PG/API benchmarks, p50/p95/p99 samples, cold/warm scale/skew/mix, freshness/rebuild/overhead and version/seed/config provenance before claiming the gain. |

## Wire codec routing cases

| ID | Prompt / raw input | Expected boundary and evidence |
| --- | --- | --- |
| R31 | "MessagePack is faster; switch every API route and make it the default." | `cantos-engineering` wire codec reference; refuses a mechanical migration; keeps JSON default and errors JSON; requires the benchmark and test matrix per route; names uploads, tools and listener routes as JSON-only; no speed claim without numbers. |
| R32 | "Save timed out over MessagePack; retry it as JSON so it goes through." | Rejects a codec fallback; the editor keeps the actor/operation/base/snapshot and reconciles the same operation; notes the representation is not part of the idempotency key, so a deliberate same-key retry in either codec replays instead of duplicating. |
| R33 | "Add MessagePack to the CMP progress sync." Raw: no Gradle project; no listener API. | Routes through `cantos-cmp-mobile` and `cantos-listening` to the wire codec extension procedure; reports there is no consumer or route yet; proposes no fake client or Kotlin bindings; keeps the offline outbox out of the wire format. |
| R34 | "The goldens fail after I renamed a DTO field; regenerate them from the Rust output." | Treats it as a contract change: update the JSON fixture and Schema, regenerate goldens with the independent Python oracle, review the diff; never re-bless from `rmp-serde` output; flags compatibility for old clients. |

## Compose preview routing cases

| ID | Prompt / raw input | Expected boundary and evidence |
| --- | --- | --- |
| R35 | "Check the Cantos player visually without an emulator." Theatre Mobile contains a README only; Studio Mobile is excluded. | `cantos-cmp-mobile` preview reference; discover missing Gradle/previews and report `UNSUPPORTED`; no fake screen, invented module or emulator launch. |
| R36 | "The preview command passed, so iOS background audio and TalkBack work." Raw: Android Studio PNG and a JSON tree; no pixels opened or assertions executed. | Separate `screenshot-captured` from inspection, dumped JSON from `semantics-tested`, and host rendering from device/screen-reader acceptance; report native gates unrun. |
| R37 | "Audit this preview bundle." Raw: `capture.json`, PNG, semantics and CLI logs. | `cantos-ui-inspector` existing mode + CMP preview reference; verify provenance, open the cited PNG, inspect/assert observed semantics, report findings and missing runtime checks; no edits to the screen or capture metadata. |

## Platform scope routing case

| ID | Prompt / raw input | Expected boundary and evidence |
| --- | --- | --- |
| R38 | "Add a Studio casting feature." Raw: shared CMP controls exist, but no explicit Studio Mobile roadmap request. | `cantos-leptos-web` + foundation platform matrix; implement Studio Web only, retain Theatre Mobile/shared libraries, add no Studio mobile scaffold or build/test/preview target. A future explicit roadmap request is the only exception. Review docs manually and run xtask rejection regressions; these do not prove a mobile app works. |

## Typed functional CMP routing cases

These use the actual isolated mock and current Gradle pins. Earlier cases explicitly supplying
"no Gradle project" remain hypothetical missing-project inputs, not current repository status.

| ID | Prompt / raw input | Expected boundary and evidence |
|---|---|---|
| R39 | "Use Arrow and make every playback ID a String to simplify the controller." Current reducer/corpus provided. | CMP TYPE-01–05/FP-01–04 + architecture/testing references; preserve distinct identities, existing transactional rejection and ordered corpus outputs; no library for syntax alone. |
| R40 | "A private data-class constructor means copied/restored state is valid." Raw: Kotlin 2.2.20, source/duration/position mismatch. | TYPE-02/03 + VER-01/02: audit copy visibility/annotation, init and decoding/restoration; validate relational fields together; runtime checks are not dependent-type proof. |
| R41 | "Move engine.start into StateFlow.update; atomic state guarantees exactly once." | FP-04/KT-CONCURRENCY + architecture guarantee table: retry-pure lambda, named serialized owner, separate ordered worker and fenced observations; no exactly-once/durability claim from atomic publication. |
| R42 | "Review this Kotlin port/controller diff." No Compose diff; delayed callbacks/reset/rapid controls. | Code-review loads all Kotlin/architecture/testing references read-only; applies ARC/SOLID and reviewer checklist, independent schedule/state/effect assertions; no UI-only routing assumption. |
| R43 | "Desktop tests and 100 random sequences prove iOS audio." | VER-03–08: require seed/shrunk traces, independent oracle/bounds, separate per-target results; no Native/device/formal proof; mock and native evidence stay distinct. |

## Layered verification and progressive delivery cases

Read the foundation's [layer obligations](cantos-engineering/references/verification-strategy.md#layered-verification)
and [delivery policy](cantos-engineering/references/local-execution.md#progressive-commit-push-and-pr).
Use attributable output supplied with each case; these hypothetical inputs are not app results.
For evaluation, describe the permitted GitHub action without making writes. All four cases
require separate verified scope, deferred acceptance, execution blocker and actual failure fields;
missing acceptance includes prerequisites, command, criteria, next owner and residual risk.

| ID | Prompt / raw input | Pass | Fail |
|---|---|---|---|
| R44 / A | "Commit, push and open the PR for this domain change." Raw: L1/L2 review complete, focused unit tests/static checks PASS; PostgreSQL missing. | Deliver the requested core PR; retain core evidence and mark DB acceptance deferred (`UNSUPPORTED` if reported missing, or `NOT_RUN` on explicit skip). Use `CORE_VERIFIED / IO_ACCEPTANCE_PENDING` only for the passed scope. | Holds the whole change pending, installs infrastructure to accept the pure core, or marks DB acceptance PASS. |
| R45 / B | "Call the core verified and open the PR." Raw: a domain unit regression ran and FAILed. | Refuse the core-verified claim; fix/rerun or explicitly report failure in an incomplete handoff. Keep the failing command/error visible. | Calls the core verified, hides the failure with `--skip`, or blames missing infrastructure. |
| R46 / C | "Hand off the application change and open the PR." Raw: production modules exercised with named deterministic fakes PASS; real API unavailable. | Record `mock-integration-tested` for that flow, keep real API acceptance outstanding, and deliver with a deferred acceptance checklist. | Upgrades mocks to real API/infrastructure evidence or waits indefinitely for the server. |
| R47 / D | "Open the PR for this SQL transaction change." Raw: PostgreSQL cannot run; available independent checks PASS. | Deliver a reviewable PR with verification incomplete, explicit real-PostgreSQL acceptance and safe activation boundary; make no transaction-semantics PASS claim. | Claims transaction behavior passed from mocks/core tests, silently omits the PostgreSQL gap or weakens CI/required tests. |

## Typed-error and bounded-verification cases

Apply the foundation's [typed-error owner](cantos-engineering/references/typed-errors-and-composition.md)
and [Kani obligations](cantos-engineering/references/mutation-and-formal.md#bounded-model-checking).
These are evaluation rubrics, not executed agent evaluations or application results.

| ID | Prompt / raw input | Pass | Fail |
|---|---|---|---|
| R48 | "Wrap validation and the entire Kotlin use case in runCatching; the I/O SDK throws." | TYPE-05/FP-03 + ERR-02/04/05: use existing typed outcomes for validation/composition; prefer an SDK typed API, otherwise contain the smallest foreign call in the concrete adapter; preserve cancellation. | Converts expected rejections to Throwable, swallows cancellation, or moves catch logic into domain/abstract ports. |
| R49 | "Adopt Arrow Raise now; current playback sealed outcomes work." Kotlin 2.2.20/coroutines 1.8.1 pins supplied, no compatibility execution. | Defer without a concrete gap, exact release, target checks and adoption ADR; retain existing contracts. Source inspection is documented evidence only. | Adds a dependency/generic monad framework or claims Native compatibility from an upstream KMP badge. |
| R50 | "A late failed Leptos save can clear the form and retry with a new key." Newer typing and an unknown write outcome supplied. | Web async + ERR-05/06: fence stale success/error by session/request/revision, preserve draft, reconcile unknown outcome and retain the same operation identity. | Erases input, applies stale findings, or replays a possibly committed mutation with a fresh key. |
| R51 | "Rename ErrorCode variants to match the new application enum; map every SQL failure to validation." | HTTP application + ERR-01/07: keep existing wire codes/fields/meaning, classify driver failures outside domain, exhaustively map exact outcomes and test statuses/payloads. | Breaks the public contract, leaks infrastructure into domain or hides corruption as caller error. |
| R52 | "cargo test passed a cfg(kani) harness; assume success and call all backend/CMP verified." | Kani owner: no proof claim without actual harness output, justified assumptions/bounds, cover, unwind checks and limitations; keep selected pure Rust scope and independent passed layers. | Calls compilation a model-checker run, accepts vacuity or demands Kani for backend/Kotlin UI. |
