# Production Brief v1 — Implemented development contract

[ADR0018](../../../../../../docs/decisions/0018-external-audio-production.md) owns external production;
[ADR0019](../../../../../../docs/decisions/0019-content-identity.md) owns Content identity. This backend
exports instructions; its local identity/HTTP host remains development-only. Deployment TLS,
credential delivery and production identity acceptance remain separate.

## Resources and routes

| Method | Current Studio path after `/api/v1/studio` | Authorization |
| --- | --- | --- |
| POST | `/content-units/{unit}/production-briefs` | Active Work owner, Studio session |
| GET | `/content-units/{unit}/production-briefs/{version}` | Active Work owner, Studio session |
| POST | `/content-units/{unit}/production-briefs/{version}/producer-grants` | Active Work owner, Studio session |
| DELETE | `/content-units/{unit}/producer-grants/{grant}` | Active Work owner, Studio session |
| GET | `/producer/content-units/{unit}/production-briefs/{version}` | Independent exact read bearer grant |

JSON only, private `no-store` responses. Creation returns201; owner read and producer export
return200; idempotent revocation returns204. None of these routes is mounted in the deprecated
alias, Theatre or Admin. Studio mutations require the configured Origin. Producer GET requires
one `Authorization: Bearer …`; cookies alone never authorize it. Existing session tokens are
refused at grant issuance. Work readers/editors do not gain source/script access or brief export.

## Immutable document

The [raw DTOs](../../../../src/contract/studio/briefs.rs) and [schema](briefs.schema.json) describe version1.
Raw DTOs are not authorization or integrity witnesses. Pure domain admission constructs private
Brief IDs/digests, guidance and producer scopes; storage re-admits every snapshot, compares its
relational pins and digest, and rechecks exact owning Content/source/script records on read.

A Brief pins its own UUID and monotonically increasing per-unit version, Work identity/owner/
category/title, Work rights set and `pb-rights1`, exact ContentRevision UUID/full `cc-r1`,
content details/provenance/rights and the copied instruction body. Its creation actor/time and
all guidance are immutable. Version1 `pb-r1` hashes UTF-8 canonical JSON (recursively sorted
object keys, ordered arrays, no floats), prefixed by `cantos/pb-r1\n`; the digest field is empty
while hashing. Guidance is NFC/LF; raw originals and Script IR exports remain unchanged.

Guidance separates four concerns:

- Editorial intent: narration, recording or audio drama and editorial notes.
- Optional voice/pronunciation/casting and sound design: copied directions with explicit
  positive revisions. Lexicon/cast arrays are bounded and reject duplicate normalized targets.
- Required producer capabilities: a closed explicit set. Grant issuance checks the producer's
  declared capabilities; these claims establish neither tool availability nor measured quality.
- Delivery: explicit profile, permitted use, attribution, naming and QA instructions.

Profile IDs are exactly `opus-mono-20-v1`, `opus-mono-24-v1`, `opus-mono-32-v1` and
`opus-mono-40-v1`; no default, AAC fallback or arbitrary version. They retain the profiles from
PR69's engineering envelope: Ogg Opus mono,48kHz decoder timeline,32MiB/one-hour maximum and120%
nominal average coded-payload ceiling. Brief export does not inspect bytes, certify quality or
settle Theatre playback compatibility. PR69 retains its own runtime evidence and migration0010.

## Body resolution and rights

Text and recording work without Script IR, casting, voice/model IDs or a provider. A Text
body copies admitted normalized manuscript text. A recording body contains no synthetic text.
A Source body copies the validated exact import receipt, extraction/outcome, extractor version
and receipt digest; source labels/cues are inspectable source material, not automatic speech.
Failed extraction and legacy text-only sources without an import receipt block Source briefs.
Owners can import the original or append an explicit Text revision instead. Copied source
receipts and the whole Brief are bounded to256KiB; oversized input fails without writing a head.

A Script body copies the exact accepted export through Editorial's owner-scoped entrypoint.
Its effective line languages and inline pronunciation stay authoritative; wrapper language
does not override them. Optional Text/Script source references are revalidated as provenance,
never concatenated into speech. No content/latest or legacy Catalog link is resolved on delivery.

Rights references, attribution and permitted-use text are declarations. They grant no legal
clearance, editorial approval, QC, upload permission or publication authority. Old planning
settings/casting/approvals are never implicitly used as current Brief guidance.

## Current comparison, retries and permissions

Owner reads preserve historical documents and return `current`, `content_changed`,
`rights_changed`, `guidance_changed` or `archived`. Comparison uses exact Content ID/`cc-r1`,
current Work rights set, latest Brief version and Work/unit lifecycle. Content provenance edits
stale a Brief even if `cc-c1` stays equal. A new guidance/profile declaration creates a new Brief.
Old records never retarget. Unrelated Work title/order changes do not change content authority.
An exact create replay recovers its original document and reports current freshness.

Creation serializes through the Server writer and current Catalog facts. Per-owner operation IDs bind normalized
requests and exact unit pins. Contradictory replay or duplicate IDs conflict; competing writers
cannot both advance the same expected version. Brief+head commit atomically. SQL rejects settled
snapshot/grant/revocation UPDATE/DELETE and replacement/upsert; correction means a new immutable record.

Owner-supplied producer tokens contain32 randomly generated bytes represented as64 lowercase
hex characters. Only a domain-separated SHA-256 hash is persisted. The caller retains its token;
no API receipt or error returns it. Grant permission is exactly `read_instructions`, bound to
Work/unit/Brief ID/digest, issued for at most24hours and permanently revocable. Its creation retry
returns the original scope even after expiry/revocation; this grants no renewed authority.

Producer export requires current active owner, matching scope, non-revocation, unexpired time
and current Brief inputs. The same-unit current-authority checks and serialized writer order it against mutations and
revocation; time is rechecked immediately before commit. Expiry/revocation return401, wrong
unit/owner returns404 and stale inputs return409 `stale_production_inputs`. Invalid guidance/
profile/capability returns422 with a safe rule; version conflict uses409 `stale_revision`.
No error includes private content, SQL, bearer hashes or storage paths.

## Compatibility and next consumer

Native migrations 13 and 21 own Briefs, heads and producer grants/revocations; the Server
uses its existing private single-file owner, not deployment-role grants. The
[native mapping](../../../../../../docs/architecture/server-turso/remaining-pg-retirement.md)
records constraints, receipts and tests. Original PG migration bytes/checksums remain historical.

AudioLifecycle must consume exact content/brief/profile pins plus current authorization and
rights comparison. Read grants do not authorize intake or publication. Full uploads, external
storage, review/release, TTS/mixing, UI and production credentials are outside this contract.
Local evidence and independent review are recorded in
[Production Brief evidence](../../../../../../docs/evidence/production-brief.md).
