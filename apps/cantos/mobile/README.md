# Mobile implementation guide

This directory is reserved for the Kotlin Compose Multiplatform Android/iOS Theatre app. No Gradle/Xcode application exists yet.

Use shared Kotlin for UI, feature state and API models, with platform-native playback, media controls, downloads and lifecycle adapters. Do not implement listening with a WebView. Follow the [mobile architecture](../../../docs/architecture/mobile.md) and [UI system](../../../docs/design/ui-system.md).

After a Gradle Android target and production preview functions exist, use
[`cargo xtask compose-preview`](../../../xtask/README.md#compose-preview) for PNG and semantics
capture without an emulator. Read the [preview workflow](../../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md)
for Android CLI/Studio Quail 2+ setup and inspection. This host capture does not close native
playback, Android/iOS runtime or screen-reader acceptance.

The first mobile slice must be verified on Android and iOS for background listening, lock-screen controls, speed, sleep timer, offline media and progress synchronization. Studio mobile authoring is a later product decision, reserved in [`apps/cantos-studio/mobile`](../../cantos-studio/mobile/README.md); the MVP mobile priority is listening. The two apps share no module until code is actually common.
