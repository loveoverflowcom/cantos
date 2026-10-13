# ADR 0022 — Provider-independent private asset storage

- Status: proposed; implementation and production/provider acceptance remain separate.
- Date: 2026-10-13.
- Decision owner: Cantos maintainers.
- Related: [ADR 0018](0018-external-audio-production.md),
  [ADR 0019](0019-content-identity.md), [ADR 0020](0020-studio-private-audio-intake.md),
  [ADR 0021](0021-podman-service-runtime.md),
  [#7](https://github.com/loveoverflowcom/cantos/issues/7),
  [#10](https://github.com/loveoverflowcom/cantos/issues/10),
  [#11](https://github.com/loveoverflowcom/cantos/issues/11),
  [#12](https://github.com/loveoverflowcom/cantos/issues/12).
- Refines ADR 0020's storage limitation; does not supersede its admission/retry/isolation rules.

## Context

Media already accepts bounded, inspected Ogg/Opus bytes into private filesystem objects and
immutable PostgreSQL receipts. It has no S3 adapter or owner audio byte-read route at this
audit baseline. Existing objects use digest basenames, owned private directory descriptors,
atomic non-overwriting links, synchronized read-only files and actual-byte verification before
metadata acceptance. A failed metadata commit can leave a private orphan; deleting shared
objects on rollback would break other receipts.

Catalog owns Work/ContentUnit and immutable ContentRevision. Its canonical manuscript text is
bounded to 65,536 bytes. Ingestion already preserves bounded original files and receipts in
PostgreSQL; Editorial already stores exact Script IR exports there. They are different content
authorities, not generic interchangeable blobs. Publication and listener authorization remain
unimplemented, so accepting or previewing audio cannot create listener access.

Provider switching must preserve historical reads. Resolving every receipt against the current
write provider would make existing Long Vân assets unreadable after selecting R2. S3-compatible
labels also hide differences in conditional writes, ranges, signing and access policy. A
provider that ignores create preconditions cannot preserve immutable acceptance.

## Decision

1. Add a narrow private storage port inside Media with filesystem, generic S3-compatible and
   deterministic fake implementations. Its operations serve existing immutable intake,
   verification and authorized reads. Add no service, crate, generic repository, TTS engine,
   transcoder, mixer, publication promotion or speculative multipart/migration machinery.
2. Keep Catalog manuscript text, Ingestion originals and Editorial Script IR exports in their
   existing PostgreSQL stores. A future genuinely large artifact gains an exact revision-bound
   location through its existing owner; storage never becomes content authority.
3. Use fallible private identities, closed errors, pure integrity/range/access decisions and
   exhaustive outcomes. Only matching actual SHA-256/length plus existing audio admission may
   yield an accepted rendition. Object keys are internal and server-derived; public IDs remain
   independent of provider, endpoint, bucket and path.
4. Persist an opaque stable target ID and immutable private location for every asset. Preserve
   legacy receipt/rendition bodies and digests through additive location metadata bound to an
   explicit legacy filesystem target. New writes use the configured active target; historical
   reads use their recorded target, never a mutable default. Unavailable targets fail closed.
5. Select adapters in the composition root using checked non-secret TOML and external secret
   injection. Validate unknown/inconsistent fields, active target, required secrets, HTTPS,
   addressing, bounded timeouts and declared capabilities before use. Startup performs no live
   writes or provider-acceptance probe. Configuration failure has no fallback.
6. Require atomic conditional single-object creation for S3 writes; never substitute a
   `HEAD`-then-unconditional-`PUT` race. Reconcile existing/ambiguous writes by exact readback
   before acceptance/retry. Preserve no transaction during object I/O, byte-before-receipt
   ordering, final authorization and private orphan behavior from ADR 0020.
7. Add owner-authorized exact audio reads with separate metadata and binary delivery. Local
   development uses protected bounded streaming; compatible S3 targets may mint short-lived
   scoped read grants after backend authorization. Signed URLs are bearer secrets, never
   persistent identity or logged values. Provider Range/status/header contracts remain checked.
8. Support one wholly private bucket for intake. Private and publication classes stay logically
   distinct. Future publication needs reviewed physical/policy isolation and immutable approval
   manifests; a one-bucket limit never justifies public access to private drafts. Keep Theatre
   unavailable until #11 supplies publication and #12 supplies listener reads/authentication.
9. Future migration is copy → verify actual bytes/hash → register ready replica/location →
   controlled read switch → retention/rollback. Changing topology alone never migrates bytes.
   This slice implements durable original locations, not that migration engine.

The detailed [storage contract](../architecture/object-storage.md) owns the capability/security
matrix and deferred provider acceptance. Existing content identities, profile limits, rights
claims and review/publication blockers remain unchanged.

## Transport and dependency choice

Use the versions already present in the lockfile: `reqwest =0.12.15` for bounded HTTPS transport,
`hmac =0.12.1` with existing `sha2 =0.10.9` for narrow SigV4 signing, and `chrono =0.4.45` for
UTC signature timestamps. Keep them adapter-local. Pin direct dependencies when added; use
rustls TLS, explicit no-redirect/no-proxy behavior and no response decompression. Do not add
an AWS SDK or vendor branch to the domain. A manually maintained narrow signer reduces SDK
surface but makes canonical-request and signature vector coverage an explicit responsibility;
AWS recommends its SDK/CLI for general integrations.

The [official SigV4 specification](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv-create-signed-request.html)
defines canonical request, payload hashing and HMAC signing. The
[official PutObject contract](https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html)
documents conditional creation and `412`/`409` outcomes; the
[official GetObject contract](https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html)
defines ranged retrieval. These are reference semantics, not proof a compatible vendor supports
them. Version documentation for [reqwest 0.12.15](https://docs.rs/reqwest/0.12.15/reqwest/),
[hmac 0.12.1](https://docs.rs/hmac/0.12.1/hmac/),
[sha2 0.10.9](https://docs.rs/sha2/0.10.9/sha2/) and
[chrono 0.4.45](https://docs.rs/chrono/0.4.45/chrono/) accompanies the pinned implementation;
compilation and controlled signing/HTTP tests must establish actual integration behavior.

## Alternatives

| Option | Benefits | Costs and limitations | Decision |
| --- | --- | --- | --- |
| Keep filesystem only | Retains proven local isolation and simple operations | No provider-independent deployment or historical-target support | Retain as an adapter, extend the capability |
| Broad cloud SDK/provider framework | More protocols and signing cases covered | Large surface with no multipart/list/migration consumer; provider details can leak inward | Defer; revisit if concrete scope outgrows the narrow transport |
| `HEAD` then unconditional `PUT` | Appears compatible with limited providers | Race silently overwrites immutable bytes | Rejected |
| One default provider for every read | Minimal metadata | Provider switching misroutes accepted historical assets | Rejected |
| Move every content body to object storage | Symmetric storage shape | Competing authorities and needless consistency/migration risk | Rejected |
| Two physical buckets required immediately | Simpler future publication isolation | Excludes private-only one-bucket plans without improving MVP access | Defer publication storage decision; intake bucket stays wholly private |

## Consequences and validation

The storage location is durable private metadata, separate from public identity. Operators must
retain historical targets and their data, guard target configuration changes, keep least-
privilege secrets outside Git and manage storage costs/retention. Missing objects or a provider
outage can make an accepted asset temporarily unavailable without changing its immutable
identity. There is no distributed database/object transaction or automatic orphan collector.

Validate L1 ownership/dependencies, L2/L3 pure barriers/ranges/integrity, L4 production
orchestration with deterministic storage and controlled HTTP responses, then L5 disposable
PostgreSQL, real API/auth and actual filesystem. Real Long Vân/R2/B2 or disposable Podman
storage acceptance requires explicit operator authorization; until run it is `NOT_RUN`.
Mocks do not establish provider privacy, throughput, CDN behavior or production streaming.

Revisit for a larger artifact/multipart consumer, temporary credentials, a provider that cannot
honor required conditional creation, publication isolation/CDN/retention decisions, or measured
transport/performance needs. No config-only switch may be described as a data migration.
