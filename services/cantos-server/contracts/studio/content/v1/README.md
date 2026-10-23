# Studio Content Identity v1

This implemented JSON API adds immutable content beneath existing Catalog Work/ContentUnit
identity. [ADR 0019](../../../../../../docs/decisions/0019-content-identity.md) owns the authority split.
Shared raw DTOs: [content.rs](../../../../src/contract/studio/content.rs); [JSON Schema](content.schema.json);
[synthetic fixtures](fixtures/README.md). It creates no audio or production brief.

## Routes

Only `/api/v1/studio` mounts the following, with the existing Studio cookie and Origin policy.
The deprecated alias, Theatre and Admin gain no endpoints. JSON-only negotiation, 128 KiB body
limit, `no-store`, `nosniff` and shared error envelopes match [Catalog](../../catalog/v1/README.md).

| Method / path below Studio | Input | Output |
| --- | --- | --- |
| POST `/content-units/{unit_id}/content/revisions` | AppendContentRequest | 201 AppendContentResponse |
| GET `/content-units/{unit_id}/content/head` | — | ContentRevisionResponse; 404 when absent |
| GET `/content-units/{unit_id}/content/revisions/{revision_id}` | exact UUID | ContentRevisionResponse |
| GET `/content-units/{unit_id}/content/history` | `after_revision=0`, `limit=20` defaults | ContentHistoryResponse |

History accepts limit 1–50 and a nonnegative signed-64-bit `after_revision`, orders ascending
sequence, omits text and reference bodies, and returns `next_after` or null. New appends can
appear on later pages; existing sequences never move. Current access is checked per page.
There is no edit, delete, review, approval, upload or publication endpoint for a revision.

## Content body and declarations

Append requires a new canonical lowercase UUID `id`, actor-scoped UUID `operation_id`,
`expected_revision` (content sequence, 0 for first append), positive `expected_work_revision`
and `details`. Sequence counters stay within PostgreSQL signed 64-bit range. No caller may
supply owner, timestamps, lifecycle, recorded actor, digests or current head. Unknown fields
are rejected, including fields on the empty recording variant.

Details contain `language`, `body`, `rights_refs` and a nonblank `provenance_note`:

| Body kind | Required content | Optional binding |
| --- | --- | --- |
| `recording` | no manuscript or script | none; audio admission is separate |
| `text` | nonblank `text` | `source` |
| `source` | exact preserved `source: {id, sha256}` | none |
| `script` | exact `script: {id, revision, export_digest}` | `source` |

Text is capped at 65,536 UTF-8 bytes before and after NFC normalization. CRLF/CR becomes LF;
other whitespace is preserved. Controls except LF/tab are refused. Source bytes stay byte-exact.
Language follows the existing bounded Catalog BCP-47 spelling. Provenance is at most 2,048
UTF-8 bytes, NFC, without control characters. It records caller-declared authorship/import/
adaptation/recording facts; it does not prove them. At most 64 distinct rights reference tokens
are sorted; their owner-bound evidence records must exist. Empty rights remain explicitly
uncleared. Neither the note nor the references prove permission to produce or publish.

Source admission rechecks the authoritative checksum and receipt under the Work owner. Script
admission rechecks the exact owned accepted export and its `sir-e1` digest. No latest pointer
is accepted. An additional source on text/script records provenance; it is not an assertion
that derived text or Script IR faithfully covers that source. Adaptation coverage still belongs
to the existing review workflow.

## Identity, hashing and lifecycle

ContentRevision UUID identity is minted by the caller shell, never derived from text, digest,
title or position. Each unit has its own sequence starting at 1. Metadata edits/reorder/archive
leave immutable content intact. Appending content advances the Work metadata guard and Catalog
epoch atomically; unit metadata revisions do not advance. Archived Work/unit refuses a new
append, while historical authorized reads and exact retries remain available.

Both schemes use SHA-256 over a domain prefix plus canonical UTF-8 JSON: recursively sorted
object keys, compact separators, no floats, explicit null source in text/script, normalized
text/note and sorted rights refs. Semantic body variant and language remain explicit.

- `cc-c1:sha256:<hex>` hashes `b"cantos/cc-c1\n"` plus `{body, language}`. Body bindings include
  exact source checksum and complete ScriptRef export identity. This is a conservative content
  binding digest, independent of the Script IR performed-content/speech digest.
- `cc-r1:sha256:<hex>` hashes `b"cantos/cc-r1\n"` plus the complete response object **without**
  `revision_digest`: identity, parent/unit, sequence, details, `content_digest`, actor and time.
  Rights or attribution changes alter it even when `cc-c1` stays equal.

Stored DTOs are reparsed through domain constructors; recomputed digests and relational IDs,
sequence and actor must agree. The head foreign key binds unit, revision ID and sequence.
UPDATE/DELETE/TRUNCATE of revisions are rejected by PostgreSQL; corrections append.

## Authorization, retries and errors

Owner reads/writes; reader reads only; editor appends with unchanged rights declarations.
Rights changes require owner: on first append compare with current Work rights; subsequently
compare with previous ContentRevision rights. The immutable receipt preserves that original
requirement for replay. Strangers receive 404; no access is inferred from public visibility or
membership of a linked script. Work membership grants no separate source/script endpoint access.

Writes lock the existing Catalog clock, recheck session/actor after waits, authorize the Work,
check current rights privileges, and recheck session before commit. Exact retry returns the
original content and original `work_revision`, even when heads have moved. Normalized command
identity includes target, supplied revision ID, both expected counters and complete normalized
details; equivalent NFC/line endings and rights ordering replay. Operation IDs share the
Catalog actor namespace; different intent/endpoint yields 409 `operation_reused`.

Stale Work or content sequence yields 409 `stale_revision` with the conflicting sequence;
clients distinguish by their supplied guards and may read both current heads. Duplicate ID or
archived target uses `catalog_conflict`. Invalid semantics use 422 `invalid_catalog` (one exact
issue); unavailable/foreign/mismatched evidence uses 422 `evidence_unavailable`. Malformed DTO,
query or operation UUID uses 400 `invalid_request`. Other access/representation/storage errors
follow Catalog, including 500 `corrupt_revision` and 503 `unavailable`. Failed writes leave
content head, metadata history, receipt and epoch unchanged.

New briefs must pin full `cc-r1` revision identity, not the head, legacy Catalog links or equal
`cc-c1` alone. Existing Script IR 0.1.0 and `sir-c1`/`sir-e1` schemes are unchanged.
