# Discover and listen in Theatre Web

## Why

Published production becomes valuable when listeners can find an audio drama, follow its episodes, and continue listening reliably. The web player also establishes the listening contracts consumed by native mobile.

## Scope

Build the Leptos Theatre discovery/library flow, work and episode views, a mini-player and full player, playback speed, bookmarks, and listening-progress persistence/synchronization. Use the shared compact Material 3 Expressive design with accessible controls and clear loading/error states.

Consume published manifests and stored media. Define how progress identifies the episode and media version, how resume/seek behaves, and how competing updates are resolved. Include the smallest listener identity/access boundary needed for synchronized progress; do not expand into a complete social-account platform.

## Non-goals

Recommendation engines, subscriptions/payments, social feeds, live broadcasting, native background audio guarantees, or browser offline downloading.

## Dependencies

[030 — Mix, review, upload, and publish an episode](030-mix-review-and-publish.md). Inspect actual publication/delivery contracts and retention rules before choosing player and progress behavior. Confirm supported browsers and authentication/session conventions.

The [private content-read contract](../../packages/contracts/private-content-reads-v1.md)
documents a proposed publication-bound projection only. Media's owner preview/storage capability
supplies no listener access. [#11](https://github.com/loveoverflowcom/cantos/issues/11) must
provide the approved immutable manifest and isolation/retention authority;
[#12](https://github.com/loveoverflowcom/cantos/issues/12) must provide separate listener
identity, authorization and exact publication reads. Keep the Theatre API unavailable until
those owning boundaries exist; neither Studio cookies nor a private S3 grant is a substitute.

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
