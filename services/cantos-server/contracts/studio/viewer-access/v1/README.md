# Studio viewer access v1 — Work and Unit control plane

Implemented Server-owned JSON control plane for [ADR 0054](../../../../../../docs/decisions/0054-publication-generation-and-access.md) D01/D04/D05. [Schema](viewer-access.schema.json), [fixtures](fixtures/corpus.json).
Studio Web consumes these routes in its Work Details access panel (#654), with frozen policy versions, separate historical receipts and explicit reload. It keeps Catalog drafts/publication state independent. Destructive global group membership/revoke controls remain unavailable until a complete affected-Work projection exists. The state route supplies only current grants/pending shares, not claimed/expired/revoked share history. The public representation is unchanged.

- `GET/POST /api/v1/studio/works/{work}/viewer-access`: current owner only. POST carries
  `operation_id`, `expected_version` (0 before the first change) and one closed `change`.
- `GET /api/v1/studio/viewer-groups`: at most 100 live owner groups, each at most 1,000 current
  live members. Names are display only. No nested group or Studio role can be supplied.
- `POST /api/v1/studio/viewer-groups/{group}`: current group owner; expected group version,
  immutable ID/name, create/add/remove/revoke. Revoke is terminal for that ID.

Every response is `no-store`. Studio cookie, exact Origin, current session and settling owner
checks apply. Theatre, Bearer, delegation and editor membership supply no Studio authority.
Typed actor grants and groups are Server-owned. At most 1,000 live grants per Work; duplicates
are refused. Expiry is trusted Unix microseconds, exclusive (`now < expiry`). Member removal,
expiry, group revoke and disabled viewer deny the pure access evaluator. S07 must resolve fresh
Identity/session state and memberships at every serving admission; these rows alone grant no bytes.

`share_mailbox` calls Identity's syntax-only canonicalizer outside the writer, then stores a
pending row for exactly 14 days, at most 100 live unclaimed rows per Work. Domain case folds;
local-part case, dots and `+` remain exact. No account-existence result or invitation is sent.
A [Theatre claim](../../../theatre/viewer-access/v1/README.md) consumes only the signed-in
subject's verified mailbox. A later-created account may claim; the resulting actor binding
never follows future mailbox reuse. Claim of an already-granted actor consumes the pending row
against that existing grant, preserving its expiry. Revoking a claimed share revokes its bound
grant. A fresh owner command may issue new authority; replay never does.

Policy/group versions and immutable audit/operation receipts commit with the change. Same actor,
operation, resource and canonical payload replays the historical receipt; changed payload or
resource returns `409 operation_reused`. Replays do not return current state. Re-read after an
unknown acknowledgement. Group membership changes advance group version; policy changes/claim
advance the Work policy version. Neither moves Catalog revisions or publication generations.

`set_audience:public` requires explicit rights confirmation and rechecks the published Work pin
against the current rights evidence in the settling transaction. A never-published Work gains
no visibility from audience alone. Restricted Work policy immediately excludes the Work from
public generation reads, including their associated manifest/media/cover gates. Restricted serving uses the separately opted-in S07 router; ordinary executable assembly stays closed.
Unit persistence and the common serving gate are implemented by S10 (#656).

Errors are fixed, content-free: invalid request 400; absent/foreign owner/resource 404; stale
policy (with current version), reused operation, duplicate, bounds, rights or exhausted version
409; unauthenticated 401; infrastructure or uncertain commit 503. No SQL or mailbox is echoed
in an error. Production migration, restore fences and deployed acceptance remain separate.

## Unit policy and chapter matrix (#656)

`set_unit_policy` on the Work POST replaces exactly one Unit policy: `inherit`, or
`restricted` with typed actor/group grants and exclusive nullable expiry. There is no child
PUBLIC mode. All Unit restrictions share the Work policy CAS, immutable event and operation
receipt; audience-only changes move neither editing heads nor publication generations. At most
1,000 current Unit grants exist across a Work (separate from the Work grant bound). Empty
restriction hides the Unit from every listener. INHERIT requires explicit rights confirmation
and the settling published-Work rights check; it never publishes or restores an absent Unit.
Current grants are replaceable projections; the full command is sealed in immutable history.

- `GET /works/{work}/units/{unit}/viewer-access` (under the same Studio prefix): current owner,
  exact Work/Unit binding, Work policy version and current Unit policy.
- `GET /works/{work}/chapter-matrix?limit=20&cursor=...`: at most 50 rows per response (Studio
  requests 20), saved draft titles/revisions, exact published pins/order, current Work audience,
  policy/publication versions and lifecycle. Audio is the generation association, **not** a live
  readiness promise. An opaque continuation binds owner, Work, Catalog clock and both versions;
  stale continuation returns `409 stale_policy` and requires explicit reload.

Studio selects exactly one loaded row, preserves dirty policy input through failures, and
explicitly reloads/compares before rebasing a conflict. Publish/withdraw uses the existing
metadata-publication endpoint with the Work expected-generation CAS, exact saved Unit revision
and explicit published position; it preserves the pinned Work and all other entries. No hidden
or unloaded row is selected implicitly. Requests retain the existing operation for unknown-ack
retry. Current state is read separately from historical receipts.
