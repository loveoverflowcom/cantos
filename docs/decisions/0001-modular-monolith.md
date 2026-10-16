# ADR 0001 — Start with a modular Rust backend

- Status: accepted baseline from the project brief; the modular monolith is implemented.
  Live deployment and capability acceptance remain separate from this stack decision.
- Date: 2026-10-09.
- Scope: starting architecture and platform stack.

## Context

Cantos must deliver an entire creator-to-listener flow while keeping AI costs, revisions, approvals and generated assets consistent. The initial team should not need to coordinate distributed service releases to import, produce and publish one episode.

## Decision

Use Rust, Axum and PostgreSQL for one modular backend; Leptos for Studio and Theatre Web; Kotlin Compose Multiplatform for Android/iOS with native playback adapters. Use S3-compatible object storage and CDN delivery for published audio. Start production execution inside the same codebase, optionally in a separately scheduled worker process.

Version Script IR at the interchange boundary. Narrative Forge reuse must not require shared persistence or tightly coupled releases. Keep AI provider and media tooling behind adapters as their first concrete consumers are implemented.

## Consequences

The server controls permissions, costs, revision transitions and publication atomically in PostgreSQL. Long-running media work needs durable resumable jobs and crash recovery. Web/mobile share contract definitions and design semantics rather than one UI runtime. A separate worker process is an operational boundary, not a new product service.

## Revisit when

Measured workload, deployment isolation or independently owned capabilities cannot be served safely by this arrangement. Record that evidence and compare a smaller module/process change before introducing a distributed service or broker.
