# ADR0042: Native streaming adapters and account runtime

- Status: proposed for platform acceptance; scoped implementation requested by the owner.
- Date: 2026-10-18
- Decision owner: Cantos maintainers
- Related: #14/#15/#17/#35/#122/#184/#191; [mobile architecture](../architecture/mobile.md),
  [step-2 evidence](../evidence/theatre-native-step2.md),
  [metadata store adoption](0026-theatre-mobile-local-storage-sandbox.md).

## Context

The production catalog has an HTTP contract and separate app hosts. The existing media port
and pure reducer must now consume immutable release identity without introducing mock playback
or sharing a UI/runtime with another app. Audio remains produced outside Cantos.

## Scoped implementation choices

Use Android SDK MediaPlayer with platform MediaSession and a mediaPlayback foreground service.
This concrete streaming consumer needs no additional Android media dependency. Native callbacks
and actual position establish engine observations; the pure reducer owns intent and resumption.
One process owner serves screens and OS controls. This retains minSdk23/target35 as build inputs,
not a newly accepted support matrix. Device codec/seek/focus/foreground-service acceptance is
required before treating this as a supported platform behavior.

Use Swift AVPlayer/AVAudioSession/KVO and MediaPlayer remote/NowPlaying APIs behind a plain
exported Kotlin driver/listener bridge. Swift owns platform item tokens and observer cleanup;
Kotlin owns the shared reducer and access/identity fencing. Cold restore does not activate the
audio session. Linux source review does not establish Swift compilation or audio compatibility.
The current approved rendition is Ogg/Opus; AVPlayer compatibility is explicitly unresolved.
Do not silently substitute another release, add backend transcode-on-play or claim iOS playback.

Keep the existing mock status/corpus for the isolated sandbox. Streaming `Playing` requires a
matching native observation and exact AudioIdentity; pending seek does not replace confirmed
position. Logout clears the reusable native item, system metadata and old account projection.

Use the existing Cantos native identity API and secure-store/browser adapters. Session authority
comes only from API confirmation. A401/session-fence failure hides old state and revalidates the
same credential/actor; a delayed old-session failure cannot log out a new account. Browser-facing
issuer/registration activation stays with #191/#174; no Web Origin or provider API is fabricated.

SQLDelight2.1.0 adoption for the concrete account metadata/outbox consumer is recorded in
ADR0026. Checkpoint settings, API rows and original operation bodies stay account-local; secrets
and expiring media locations stay outside SQLite. Publication authorization remains server-owned.

## Alternatives and consequences

Media3 remains a future option with a separate pinned-dependency/device evaluation. Platform SDK
APIs reduce this change's dependency surface, while making service, notification, callback and
focus lifecycle translation an explicit adapter responsibility. Pure tests cannot accept it.
Kotlin/Native direct AVFoundation bindings would avoid the Swift bridge, but increase uncompiled
interop surface here; source selects Swift's normal observation APIs and retains macOS gates.

There is no new listener policy, server schema, paid service or automatic audio production.
The detailed layer/status ledger and all remaining device, codec, identity and distribution
acceptance gates belong to the linked evidence record. This proposed record is not broad
maintainer/platform approval inferred from a local build.
