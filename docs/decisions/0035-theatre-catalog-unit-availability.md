# ADR 0035: Theatre catalog unit availability, unknown duration and walk recovery (#172)

- Status: **accepted** — by the user who requested PR #204, in that PR's session on 2026-10-16, choosing
  option A as recommended; no wire, schema, migration or route change. No separate Product, Listening or
  Publication owner sign-off is recorded; Decision 3's schema tightening and retained-release access stay open
- Date: 2026-10-16
- Decision owner: the requesting user, who accepted it; Product, Listening and Publication owners remain
  the audience for any revisit via [#172](https://github.com/loveoverflowcom/cantos/issues/172)
- Related issue or PR: consumers [#158](https://github.com/loveoverflowcom/cantos/issues/158)
  (Web W01–W03) and [#159](https://github.com/loveoverflowcom/cantos/issues/159) (CMP M01–M06);
  [#12](https://github.com/loveoverflowcom/cantos/issues/12)/[#35](https://github.com/loveoverflowcom/cantos/issues/35)
  (Listening/Library); [ADR 0030](0030-release-safe-public-metadata.md) (P1 metadata gate)
- Supersedes / superseded by: N/A. Builds on the [Theatre Discovery v1 contract](../../services/cantos-server/contracts/theatre/discovery/v1/README.md)
  and the [Library resume status](../../services/cantos-server/contracts/theatre/library/v1/README.md#resume-and-changed-access)

## Context

Known facts (read from `develop` at `43db968`; the [coverage matrix](../work-plan/theatre-catalog-state-handoff.md)
cites each):

- `GET /works/{work}/units` lists only units whose release is **currently** eligible: active
  pointer, unexpired rights window, current approval and clearance, active owner, no withdrawal
  or revocation, and public pinned and current Work. Everything else is omitted, and a Work with
  no such unit answers the same anonymous `not_found` as an absent one. The contract states that
  no previous-authorization history exists and no public 410 or existence disclosure is made.
- `UnitSummary` has no availability field. The wire types and the schema are strict
  (`deny_unknown_fields`, `additionalProperties:false`), so any new field is a deliberate v1 review.
- `UnitSummary.duration_ms` is `u64`, copied from the verified rendition. The domain admits only
  `1..=3_600_000` ms (`AudioRendition::restore`); the public schema allows `0..=2^53-1`. No code
  path emits `0`.
- Library already exposes a closed resume status `exact_available | release_replaced |
  access_unavailable` for units a **listener-authenticated** account has progress or Saved for.
- Cursors are bounded opaque references into a **process-local** vault: bound to query, limit,
  route and (for units) Work; 15-minute expiry; not consumed by a read; lost on restart.

The FE designs for #158/#159 need row states `selected`, `playing`, `completed`, `unavailable`,
`unknown duration`, `empty`, `filter-empty`, `loading-next` and `retry-page`. Some are UI-only,
some belong to Listening, and `unavailable` and `unknown duration` look like server-owned gaps.
The question is whether the anonymous catalog must grow a field for them.

## Decision

**Accepted for #172 (option A below); it adds no wire field:**

1. **Keep "omit published-only" for the anonymous unit list.** The list means "units listenable
   now", not "every unit the Work ever had". No `availability`, `reason` or count is added.
2. **A row's `unavailable` is a client-derived state**, set only by evidence the client already
   holds: (a) a refetch that no longer lists a `unit_id` it had rendered; (b) `not_found` on
   `GET /publications/{id}` or the playback authorization for a `publication_id` it holds;
   (c) Library `access_unavailable`/`release_replaced` for a unit the listener owns progress or
   Saved for. `not_found` deliberately merges withdrawn, expired, revoked, superseded, private and
   absent; the client must not name a reason it was not given.
3. **Unknown duration is not a wire state.** Keep `u64`; never reinterpret `0`. The FE's
   "unknown duration" is a UI placeholder for a row it cannot yet size (cached or pre-manifest).
   A follow-up, not made here, may tighten `UnitSummary.duration_ms` and
   `PublicationManifest.duration_ms` to `minimum: 1` in the schema (compatible, because no producer
   emits `0`) so readers may treat `0` as corrupt.
4. **Walk recovery rules the clients implement** (details and evidence in the matrix):
   retry-page resends the same cursor within its lifetime; `invalid_cursor` drops the cursor and
   offers a restart from page one, de-duplicating by `unit_id`; an empty page with `next_cursor`
   is not the end; a filter or `limit` change discards the cursor; `position` is a display order
   that may have gaps and is never an episode number; a republish keeps `unit_id` and
   `position` and changes `publication_id`, which is a version change and never a silent swap
   under active playback.
5. **Deployment constraint:** because the cursor vault is process-local, an API behind more than
   one process without sticky routing makes `invalid_cursor` spurious. No replicated deployment is
   documented; a shared or stateless cursor needs its own decision before one is.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| **A. Omit published-only** (selected) | No wire change; no existence or reason disclosure; matches the accepted anonymous `not_found` rule and Library's separate owned-unit status; backward compatible | Anonymous clients cannot render a greyed "withdrawn" row; "unavailable" needs client evidence | Selected: the cheapest option that keeps the accepted privacy rule |
| **B. Project availability and reason in `UnitSummary`** | One list can show every state; simple FE | Needs a retained-history store (none exists), discloses unpublished or withdrawn units and why to anonymous callers, contradicts the no-410/no-disclosure rule, breaks strict v1 readers, and widens the rights/leak surface | Rejected for the anonymous catalog. Reconsider only for an account-scoped surface if retained-release access is approved |
| **C. Separate state lookup (batch unit-state endpoint)** | List stays minimal; state is queried on demand | Anonymous callers could probe ids and learn existence/state unless authenticated; duplicates Library's resume status; a new endpoint with no accepted consumer | Rejected for now; Library already covers the owned-unit case. Revisit with an authenticated consumer |
| **D. Make `duration_ms` optional to express "unknown"** | Models an unknown value directly | Server has no unknown case, so every reader gains a branch that never runs; changing `u64` to `Option` silently breaks consumers of v1 | Rejected: it represents a state the server cannot produce |

## Consequences

- Product and user impact: public lists show only listenable units. Row-level "unavailable" copy
  appears only where the client holds the evidence in Decision 2; anonymous visitors see a unit
  vanish, not a reason.
- Compatibility and migrations: none. PR #204, which records it, adds fixtures and regressions for
  existing shapes only. Choosing B or C later is an additive-but-deliberate v1 review: the
  contract test `public_list_summaries_reject_private_and_row_state_fields` is the tripwire and
  is updated by that decision.
- Operations, storage, and production cost: none now. Decision 5 constrains a future replicated
  deployment.
- Implementation and recovery requirements: Web (#158) and CMP (#159) implement Decision 2 and 4
  against the same fixtures; the real-PostgreSQL walk acceptance in the matrix remains open.
- Deferred questions: tightening the duration schema (Decision 3); retained-release access for
  owned units (Listening/Publication); a versioned public reorder policy; P1 season/credits per
  ADR 0030.

## Validation and revisit conditions

Evidence so far: the matrix maps each FE state to an existing field or a named gap; the new
regressions pin the facts it relies on (positive bounded duration, retryable cursor, strict
summaries, empty/version-change shapes). That is `example-tested` at the contract and application
layers; no real PostgreSQL multi-page walk, browser or device run supports it yet.

Revisit if retained-release access is approved, if a replicated API deployment is planned, if
product requires a visible "withdrawn" row for anonymous listeners (which needs a disclosure
decision), or if a real-PostgreSQL walk shows duplicates or skips the in-memory tests cannot.
