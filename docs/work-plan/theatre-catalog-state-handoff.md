# Theatre catalog state coverage (#172)

Review input for [#172](https://github.com/loveoverflowcom/cantos/issues/172), consumed by
[#158](https://github.com/loveoverflowcom/cantos/issues/158) (Web W01–W03) and
[#159](https://github.com/loveoverflowcom/cantos/issues/159) (CMP M01–M06). It maps each client
state to the field or endpoint that exists today, names who owns the truth, and lists what is
missing or deferred. It is **not** acceptance of #172, adds no wire field, and the policy it
relies on is [ADR 0035](../decisions/0035-theatre-catalog-unit-availability.md), accepted by the
user on 2026-10-16 (no separate Product, Listening or Publication owner sign-off is recorded).

Original baseline: `develop@43db968`, re-read after the issue's `a079e72` survey.
The PostgreSQL walk follow-up starts from `develop@64b5a60`; it exercises the accepted policy
without changing the DTO, schema, routes, SQL or deployment composition. Sources:
[Theatre DTOs](../../packages/contracts/api/src/theatre.rs), [routes](../../services/cantos-server/src/adapters/inbound/http/theatre.rs),
[application](../../services/cantos-server/src/contexts/media/listener_application.rs),
[projection](../../services/cantos-server/src/contexts/media/listener_projection.rs),
[Turso reader](../../services/cantos-server/src/contexts/media/turso/listener.rs),
[cursor vault](../../services/cantos-server/src/contexts/media/listener_cursor.rs) and the
[Discovery v1 contract](../../packages/contracts/theatre-discovery-v1.md). Nothing below is a
provider, device or browser observation.

## How to read the tables

| Owner | Meaning |
| --- | --- |
| **UI** | Client-only state. The server never sees or decides it. |
| **Catalog** | Anonymous public projection (`/api/v1/theatre/works…`). Current eligibility is re-read on every page. |
| **Listening** | Listener-authenticated Library/Progress ([contract](../../packages/contracts/listener-library-v1.md)); absent for a guest. |
| **Delivery** | Playback authorization and media bytes ([ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md), composition-gated). |

`P0` = supported by existing wire. `Gap` = a named gap below. `Deferred` = owned elsewhere.

## A. Work detail and unit list (W01, M01, M02)

| Client state | Owner | Existing field or endpoint | Server authority | Status |
| --- | --- | --- | --- | --- |
| Title, description, kind, language | Catalog | `WorkSummary` from `GET /works/{work}` | Work metadata revision **pinned** by the newest eligible release; `language` is the released audio language, `kind` is `novel`/`story`/`podcast` | P0 |
| "First published" | Catalog | `first_published_at_ms` | Immutable first-index anchor; republish never moves it | P0 |
| Work missing, private, withdrawn, expired, owner-disabled | Catalog | `404 not_found` (`not_found.json`) | One merged anonymous answer; no reason, no 410 | P0, by design |
| Non-canonical Work id | Catalog | `400 invalid_request` | Strict canonical lower-case UUID | P0 |
| Cover or fallback art | Deferred | none (`WorkSummary` has no cover) | [#157](https://github.com/loveoverflowcom/cantos/issues/157), [ADR 0033](../decisions/0033-release-pinned-public-cover.md) proposed | Deferred; FE shows its fallback |
| Genres, credits, seasons, transcript, markers | Deferred | none | P1, [ADR 0030](../decisions/0030-release-safe-public-metadata.md) | Deferred; fixtures stay labelled in preview only |
| Unit count or total duration | — | none | Would disclose hidden-unit existence | Gap G5; FE must not invent it |
| CTA "Nghe" (first unit) | Catalog | first page of `GET /works/{work}/units`, first item | Display order, see `position` below | P0; CTA disabled until the page loads; never autoplays |
| CTA "Tiếp tục", saved, completed | Listening | `/library/continue`, `/library/saved/{work}`, `/progress/{publication}` | Listener-authenticated registers | #12/#35; guest sees none |
| Row identity | Catalog | `unit_id` (stable), `publication_id` (exact audio) | Immutable ids; never title, slug or order | P0 |
| Row order and number | Catalog | `position` (`u32`) | First public index anchor, `(position, unit_id)`; later Studio reorder is **not** adopted | P0. May have gaps; not an episode number |
| Row kind | Catalog | `UnitSummary.kind` (`chapter`/`episode`) | Pinned unit metadata | P0; read per row, do not infer from Work kind |
| Row title, description | Catalog | `UnitSummary.title/description` | Pinned unit metadata revision | P0 |
| Duration known | Catalog | `duration_ms` | Verified rendition, `1..=3_600_000` ms | P0 |
| Duration **unknown** | UI | none, and never emitted | Server has no unknown case (ADR 0035 §3) | UI placeholder only; Gap G4 |
| Selected | UI | — | — | UI. Row select ≠ play |
| Playing, buffering | UI | — | Engine acknowledgement, never the server | UI; not set before engine ack |
| Completed | Listening | `/progress/{publication}`, `/library/history` | Explicit completion, not drag-to-end | Listening |
| **Unavailable** row | UI (evidence-derived) | absent from list; `not_found` on `/publications/{id}`; Library `access_unavailable`/`release_replaced` | Eligibility is re-read each request | ADR 0035 §2; no wire field |
| Empty Work (no listenable unit) | Catalog | `404` on first `/units` call | Same answer as an absent Work | P0 |
| Filter-empty | Catalog | `200` `WorkPage{items:[],next_cursor:null}` (`empty.json`) | Filters apply after representative selection | P0; filters exist on Works only |
| Loading-next | UI | `next_cursor` | Opaque cursor | P0 |
| Retry-page | UI | resend the same cursor | Cursor is not consumed by a read | P0, [evidence](#c-evidence-ledger) |
| Page failure keeps prior rows | UI | `503 unavailable`, `retryable:true` | Typed error, no catalog fallback | P0; client keeps rendered rows and focus |
| Version change (republish) | Catalog | same `unit_id` and `position`, new `publication_id`, later `published_at_ms` (`units_republished.json`) | Atomic active pointer | P0; never swap bytes under active playback |

## B. Player, offline and recovery (W02, M03, M06)

| Client state | Owner | Existing field or endpoint | Status |
| --- | --- | --- | --- |
| Timeline length | Catalog | `UnitSummary.duration_ms` equals the manifest value | P0 |
| "Next unit" | Catalog + UI | next row in list order; may lie on the next page and may have been withdrawn | P0; fetch the next page, handle an empty page with a cursor |
| Marker next, queue panes | Deferred | none | P1; distinct from unit next |
| Authorize playback | Delivery | `POST /publications/{id}/playback-authorizations` | Default host answers `rendition_unavailable` until origin delivery is composed (#10/#11) |
| Revoked vs network error | Catalog + Delivery | `404 not_found` (stop) vs `503 unavailable`/`rendition_unavailable` (retryable) vs transport failure | P0; do not merge them |
| Expired media access | Delivery | `expires_at_ms`; re-POST for the same `publication_id`, same `sha256` resumes | P0 once delivery is composed |
| Cached metadata, audio absent | UI | none; no public grant is stored | UI. Do not fake offline playback |
| No autoplay, no "playing" before ack | UI + Listening | history starts only on an acknowledged engine start | Listening/UI |

## C. Evidence ledger

Evidence uses the [foundation vocabulary](../../.agents/skills/cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified).
The mutant results below belong to the original state/G2 slices, not a new mutation campaign.
The registered Kani harness was not re-executed by the PostgreSQL walk follow-up. Ordinary
`rust-tests` skips the ignored PostgreSQL tests: only the disposable `postgres` gate executes
them. Local SQL evidence and named delivery doubles are recorded separately below.

| Claim | Failure mode | Oracle | Level | Source |
| --- | --- | --- | --- | --- |
| A published duration is positive and bounded, so no "unknown" reaches the wire | A zero or unbounded duration is read as unknown | table over `0, 1, 3_600_000, 3_600_001, u64::MAX` with the exact `BindingMismatch` | example-tested | `domain::lifecycle::tests::a_verified_rendition_admits_only_a_positive_bounded_duration`; mutant "drop zero guard" killed |
| A cursor survives a retry and replays the same page | Consumed-on-read cursor strands a retry | same keyset sent twice; identical pages | example-tested (reader double) | `listener_application::tests::retrying_a_unit_cursor_replays_the_same_continuation`; mutant "consume on read" killed |
| Cursor binds query, limit, route and Work; expires; tamper rejected | A cursor reused across filters or Works | exact `InvalidCursor` | example-tested | `listener_cursor::tests::opaque_tokens_bind_query_expire_and_never_encode_internal_values`, `listener_application::tests::empty_first_unit_page_preserves_continuation_and_cursor_scope` |
| An empty page with a cursor is not the end; withdrawal after the scan is tolerated | Client stops early or loops | scripted reader | example-tested | `empty_first_unit_page_preserves_continuation_and_cursor_scope` |
| Empty catalog, absent Work and withdrawn release differ only where intended | Leak or wrong state | exact results | example-tested | `empty_catalog_and_private_or_absent_release_are_distinct` |
| Wire uses the immutable listing anchor, not the Studio position | Reorder leaks into public order | `position` 9 ≠ Studio 1 | example-tested | `discovery_projects_resolved_publications_with_listing_anchors`, `summaries_use_published_audio_language_and_listing_anchors` |
| Works keyset is a strict total order; insert/delete/replacement page each stable work once | Duplicate or skipped Work in a walk | proptest model and bounded proof | property-tested; bounded-model-checked (harness not run here) | `domain::listening::tests::{keyset_is_strict_total_order, stable_work_anchors_page_once_under_insert_delete_and_rendition_replacement}`, `kani_proofs::listener_keyset_and_access_window` (model, not SQL) |
| Units keyset `(first_position ASC, unit_id)` is a strict total order whose id comparison equals SQL `COLLATE "C"` byte order for canonical UUID text | A reversed or non-strict comparison hands a client a duplicate or skipped chapter | extremes table; proptest over a small position domain (ties are common) and the independent oracle `(position, uuid_text_bytes)` | example-tested; property-tested; mutants "flip position" and "flip id tie-break" killed | `domain::listening::tests::{unit_keyset_direction_matches_ascending_position_and_ascending_identity_at_extremes, unit_keyset_is_strict_total_order, unit_keyset_matches_sql_c_collation_of_canonical_uuid_text}` (model, not SQL; no Kani harness for Units) |
| `ListenerService::units` rejects a reader row that is not strictly after the cursor key | A faulty SQL ordering or reader reaches a client as a duplicate or skipped chapter | reader double ignoring the key, rows at positions 6, 7 (same key) and 8 | example-tested (reader double); mutant "guard bypassed" killed | `listener_application::tests::reader_row_not_strictly_after_the_unit_cursor_is_corrupt` |
| `WorkSummary`/`UnitSummary` carry no private evidence and no per-row state | A silently added `availability` or `owner_id` | schema and strict serde both reject each injected field | example-tested; mutant "drop `deny_unknown_fields`" killed | `theatre_discovery_wire::public_list_summaries_reject_private_and_row_state_fields` |
| State samples are valid v1 and show version change | FE integrates against invented shapes | schema + shared Rust round trip | example-tested | `public_fixtures_match_strict_schema_and_shared_rust_types`, `unit_state_fixtures_pin_empty_continuation_and_version_change` |
| Single Work: publish, search, replacement keeps the anchor, withdrawal removes it; private and other-owner stay invisible | Wrong eligibility | real PostgreSQL | integration-tested **in CI only** | `publication_http_postgres::{theatre_public_discovery_manifest_search_replacement_and_withdrawal, theatre_private_published_release_and_other_owner_remain_invisible}` |

### PostgreSQL walk follow-up

Review boundary: prove the existing Work/unit keysets and current-public projection through
the real Router, application and app-role PostgreSQL reader when publication changes between
page requests. This is regression coverage of accepted ADR 0035, not a new availability,
duration, reorder or delivery policy. The original G1 environment lacked PostgreSQL tools;
that historical `UNSUPPORTED` result is not the status of this follow-up.

The fixture creates several Works/units in **one database**, uploads owned synthetic Opus
signals through private intake and records rights/review/publication via Studio HTTP. It
composes the existing `VerifiedDeliveryFake` only in tests. That double supplies publication
delivery observations; it proves no publication-class object store, public media fetch or CDN.
No credential, production resource, Firebase project or provider is used.

The tests live in
[`theatre_catalog_walk_http_postgres.rs`](../../services/cantos-server/legacy-pg-tests/tests/support/theatre_catalog_walk_http_postgres.rs),
nested in the existing `catalog_http_postgres` target and run by `cargo xtask verify postgres`.
Execution results and checked source identity belong to the follow-up PR, not the historical
results above. Browser/device acceptance belongs to #158/#159 and remains separate.

| Test | Exact observable assertions |
| --- | --- |
| `theatre_units_walk_survives_mid_walk_withdraw_publish_and_republish` | Four units in one Work; after page one, withdraw an unseen unit, reorder Studio, publish a new unit and republish the seen unit with a fresh candidate. The seen unit keeps public position 0 despite Studio position 3; a newly indexed unit also has position 0 and the UUID tie-break is exercised by SQL. Every stable remaining unit is reached once, the publication changes, and route/Work/limit misuse does not consume the valid cursor. |
| `theatre_works_walk_rechecks_visibility_and_binds_filter_cursors` | Multiple Works in one catalog; withdraw an unseen Work, republish the seen Work and first-publish a newer Work between pages. The original anchor remains fixed, the newer Work stays before the cursor, remaining stable Works are reached once, and retries/filter/limit/route bindings hold. |
| `theatre_work_filters_follow_the_latest_eligible_representative` | Two releases of one Work pin different metadata/audio languages. Search/kind/language filter the newest eligible representative, rather than choosing an older matching release. Withdrawal makes the older eligible representative visible again. |
| `theatre_units_walk_rechecks_rights_expiry_on_the_database_clock` | An unseen unit is visible before its rights expire; after bounded polling of the real database clock, continuation omits it and reaches the following unit. Its publication answers the exact anonymous `404 not_found` envelope. No SQL scalar/snapshot is rewritten to simulate expiry. |

No catalog-pagination capability is registered in the [behavior catalog](../../specs/behavior/catalog.json).
This follow-up changes no accepted rule or pure decision; it adds SQL observations for existing
rules and leaves the pure ordering/cursor tests and draft delivery models unchanged. A model-only
PASS would not establish these PostgreSQL claims.

No production read/query or index changes are made. The existing read inventory and deferred
scale/EXPLAIN/DBSP assessment remain in the [source-bound discovery evidence](../evidence/theatre-discovery.md#read-inventory-targets-and-dbsp-decision);
these small correctness fixtures establish no latency, throughput or optimization result.

## D. Gaps and next owners

| Id | Gap | Why it matters | Proposed next step | Owner |
| --- | --- | --- | --- | --- |
| G1 | Regression coverage added by the PostgreSQL follow-up above: several Works/units, publication changes between pages, rights expiry, retry/filter scope, immutable anchors and equal-position UUID ordering | SQL regressions can duplicate/skip stable rows even when the pure keyset model passes | Run the existing full `cargo xtask verify postgres` gate; the follow-up PR records actual local results. These bounded interleavings do not establish arbitrary simultaneous schedules or query performance | BE; Web/CMP recovery acceptance stays with #158/#159 |
| G2 | **Closed by the earlier pure-core/service-guard slice.** `after_unit` sits next to `after_work`; `ListenerService::units` returns `Corrupt` for a reader row not strictly after the cursor key. No Units Kani harness was added | A faulty reader is rejected instead of silently producing a duplicate/skip | G1 now exercises the real SQL continuation and tie-break alongside the unchanged guard; formal proof of the complete SQL query is not claimed | BE |
| G3 | Cursor vault is process-local | Behind more than one API process, `invalid_cursor` is spurious | [ADR 0035 §5](../decisions/0035-theatre-catalog-unit-availability.md#decision); decide a shared/stateless cursor before replication | Platform/Publication |
| G4 | Schema allows `duration_ms: 0`; the domain never emits it | Readers cannot tell `0` is corrupt | Optional `minimum: 1` tightening, compatible; ADR 0035 §3 | Contract owner |
| G5 | No Work-level unit count or total duration | Hidden-unit disclosure | Leave absent; FE derives only from walked pages | Product |
| G6 | P1 cover, credits, genres, seasons, transcript, markers | Not P0 | #157/#171 and ADR 0030 gates | Product/Catalog |
| G7 | No live Web/CMP consumer; Kotlin wire generation and drift check not implemented | A client could drift from the strict schema | Adopt the shared fixtures above; drift check before claiming wire acceptance | #158/#159 |
| G8 | Playback delivery is composition-gated | Listening cannot be accepted end to end | #10/#11 and ADR 0032 | Publication |

## E. Client integration rules (from the above)

1. Treat the unit list as "listenable now". Do not synthesize a withdrawn row; derive
   `unavailable` only from the evidence in ADR 0035 §2.
2. Keep `limit` and the filter constant for the life of a cursor. Changing either, or switching
   the Work, discards the cursor.
3. Retry a failed page with the same cursor until it expires. On `invalid_cursor`, keep the rows
   already rendered, offer a restart from page one and de-duplicate by `unit_id`.
4. Continue until `next_cursor` is `null`; an empty page with a cursor is not the end.
5. Show `position` only as order. Show `duration_ms` as given; use a placeholder, never `0`.
6. On a changed `publication_id` for a known `unit_id`, treat it as new exact audio; offer, never
   force, a switch, and never replace bytes under active playback.
7. Never show "playing" before engine acknowledgement and never mark completion from a seek.
