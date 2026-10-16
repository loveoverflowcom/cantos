# Production Brief boundary

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/production-brief-implementation.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

One immutable, versioned instruction snapshot for an exact Catalog ContentRevision, readable
by its Studio owner and by a separately scoped, expiring, revocable external producer grant.
Authority: [ADR 0018](../../decisions/0018-external-audio-production.md) and the
[Content handoff](content-identity-production-brief-handoff.md).

## Invariants and evidence targets

| Claim | Failure mode | Oracle / target |
| --- | --- | --- |
| Brief pins UUID and full `cc-r1`, Work rights and explicit profile | stale text or rights silently retargets production | pure decision examples + actual HTTP/PostgreSQL |
| Editorial intent, voice guidance, capabilities and delivery are separate | provider choice becomes content authority | typed admission and compatibility examples |
| Prose/recording needs no Script IR or cast | ordinary podcast cannot export | text/recording HTTP regressions |
| Optional source/script is copied through owning entrypoints | Work reader acquires unrelated manuscripts | source receipt/export digest + cross-owner regressions |
| Records are immutable and exact retry is atomic | response loss creates divergent versions | SQL constraints, rollback and competing writes |
| Producer read grant pins owner/unit/brief digest and current permission | Studio cookie or wrong grant leaks private text | real router, expiry/revocation/wrong-scope cases |
| Instructions never call an audio producer | export incurs paid inference or upload | static boundaries, no producer adapter |

## Dependencies and review boundary

Fresh base: `c28a3d0f4f1c1d1e262cfd56f02120e6a62966f0` (PR70). PR69 remains a separate
unmerged draft; only its four canonical `opus-mono-20/24/32/40-v1` profiles are compatibility
inputs. Migration0013 follows0012 while0010 remains absent; applied SQL bytes stay unchanged.

No new architectural decision is needed: ADR0018 selects external production. This slice
defines a bounded version1 encoding and read-only grant, with no new crate, engine, SDK,
worker, spending reservation, mixer, upload framework or publication machine. Legacy
planning inputs remain separate; they are never implicitly read as current brief guidance.

## Next boundary

AudioLifecycle must bind accepted intake to exact Catalog content/brief/profile and current
rights, introduce immutable review/release rules, and preserve private objects on failure.
PR69 migration0010 and its runtime evidence need reconciliation there. Studio/CMP live API
integration is a subsequent task, not part of this backend slice.
