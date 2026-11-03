# Listener Library — vision, journeys and decision register

**Status: proposed.** This records the owner's 2026-11-03 Library draft after review against
`develop@1b8864d6`. It is not accepted policy: the implemented
[Library v1 contract](../../services/cantos-server/contracts/theatre/library/v1/README.md) still
governs current behavior, and each decision below becomes policy only when the acceptance slice
of the Library epic ([#701](https://github.com/loveoverflowcom/cantos/issues/701), acceptance slice [L01 #702](https://github.com/loveoverflowcom/cantos/issues/702)) records it (ADR or
owning-doc update). The technical design is in
[Listener Library projection](../architecture/listener-library-projection.md); the draft
executable expectation is `cantos.listening.library-projection`
([inventory](../../services/cantos-server/behavior/listening/library_projection.md)).

## Vision

> **Cantos là thư viện nghe cá nhân: người nghe giữ những tác phẩm mình quan tâm, tiếp tục đúng
> chương và đúng vị trí sau mỗi lần gián đoạn, tìm lại những gì đã nghe, và chuyển giữa Web với
> điện thoại mà vẫn giữ được lựa chọn của mình.**

Cantos is a personal listening library: a listener keeps the Works they care about, resumes the
right chapter at the right position after every interruption, finds what they listened to and
moves between Web and phone without losing those choices. The core value is that **the listener
never has to remember which Work, chapter or minute they were on**.

Audience hypothesis, not research: people who listen to novels chapter by chapter, short stories
and podcasts in fragmented time (commute, chores, before sleep). Validate it before treating it
as a segment decision.

## The Library destination

The Library lives in the existing Theatre shell destination (Web `theatre.shell.library`,
“Thư viện”; CMP `theatre.cmp_shell.library`). Navigation placement follows the shell owner
([#690](https://github.com/loveoverflowcom/cantos/issues/690)). It has three sections, not three
navigation tabs:

| Section | Shows | Primary action | Source register |
| --- | --- | --- | --- |
| **Nghe tiếp** (Continue) | Work, chapter/episode and either the stored position or the next chapter | Nghe tiếp / Nghe chương 13 | progress + published Unit order + current audio |
| **Đã lưu** (Saved) | Works the listener deliberately kept, including Works without audio yet | Open the Work; Bỏ lưu | Saved register + published Work metadata |
| **Gần đây** (Recent) | Works whose playback actually started, newest first, including finished ones | Open again; Xem tất cả lịch sử | history (latest confirmed start per Work) |

Recommended layout, to be confirmed through [`cantos-ui-design`](../../.agents/skills/cantos-ui-design/SKILL.md):
Continue first and most prominent (one to three items), then Saved, then a compact Recent entry
with “Xem tất cả lịch sử”. Continue and Recent overlap heavily, so Recent should not repeat
Continue's cards at the same weight on the Library page.

## Journeys

| ID | Journey | Observable outcome |
| --- | --- | --- |
| J1 | Save for later: Discover → Lưu → close and reopen | The Work is in Đã lưu, also when it has no audio yet (“Chưa có bản nghe”) |
| J2 | Interrupted listening: play → pause → come back | Nghe tiếp opens the same exact release at the stored position; a replaced release is offered, never substituted |
| J3 | Long-form listening: finish a chapter | Nghe tiếp shows “Nghe chương 13” for the immediately following published chapter |
| J4 | Device switch: save or listen on Web, open CMP with the same account | The same Saved/progress state after synchronization; account B never sees account A |
| J5 | Caught up, then a new chapter | “Bạn đã nghe đến chương mới nhất” moves the Work out of Continue; when the next chapter becomes playable it returns without a notification subscription |
| J6 | Clear recent history | Gần đây is empty; the copy says that Nghe tiếp, Đã lưu and progress are kept |

## Concepts that stay separate

| Concept | Meaning | Not |
| --- | --- | --- |
| Saved Work | A Work-level desired boolean in the library | A download, a notification subscription, a bookmark or playback permission |
| Bookmark | One immutable exact-release position | A Saved Work or progress |
| Progress | One revisioned register per exact release | A history log or permission to fetch bytes |
| Recent history | Latest confirmed engine start per Work | A per-session log, a play count or analytics |
| Unit completion | `mark_completed` of one exact release | Work completion |
| Caught up | Every currently published chapter after the anchor is finished or absent | “Đã hoàn thành tác phẩm” |
| Work completion | The listener finished a Work the owner declared complete | Available in MVP (no series status exists) |
| Download, notification, collection | Separate resources with their own lifecycles | Side effects of Save |

## Decision register

Each row is a proposal with a recommended disposition. “Changes v1” means the implemented
contract text changes before code.

| ID | Decision | Options | Recommended | Changes v1 |
| --- | --- | --- | --- | --- |
| D-L01 | Library structure | three sections in one destination; three tabs | Three sections in one destination; “Xem tất cả” per section | No |
| D-L02 | Saved presentation source | audio representative (current); published Work metadata | Active published metadata generation, same source as Discovery; audio availability `playable`/`no_audio_yet` as a separate field; never `null` for a shown row | Yes |
| D-L03 | Continue anchor and next action | newest unfinished register (current); newest register per Work plus a derived next action | Newest register per Work, completed or not; closed next-action union; next = immediately following published Unit, never a skip | Yes |
| D-L04 | Caught up / next chapter without audio | keep in Continue disabled; leave Continue | Leave Continue and show the state on Saved/Recent; re-enter at read time when the next chapter is playable | Yes |
| D-L05 | What completes a chapter | explicit only (current); engine natural end too | Explicit action **or** the engine's acknowledged natural end of the current session/source at the exact duration; checkpoint or seek to the end still never completes; auto-next playback is a later opt-in | Yes |
| D-L06 | Work completion | derive from chapters; owner series status | Defer until Catalog has an owner-declared series status; MVP shows only “caught up” | No |
| D-L07 | Podcast “next” | published position; none; newest | Kind-neutral published position for MVP; revisit with podcast listening evidence | No |
| D-L08 | Scope of “Xóa lịch sử gần đây” | history only (current); all listening data | Keep history-only and state it in the confirmation copy; per-Work “Ẩn khỏi Nghe tiếp” and full data deletion are later, separately designed with retention | No |
| D-L09 | Shared/restricted Works in Library | public-only (current); viewer-aware | Public-only until the #634 shared projection supplies a complete viewer-aware predicate; keep the Catalog-owned predicate seam | No |
| D-L10 | Guest Library | local guest saves; sign-in invitation | Sign-in invitation only; no guest persistence or merge (#34 has no merge policy) | No |
| D-L11 | Representation rollout | replace in place (ADR 0055); additive `presentation=2` then removal | Additive opt-in `presentation=2` on Saved and Continue pages, Web and CMP adopt in their slices, then remove the superseded lanes once no current client uses them | Yes (additive) |

## Success signals

These are evaluation hypotheses; any telemetry is a separate reviewed decision (no analytics
pipeline follows from this document).

- A signed-in listener reaches the right chapter and position from a cold start in at most two
  actions, on Web and on CMP.
- After finishing a chapter, the next action is visible without opening the Work.
- No journey shows another account's rows, a fake Play button or a silently substituted release.

## Later, not MVP

Per-session listening history and play counts; named collections (“Nghe trước khi ngủ”); a player
queue or playlist; per-Work dismiss from Continue; full listening-data deletion with a receipt and
tombstone retention horizon; downloads; new-chapter notifications; recommendations. The design
document records an extension path for each so MVP choices do not block them.
