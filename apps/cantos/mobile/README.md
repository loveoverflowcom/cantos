# Mobile implementation guide

This directory is reserved for the Kotlin Compose Multiplatform Android/iOS Theatre app. No Gradle/Xcode application exists yet.

Use shared Kotlin for UI, feature state and API models, with platform-native playback, media controls, downloads and lifecycle adapters. Do not implement listening with a WebView. Follow the [mobile architecture](../../../docs/architecture/mobile.md) and [UI system](../../../docs/design/ui-system.md).

The first mobile slice must be verified on Android and iOS for background listening, lock-screen controls, speed, sleep timer, offline media and progress synchronization. Studio mobile authoring is a later product decision, reserved in [`apps/cantos-studio/mobile`](../../cantos-studio/mobile/README.md); the MVP mobile priority is listening. The two apps share no module until code is actually common.
