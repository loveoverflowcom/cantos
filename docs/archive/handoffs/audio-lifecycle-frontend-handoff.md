# Audio Lifecycle → real frontend handoff

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/audio-lifecycle-frontend-handoff.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

Session3's bounded server/contract slice is [Audio Lifecycle v1](../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md).
It builds on [Content](content-identity-production-brief-handoff.md), [Brief](production-brief-audio-lifecycle-handoff.md)
and the attributed PR69 intake. [Evidence](../../evidence/audio-lifecycle.md) and
[independent review](../../evidence/audio-lifecycle-review.md) delimit actual checks.

## Live server surfaces

Catalog Work/unit and ContentRevision mutations/history, owner Brief create/read, independent
producer read grants/export, legacy script-owner audio prepare/PUT/status, and the Content/Brief
AudioSubmission prepare/status/revoke, producer binary PUT/status, named review and private
ReleaseCandidate create/read routes are implemented. See the exact method/path table in the
[contract](../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md#concrete-routes).

Studio owner routes use the existing development `cantos_session` cookie. External producer
read and upload bearer credentials are separate from each other and from Studio sessions.
Prepare creates the upload capability; a read grant does not authorize upload. JSON only on
Catalog/Brief/lifecycle; binary PUT is octet-stream with JSON response. Origin is required for
writes, matching configured development Web origin; no-store responses are private. An
intake-enabled host serves API only; serve Studio through its separate existing Trunk proxy.
Do not give Studio cookies to a third-party plugin or put bearer tokens into logs/UI artifacts. Browser calls to producer status/PUT must explicitly omit credentials/cookies (`fetch` with `credentials: "omit"` or equivalent), sending only the upload Authorization bearer; owner prepare/review/candidate calls retain the Studio cookie.

The reusable Rust DTO crate `cantos-contracts` and JSON schemas are available. There is no
schema-generated Kotlin/TypeScript lifecycle client and no live Leptos lifecycle view or CMP API
adapter from this session. Generate/implement a client against these actual contracts in the
subsequent task, handling401/404/409/413/415/422/503 and exact operation replay explicitly.

## Permissions and missing surfaces

Owner review/candidate scope belongs exclusively to Cantos Studio Web. A producer may upload
one exact prepared submission only. Cantos/Theatre clients must never reuse a Studio session or
producer bearer, call these handlers to obtain playback, or expose private upload facts.
`/api/v1/theatre/*` and `/api/v1/admin/*` remain404; no catalog/playback/progress/range/download
manifest route exists. Publication always reports `public_delivery_not_implemented`; there is
no publisher credential, release activation or delivery URL to connect. A future Theatre API
requires a separately reviewed publication/delivery slice before actual native/Web playback.

Do not scaffold Studio Mobile. Theatre CMP and Leptos, plus Studio Web, are later consumers
within their own permissions/platform scope; no frontend changes are requested here.

## Next order and retained risks

1. Integrate Studio Web against real owner Catalog/Content/Brief/lifecycle endpoints, with
   explicit pending/stale/revoked/expired/error states and immutable review subjects.
2. Establish production identity/secure credential delivery and object-store upload/retention.
3. Define stronger rights/QC review and approved immutable publication/delivery, preserving
   the prior active release on every failure; only then expose Theatre manifests/media.
4. Demonstrate Ogg Opus compatibility/range/seek on Web and native Android/iOS under #51/#64.

Checksums/decoder success establish no listening, loudness/true-peak or fidelity acceptance.
Upload admission preserves caller declarations separately from server media facts. Private
orphans may remain after interrupted metadata acceptance. macOS inspection samples tool-leader
RSS; Linux hard memory limits are source-reviewed but not executed on this Mac. The global
Catalog fence still limits write concurrency. Broader #51/#64/#66 remain open.
