# Native CMP Listening Architecture

Status: proposed implementation boundaries and acceptance requirements. Native libraries and minimum platform versions are not selected or pinned yet.

## Scope

Cantos Theatre mobile uses Kotlin Compose Multiplatform for Android and iOS. It plays published, cached audio through native platform integrations; it does not embed the Web player in a WebView or synthesize TTS for listeners. Studio production remains a server-driven workflow, with Theatre listening the mobile MVP priority. The Theatre app lives in `apps/cantos/mobile`; `apps/cantos-studio/mobile` only reserves a location for a possible Studio client and shares no module with it.

Keep one modular backend and one shared mobile application layer initially. Introduce services or additional applications only after operational or ownership needs justify them.

## Boundaries

| Layer | Responsibilities | Platform differences |
| --- | --- | --- |
| Shared CMP UI | Discovery, work/episode library, mini/full player, bookmarks, downloads, settings, accessible state rendering | Insets, navigation/back gestures, font scaling, and semantics require per-target validation. |
| Shared application layer | Catalog access, auth/session handling, playback commands and state, download manifest, local queue, progress sync/reconciliation | Interfaces isolate media engine, secure credential store, durable local storage, network/lifecycle signals, and download execution. |
| Android integration | Native audio session/engine, OS media controls, notifications, audio focus, interruptions, background playback/download coordination | Select supported media and lifecycle libraries during implementation; document current permissions and restrictions. |
| iOS integration | Native audio session/engine, Now Playing/remote controls, interruptions, route changes, background playback/download coordination | Select supported APIs during implementation; document capabilities, entitlements, and lifecycle limits. |
| Rust backend | Catalog, access authorization, immutable publication manifests, media delivery authorization, progress/bookmarks, device sessions | Rust domain logic and API schemas stay server-side; no requirement to share Rust UI with CMP. |

Native integration is an explicit interface with observable states and error reasons. A single media session owns playback; UI screens attach to that session instead of creating an audio engine each time navigation changes.

## API contracts

Use versioned HTTP/JSON contracts for catalog, publication manifests, session handling, progress, bookmarks, and download authorization. Maintain OpenAPI/JSON Schema as appropriate alongside backend DTOs, then evaluate Kotlin client/type generation. Generated contracts are practical reuse; duplicating domain logic into unrelated handwritten DTOs or coupling CMP to Rust memory layouts is not.

The contract must identify `work_id`, `episode_id`, immutable `publication_id`/revision, playable asset, duration, content checksum, access policy, and available formats. Client requests include stable operation IDs for retriable writes. Server timestamps/revisions, pagination, error codes, and compatibility rules must be explicit before client implementation. Avoid binding API identifiers to mutable titles or storage object paths.

## Playback behavior

Playback state includes idle, loading, playing, paused, buffering, seeking, ended, and failed, with a separate reason for temporary access or connectivity failures. The UI distinguishes a paused user intent from buffering or an interruption. An incoming call or audio-focus loss must not make the player claim successful user playback.

- Provide play/pause, seek, playback speed, next/previous episode behavior, sleep timer, bookmark, and a reliable resume position.
- Keep lock-screen controls, supported notification controls, artwork/title/duration, and seek position consistent with the active publication.
- Respond to interruptions, headphone/route changes, and competing audio according to platform policy and explicit app settings. Do not resume unexpectedly after an explicit pause or sleep timer expiry.
- Persist the active publication, position, user intent, speed, and timer semantics at suitable checkpoints; restore safely after process death. A restored session should offer resume instead of automatically playing after a cold launch.
- Streaming access may use short-lived signed URLs. Refresh authorization before expiry or after a supported authorization failure, preserve position, and bound retries. Credentials and signed URLs must not appear in logs or UI.
- The CDN/object store must support the chosen media seek/delivery strategy. Select formats and codec/platform support through a recorded compatibility decision and representative device tests.

Background playback requires native media integration and configured OS capabilities. Background work is subject to OS lifecycle limits; the app must persist recoverable state and cannot promise to continue every download or sync immediately while suspended. Verify current platform behavior during implementation rather than relying on this document as API guidance.

## Downloads and access rights

Separate cached streaming segments from explicit offline downloads. A download manifest identifies the immutable publication, asset checksums and sizes, format, authorization policy, and local availability. It must not store an expiring signed URL as the durable identity of a file.

1. Check entitlement and the publication's offline policy before requesting download authorization.
2. Stage data in a temporary location; record progress and validated resume metadata durably.
3. Retry/resume only when the asset identity and server delivery semantics permit it; refresh access separately from file identity.
4. Verify size/checksum before marking the download complete, then make it atomically available for playback.
5. Record failure/cancellation and offer retry, remove, or storage cleanup. Preserve existing completed downloads when a replacement fails.

Require explicit handling for storage exhaustion, metered-network preferences, partial files, auth expiry, process death, changed publications, logout/account changes, and revoked access. Completed local assets are scoped to the authorized account; do not expose another account's files after a switch. Keep credentials in platform-secure storage and treat any download license as a separate policy-controlled record.

Copyright permission to publish does not automatically grant offline redistribution rights. The backend must expose whether downloads are allowed and any expiry/revalidation policy. Strict immediate revocation cannot be guaranteed while a device is offline; the product must choose an enforceable offline validity/revalidation policy and disclose it. Do not invent DRM guarantees. Cancellation, local cleanup, or account removal must not delete server publication assets.

## Durable listening progress

Progress is scoped to account, episode, and immutable publication. Store playback position and completed state locally first, then sync through a durable outbox at bounded intervals and on pause/stop where lifecycle permits. Avoid per-second server writes. Persist unresolved operations across network loss and process restarts.

Recommended reconciliation contract:

- Each device has a stable identifier and monotonic sequence for progress operations; each write has an idempotency key.
- A client sends the server progress revision it last observed. The server accepts a compatible update atomically, returns the new revision, and treats duplicate operations as duplicates.
- A stale update does not blindly overwrite an unseen concurrent device session. Return the current progress and revision; retain the local candidate until the application reconciles it.
- Within the same playback session, explicit seeking is valid and may move progress backward. Never reconcile only by maximum position, which breaks replay and backward seeks.
- For competing sessions, prefer an explicitly selected resume position. If no automatic rule preserves clear intent, show “Continue here” and “Use other device position,” then submit the chosen value against the current server revision.
- Do not order competing updates solely by untrusted device clocks. Server acceptance time and revision describe ordering; device timestamps can provide context.

Bookmark writes use stable IDs and operation IDs. Preserve explicit deletion intent with a tombstone or equivalent server revision so offline replay cannot recreate a removed bookmark. Define completion as a product rule independently of media engine “ended” events; seeking to the end alone must not silently imply a completed listen unless that rule is chosen.

When publication audio changes, retain prior progress/bookmarks keyed to the original revision. Resume the prior revision if access allows; otherwise explain that audio changed and let the listener choose a new starting point. A future mapping by stable dialogue/scene identity is optional and must be validated before automatic migration.

## UI and accessibility

Use the [shared UI specification](../design/ui-system.md) for vocabulary, semantic theme roles, density, state treatment, and motion. Leptos and CMP map the same design intent into their own components. CMP framework support for expressive components and motion must be verified for the selected version and every target.

Test native semantics and focus order, readable time/position, large text, screen-reader playback controls, touch targets, safe areas, route changes, and reduced motion. Provide accessible alternatives to gestures. App-level sleep timer and progress states must remain understandable when the OS controls expose only a subset of actions.

## Implementation decisions to record

Before feature implementation, add an ADR identifying supported OS versions; CMP/Kotlin versions; Android/iOS media integration; local metadata store; native background download strategy; credential storage; codec/delivery strategy; offline rights policy; and API generation approach. Pin dependencies only after checking official documentation and testing the intended integrations. This document does not prescribe currently verified APIs.

## Future acceptance matrix

| Area | Required scenarios on Android and iOS | Evidence to collect |
| --- | --- | --- |
| Native playback | Foreground/background, lock/unlock, media controls, speed, seek, sleep timer | Device/emulator recordings and state/log evidence with secrets redacted. |
| Interruption | Incoming call/focus loss, headset removal, route change, user pause during interruption | User intent and position preserved; no unexpected resume. |
| Delivery | Buffering, offline transition, expired URL, access denial, bounded retry | Position preserved and a distinct recoverable/final error state. |
| Downloads | Resume, cancellation, insufficient space, process death, checksum mismatch, new publication | Manifest/file state checked; incomplete assets never offered as complete. |
| Rights/account | Download forbidden, expiry/revalidation, revocation while online/offline, logout/account switch | Behavior matches the selected policy without cross-account exposure. |
| Progress | Two devices, delayed outbox, duplicate operation, backward seek, revision conflict | Durable local state and documented reconciliation behavior. |
| Lifecycle | App restart, suspended sync, OS termination, restored session | Recovery works within platform limits; cold launch does not auto-play. |
| Accessibility/UI | Large text, TalkBack/VoiceOver, small window, reduced motion | Per-target captures and assistive-technology walkthrough. |

No native playback, background behavior, downloads, progress synchronization, API generation, or accessibility validation has been performed during repository initialization. Web or desktop previews cannot establish native mobile acceptance.
