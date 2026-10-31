# ADR 0056: A refused Shared read stops the active private source of that Work

- Status: **accepted (owner, 31 October 2026)**: option A, chosen in the #676 follow-up
  session. Implemented for Theatre Web; native private Play is still closed.
- Date: 2026-10-31
- Decision owner: Cantos product owner
- Related issue or PR: review ledger [#676](https://github.com/loveoverflowcom/cantos/issues/676)
  for epic [#634](https://github.com/loveoverflowcom/cantos/issues/634); found by the PR #683
  real local browser campaign (`docs/evidence/private-web-teardown-676.md` on that branch)
- Supersedes / superseded by: N/A. It would amend one sentence of the
  [Theatre shared v1 contract](../../services/cantos-server/contracts/theatre/shared/v1/README.md)
  and leaves [ADR 0054](0054-publication-generation-and-access.md) D08b unchanged.

## Context

Known facts at `develop@7875453d` plus PR #683:

- Theatre Web clears an active private (shared-delivery) source in three cases only: the account
  fence changes (logout, account switch, signed-out refresh); the player fails a shared media
  fetch as unauthenticated, forbidden or unavailable; the shared listen manifest answers
  `NotFound`. The effect lives in `apps/cantos-theatre/web/src/view/live_shell.rs`. The shared
  v1 contract states exactly that: "losing the account clears the element … A current
  media/manifest denial also removes private detail facts."
- In the #683 campaign, cookies were cleared while a private source played, then `#shared/{work}`
  was reread. The Server answered `404`, the page showed the generic Shared denial, and the
  client still believed the session was live, so no account change fired. The player kept
  playing already fetched bytes for about 6 s, until its next fetch was refused.
- ADR 0054 D08b (accepted) lets an admitted response finish within the 60 s body deadline and
  does not recall buffered bytes. It rejected "cancel admitted streams". That governs Server
  transport, not what a client does after it sees a refusal.
- The [handoff plan](../work-plan/publication-access-handoff.md) records the intent that
  "logout or a current denial" clears private media/display facts, and that the active-source
  browser claim is unaccepted. It does not say whether a metadata (Work/Unit detail) refusal is
  such a denial.
- A Shared `404` is deliberately merged: lost session, revoked grant, member removal, expiry,
  withdrawal and never-granted all look the same to the client.
- Shared Web listen controls and their entrypoint are closed, and restricted serving is
  default OFF. Native private Play has no dispatch path. Nobody can reach this today outside an
  explicit test fixture with a campaign-only unlock.

Why now: #683 records this as a rule that must be settled before Web listen is enabled.

## Decision

When a Shared Work read made in the current session settles as `NotFound`, for either the Work
summary or a Unit page, and the active player source is a shared-delivery track of that same
Work, the client stops it exactly as it does for a shared media denial: player `AccessCleared`,
`now_playing` and listen state cleared, Work detail refused and the Shared catalog purged.

Boundaries:

- Only `NotFound`, the contract's merged denial. Transport, `503`, timeout, protocol and
  invalid-cursor failures never stop playback.
- Only a shared track whose `work()` equals the refused Work. Public tracks and shared tracks of
  another Work are untouched.
- No Server change. D08b stays as is: admitted responses may finish and bytes are not recalled
  server-side. This rule is client conduct only.
- It adds no listen entrypoint. Shared listen controls stay closed and restricted serving stays
  default OFF until the HTTPS cookie/Range/seek/revoke qualification in #676 exists.
- Native clients adopt the same rule when native private Play is opened. Under
  [ADR 0055](0055-development-phase-compatibility.md), that delivery updates every current
  client.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| **A. Matching-Work refusal stops that shared source** (selected) | The page and the audio agree. The rule is deterministic and local, reuses the existing `AccessCleared` path, and needs only a pure-state regression | A Shared reread of the same Work while it plays is required to notice. Revocation of a different Work, or session loss noticed only elsewhere, still relies on the next fetch or account refresh | Smallest rule that closes the observed gap without guessing what an ambiguous `404` means for other Works |
| B. Keep today's triggers; rely on D08b tolerance | No change; consistent with "admitted bytes may finish" | The screen says "no access" while private audio continues; it diverges from the handoff intent "a current denial clears private media" | Rejected unless the owner prefers D08b tolerance on the client too |
| C. On any Shared `404`, re-confirm the session and clear only through the existing account-loss path | Handles a lost session on any Work, and the account state becomes truthful | One extra request per refusal. A revoked grant with a live session keeps playing. Adds a refresh/fence interaction to qualify | Could be combined with A later. Not needed to close the observed case |
| D. Any Shared `404` stops every shared source | Strongest privacy reaction | A refusal for an unrelated Work (for example a stale link) would stop audio the listener is still entitled to | Rejected: the merged `404` says nothing about other Works |

## Consequences

- Product and user impact: after a refused Shared reread of the playing Work, playback stops
  and the full/mini player release the source, as with a refused seek today. No new copy.
- Compatibility and migrations: none. Client-only, no wire or data change.
- Operations, storage, and production cost: none.
- Implementation and recovery requirements: one predicate beside `media_denied`/`manifest_denied`
  in `live_shell.rs` (route `SharedWork(id)`, `WorkDetailPhase::NotFound` for `id`, active
  shared track with `work() == id`); a pure/native Leptos regression proving it clears only the
  matching shared source, never a public track or another Work's shared track; amend the shared
  v1 contract sentence and the Theatre Web README. Behavior SSOT: no catalog capability owns
  client playback clearing (`cantos.catalog.viewer-access` models only the Server decision and
  is unchanged), so the gap is recorded, not modeled.
- Deferred questions: whether to add alternative C (session re-confirmation on refusal) when
  Web listen is enabled.

## Validation and revisit conditions

Validate with the native Leptos regression above, then with the explicit
`web_sharing_browser_host` campaign recipe from the #683 evidence record (campaign-only unlock,
never committed): cookies cleared, Shared Work reread, audio stops within the same tick as the
denial. Revisit when Web listen is enabled, when native private Play opens, or if the Shared
contract stops merging refusals into `404`.
