# Listen natively on Android and iOS

## Why

Listeners expect an audio episode to continue with the screen locked, remain controllable from the operating system, work offline, and resume across devices. A visual mobile shell alone does not deliver that experience.

## Scope

Implement a Kotlin Compose Multiplatform Theatre client with native platform playback rather than a WebView. Cover discovery/library, mini/full player, background audio, lock-screen/media controls, appropriate notifications, speed, sleep timer, offline downloads, bookmarks, and synchronized progress.

Share business/API contracts with Rust where practical through a versioned interoperable boundary. Keep Android/iOS audio-session and lifecycle behavior in platform adapters. Use the common design tokens and compact adaptive layouts, with accessible labels, text scaling, and screen-reader navigation.

Make downloads identify a published media version, verify completion/integrity, survive interruption, and reconcile publication changes without corrupting an existing usable download. Persist playback/progress locally and synchronize with the server when connectivity returns.

## Non-goals

Studio production on mobile, a shared native renderer for every future product, Rust business-code FFI without an immediate need, wearable/car integrations, or DRM.

## Dependencies

[030 — Mix, review, upload, and publish an episode](030-mix-review-and-publish.md) and [040 — Discover and listen in Theatre Web](040-theatre-web-listening.md). Verify actual publication/progress contracts and native playback libraries before selecting adapters. Android runtime checks and an iOS/macOS build/runtime environment are required for platform evidence.

## Suggested sequence

1. Connect CMP navigation/library/player UI to the established API contracts.
2. Integrate native playback and operating-system media controls on both platforms.
3. Add verified downloads, local persistence, and offline progress synchronization.
4. Verify lifecycle interruptions, sleep timer, accessibility, and republishing behavior with real media.

## Acceptance criteria

- Android and iOS each stream an actual published episode through native audio playback; the UI reports the real player state.
- Playback continues in the supported background/locked-screen states; media controls, relevant notifications, speed, and sleep timer behave correctly.
- Calls/audio interruptions, headphone removal, route changes, app backgrounding, and process restart follow documented platform-appropriate behavior without duplicate playback.
- A completed download plays without network access. Interrupted, corrupt, or insufficient-storage downloads are visible and recoverable; incomplete files are not presented as ready.
- Download removal cancels related work safely; republishing preserves or replaces downloaded versions according to the explicit media-version policy.
- Offline bookmarks/progress survive restart and later synchronize without regressing newer valid updates; demonstrate conflict handling with Theatre Web.
- Narrow layouts, large text, screen-reader labels, and primary playback controls remain usable.
- Report the Android and iOS devices/emulators and runtime checks performed. A platform whose runtime could not be exercised remains an explicit release gap.

## Review boundary

Demonstrate discovery → native play → lock screen/background → offline download/play → restart/resume → cross-device sync on both platforms. Review platform audio/lifecycle behavior separately from common UI and API contracts.

## Risks / unknowns

iOS tooling cannot be replaced by Linux-only validation. Background permissions, audio focus/session handling, and download persistence differ between platforms. Signed URLs can expire during downloads; storage capacity and media-version retention affect recovery.

## Follow-ups

Studio is web-only; Studio Mobile requires an explicit future roadmap request. Defer car/wearable integrations and advanced download scheduling. Any interruption, offline corruption, or synchronization failure is a release-blocking correctness follow-up for this slice.

## GitHub execution

[#14](https://github.com/loveoverflowcom/cantos/issues/14), [#15](https://github.com/loveoverflowcom/cantos/issues/15), [#16](https://github.com/loveoverflowcom/cantos/issues/16), [#17](https://github.com/loveoverflowcom/cantos/issues/17). See the [issue/dependency map](project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.
