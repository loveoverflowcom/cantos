# Cantos Architecture Overview

> Current architecture from [ADR 0018](../decisions/0018-external-audio-production.md).
> Prior TTS/worker/DBSP job-summary descriptions are historical only.

## System boundaries

```text
                         ┌──────────────────────────┐
                         │ Cantos Studio — Web CMS  │
                         └────────────┬─────────────┘
                                      │
             read brief / upload       ▼        public releases
 External AI / human producer ──► Cantos API ──────────► Cantos Theatre
 (TTS/record/mix/encode)          │                      (Web/CMP)
                                  ▼
                       PostgreSQL + object storage
```

Cantos **does not** execute a speech model, call a provider, synthesize dialogue or mix audio.
Plugin/agent production occurs outside the server, which accepts private verified media and
manages releases. All apps speak versioned API contracts; no model credentials enter clients.

## Layers and contexts

| Layer/context | Responsibility | Must not do |
| --- | --- | --- |
| Script IR shared kernel | Versioned structured text, IDs, validation and diff | HTTP, SQL, model calls |
| Editorial | Access, immutable accepted revisions, reviews, provenance | Run synthesis |
| Ingestion | Safely preserve imported source and extraction receipts | Create speech |
| Adaptation | Frozen context and externally authored proposal acceptance | LLM inference |
| Production | Versioned planning-only voice/performance/rights/budget metadata | Dispatch providers |
| Media | Pure byte/format inspection; future private audio rendition admission | TTS, mixing, encoding |
| Future catalog | Work/novel/story/podcast, chapter/episode hierarchy | Force Script IR on a recording |
| Future publication | Rights/QC approval, immutable release manifest | Trust unverified uploads |
| Theatre | Listener-visible releases and playback progress | Production work |

Use domain → application → adapters: pure decisions return typed outcomes; application owns
authorization and transactions; HTTP and PostgreSQL adapters only execute effects. See
[API bounded contexts](api-contexts.md) and [repository layout](repository-layout.md).

## Data and delivery

PostgreSQL is authoritative for work metadata, revisions, rights, upload intents, accepted
artifacts, QC decisions, release pointers and listening progress. Use S3-compatible **private**
staging and immutable checked objects, then a public/CDN delivery path for approved releases.
Never assume a Postgres transaction and object-store PUT are atomic. Use checked finalize plus
bounded reconciliation/cleanup. Prevent replay conflicts via exact operation IDs/digests.

Historical migrations 0007–0009 for TTS jobs and speech clips stay intact for checksum/
upgrade compatibility. They are not part of the new write model. Do not drop legacy tables
without a separate data retention decision and upgrade test.

A generic optional DBSP engine may serve future metadata projections, but the removed
TTS-job summary view is no longer a backend feature and there is no current DBSP-backed
business use case.

## Authentication and security

Current development auth uses same-origin Studio session cookies. External producers will
require their **own** scoped, expiring, revocable credentials and short-lived object-store upload
grants. Never share a user browser cookie or a provider API key with a producer plugin.
Bind every request to actor, work, content unit, frozen brief and rights scope.

## Media interoperability

Audio format profiles are versioned. Opus mono at low bitrates is a proposed preference;
AAC-LC is a possible compatibility fallback. The server may inspect bytes and record measured
facts, but audio mastering/encoding and listening reviews are external/editorial work.
Theatre playback only reads accepted published files.

## Verification

Use `cargo xtask` for repository, Rust/contracts, PostgreSQL and Web tests. The new Work/Audio
contracts are **proposed**; no existing API route implements their lifecycle yet.
Read [external handoff contract](../../packages/contracts/external-audio-handoff-v1.md) before
adding a route and write adversarial/replay/owner-isolation tests as part of its implementation.
