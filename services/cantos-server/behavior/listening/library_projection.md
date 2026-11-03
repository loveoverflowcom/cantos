# Listener Library projection: accepted Work-level expectation

Lifecycle: **draft**. Authority: [ADR 0057](../../../../docs/decisions/0057-listener-library-work-projection.md),
accepted by the owner on 2026-11-03, with the [projection design](../../../../docs/architecture/listener-library-projection.md)
and the [decision register](../../../../docs/product/listener-library.md#decision-register). The
requirements are accepted; the model stays draft because no slice implements them yet, and the
current code still follows the 1.2.0 part of the
[Library v1 contract](../../contracts/theatre/library/v1/README.md). It is an expectation for
L02–L06, not a description of `develop`.

| Requirement source | Implementation fact at `1b8864d6` | Finite assumption | Unmodeled / next owner |
| --- | --- | --- | --- |
| D-L02: a shown Saved row always has published Work metadata; audio availability is separate | Admission already uses `catalog::public_work`; presentation uses the audio representative and can be `null` for a metadata-only Work (source-inferred, NOT_RUN) | One Work; metadata visibility is one flag; any listenable Unit makes it `Playable` | Exact DTO, cover and count fields; Turso batch query; Web/CMP rendering |
| D-L03: the Continue anchor is the latest register of the Work, completed or not | `row_number() … pick=1 AND completion='in_progress'` drops a Work whose latest chapter is completed (`latest_completed_release_suppresses_older_unfinished_work_…`) | At most one register per Unit; a new release replaces it in the model | Two keys per Unit after an explicit new-release choice; ordering ties |
| D-L03: next is the immediately following published Unit, never a skip | No implementation | Smallest published position after the anchor; four Units | Podcast ordering (D-L07); restricted Units (D-L09) |
| D-L04: caught-up and waiting-for-audio Works leave Continue and return when playable | No implementation | Membership is a pure read-time function | Cursor frontier when a Work re-enters mid-walk; UI badges |
| Hidden anchor fails closed with no older-chapter fallback | Current HIDE predicate before keyset/LIMIT | Work and Unit gates collapse into `visibleUnit` | Shared/restricted viewers |
| D-L05: checkpoint or seek to the end is not completion | `mark_completed` is explicit; checkpoints never complete | Position is only "at end" or not | Engine natural-end acknowledgement policy; auto-next |
| D-L08: no clear-history use case; history changes only on an acknowledged start; unsave keeps progress/history | Clear route and epoch still implemented until L06 removes them | History is one latest Unit per Work | Account-level data export/deletion; per-Work dismiss |
| Access loss retains account registers | Implemented: replacement/withdrawal retain owned progress | Catalog/Media facts change without touching registers | Revocation of cached client projections |

Every property has a named run, a reachable witness and one typechecking negative control. Three
controls deliberately encode today's implemented behavior (`saved-presentable`,
`continue-membership`) or a plausible regression (`saved-admission` gated on audio), so a model
PASS records that the proposed rule differs from `develop` rather than accepting either.

Out of scope: revisions, device sequences, receipts, operation replay, cursors and paging,
history epochs (removed by D-L08), bookmarks, guests, multiple accounts or Works, Web journal/CMP
outbox, real time, bytes and sessions. Those keep their native/HTTP/client oracles. No pure-core
observer is registered; `--conformance` does not apply. A future `work_next_action` core can
reuse the named-run IDs for native tests and, after a reviewed protocol change, an observer.

## Retired IDs

D-L08 removed the clear action. Retired with no successor meaning, never reused:
`cantos.listening.library-projection.run.clear-history` and
`cantos.listening.library-projection.witness.cleared-keeps-continue` (both existed only in the
unaccepted draft of PR #700). `run.unsave-retains` and `witness.unsaved-keeps-continue` are new
IDs for the Unsave half of `property.registers-independent`, whose control now mutates Unsave.
