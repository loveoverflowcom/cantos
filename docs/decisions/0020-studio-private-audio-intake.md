# ADR 0019: Revision-bound private Studio audio intake in development

- **Status:** Accepted for the narrow development scope; runtime verification incomplete.
- **Date:** 2026-10-12
- **Decision owner:** Cantos project, explicit current intake/profile request.
- **Related:** [ADR 0018](0018-external-audio-production.md), [implementation contract](../../packages/contracts/studio-audio-intake-v1.md).

## Context

The existing backend already owns accepted Script IR revisions and Studio development sessions.
The Catalog API now owns work/content-unit identities. Catalog-bound immutable production
briefs, scoped external-producer credentials and S3-compatible storage are not implemented. A small intake slice can exercise actual byte
admission and durable receipt binding without pretending those foundations exist or sharing
Studio cookies with third-party producers.

## Decision

1. Only the current script owner with an active Studio session may create an immutable upload
   intent for the exact current accepted revision/content digest and upload to that intent.
   Intents pin an explicit canonical versioned profile separately from the raw byte request.
2. Adopt four **intake envelopes**: `opus-mono-20-v1`, `opus-mono-24-v1`,
   `opus-mono-32-v1`, `opus-mono-40-v1`. There is no default, automatic selection or AAC
   acceptance. All require Ogg/Opus mono, 48 kHz decoding timeline, at most one hour and 32 MiB.
   Observed average coded payload bitrate must not exceed 120% of the selected nominal target;
   silence/VBR has no lower bitrate floor. This does not settle listening/interoperability
   [#51](https://github.com/loveoverflowcom/cantos/issues/51), mastering or subjective QC.
3. Verify actual received bytes with bounded structure and decode-to-discard inspection.
   Cantos performs no recording, synthesis, mixing, resampling or encoding. Only that inspector
   constructs the accepted-media proof used by storage.
4. Keep media bytes **outside PostgreSQL** in explicitly configured, owned private filesystem
   objects for this development host. Require absolute 0700 directories, server-derived content
   keys, atomic no-overwrite publication, read-only files and file/directory synchronization.
   PostgreSQL owns immutable intent/receipt metadata and exact revision/profile binding.
5. Finish inspection and object writes outside database transactions. The final acceptance
   transaction rechecks current session, owner, exact revision and intent scope under locks.
   Repeated accepted bytes return the original receipt; contradictory retries conflict.
   No accepted receipt is inserted for rejected bytes or a failed transaction.
6. A crash or failed metadata transaction may leave a private orphan. Rollback never deletes
   a shared content-addressed object. Retention, orphan collection and live S3 integration are
   separate work; this slice does not promise a distributed filesystem/database transaction.
7. Disable same-process static serving whenever private audio storage is enabled. Canonical
   disjoint roots alone do not prevent nested public symlinks from reaching private objects.
   This development host is API-only; serve Studio separately through its existing Trunk proxy
   with the matching Web origin. That separate public tree must not reference private objects;
   production isolation remains a separately reviewed deployment boundary.
8. Bound raw upload admission before body buffering and retain blocking-work permits through
   inspection/storage even after request cancellation. Busy or unavailable inspection fails
   closed and creates no accepted receipt.

## Scope and verification

The implemented endpoints belong only to `/api/v1/studio/scripts/.../audio-intents`.
There are no new legacy aliases, Theatre/Admin routes or public media delivery routes.
A historical exact prepare retry can reconcile its original intent, and status can recover a
historical receipt; byte PUT still requires the exact pinned revision to remain current.

Binding intake to Catalog/content-unit identity, external producer tokens, production briefs,
provenance/rights review, QC/listening, release manifests, public/CDN delivery and production
object storage remain unimplemented. The [work queue](../work-plan/README.md) retains their sequence.

Tests are authored for contracts, actual inspection, filesystem safety and disposable PostgreSQL
authorization/retry/concurrency/failure behavior. Cargo, rustc and PostgreSQL tools were absent
in the implementation environment: required Rust/xtask/database execution is **NOT_RUN**.
This decision records scope and design, not deployment, successful runtime verification or
completion of the general external handoff roadmap.
