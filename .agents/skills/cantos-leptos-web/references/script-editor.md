# The Studio script editor

> **Scope.** Build and review the text-first Studio editor over Script IR drafts: stable dialogue
> identity in keyed rendering, Vietnamese IME composition, preserving unsaved edits, explicit
> draft/save/version states and revision-conflict UI, undo, keyboard reordering, speaker
> correction, validator-driven messages, status kept outside editable text and large scripts.
> Use for any change to editing, saving, validation display or AI-suggestion review in Studio.

Product requirements: [Script/editor scope and acceptance](../../../../docs/assurance/script-editor-acceptance.md#acceptance-criteria),
[UI system § Studio](../../../../docs/design/ui-system.md#studio-text-first-production) and
[business rules § Content and revisions](../../../../docs/product/business-rules.md#work-chapter-and-episode-identity).
What a save creates, how IDs are assigned and which validation errors exist belong to
[`cantos-script-ir`](../../cantos-script-ir/SKILL.md) — especially
[`revision-lifecycle.md`](../../cantos-script-ir/references/revision-lifecycle.md) and
[`vietnamese-text.md`](../../cantos-script-ir/references/vietnamese-text.md). This reference owns
how the browser editor honors them.

## 1. Shape: a pure editor core under a thin Leptos layer

The editor is a lifecycle, not a form. Give it a reducer in a core crate with no Leptos or
`web-sys` dependency, so its laws run as plain `#[test]` and property tests:

```rust
// Illustrative, proposed — not existing code. Names of IR types come from cantos-script-ir.
pub struct EditorState {
    base_seq: u64,                 // the draft head this content was edited from
    scenes: Vec<SceneDraft>,       // order is sequence; identity is each row's id
    status: DraftStatus,
    history: History<EditorOp>,    // structural undo/redo
}

pub enum EditorEvent {
    TextCommitted { id: DialogueId, text: String },
    SpeakerChanged { id: DialogueId, speaker: SpeakerRef },
    Inserted { scene: SceneId, after: Option<DialogueId>, row: DialogueDraft },
    Moved { id: DialogueId, to: Position },
    Removed { id: DialogueId },
    Undo,
    Redo,
    SaveRequested,
    SaveAcknowledged { snapshot: SnapshotId, version: SavedVersionId, head_seq: u64 },
    SaveFailed { snapshot: SnapshotId, failure: SaveFailure },
    SaveConflicted { snapshot: SnapshotId, head: SavedVersionId, head_seq: u64 },
}

pub fn reduce(state: EditorState, event: EditorEvent) -> (EditorState, Vec<EditorEffect>);
```

Effects (`Save(SaveDraft)`, `Announce(message)`, `Focus(id)`) are data executed by the shell. The
draft is owned at the episode-editor route root, above the inspector, timeline and narrow-screen
panes, so switching panes never unmounts it ([`web-architecture.md`](web-architecture.md#6-shell-owned-state-that-must-survive-navigation)).

## 2. Stable dialogue identity

| Rule | Failure mode | Good | Counterexample | Oracle | Status |
|---|---|---|---|---|---|
| Keyed lists use dialogue IDs, never indexes | inserting a line above moves focus, an active IME composition or a validation message onto a different dialogue | `<For each=rows key=\|row\| row.id let:row>` | `key=\|(index, _)\| *index` | real-browser test: focus row `dialogue-02`, insert above it, assert focused element still has `data-dialogue-id="dialogue-02"` and its text | proposed |
| A new row has its permanent ID from creation | the key changes later; the row remounts and loses focus mid-typing | the shell mints the ID from an injected ID source when the row is inserted ([identity across edits](../../cantos-script-ir/references/revision-lifecycle.md)) | a temporary key replaced on save acknowledgement | component test with a sequence ID source: save a new row, focused element unchanged | proposed |
| Edits follow the IR identity table: split, merge, duplicate, delete-then-undo | undo of a delete mints a new ID and the line is treated as new speech | undo restores the same ID and content | delete + re-insert with a fresh ID | reducer property over op sequences | proposed |
| Moving a dialogue changes order, not identity | reorder looks like delete + insert; speech is treated as changed | `Moved { id, to }` | remove + insert with a fresh ID | reducer property: reorder preserves the ID multiset and every row's text | proposed |
| Status, clips and findings are keyed by dialogue ID and revision | a stale-clip marker attaches to the wrong line after reorder | `HashMap<DialogueId, ClipStatus>` | `Vec<ClipStatus>` aligned by position | reducer and component tests after reorder | proposed |

Moving a dialogue does not inherently change its spoken render ([Script IR § Revision and
validation policy](../../../../docs/architecture/script-ir.md#revision-and-validation-policy)); whether
mixes are invalidated is the pipeline's decision ([`fingerprints-and-invalidation.md`](../../cantos-production-pipeline/references/fingerprints-and-invalidation.md)).

## 3. Text input and Vietnamese composition

Use native `<textarea>` (dialogue text) and `<input>`/`<select>` controls. They provide IME,
spellcheck, native undo, selection and accessibility for free; `contenteditable` gives up all of
that and is justified only by a measured need with its own ADR.

Vietnamese is typed through input methods: Telex (`dieenx` → `diễn`) and VNI (`die6n4` →
`diễn`), via macOS and iOS built-in Vietnamese input, Android keyboards, and Windows tools such as
Unikey or EVKey. Two very different event patterns reach the page:

- **Composition-based** input (marked text) emits `compositionstart`, `compositionupdate`,
  `input` events with `isComposing = true`, then `compositionend`. Engines differ on whether the
  final `input` fires before or after `compositionend`.
- **Keystroke-replacement** input (several Windows tools) emits no composition events: it sends
  rapid backspaces and replacement characters as ordinary `input` events.

| Rule | Failure mode | Oracle |
|---|---|---|
| While composing, never commit, trim, normalize, validate-and-rewrite or write `value` | the IME loses its marked text: `diễn` becomes `dieenx` or `dđ`; the caret jumps | real-browser IME walkthrough per target; CDP-simulated composition regression in Chromium ([`web-testing.md`](web-testing.md)) |
| Commit on `compositionend` *and* on non-composing `input`; commits are idempotent | double commit or lost final character, depending on event order | reducer ignores a `TextCommitted` equal to the current text; tests in two event orders |
| Per-keystroke work stays cheap and side-effect free | keystroke-replacement IMEs trigger validation flashes, announcements or saves on every intermediate state | component test: ten rapid `input` events produce no announcement and no port call |
| The model never writes `value` into a focused field it did not change | writing `value` clears native undo history in most engines and moves the caret | review; interaction test that undo works after typing |
| Unicode normalization happens at the save boundary, per the IR text policy | normalizing NFD to NFC while typing rewrites the field under the IME | example test on the save mapping with NFD input |

```rust
// Illustrative. While focused, the textarea owns its text; the model listens.
let composing = RwSignal::new(false);
view! {
    <textarea
        class="dialogue-row__text"
        data-dialogue-id=id.to_string()
        aria-describedby=described_by
        prop:value=initial_text            // set on mount and on explicit external changes only
        on:compositionstart=move |_| composing.set(true)
        on:compositionend=move |ev| { composing.set(false); commit(event_target_value(&ev)); }
        on:input=move |ev| if !composing.get_untracked() { commit(event_target_value(&ev)) }
    />
}
```

External changes (structural undo, conflict resolution, reload after reopen) are the only writes
of `value` into a mounted field; they go through one helper that restores selection when the
field is focused. Text containers must not clip stacked diacritics ([`styling.md`](styling.md)).

## 4. Draft, save and version states

Saving is explicit; the editor requires explicit save and version behavior, and silent background
autosave of server revisions is a product decision, not an implementation detail.

```rust
// Illustrative. The base head sequence lives beside the status in EditorState.
pub enum DraftStatus {
    Clean,
    Dirty,
    Saving { snapshot: SnapshotId, operation: OperationId },
    SaveFailed { snapshot: SnapshotId, operation: OperationId, failure: SaveFailure },
    Conflict { head: SavedVersionId, head_seq: u64 },
}
```

| Status | Visible text (vi, via locale resources) | Actions |
|---|---|---|
| `Clean` | "Đã lưu · phiên bản 4" | — |
| `Dirty` | "Có thay đổi chưa lưu" | "Lưu bản nháp" (Ctrl/Cmd+S) |
| `Saving` | "Đang lưu…" | save is `aria-disabled` with that reason; typing continues |
| `SaveFailed` | "Chưa lưu được. Nội dung của bạn vẫn còn trên trang này." | "Thử lại": same operation ID if the draft still equals the snapshot, a new save otherwise |
| `Conflict` | "Kịch bản đã được lưu ở nơi khác (phiên bản 5)." | "Xem khác biệt"; then, explicitly, "Lưu bản của tôi thành phiên bản 6" or "Mở phiên bản 5, bỏ thay đổi của tôi" |

The save request and its server decisions (`Append`, `AlreadyApplied`, `Conflict`,
`OperationReused`) are owned by [`revision-lifecycle.md`](../../cantos-script-ir/references/revision-lifecycle.md):
each save names `base_seq` and carries an operation ID minted at intent time. Rules, each with a
reducer or component test:

- Edits made during `Saving` survive the acknowledgement: on `SaveAcknowledged`, status is
  `Dirty` against the new head if the draft differs from the *snapshot that was sent*, otherwise
  `Clean`. Compare with the sent snapshot, never with the server's echo, which may be normalized.
- `AlreadyApplied` is an acknowledgement (the first attempt committed). `OperationReused` is a
  client bug: show a generic failure, log it, never retry it.
- A retry reuses the operation ID only while the draft equals its snapshot; changed content is a
  new save with a new ID, or the server rightly answers `OperationReused`.
- A failed or conflicting save never changes rows. Never reload the server copy over the draft.
- A conflict is never resolved by last write wins. The creator compares (per dialogue ID: changed
  here, changed there, changed in both) and chooses; the resolution is an ordinary save whose
  base is the current head. Automatic merging is out of scope for the editor; a line-level three-way
  merge keyed by dialogue ID is the later shape, as a pure function with laws
  `merge(b, b, t) == t`, `merge(b, m, b) == m`, and lines edited on both sides always surfacing.
- The status region is outside the editable text, uses `role="status"`, and announces a
  transition once, never per keystroke.
- Leaving the route with a dirty draft asks for confirmation in-app and registers
  `beforeunload` only while dirty ([`dom-and-media-interop.md`](dom-and-media-interop.md#5-storage-files-and-unload)).

## 5. Status outside volatile editor text

Generation status, stale-clip markers, validation findings and AI suggestions are rendered next to
the row (a gutter or a row footer linked by `aria-describedby`), never inserted into the textarea
value or as markup inside the text. After an edit, the row shows that its clip predates the change
("Âm thanh tạo từ phiên bản 3 — đã sửa sau đó"); after save, the server's dependency plan is the
authority on what must regenerate. Regeneration scope is shown before it starts
([UI system § Studio](../../../../docs/design/ui-system.md#studio-text-first-production)).

AI-inferred speakers, emotions and cues are suggestions until accepted ([casting/performance guidance](../../../../docs/product/business-rules.md#casting-and-performance)).
Render each suggestion as a distinct, labeled proposal with accept and reject actions; accepting
is an ordinary undoable editor operation. Never merge a suggestion into the text silently.

## 6. Speaker correction

- The speaker is a choice among the adaptation's characters by `CharacterId`, displayed by name
  ("Người dẫn chuyện", "An", "Minh"); a native `<select>` or an accessible combobox, not free text.
- An unresolved speaker is the draft state `SpeakerRef::Unresolved { label }` from
  [`cantos-script-ir`](../../cantos-script-ir/SKILL.md) (an AI-proposed "Bà cụ" not yet in the
  cast), shown with a warning icon and text; submission rejects it through the validator, not the UI.
- Changing a speaker keeps the dialogue ID. A bulk correction ("Gán tất cả lời của “Bà cụ” cho
  Người dẫn chuyện") is one undoable operation over a set of IDs, previewed with its count; adding
  a new character to the cast is a separate, explicit action.

## 7. Undo and reordering

| Concern | Rule | Test |
|---|---|---|
| Text undo inside a field | native browser undo; the app does not intercept Ctrl/Cmd+Z inside a textarea | interaction test: type, undo, text restored |
| Structural undo (insert, delete, move, speaker, accepted suggestion) | app-level history in the reducer, exposed as "Hoàn tác"/"Làm lại" buttons and chords when focus is outside a text field | property: `undo(apply(s, op)) == s` on rows for every op |
| Keyboard alternative to drag | row actions "Chuyển lên"/"Chuyển xuống" and a chord on the row handle; drag and keys dispatch the same `Moved` event | interaction test: move by keyboard, focus stays on the moved row, one announcement ("Đã chuyển lời thoại lên vị trí 3 trong cảnh Sau buổi diễn") |
| Chord conflicts | Option/Alt+Arrow moves the caret by word on macOS; bind reorder chords on the row handle, never in a text field | interaction test with focus in the textarea: chord does not move the row |
| Global shortcuts | none fire while typing ([`dom-and-media-interop.md`](dom-and-media-interop.md#4-keyboard-shortcuts-never-fire-while-typing)) | suppressed-case test |

## 8. Validation messages from the Script IR validator

The validator and its `Diagnostic { path, issue, severity }` report are owned by
[`schema-versioning.md`](../../cantos-script-ir/references/schema-versioning.md). The editor maps
every `ValidationIssue` variant exhaustively to a message key and resolves the stable-ID path to a
row, so a new variant fails compilation instead of rendering nothing:

```rust
// Illustrative. Variants belong to the validator; no `_` arm, no parsing of message strings.
fn message_key(issue: &ValidationIssue) -> MessageKey {
    match issue {
        ValidationIssue::DuplicateId { .. } => MessageKey::DuplicateId,
        ValidationIssue::MissingSpeaker => MessageKey::MissingSpeaker,
        ValidationIssue::UnknownSpeaker { .. } => MessageKey::UnknownSpeaker,
        ValidationIssue::EmptySpokenText => MessageKey::EmptySpokenText,
        ValidationIssue::CueAnchorUnresolved { .. } => MessageKey::CueAnchorUnresolved,
        ValidationIssue::CueLikeMarkupInSpokenText { .. } => MessageKey::CueLikeMarkup,
        ValidationIssue::IntensityOutOfRange { .. } => MessageKey::IntensityOutOfRange,
    }
}

fn present(diagnostic: &Diagnostic, script: &EditorState) -> Finding {
    Finding {
        target: script.locate(&diagnostic.path),   // row, cue or document; never an index
        message: message_key(&diagnostic.issue),
        blocking: matches!(diagnostic.severity, Severity::Blocking),
    }
}
```

- Run the shared validator in WASM for immediate feedback after `compositionend` or an idle pause,
  not on each `input`; the server re-validates on save and its findings win.
- Show findings in two places: inline on the row (`aria-invalid` plus text, never color alone)
  and in a summary list whose entries move focus to the row.
- Keep the validator's document order so the list does not jump between saves. `Blocking` and
  `Review` findings look different and say so in text; which issues block is the validator's
  classification, never re-decided in the editor, and never hidden to make a save look clean.

## 9. Large scripts

Measure before restructuring: dialogue count, input latency while typing in the last scene
(DevTools performance or Event Timing), memory after a long session. Report before/after numbers.
Apply remedies in this order:

1. **Narrow reactive scope.** One signal or store field per row (candidate: `reactive_stores`),
   so typing in one row re-renders that row only. Never clone the whole script per keystroke.
2. **Render less by structure.** Collapse scenes, render the selected scene, paginate acts.
3. **Virtualize last.** Windowing breaks browser find, screen-reader browsing of unmounted rows,
   focus restoration and an active composition in a row scrolled out of view. If adopted, never
   unmount the focused or composing row, provide in-app search, and verify with assistive technology.

## 10. Regression ledger seeds

| Claim | Cheapest evidence |
|---|---|
| Insert above the focused row keeps focus and text on the same dialogue | real-browser interaction test |
| Composition is never interrupted by commits, validation or `value` writes | CDP composition regression plus a recorded IME walkthrough per OS |
| A failed save keeps every row and offers retry | component test with a failing fake port |
| A conflict never overwrites and offers compare/rebase/discard | component test with a conflicting fake port |
| Edits during save survive the acknowledgement | reducer example and property test |
| Undo inverts every structural operation, restoring the same IDs | reducer property test |
| A retry after a lost response is acknowledged, not a conflict | component test: fake answers `AlreadyApplied` |
| Every `ValidationIssue` variant has a message and a located target | exhaustive match (`type-enforced`) plus one example per variant |
| Moving a row by keyboard keeps focus and announces once | real-browser interaction test |
