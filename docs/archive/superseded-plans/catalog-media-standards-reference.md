# Catalog / Media standards reference for the artwork MVP

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/catalog-media-standards-reference.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

Status: proposed implementation reference, not feature evidence. Checked on
2026-10-13 UTC for [issue #104](https://github.com/loveoverflowcom/cantos/issues/104).
The current user separately requested a small API/DB MVP plan and a subsequent implementation
session; the issue's research-only boundary does not itself authorize implementation.

The active production compiler is now pinned separately in
[`rust-toolchain.toml`](../../../rust-toolchain.toml); the dated source inspection below
retains its original Rust baseline. Keep image 0.25.8 and existing decoder semantics locked.

## Use of the sources

These are semantic references. They do not make RSS/XML the Cantos protocol, storage format,
identifier authority or runtime. Cantos keeps its existing Work/ContentUnit identities, JSON
compatibility and Protobuf-native direction. No feed, XML reader, syndication, archive or RSS
endpoint is proposed. Requirements in an external format are normative for that format;
our selected Cantos fields and bounds are product choices, not claims of format compliance.

| Source and classification | Version / status checked | Relevant semantics and Cantos decision |
| --- | --- | --- |
| [RSS Advisory Board specification](https://www.rssboard.org/rss-specification), format authority | Current 2.0.11, published 2009-03-30; no RSS 3.0 | Channel/item distinction, opaque item identity, enclosure MIME/length and publication dates inform existing Work/unit/media/release boundaries. Preserve Cantos IDs and millisecond timestamps; do not copy feed-specific URL fields or RSS image size limits. |
| [RSS Best Practices Profile](https://www.rssboard.org/rss-profile), interoperability guidance | 1.0, published 2007-10-15 | Clear identity, metadata and delivery details are useful references. This is not an additional Cantos validator or a required feed export. |
| [RFC 5005](https://www.rfc-editor.org/rfc/rfc5005), feed protocol reference | Published RFC, 2007 | Snapshot/current/archive consistency is informative. Preserve existing keyset/cursor semantics; no RSS paging implementation. |
| [Canonical Podcast Namespace 1.0](https://podcasting2.org/docs/podcast-namespace/1.0) and [source repository](https://github.com/Podcastindex-org/podcast-namespace), namespace authority | Namespace URI 1.0; roadmap Phase 8 is open | Roadmap phase and each tag's stability are separate facts. Formalized and finalized have different definitions. Do not label every Phase 8 concept frozen. |
| [Image](https://podcasting2.org/docs/podcast-namespace/tags/image), namespace semantic reference | Phase 8, open-phase reference; no independent finalized/frozen claim | Work/item art, alt text, dimensions, aspect and purpose are useful. Select one Work cover, required selection alt, inspected width/height and derived aspect. Cantos stores opaque asset refs, never a caller-provided href. Additional purposes, motion and art directions are deferred. |
| [Images](https://podcasting2.org/docs/podcast-namespace/tags/images), historical namespace reference | Deprecated, also marked deprecated in the canonical 1.0 index and source README | Do not base a new collection, transport or thumbnail schema on podcast:images. |
| [Medium](https://podcasting2.org/docs/podcast-namespace/tags/medium), namespace authority | Phase 4 adopted semantic reference | Audiobook can consist of book chapters; medium describes content form rather than subject/category. Preserve novel/story/podcast WorkKind; add no audiobook enum or delivery-format field without a consumer. |
| [Person](https://podcasting2.org/docs/podcast-namespace/tags/person), namespace authority | Phase 2 adopted semantic reference | Work and episode credits can carry roles/groups. Episode credits replace the Work list in this format; do not casually copy that inheritance policy into Cantos. Contributor identity, credit publication and permissions need a separate slice. |
| [Season](https://podcasting2.org/docs/podcast-namespace/tags/season) / [Episode](https://podcasting2.org/docs/podcast-namespace/tags/episode), namespace authority | Phase 2 adopted semantic references | Season uses an integer; episode numbering may be decimal, with a display label. Neither is Cantos unit.position. No numbering fields are selected now. |
| [Chapters](https://podcasting2.org/docs/podcast-namespace/tags/chapters), namespace authority | Phase 1 adopted semantic reference | References a separate timed chapter file for one audio item. This is distinct from Catalog chapters/episodes. Defer markers and any timeline object; later bind them to an immutable rendition and its duration. |
| [Transcript](https://podcasting2.org/docs/podcast-namespace/tags/transcript), namespace authority | Phase 1 adopted semantic reference | Transcript/caption files have MIME and optional language/role metadata. A public transcript needs its own asset and publication permission. It is not the nullable private Script IR or canonical manuscript. Defer it. |
| [Alternate enclosure](https://podcasting2.org/docs/podcast-namespace/tags/alternate-enclosure), [Source](https://podcasting2.org/docs/podcast-namespace/tags/source), [Integrity](https://podcasting2.org/docs/podcast-namespace/tags/integrity), namespace authorities | Phase 3 adopted semantic references | Alternative media locations/formats and integrity information are separate from identity. Integrity supports SRI or PGP-signature semantics, not a claim that a supplied checksum proves bytes. Keep Cantos actual-byte SHA-256 verification and existing Opus profiles; no AAC, SRI, PGP or alternative transport implementation. |
| [GUID](https://podcasting2.org/docs/podcast-namespace/tags/guid), namespace authority | Phase 3 adopted semantic reference | Feed GUID is assigned once and retained across moves, with feed-specific UUIDv5 rules. Cantos has no feed URL; do not derive or replace existing UUIDs using those rules. |
| [License](https://podcasting2.org/docs/podcast-namespace/tags/license), namespace authority | Phase 3 adopted semantic reference | Declared licenses describe audio/video use; custom licenses have a legal-language link. This does not grant distribution rights, make visibility a license or clear an uploaded image. Preserve explicit publication authority; no new public-license field now. |
| [Podcast XSD](https://github.com/Podcastindex-org/podcast-namespace/blob/main/podcast.xsd), machine-readable reference | Main-branch XSD inspected; not a complete/current substitute for individual tag docs | Useful as corroboration only. No XSD dependency or XML validation is selected. The separate schema-guidance link in #104 could not be fetched through either canonical GitHub or raw-source route during this check; its contents are not asserted. |
| [Apple feed requirements](https://podcasters.apple.com/support/823-podcast-requirements) / [validation](https://podcasters.apple.com/support/829-validate-your-podcast), platform interoperability | Current pages checked, not a Cantos protocol authority | Stable identity, delivery and artwork checks are informative. No Apple submission, artwork dimensions, codec fallback or broad Opus compatibility claim is implied. |
| [image 0.25.8](https://docs.rs/image/0.25.8/image/) and [pinned manifest](https://github.com/image-rs/image/blob/v0.25.8/Cargo.toml), implementation-library primary sources | Version 0.25.8 declares Rust 1.85.0; Cantos stays on 1.87.0 | Candidate selected for PNG/JPEG decode only, with default features disabled. Lock transitive dependencies under 1.87.0 and recheck security advisories before adoption. No upgrade to the production toolchain or image encoder is needed. |
| [Pinned PNG decoder source](https://github.com/image-rs/image/blob/v0.25.8/src/codecs/png.rs), implementation primary source | 0.25.8 source inspected | Construct with limits before reading PNG metadata; late set_limits does not retroactively bound all internal PNG allocation. Reject APNG, decode the entire selected frame and retain encoded bytes unchanged. |
| [Pinned JPEG decoder source](https://github.com/image-rs/image/blob/v0.25.8/src/codecs/jpeg/decoder.rs) / [Limits](https://docs.rs/image/0.25.8/image/struct.Limits.html), implementation primary sources | 0.25.8 source inspected | Pass only an already byte-bounded buffer; check dimensions/output allocation before decoding. No orientation transformation, metadata stripping or blanket hard-RSS-memory claim. Verify codec-specific limit behavior with hostile fixtures. |

All check dates above mean 2026-10-13 UTC. The namespace site and main-branch source are mutable;
recheck them when a deferred concept is selected. The tagged image-library source is pinned.
Audiobookshelf links in #104 are optional reference implementations; no selected field depends
on them, so they were not used as standards authority or copied into a Cantos schema.

## Separations the implementation must retain

1. Work chapter/episode is a Catalog identity; timed audio chapters are a separate future timeline.
2. Script IR is a private, optional editorial asset; a public transcript is separately approved.
3. Work visibility is editorial metadata; publication clearance controls distribution.
4. Caller dimensions, digest, MIME, provenance or rights claims are not byte/rights proof.
5. Asset identity/checksum persists; provider location is internal and signed URLs are transient.
6. Existing Catalog routes remain JSON-only at the inspected base. The merged Protobuf pilot
   covers only four revision/validation routes. The new artwork metadata contract is a bounded
   extension, not a claim that Catalog, Theatre, identity or audio already use Protobuf.

See the [implementation plan](012-private-work-artwork.md) and
[separate-session handoff](../handoffs/catalog-media-mvp-ultra-handoff.md).
