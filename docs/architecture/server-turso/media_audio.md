# External audio release chain and listener reads on `server.db` (migration 14)

> **Historical migration-14 slice.** The Media write ports below were subsequently implemented
> by migrations 20–23; v24 adds cover disclosures. Read the
> [current native mapping](remaining-pg-retirement.md) before using this initial read-only scope.
> This record's unported-write statements are historical, not the current runtime boundary.

Scope: the rows Theatre discovery, manifests, playback authorization and the origin media route read,
and the discovery indexer that writes the listener index. It is **not** the Media context port: upload
intents, intake receipts, reviews, publication events/operations, the lifecycle and publication
write transactions, artwork and images still carry PostgreSQL SQL. The reserved placeholder versions
9 and 10 are unchanged. See the [slice record](cmp-business-api.md).

Version 14 `server_audio_release_chain`: `audio_submissions`, `audio_renditions`,
`audio_release_candidates`, `audio_distribution_clearances`, `audio_publication_approvals`,
`audio_releases`, `audio_publication_pointers`, `audio_release_withdrawals`,
`audio_submission_revocations`, `audio_listener_works`, `audio_listener_units`,
`audio_listener_releases`. Timestamps are the domain's Unix milliseconds (the unit of the snapshots).

Persistence is `contexts/media/turso/listener.rs` (renamed from `postgres/listener.rs`);
`ListenerService::new` composes `TursoRead`, so the production assembly serves from Turso.

## Mapping

| Source rule (PostgreSQL object / adapter behavior) | Turso mechanism | Test |
| --- | --- | --- |
| `reject_settled_change` on submissions, renditions, candidates, clearances, approvals, releases, withdrawals, revocations and the three listener index tables | per table `BEFORE UPDATE`, `BEFORE DELETE` and `BEFORE INSERT … WHEN EXISTS` over every unique key (id, token hash, `(owner, operation)`, `(candidate, sequence)`, `submission_id` …) so `REPLACE` and upsert bypasses are refused | `the_audio_release_chain_is_digest_checked_bound_and_immutable_and_the_pointer_only_moves_forward` |
| `guard_audio_publication_pointer` | `BEFORE UPDATE … WHEN unit/owner changed OR NEW.version <> OLD.version + 1`; `BEFORE DELETE` and insert-over-key guards | same |
| pointer `(active_release_id, unit_id, owner_id)` → release | composite foreign key; a `NULL` release (withdrawn) skips it, as PostgreSQL's `MATCH SIMPLE` | same (another unit's release refused) |
| digest schemes `as-r1`, `ar-r1`, `rc-r1`, `dc-r1`, `pa-r1`, `rl-r1`, `rm-r1` | `CHECK (coalesce(… REGEXP '^<scheme>:sha256:[a-f0-9]{64}$', 0))` | same |
| `audio_listener_releases` window `rights_end > rights_start`, kind set, `(unit_id, work_id)` pair | table `CHECK`s; composite foreign key to `audio_listener_units(unit_id, work_id)` | same |
| rights and approval: *latest* by `sequence` (`JOIN LATERAL … ORDER BY sequence DESC LIMIT 1`) | `JOIN … ON clearance.sequence = (SELECT max(sequence) …)`; `UNIQUE (candidate_id, sequence)` makes it one row, and the row must still carry the digest the listener row names | `private_staged_…_are_refused` |
| `DISTINCT ON (x.work_id) … ORDER BY published_at DESC, release_id` | `row_number() OVER (PARTITION BY work_id ORDER BY published_at DESC, release_id)` filtered to `1` **before** the kind, language and search filters | `discovery_orders_filters_searches_and_pages_by_immutable_anchor` |
| `count(*) OVER`, `row_number() OVER` for presentation | unchanged window functions | `presentation_is_an_opt_in_with_exact_counts_and_ordinals_over_the_eligible_relation` |
| `= ANY($n)` / `unnest` | `IN (SELECT value FROM json_each(?n))` with a JSON array parameter | library suites |
| `position($q in search_text) > 0` | `(?q = '' OR instr(search_text, ?q) > 0)` | `discovery_…` (Vietnamese NFC/NFD, case) |
| `(first_position, unit_id COLLATE "C") > ($3, $4)` row comparison | `first_position > ?3 OR (first_position = ?3 AND unit_id > ?4)`; `BINARY` equals `"C"` | `discovery_…`, unit cursor walk |
| `floor(extract(epoch FROM clock_timestamp())*1000)` | the transaction's trusted stamp converted once to milliseconds; the read path re-reads the owner clock after the page is assembled and drops an item whose access window closed meanwhile | `playback_is_authorized_for_the_exact_release_and_the_bytes_are_those_it_names` |
| `RepeatableRead` read-only transaction | `conn.read()`: a pinned snapshot that never waits for the writer | `concurrent_readers_see_a_whole_publication_before_or_after_a_withdrawal_never_a_torn_one` |
| `index_release … ON CONFLICT DO NOTHING` (first committed index writer wins) | `INSERT … SELECT … WHERE NOT EXISTS`: the index tables refuse an insert over an existing key before conflict resolution, so `ON CONFLICT` would raise instead of ignoring | `seed_work` (the fixture publishes through the production indexer) |
| `RELEASE_OWNER_ACTIVE_FILTER` | unchanged text, alias `rel`; the owner (creator), not the listener, must be active | `…_owner_disabled…` case |
| `lock_listener_catalog` fence | deleted (serialized writer) | see Catalog |

Eligibility is not a projection: the SQL relation only selects candidates; `ListenerPublication::resolve`
(domain) decides from the decoded, digest-checked values, pinned and current, and the grant and the
media route re-resolve on every request.

## Trust shift

PostgreSQL granted `SELECT, INSERT` and a column-limited `UPDATE` on the pointer to the runtime role.
Here the immutability lives in the triggers above, not in a role; the owner process is the only
writer. Rows are re-decoded and digest-checked on every read.

## Not established

- The publication transaction (preflight → external observation → final transaction/recheck, the
  expected-pointer CAS, events and operation receipts) on Turso. This slice reads releases; it does
  not create them outside the fixture.
- Ogg/Opus admission of real bytes, S3/object storage and any CDN. Media tests place exact
  content-addressed transport bytes in the publication root.
- Statement-count evidence for the "no N+1" property is by construction (one batched query per
  request, one `json_each` metadata join), not measured.
