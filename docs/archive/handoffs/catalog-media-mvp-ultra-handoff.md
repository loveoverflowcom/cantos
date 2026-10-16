# New-session Ultra handoff: private Work cover

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/catalog-media-mvp-ultra-handoff.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

Planning completed on 2026-10-13 UTC. Implementation is **not started**. This document is the
handoff for the user-requested new, separate GPT6.1Sol Ultra implementation session, not an
instruction to continue implementation in the planning session.

## Authoritative inputs and local artifact

- Fresh origin/develop, initially fetched and rechecked at 18:20 UTC:
  **b28b87856b30c78c2921aa1134ac657b0584ebd7** (merged PR102).
- Planning branch: plan/issue104-catalog-media-mvp.
- Worktree: /workspace/scratch/429f9ca90843/cantos-catalog-media-plan-20261013.
- Main checkout remains untouched at 389c5cb; other worktrees/evidence/caches are preserved.
- [Exact implementation plan](../superseded-plans/012-private-work-artwork.md): coverage, every field, SQL, routes,
  transaction/replay policy, Proto field numbers, limits, L1–L5, commits and acceptance.
- [Checked primary-source semantics](../superseded-plans/catalog-media-standards-reference.md): RSS 2.0.11,
  Podcast namespace 1.0/Phase8 image and deprecations, private/public separations, decoder source.
- [Issue104](https://github.com/loveoverflowcom/cantos/issues/104) was read in full with zero
  comments. Its original research-only scope is superseded only by the user's explicit request
  to plan necessary MVP API/DB fields and then implement that plan in a new session.

## One deliverable, no product decision pending

Owner-only Work cover: prepare a known UUID intent with expected PNG/JPEG hash/length, bounded
provenance and existing owner rights refs; upload exact raw bytes; fully decode, hash, persist
and read back; obtain immutable inspected metadata/private preview; choose/replace/clear with
operation UUID + expected Catalog Work revision. Keep exact historical selections and receipts.
One image asset contains one externally prepared variant. No extra generic media framework.

Media owns immutable image facts and the artwork-change ledger; Catalog owns Work/owner/lifecycle
and atomic metadata touch/history/epoch through one concrete facade. No reverse Catalog → Media
dependency. Sidecar endpoints keep existing strict WorkDetails/Catalog snapshots, JSON clients,
audio/publication digests and public DTOs unchanged. Set/clear touches Work metadata only; old
audio candidates retain the existing MetadataChanged guard. Existing immutable release remains
under its current retained-release policy. A private cover grants no distribution.

## Dependency gates: core can start immediately

1. Fetch develop, inspect clean Git and preserve all local work. Create an isolated implementation
   worktree/branch from current develop; bring in these planning docs without product edits from
   other worktrees. Read current AGENTS, owning skills/contracts and actual code.
2. Implement/test pure image bounds, IDs, receipt/selection/history/CAS rules, JSON/Proto adapters
   and independent corpus first. No runtime route activation is needed for those commits.
3. Parent owns separately authorized identity PR92/98/101/103 review/integration and then PR90
   storage review/conflict repair/merge. PR90 inspected head was
   9306d22954b44aa9be390495aa0b2581c0738d22, still open/unmerged/unmergeable. Prefer its final
   merged develop boundary. If required, only parent may provide the final approved dependency
   SHA/authorization to incorporate its verified branch into the isolated feature branch.
   Do not merge unrelated PRs, fix their worktrees or duplicate their S3 adapter.
4. PR90 currently writes VerifiedAudio and hard-codes audio/ogg on S3. Add sealed VerifiedImage,
   then minimally adapt that shared private-object write capability to verified audio/image
   payloads and actual MIME. Preserve 32 MiB audio bounds, immutable version/receipt digest,
   recorded targets, range/read-grant authorization and concurrency limits. No fake audio proof.
5. **0023_private_work_artwork.sql is reserved for this MVP by parent coordination**, pending
   a fresh ordered migration/reservation check. Existing reservations: 0015 Storage, 0016
   Publication, 0017 Discovery, 0018 Identity, 0019 Library, 0020 Studio OIDC, 0021 Theatre,
   0022 Admin. Use the next coordinated free number only if 0023 is already claimed. Additive
   only: never edit published/applied files/checksums or renumber dependency migrations.

## Fixed limits and edge contracts

- PNG/JPEG only; encoded 1..8 MiB; each dimension 1..4096; <=16777216 pixels / <=64 MiB decoded.
  Two bounded blocking decode slots; permits survive cancellation until actual decode completion.
  image 0.25.8 with only png/jpeg is the compatible candidate, subject to current advisory/MSRV
  and hostile-fixture checks. PNG limits precede metadata decode; reject APNG/nonidentity JPEG
  orientation. No re-encoding/rotation/metadata stripping or hard total-RSS/CPU proof claim.
- Studio only: POST/GET image-intents; raw PUT intent bytes; GET asset metadata; private GET/HEAD
  asset bytes; GET artwork at exact optional Work revision; PATCH required artwork null-or-value
  with operation/CAS. Routes and exact request/response fields are fixed in plan sections 4/6/7.
- New metadata JSON default + opt-in application/protobuf only; raw media/error envelopes stay
  distinct. artwork_v1.proto proto2, all fields <=15, bounded enum/oneof prescan; omission is
  not clear. Cross-transport identical semantic operation fingerprints and original replay.
- Prepare/admit is unselected. After byte I/O, reauthorize current owner, lifecycle, claims,
  intent/time; finalize once. Set verifies exact stored asset outside the final SQL transaction,
  then reauthorizes and commits Work touch + ledger + receipt atomically. Current authorization
  is required even for original-response replay and historical read; all foreign/member access
  fails closed. No storage I/O in held SQL transactions, URL fetch or public staging.

## Delivery evidence and residual acceptance

Follow plan section9 L1→L5. Unit/property/type/independent wire corpus before fake application
tests, then disposable PG17 + actual HTTP + filesystem/final controlled S3 peer, negative auth,
two-connection CAS, version/target switch, lost response and host/PG restart fault points.
The only new optional Kani candidate is actual pure image-bounds arithmetic, registered through
existing xtask only after ordinary tests. Catalog revision selector exists; image-bounds does
not yet. No decoder/SQL/auth/concurrency proof is claimed. Use actual gate statuses and exact
source/tool/bound identities; keep full logs in ignored artifacts and safe evidence in docs.

Planning-only checks executed on 2026-10-13 UTC:

| Command | Actual scope and result |
| --- | --- |
| cargo xtask verify repository | PASS: typed/static service examples, local links/architecture, 319 script tests, staged/unstaged diff hygiene |
| cargo xtask preflight --changed-only | PASS: four docs paths selected repository + rust-format; no feature integration/proof gate selected |
| git diff --check / --cached --check | PASS; only these four planning Markdown files are staged |

Full logs: artifacts/agent-runs/issue104-planning/ in the planning worktree. Repository script
tests include fake verifier-output fixtures; their printed Kani lines are not a new proof run.
All feature
unit/property/Kani/application/SQL/HTTP/storage/restart evidence is **NOT_RUN** at handoff.
An unavailable infrastructure gate cannot erase passed core evidence or become a fictitious PASS.

Do not expand scope: no P1 credits/genres/season/markers/transcript, WorkKind addition, feed runtime,
AAC/TTS/mixer, automatic thumbnails, public projection/delivery, UI/native feature, DBSP or new
auth/provider setup. Current value is durable private Work identity/preview through API/DB.

Residuals to name: PR90/final owner session integration, decoder/transitive security and total
resource behavior, retained PNG/JPEG private metadata, caller rights claims rather than legal
approval, private orphan retention, global Catalog clock contention, no current UI acceptance,
and existing public/issuer/native acceptance. Dangerous decoder behavior blocks runtime
activation; it need not hold a clean pure-contract commit pending. Roll back routes/old binary,
retaining additive tables, immutable private bytes and historical targets. No deletions.

Reviewable sequence: pure contracts/core → verified image/shared storage → additive persistence
and use cases → Studio HTTP/real integration → honest evidence/queue. Tell parent the refreshed
base/dependency SHAs, coordinated migration, core results, required deferred gates and actual
blockers. No user clarification is needed for the conservative choices above.
