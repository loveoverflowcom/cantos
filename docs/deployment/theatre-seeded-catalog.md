# Theatre development catalog seeding

## Prepared sample dataset

The explicit `cargo xtask seed theatre --sample-input ... --sample-media-source ...
--sample-root ...` lane imports the prepared [#427 sample](https://github.com/loveoverflowcom/cantos/issues/427)
into a new, isolated Server-owned Turso file. It uses the current Server registry and existing
Studio catalog and artwork HTTP flows. It is a local developer fixture, never an automatic
build, verification, preflight, CI or production startup step.

The dataset contains exactly six fictional Works and two draft chapters per Work:

- Bến Sương
- Người Giữ Đèn
- Thành Phố Ngủ Quên
- Khu Vườn Trên Mây
- Chuyến Tàu Cuối Cùng
- Mùa Gió Qua Nhà

The six provided PNG files are admitted as private Work artwork. Each original must match its
prepared SHA-256/byte-length mapping and decode successfully before import. The source JSON
contains twelve `.invalid` MP3 references, with `is_mock: true`, `file_exists: false` and unknown
duration, hash and length. These are retained input metadata, never accepted audio assets.
Import creates no audio submission, rendition, review, clearance, approval or release.

The Server's current catalog fields hold title, synopsis, language, visibility and chapter
metadata. The manifest retains the complete input, including author, narrator, genre, tags and
mock references. The original JSON is also retained verbatim as a Server source record, so
this metadata is present in Turso without adding production catalog/wire fields.

## Prepare the input outside Git

Use the authorized Drive connection to download the complete source files from the
[issue's folder](https://drive.google.com/drive/folders/1jCgnXSwbRKGZKZBrEYhVyp7h3fePIfbe).
Keep the original JSON, package and PNG bytes outside every Git checkout. Retain
`checksums.json` from the metadata package for provenance (the importer pins these six
checksums in its source), and arrange the six
PNG files at the JSON's `covers/<sample-key>.png` paths. Do not replace them with placeholders
or use temporary Drive thumbnail/download URLs as runtime URLs.

```text
<source>/
  sample-catalog.json
  checksums.json
  covers/
    ben-suong.png
    nguoi-giu-den.png
    thanh-pho-ngu-quen.png
    khu-vuon-tren-may.png
    chuyen-tau-cuoi-cung.png
    mua-gio-qua-nha.png
```

The source directory and output directory must be separate. Choose an absolute output path
outside Git, with an existing parent directory. The importer refuses an unowned existing DB or
an output directory overlapping the input/media source. It never opens Identity's file or a
configured production database.

## Create, reopen and reseed

Run from the intended checkout with Rust 1.98.1. The same command performs initial import and
idempotent reseeding; use the identical source input and media directory for replay.

```sh
cargo xtask seed theatre --help

cargo xtask seed theatre \
  --sample-input /absolute/sample-source/sample-catalog.json \
  --sample-media-source /absolute/sample-source \
  --sample-root /absolute/sample-output
```

The Server-owned `sample_catalog_turso` integration target's exact ignored `sample_catalog`
test is the implementation primitive. It validates the sources, opens the pinned in-process
Turso engine, drives the real Studio handlers, closes the owner and reopens the file. Stable
Work/chapter IDs and operation receipts allow an interrupted or repeated import to reconcile
the same dataset instead of creating duplicates. A different input fingerprint is refused
instead of silently changing an accepted fixture.

Private per-Work `image-attempts/*.json` journals bind the input, cover and exact prepare
request before any HTTP effect. Attempt zero preserves the original sample IDs. An admitted
asset is reused even after its intent expires or its upload response was lost. Only a Server
observation of the exact expired intent with no admitted asset permits another durably recorded
attempt, with distinct intent and prepare-operation IDs. Recovery is bounded to eight attempts
per Work; exhaustion or inconsistent journal/manifest bindings stop with an exact recovery error
and preserve the output. They never trigger an automatic reset. Unknown transport outcomes stop
the invocation with the same attempt retained; a later invocation reconciles only after the old
host and DB owner have closed. No production TTL, admission rule or immutable receipt is changed.

Source copies and ownership markers are fully written and synced under unique same-directory
staging names before atomic no-clobber publication. The output root appears only with a complete
ownership marker. Partial writes never publish truncated final files. Retries start from the
original validated bytes, retain unknown prior staging, and refuse conflicting final files or
unmarked existing roots. Do not delete staging by a filename glob; partial sibling initialization
directories are preserved separately from the marked output and are not adopted automatically.

The output directory contains:

| Path | Purpose |
| --- | --- |
| `server.db` and engine sidecars | Current Server registry/schema and accepted private sample metadata; one owner process at a time |
| `sample-owner.json` | Dataset/input/source/output binding used to refuse adoption or unsafe reset |
| `manifest.json` | Input mapping, stable IDs, cover integrity and explicit mock/unavailable audio facts |
| `image-attempts/` | Private, input-bound prepare/admission recovery journals; completed assets stay pinned |
| `storage.toml`, `media/objects/` | Filesystem storage target and immutable private objects, reached through the existing storage registry |
| `creator.json`, `studio-token` | Local fixture actor and a short-lived development token; keep private and out of logs/Git |
| `sample.db` | Engine-supported verified snapshot, independently reopened through the Server registry |
| `sample-catalog.json`, `covers/` | Preserved input JSON and six original covers |

To reproduce in another clean output directory, run the same command with a new
`--sample-root`. This executes the current registry/import again rather than copying a live
main file or inventing a SQLite-compatible schema. The actual output DB is the artifact; no
SQL dump is required. Never open it with Python `sqlite3` as a substitute for target-engine
acceptance. Export/copy only after the Server owner has closed, or through the shared store's
supported verified snapshot procedure; preserve required sidecars and private media.

The fixture checks Server reads and authenticated artwork delivery through its real router,
including missing/corrupt object failures and anonymous denial. This is distinct from a
long-running Server executable, browser session, CMP runtime or device acceptance. Keep each
result and command in the handoff evidence; this runbook does not claim those executions.

To keep the verified sample open for local HTTP inspection, add `--sample-serve` to the same
command. It serves the real Server router and Store on `127.0.0.1:8080` until Ctrl-C, refusing
an occupied port. This is a test-hosted development composition, with private development
session material in `studio-token`; never print or share that file. It starts no Identity and
does not provide a browser login workflow. Stop the host before reseeding, reset, operator
access or any other process that opens this DB.

## Reset without deleting evidence

Stop any process owning the sample DB first. Reset is explicit and limited to the marked
sample output:

```sh
cargo xtask seed theatre \
  --sample-root /absolute/sample-output \
  --reset
```

Reset verifies the owner binding and acquires both the runtime and snapshot DB owner locks. It refuses an active owner,
unmarked/foreign output or an output within Git. The importer prevents source/output
overlap when establishing the marker. It archives the entire output
as a sibling `<sample-output>.retired-<timestamp>-<pid>` directory instead of deleting DB/WAL,
media, credentials or evidence. Re-run the create command to start a fresh sample output.
Reset needs only the sample output path; it does not require the original source files.
Do not remove the archived copy until a separate cleanup decision establishes retention and
confirms that no needed data remains there.

## Public catalog and client boundary

These Works remain private drafts. The Theatre discovery contract requires an eligible active
release, so nonexistent MP3 bytes cannot make six listener cards or playable chapters. The
fixture must observe their absence from anonymous discovery and refusal of anonymous private
artwork reads. No whole private-media root is served, and no host path, `file://` URL or private
storage key is added to a client contract.

[ADR 0033](../decisions/0033-release-pinned-public-cover.md) is **accepted** (#157): an owner's
digest-bound disclosure, Discovery v1.2 `presentation=2` and an origin cover route. Its
implementation belongs to [#171](https://github.com/loveoverflowcom/cantos/issues/171); the
sample's private covers stay private until a release pins them and the owner discloses each. Private image admission and public
Work visibility cannot stand in for that decision. Public cover delivery and real Web/CMP
observations remain separate acceptance work; do not add a hardcoded frontend catalog or
weaken publication/QC/auth gates to display this fixture.

## Future media-only S3 migration

Turso continues to own catalog metadata, revisions, receipts and logical image identity. A
future separately authorized migration copies media bytes through the existing
`storage_target`/`object_key`/integrity ports into a configured S3 target, verifies actual
SHA-256 and length, then changes location through the owning storage workflow. Keep original
target resolution and old bytes until new-target reads and recovery are accepted. A new-write
target switch alone does not migrate historical objects. Never upload `server.db` to S3 or
persist a presigned URL as media identity. No cloud credentials, live S3 writes or local media
cleanup are part of this sample lane.

## Retained synthetic publication lane

Without the sample flags, `cargo xtask seed theatre [--reset]` retains its existing native
development composition. Its `catalog_http` target is absent, so ordinary seeding refuses
before setup. The existing `catalog_http_turso` publication campaign is a different synthetic
fixture, including valid fixture audio; it is not the six/twelve prepared dataset and is not
selected as a replacement. `--environment` and `--test-cluster` remain retired refusals.

[Original PostgreSQL seeding record](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/theatre-seeded-catalog.md)
retains historical synthetic dataset/journal/recovery provenance. It is not a runnable native
workflow or authority to restore PostgreSQL services.
