# 0010 — Separate listener shells from Studio

Date: 2026-10-11. Status: accepted for the user-requested local skeleton scope; no deployment,
remote publication or production listening acceptance.

## Context

Cantos needs a listener website and native Android/iOS app alongside its working creator Studio.
Studio already owns authenticated source/revision/adaptation/casting flows. Renaming it or adding
a cross-language UI layer would create compatibility work without a concrete consumer. Supplied
web/mobile references distinguish listener discovery/playback from creator authoring. Published
listener APIs and native playback are absent; the Mac has limited disk and partial native caches.

## Decision

Keep `apps/web` as Studio. Add independent Leptos package `apps/theatre-web` for Theatre and one
CMP project at `apps/mobile` with shared UI/navigation/theme, Android host and SwiftUI/Xcode host.
A JVM test target supports common checks only. Keep Rust/Axum/PostgreSQL and existing gates.

Share semantic JSON tokens through generated CSS/Kotlin; each renderer owns components/layout.
Label synthetic fixtures and keep them separate from domain records. Missing playback/account/
download actions remain unavailable. Add no mock success adapters, listener wire fields or services.

## Consequences and evidence

Each audience has an entry/build target. Studio compatibility uses existing tests/WASM build;
Theatre navigation is inspectable before publication exists. Token/contrast checks guard drift.
CMP common compilation does not establish native acceptance. Offline Android/iOS dependencies
are missing; native rendering/accessibility and linked Swift host remain unverified. Follow
[evidence](../evidence/client-shells.md) and the [surface map](../architecture/client-surfaces.md).
This decision does not advance durable production or close listening issues.
