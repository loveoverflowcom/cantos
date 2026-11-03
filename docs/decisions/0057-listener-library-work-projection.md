# ADR 0057: Work-level Listener Library projection

- Status: **accepted (owner, 3 November 2026)**. Decisions only: no Server, Web or CMP
  behavior changes until slices L02–L06 land with their own evidence.
- Date: 2026-11-03
- Decision owner: Cantos product owner
- Related issue or PR: epic [#701](https://github.com/loveoverflowcom/cantos/issues/701), acceptance
  slice [L01 #702](https://github.com/loveoverflowcom/cantos/issues/702); proposal and draft model in
  [PR #700](https://github.com/loveoverflowcom/cantos/pull/700)
- Supersedes / superseded by: N/A. Amends the completion and clear-history text of
  [Library v1](../../services/cantos-server/contracts/theatre/library/v1/README.md) and adds its
  1.3.0 specification; [ADR 0054](0054-publication-generation-and-access.md) and
  [ADR 0055](0055-development-phase-compatibility.md) are unchanged.

## Context

Known facts at `develop@1b8864d6` (source-inspected, not executed):

- A new Save is admitted on public Work metadata, while Saved `presentation=1` uses one current
  audio representative, so a saved Work without audio can show `presentation:null`.
- Continue takes each Work's newest progress register and drops it when completed, so finishing
  a chapter removes a multi-chapter Work from Continue.
- `mark_completed` is an explicit listener intent; checkpoint or seek to the end is not completion.
- `POST /library/history/clear` removes recent history only and introduces a history epoch that
  every playback start, history page and cursor carries.
- Discovery already serves one published-generation Work representation (ADR 0054/0055).

The owner's 2026-11-03 Library draft, the [decision register](../product/listener-library.md#decision-register)
and the [projection design](../architecture/listener-library-projection.md) proposed eleven
decisions. The owner selected them on 3 November 2026; D-L08 differs from the recommendation.

## Decision

| ID | Accepted disposition |
| --- | --- |
| D-L01 | One Library destination with three sections: Nghe tiếp, Đã lưu, Gần đây. |
| D-L02 | A shown Saved row always carries the Work card of the active published metadata generation and a separate `listen` state (`playable`, `no_audio_yet`); never `null` for a shown row. |
| D-L03 | The Continue anchor is each Work's newest progress register, completed or not. A completed anchor points to the immediately following visible Unit; a Unit without audio is never skipped. |
| D-L04 | A Work whose following Unit has no audio yet, or that has no following Unit, leaves Continue and re-enters at read time when the following Unit becomes playable. |
| D-L05 | `mark_completed` is sent on an explicit listener action **or** when the engine acknowledges the natural end of the current account/session/source at the exact duration. Checkpoint or seek to the end is still never completion. Auto-next playback is out of scope. |
| D-L06 | No Work completion state until Catalog has an owner-declared series status; “caught up” is a derived read state only. |
| D-L07 | “Following Unit” uses published order for every kind, podcasts included. |
| D-L08 | **The “Xóa lịch sử gần đây” use case is removed.** Web and CMP drop the action in L04/L05; L06 removes `POST /library/history/clear`, `HistoryClear`, `history_epoch_conflict` and the history-epoch wire fields in the same delivery that updates every client. Engine-session de-duplication of playback starts stays. |
| D-L09 | Library stays public-only until the #634 shared projection supplies a complete viewer-aware predicate. |
| D-L10 | A guest sees a sign-in invitation in Library; no guest persistence or merge. |
| D-L11 | Additive `presentation=2` on the Saved and Continue page reads (Library 1.3.0); Web and CMP adopt it; L06 removes the superseded lanes once no current client uses them. |

Authoritative now: this table, the amended Library v1 text and the 1.3.0 schema definitions and
fixtures as a specification. The routes keep serving 1.2.0 behavior until L02/L03.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Keep the current Continue rule | No change | Long-form listening loses its place after every chapter | Rejected (D-L03) |
| Keep caught-up Works in Continue, disabled | Always visible | Non-actionable rows crowd the primary section | Rejected (D-L04) |
| Skip to the nearest Unit with audio | Always something to play | Silently skips a chapter of a novel | Rejected (D-L03) |
| Explicit-only completion | No contract change | The next-chapter journey needs a manual step per chapter | Rejected (D-L05) |
| Keep history clear (history only) | Existing, tested | Confusing scope: Continue still shows the same Works; owner sees little value | Rejected by the owner (D-L08) |
| Replace representations in place | No temporary lane | Server, Web and CMP must land in one delivery | Rejected (D-L11) |

## Consequences

- Product and user impact: Continue follows the Work, Saved shows Works without audio, and there
  is no user-facing way to delete recent history. Listening history is personal data: a future
  account-level data export/deletion flow, designed with receipt and tombstone retention, is the
  place for that right; it is not scheduled here.
- Compatibility and migrations: 1.3.0 is additive until L06. L06 is a breaking wire change under
  ADR 0055 (all current clients updated and tested in the same delivery). Registered migrations
  are never edited; a now-constant epoch column may stay or be dropped by an appended migration.
- Operations, storage, and production cost: none now. L02/L03 measure Saved/Continue on a
  dataset profile under #384; a derived `listener_work_activity` index is added only if measured.
- Implementation and recovery requirements: Server decides the next action from Catalog/Media
  facts in one read snapshot; clients never infer access or substitute a release. Kotlin and Web
  decoders must enforce the per-kind constraints that the schema expresses with `allOf`.
- Deferred questions: shared Library (D-L09), per-Work dismiss from Continue, data
  export/deletion, collections, per-session history, auto-next, Work completion.

## Validation and revisit conditions

The draft model `cantos.listening.library-projection` encodes these decisions; its lifecycle
stays draft until L02/L03 implement them. Validate with the slice acceptance criteria of #701
(Turso/HTTP/contract tests, browser and device observations, the L06 synchronization matrix).
Revisit D-L07 with podcast listening evidence, D-L08 if listeners or a legal requirement ask for
self-service history deletion, and D-L04 if usability testing shows caught-up Works are missed.
