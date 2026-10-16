# Cantos — Product Brief

> **Current direction (2026-10-12).** [ADR 0018](../decisions/0018-external-audio-production.md)
> supersedes any proposal for in-backend TTS, model dispatch or worker-based audio creation.
> Existing experimental implementation/evidence records remain historical, not active scope.

## Identity and purpose

**Cantos — Vọng Đài** is the system of record for works and their audio editions.
Studio is a **Web-only content management application** for creators and editors; Theatre
is the listening application (Web, Android and iOS).

[Account access policy](../decisions/0032-theatre-signup-and-provisioned-studio.md): Theatre
listeners self-register; Studio has no public registration and admits only one or two securely
provisioned administrator accounts. Actual issuer capability and live acceptance are separate
from this accepted product policy.

Studio supports three top-level work categories: **novel**, **story** and **podcast**, without
assuming every work is an audio drama. A story can be read by one narrator, a podcast can
contain recorded human dialogue, and a drama can have casts, music and effects.

## Product structure

```text
Work (novel | story | podcast)
  └── Content unit (chapter | episode; stable ID, ordering, text/notes optional)
       ├── Source and rights references
       ├── Editorial/script revision (optional; Script IR if structured)
       ├── Production instructions (versioned, independently fetchable)
       └── Audio renditions (immutable objects, language/edition/profile, checksums)
            └── Reviewed and published release pointers
```

A content unit may have multiple audio renditions; one immutable release selects exactly
the accepted set. Work/category metadata must never be conflated with audio blob identity.
A simple narrated novel must not require characters, scenes, a script adaptation or casting
before its owner can upload an authorized recording.

## Responsibilities

**Cantos Studio/API own:** Work and episode metadata, editorial revisions, rights/provenance,
versioned instructions and skills, upload authorization, untrusted artifact admission, QC
evidence, human review, immutable release metadata, and listener-facing distribution.

**External producer AI/plugin owns:** Selecting paid/local TTS tools, any inference, recording,
pronunciation execution, voice or music generation, cue timing, mixing, resampling, codec
encoding and production retries/costs. Cantos can store submitted claims/receipts as unverified
provenance, never as evidence that it executed a provider.

A producer may be a local CLI, external SaaS, a custom agent or a human editor. All use the
same stable instruction/asset handoff. Do not bind domain types to any particular model vendor.

## User journeys

1. Create/import a work; organize chapters or podcast episodes and permission records.
2. Edit text/notes or optional Script IR; freeze a content revision and production brief.
3. External AI reads the brief through a scoped API and creates the audio outside Cantos.
4. External AI requests a private upload, transfers bytes and finalizes with checksums,
   format/duration, producer/tool provenance and any QC measurements.
5. Studio verifies stored bytes/metadata, collects editorial review and approves a version.
6. Publish atomically; Theatre discovers, streams, downloads and resumes the accepted file.

## MVP scope

Prioritize **work catalog → content units → versioned instructions → external upload → review →
release → Theatre playback**. Script IR and drama-production guidance remain optional advanced
capabilities. Keep Studio Mobile excluded. Do not implement server TTS, a model marketplace,
in-backend audio mixer or inference queues.

## Implementation honesty

Currently implemented: Script IR validation/revisions, source import, host-created adaptation,
planning-only casting/rights and a pure WAV inspector. **Not implemented:** work/podcast CRUD,
external audio uploads, storage validation, publication or finished Theatre playback. The
[handoff contract](../../packages/contracts/external-audio-handoff-v1.md) is a proposed v1 API
design, not evidence that routes already exist.
