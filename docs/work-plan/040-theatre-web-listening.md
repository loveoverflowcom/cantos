# Discover and listen in Theatre Web

Current review boundary: real playback/integration under #13/#158/#170/#172. Anonymous live
discovery and separate account/readonly Library client code already exist; reuse them and
their [actual Web boundary](../../apps/cantos-theatre/web/README.md#explicit-live-and-mock-modes).
The explicit mock player is not real playback. The guest playback slice
[#269](https://github.com/loveoverflowcom/cantos/issues/269) is recorded in its
[evidence](../evidence/theatre-web-guest-playback.md); saved works, progress, history and resume
(#272) and deployed/CDN delivery stay with their owners; search, pagination, routes and the
production/preview build split of [#270](https://github.com/loveoverflowcom/cantos/issues/270)
are recorded in its [evidence](../evidence/theatre-web-discovery-routes-270.md). GitHub owns task status.

## Why

Published production becomes valuable when listeners can find an audio drama, follow its episodes, and continue listening reliably. The web player also establishes the listening contracts consumed by native mobile.

## Scope

Build the Leptos Theatre discovery/library flow, work and episode views, a mini-player and full player, playback speed, bookmarks, and listening-progress persistence/synchronization. Use the shared compact Material 3 Expressive design with accessible controls and clear loading/error states.

Consume published manifests and stored media. Define how progress identifies the episode and media version, how resume/seek behaves, and how competing updates are resolved. Include the smallest listener identity/access boundary needed for synchronized progress; do not expand into a complete social-account platform.

## Non-goals

Recommendation engines, subscriptions/payments, social feeds, live broadcasting, native background audio guarantees, or browser offline downloading.

## Dependencies

[030 — Mix, review, upload, and publish an episode](030-mix-review-and-publish.md). Inspect actual publication/delivery contracts and retention rules before choosing player and progress behavior. Confirm supported browsers and authentication/session conventions.

Publication, [Discovery](../../services/cantos-server/contracts/theatre/discovery/v1/README.md) and
[Library/Progress](../../services/cantos-server/contracts/theatre/library/v1/README.md) backend contracts already
exist. [#170](https://github.com/loveoverflowcom/cantos/issues/170) owns real public media
delivery: its origin-mode core is implemented but composition/acceptance remains gated by
[ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md). #172 owns remaining public
state/pagination gaps. Only unavailable capabilities stay fail-closed; do not disable existing
anonymous discovery. Studio cookies and private-preview grants provide no listener authority.

## Suggested sequence

1. Connect discovery and episode navigation to real published data.
2. Add playback, seeking, speed, and continuous mini-player behavior.
3. Add bookmarks, saved progress, and synchronization rules with concrete conflict examples.
4. Verify narrow-screen, keyboard, and accessibility behavior plus real CDN playback.

## Acceptance criteria

- A listener can find a work, choose a published episode, play/pause/seek, change speed, and continue playback while navigating supported Theatre views.
- Draft/unapproved episodes do not appear as playable public content. Empty libraries and unavailable episodes have usable states.
- Bookmarks and progress survive reload; authenticated progress is available in a second session through the documented sync contract.
- Older delayed progress updates cannot unexpectedly erase newer progress. Intentional rewind and episode completion follow explicit rules rather than assuming progress always increases.
- Network interruption, expired media access, invalid seek ranges, and playback failure leave a recoverable player state.
- Republishing follows the defined media-version/progress policy; active playback does not silently switch to mismatched timing.
- Streaming a published episode does not invoke adaptation, TTS, or mixing jobs.
- Primary controls work with keyboard and assistive technology; compact layouts remain usable at narrow viewport widths.

## Review boundary

Demonstrate discovery → episode → playback → bookmark → reload/resume plus two-session progress synchronization through real backend/storage delivery. Verify that UI state follows player state and that failure cases are actionable.

## Risks / unknowns

Browser autoplay and background behavior vary. Playback progress is not a strict append-only counter; rewinds and simultaneous devices require a tested policy. Changing episode media may alter timing, so version retention and resume semantics must be explicit.

## Follow-ups

Defer personalized recommendations, subscriptions, and social features. Share stable listening contracts with CMP without forcing Rust implementation details into Kotlin.

## GitHub execution

[#12](https://github.com/loveoverflowcom/cantos/issues/12), [#13](https://github.com/loveoverflowcom/cantos/issues/13). See the [issue/dependency map](project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.
