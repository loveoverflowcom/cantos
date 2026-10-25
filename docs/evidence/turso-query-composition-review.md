# Turso query composition review (#414)

> **Status:** source-bound review record for [#414](https://github.com/loveoverflowcom/cantos/issues/414)
> at `develop@f5e1512ff7d613ead60e8d3d342994cc9ae25cfb` (after #412 / PR #439; #413 not merged).
> It inventories how the Server's Turso adapters compose SQL and records the kept decision.
> It makes **no** planner, index or performance claim: no `EXPLAIN` or benchmark was run, and
> #384 still owns read performance.

## Question

Do the Theatre Discovery and Studio Catalog adapters need a structured query composer (a
service-local helper, a shared `store-turso` helper, an internal `SqlBuilder` or SeaQuery), or is
the existing raw SQL already the simpler and safer shape?

## Method

- Read every production statement in the primary scope:
  `services/cantos-server/src/contexts/media/turso/listener.rs` and
  `services/cantos-server/src/contexts/catalog/turso.rs` with its `listener.rs`, `content.rs`
  and `delegation.rs` submodules.
- Cross-checked every `format!`/`.replace` site that produces SQL in `cantos-server`,
  `cantos-identity` and `store-turso` production code (`git grep` for `format!("SELECT|WITH|INSERT…`,
  `let query = format!`, `.replace("{`, generated `?` placeholder lists and `push_str`).
- Classified each statement as **fixed** (one SQL text per call site), **bounded static variants**
  (a choice among whole static statements) or **runtime-shaped** (JOIN set, predicate structure,
  ordering or projection depending on request data).

## Inventory

| Owner | Production statements | Composition | Runtime shapes |
| --- | --- | --- | --- |
| Theatre Discovery `media/turso/listener.rs` | 12: 3 index writes and 9 reads (`works`, `work`, `units`, `publication`, `presented_works`, `presented_work`, `presented_units`, Library `library_current_items`, `library_work_presentations`) | Writes are literals. Reads join one eligible relation (the `FROM` constant, whose `{current_filter}`/`{actor_filter}` slots take the crate constants `LISTENER_CURRENT_FILTER` and `RELEASE_OWNER_ACTIVE_FILTER`) with the `COLUMNS`/`REPRESENTATIVE` constants and a literal tail per call site | Fixed: one SQL text per call site |
| Catalog `catalog/turso.rs` | 21 | 4 `format!` sites inline the `WORK_COLUMNS`/`UNIT_COLUMNS` projections; the rest are literals | Fixed |
| Catalog `catalog/turso/listener.rs` | 1 (`METADATA`) | Literal; exports `LISTENER_CURRENT_FILTER` to Media | Fixed |
| Catalog `catalog/turso/content.rs` | 7 | `{COLUMNS}` constant; `load` picks one of two whole statements by `Option<id>` | Bounded static variants (2) |
| Catalog `catalog/turso/delegation.rs` | 6 | `one(predicate, params)` with three literal predicates; `list` literal | Fixed |
| Other Server adapters (production, briefs, editorial, ingestion, adaptation, media intake) and Identity `firebase.rs` | — | `format!` only inlines a `*_COLUMNS` constant or a constant select prefix (`review_query()`, `GRANT_SELECT`); production `grant_query(column)` takes two literal columns | Fixed |

Bind forms observed:

- **Optional filters are bound, not composed.** `works` uses `(?2 IS NULL OR …)` for the cursor,
  `(?4 IS NULL OR kind=?4)`, `(?5 IS NULL OR language=?5)` and the `?6=''` empty-search sentinel;
  `units`/`presented_units` use `(?3 IS NULL OR …)`; Catalog `list_works`/`list_units` bind the
  cursor the same way and `include_archived` as a bound boolean (`?3 OR lifecycle<>'archived'`).
- **Variable-size batches are one bound JSON array** read by `json_each(?N)` (Discovery Library
  batches, `presented_works`, `listener_metadata`, `active_actors`). No production code generates
  an `IN (?,?,…)` placeholder list; the only generated placeholder list is a test projection helper.
- **Bind numbering is stable per relation.** `?1` is the read's trusted instant inside the shared
  relation; each call-site tail continues at `?2`.

Result: **no runtime-shaped query exists** in the scope or elsewhere in the Server. The number of
distinct runtime SQL texts equals the number of call sites (plus the two `content::load`
variants). The only place where a type admitted a runtime string as SQL syntax was the
`predicate: &str` parameter of two private lookup helpers (`production/turso/briefs.rs`
`grant_query` and `catalog/turso/delegation.rs` `one`); every caller passed a literal.

## Semantics a composer would have to preserve

These properties live in the exact SQL text and are what an abstraction is most likely to break:

| Property | Where | Existing oracle |
| --- | --- | --- |
| The newest eligible release represents a Work **before** kind/language/search filters apply (`pick=1` in the outer query) | `works` | `theatre_work_filters_follow_the_latest_eligible_representative` (`tests/support/theatre_catalog_walk_http_turso.rs`) |
| Works keyset `(first_published_at DESC, work_id ASC)` and Units keyset `(first_position, unit_id)` tie breaks; cursors bound to their query/Work | `works`, `units`, `presented_units` | `discovery_orders_filters_searches_and_pages_by_immutable_anchor`, `theatre_works_walk_rechecks_visibility_and_binds_filter_cursors`, listener application cursor tests |
| Presentation counts/ordinals are window aggregates over the eligible relation, before the cursor and the page limit | `presented_*` | `presentation_is_an_opt_in_with_exact_counts_and_ordinals_over_the_eligible_relation` |
| Search is NFC/NFD and case insensitive, and matches with `instr`, so `%`/`_` are literal text | `works` | `vietnamese_nfc_nfd_and_case_search_real_metadata`; the `%`/`_` case is the regression added with this record |
| Current-public, owner-active, rights window, withdrawal and revocation filters on every read | relation | `theatre_public_discovery_manifest_search_replacement_and_withdrawal`, `list_and_detail_share_one_representative_release_per_work` |

## Options

| Option | Fit here | Decision |
| --- | --- | --- |
| A. Explicit static SQL with named projection constants | What the code already is; one reviewable SQL text per call site, bound optional predicates | **Keep** |
| B. A small set of whole static variants | Already used where a shape really differs (`content::load`) | **Keep where it exists**; the first step if a real variable shape appears |
| C. Service-local helper | The two lookup helpers already are one; their fragment type was the gap | **Tighten** the fragment to `&'static str` |
| D. Generic `store-turso` helper | No two independent consumers of a composer; the [store README](../../packages/store-turso/README.md) keeps service statement facades and business rules local and adds no ORM | Reject |
| E. SeaQuery (or a ~60-line internal `SqlBuilder`) | No dynamic AST to build. Adds a dependency (porting rule 10), still cannot check the Turso schema at compile time, and would hide the exact text that carries the representative-before-filter order, the shared `?1` and the `json_each`/window/`instr` idioms | Reject |

## Decision and change

Keep raw static SQL. The bounded change with this record:

- `catalog/turso/delegation.rs` `one` and `production/turso/briefs.rs` `grant_query` take their
  SQL fragment as `&'static str`, so a runtime string can no longer become SQL syntax there;
  the generated SQL text and binds are unchanged.
- A Discovery regression proves `%` and `_` stay literal search text (`%` finds only the title
  containing it; `_` and `%_%` match nothing), a property a `LIKE`-based rewrite would break.
  Negative control: with `instr(search_text,?6)>0` temporarily replaced by
  `search_text LIKE '%'||?6||'%'`, every earlier assertion of
  `discovery_orders_filters_searches_and_pages_by_immutable_anchor` still passed and the new
  block failed at `works("%")` with `ListenerError::Corrupt` (the application's pure
  `DiscoveryQuery::matches` recheck refuses the extra rows). The source was restored afterwards.
- [Porting rule 8](../architecture/server-turso-porting.md#1-rules-that-never-bend) now states
  the composition rule.

Kept on purpose: the `FROM.replace(...)` composition of the Discovery relation. It is fixed
composition from crate constants; a mismatched slot name leaves `{…}` in the statement, which
fails every Discovery read with a parse error that each listener test reaches. Rewriting it is
cosmetic churn without new evidence.

Reconsider only when a feature needs a request-dependent JOIN set, sort key or filter set. Start
with B (an enum of whole static statements), and evaluate C/E only if the variants measurably
explode.

## Verification

Rust 1.98.1, Linux x86_64, file-backed `server.db` fixtures (real Turso engine, no network):

| Check | Result |
| --- | --- |
| `cargo check --locked -p cantos-server --lib --bins` (runtime build, no `test-support`) | PASS, no warnings |
| `cargo fmt -p cantos-server -- --check` | PASS |
| `cargo test --locked -p cantos-server --lib -- contexts::media::turso contexts::catalog contexts::production` | PASS, 102 passed |
| `cargo test --locked -p cantos-server --test delegation_turso --test catalog_http_turso` (delegation lookups by token/id/operation; producer grant issue, token resolve, revocation and expiry through Studio HTTP) | PASS, 4 + 61 passed |
| `LIKE` negative control above | FAIL as intended, then reverted |

## Not established

- No planner, index or latency observation; #384 owns any performance claim.
- No prepare-against-migrated-schema check for every statement; that is #413's scope.
- No browser, device or deployed acceptance.
