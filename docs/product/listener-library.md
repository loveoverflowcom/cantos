# Listener Library — vision, journeys and decision register

**Status: accepted (owner, 3 November 2026)** in
[ADR 0057](../decisions/0057-listener-library-work-projection.md); not implemented. The owner
selected the decisions below through the acceptance slice [L01 #702](https://github.com/loveoverflowcom/cantos/issues/702)
of epic [#701](https://github.com/loveoverflowcom/cantos/issues/701). Until slices L02–L06 land,
current behavior is still the 1.2.0 part of the
[Library v1 contract](../../services/cantos-server/contracts/theatre/library/v1/README.md). The
technical design is in [Listener Library projection](../architecture/listener-library-projection.md);
the executable expectation is the draft `cantos.listening.library-projection`
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
| J6 | Replay an old chapter | Nghe tiếp follows the chapter heard last; Gần đây shows the Work once, newest first |

## Concepts that stay separate

| Concept | Meaning | Not |
| --- | --- | --- |
| Saved Work | A Work-level desired boolean in the library | A download, a notification subscription, a bookmark or playback permission |
| Bookmark | One immutable exact-release position | A Saved Work or progress |
| Progress | One revisioned register per exact release | A history log or permission to fetch bytes |
| Recent history | Latest confirmed engine start per Work | A per-session log, a play count or analytics |
| Unit completion | `mark_completed` of one exact release | Work completion |
| Caught up | Every currently published chapter after the anchor is finished or absent | “Đã hoàn thành tác phẩm” |
| Work completion | The listener finished a Work the owner declared complete | Not in MVP: no series status exists yet |
| Download, notification, collection | Separate resources with their own lifecycles | Side effects of Save |

## Decision register

Dispositions accepted by the owner on 3 November 2026 ([ADR 0057](../decisions/0057-listener-library-work-projection.md)).
“Changes v1” means the Library v1 text changed before any code.

| ID | Decision | Options considered | Accepted | Changes v1 |
| --- | --- | --- | --- | --- |
| D-L01 | Library structure | three sections in one destination; three tabs | Three sections in one destination; “Xem tất cả” per section | No |
| D-L02 | Saved presentation source | audio representative (current); published Work metadata | Work card of the active published metadata generation, same source as Discovery, plus a separate `listen` state `playable`/`no_audio_yet`; never `null` for a shown row | Yes (1.3.0) |
| D-L03 | Continue anchor and next action | newest unfinished register (current); newest register per Work plus a next action | Newest register per Work, completed or not; closed next-action kind; next = immediately following visible Unit, never a skip | Yes (1.3.0) |
| D-L04 | Caught up / next chapter without audio | keep in Continue disabled; leave Continue | Leave Continue; re-enter at read time when the following chapter is playable | Yes (1.3.0) |
| D-L05 | What completes a chapter | explicit only (current); engine natural end too; plus auto-next | Explicit action **or** the engine's acknowledged natural end of the current account/session/source at the exact duration; checkpoint or seek to the end never completes; no auto-next | Yes |
| D-L06 | Work completion | derive from chapters; owner series status | Deferred until Catalog has an owner-declared series status; “caught up” is a read state only | No |
| D-L07 | Podcast “next” | published position; none; newest | Published order for every kind | No |
| D-L08 | “Xóa lịch sử gần đây” | history only (current); also dismiss Continue; all listening data; **remove** | **Removed.** Web/CMP drop the action (L04/L05); L06 removes the clear route, epoch conflict and epoch wire fields with every client in one delivery. Self-service data deletion belongs to a later account-level export/deletion flow | Yes (L06) |
| D-L09 | Shared/restricted Works in Library | public-only (current); viewer-aware | Public-only until the #634 shared projection supplies a complete viewer-aware predicate | No |
| D-L10 | Guest Library | local guest saves; sign-in invitation | Sign-in invitation only; no guest persistence or merge | No |
| D-L11 | Representation rollout | replace in place (ADR 0055); additive `presentation=2` then removal | Additive `presentation=2` on Saved and Continue (Library 1.3.0); Web and CMP adopt; L06 removes the superseded lanes | Yes (1.3.0) |

## Success signals

These are evaluation hypotheses; any telemetry is a separate reviewed decision (no analytics
pipeline follows from this document).

- A signed-in listener reaches the right chapter and position from a cold start in at most two
  actions, on Web and on CMP.
- After finishing a chapter, the next action is visible without opening the Work.
- No journey shows another account's rows, a fake Play button or a silently substituted release.

## Later, not MVP

Per-session listening history and play counts; named collections (“Nghe trước khi ngủ”); a player
queue or playlist; per-Work dismiss from Continue; account-level listening-data export/deletion with
receipt and tombstone retention horizons; auto-next playback; downloads; new-chapter notifications; recommendations. The design
document records an extension path for each so MVP choices do not block them.
