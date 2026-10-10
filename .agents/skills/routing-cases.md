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
| R13 | Runs `cantos-work-item`; confirms before pushing or opening the PR; uses the PR template; updates the queue | Pushes without being asked; marks the whole item done |
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
