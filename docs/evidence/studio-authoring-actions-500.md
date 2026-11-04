# Studio authoring actions and quiet review — issue 500

Source-bound implementation and local evidence, 4 November 2026. Part of
[#500](https://github.com/loveoverflowcom/cantos/issues/500); this record does not close its
runtime acceptance. Base: `develop@91c2fad2fec0c5ee3eea3b4273283a46963a301f`.
The supplied Space handoff and latest issue/comments were read. Historical review captures
were built from an unattributed 31 October dist and are reference only.

## Decision and boundary

Studio Web is a low-intensity writing surface. Short visible Undo/Redo labels and a wrapping,
Web-owned history group preserve familiar names and 48-unit shared targets. An icon-only
alternative would need a separate naming/admission review. The history group deliberately
keeps native buttons and one visible, associated reason; two ordinary ActionButton wrappers
would repeat the pending/readonly explanation. Its disabled projection now reflects the
existing callback's pending guard. No history transition changes.

The inspector stays mounted and shows real findings and changes. It uses a quiet surface
instead of collapsing the entire panel, which could hide a real blocker. Optional explanations
use native disclosures. Stable scene/dialogue/cue IDs, exact finding paths/rules and change
paths remain reachable; finding buttons retain exact paths in their accessible names.
Regular native buffered fields, keyed lists, handlers and navigation/focus are preserved.
The shared Text disabled fill remains unchanged: this patch does not create a screen-local
variant or change shared qualification inputs to remove that fill.

No domain/API/schema/permission/persistence/retry/receipt change, no Studio Mobile, no deploy.
Behavior SSOT lookup found no named authoring capability; this is presentation and an
outcome-preserving adapter migration. `impact:unknown` is the conservative selector result for
canonical resource additions, answered as NOT_APPLICABLE to model changes in the ignored
assurance record. The already-enforced buffer guard is unchanged.

## Action inventory

| Owner / action definitions | Disposition and reason |
| --- | --- |
| Authoring add act, character, scene, dialogue, narration, pronunciation | Existing `ui::ActionButton`, Tonal, `ScriptReadOnly`; callbacks and structural guards unchanged; stable namespaced IDs. |
| Authoring Undo/Redo | Native intentional: compact group with a single visible localized reason associated with both controls; empty, earlier/later edge, readonly and pending states remain distinct. |
| Authoring nav toggle and act/scene/character/metadata selection | Native intentional: existing node ref, `aria-controls`/`aria-expanded`, pressed state and narrow close/focus contract from #628. |
| Authoring finding-to-field button | Native intentional: field/ancestor disclosure focus contract and full rule/path identity retained. |
| Authoring move up/down, remove, confirm, cancel | Native intentional: per-item accessible names, trigger/confirmation node refs, Escape and focus return; existing callbacks retained. |
| Authoring add cue | Native intentional in this slice: existing combined editable/nonempty-dialogue constraint remains owned here; no generic reason replaces it. |
| Authoring pronunciation removal | Native intentional: per-item/index name and existing structural buffer guard. |
| Production load, retry, inspect snapshot, snapshot history, save rights | Existing `ActionButton`, Tonal; identical guards/callbacks, operation IDs, retry admission, immutable snapshot/revision labels and rights form retained. |
| Production discard | Native intentional: existing browser confirmation before resetting drafts and retained rights. |
| Production pronunciation add/remove | Native intentional: capability-aware add guard and removal's focus return to the stable add ID; fields retain #492 ownership. |
| Inspector history/revision actions | Already migrated by #509; no repeated implementation. |
| FormSubmit and navigation anchors | Unchanged; native submit/link semantics remain intentional. |

The production “scope changed” observation was not treated as a persistence defect. Its
state owner and meaning are unchanged. Copy removed from the default reading path remains
reachable through the existing help owner, without a new tooltip implementation.

## Local evidence and limits

- `cargo xtask build cantos-studio --web`: baseline and candidate dev builds PASS.
- `cargo clippy --locked -p cantos-studio --all-targets -- -D warnings`: PASS.
- `cargo test --locked -p cantos-studio`: 475 unit tests and 5 doctests PASS, including
  three added render regressions for grouped reasons, actionable findings and stable row IDs.
- Canonical vi-VN/en resources regenerated with the existing localization generator;
  other app projections change their catalog digest headers only.
- Browser input on the actual debug StructuredEditor fixture: typing remains buffered until
  blur (one commit); Undo/Redo restore the correct text; Escape restores committed text while
  keeping focus; caret follows typed text; native text undo removes a typing step; theme and
  locale preserve the draft; narrow scene selection folds nav and focuses canvas; an intensity
  finding opens its ancestor disclosure and focuses the corresponding field; readonly stays
  native readonly and history is disabled. These are fixture interactions, not authenticated I/O.
- Source/checkpoint screenshots and artifact manifests are kept in the ignored
  `artifacts/ui-evidence/issue-500/` directory. Viewport CSS size and JPEG pixels are separate;
  the debug wrapper is outside the component. The PR records the final opened captures.
- Shared preview uses existing Button states and no new primitive or variant. The existing
  qualification gate FAILs with the same stale Leptos records on the clean fetched baseline.
  Baseline and candidate token and both renderer digests are identical; no qualification hash
  or baseline exemption was edited. This is inherited failure, not a passing admission claim.
- Final-diff preflight FAILs at that repository gate; later steps are NOT_RUN. Broad native,
  protobuf, audio, mutation and CMP gates are explicitly skipped: the resource selector broadens
  beyond this Studio-only semantic diff and the local disk has under 5 GiB free. Scoped Studio
  lint/tests cover the changed Rust owner. Independent Web verification is recorded in the PR.

L1/L2: source-inspected, inward boundaries and guards retained. L3: native examples/render
semantics and scoped lint above. L4: debug fixture browser interactions only. L5: authenticated
API/save/conflict/production acceptance NOT_RUN. Screen reader, named OS Vietnamese IME,
Safari/Firefox, real devices, full 200% Studio zoom and production action interactions remain
NOT_RUN. The shared preview's injected 200% text scale is a separate claim, not Studio page zoom.
Current screenshots cannot close all of AC-01–04 or the original two-consumer runtime matrix.
Issue checkboxes remain unchanged; no issue closure is requested.
