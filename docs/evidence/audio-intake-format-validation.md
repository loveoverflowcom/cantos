# Private Studio audio intake: scope and local evidence

> Historical PR69 authoring evidence. The Mac integration reruns its executable tests; see [Audio Lifecycle evidence](audio-lifecycle.md). Original NOT_RUN results below remain attributable to that original environment.

- Date: 2026-10-12.
- Starting revision: `024065a72245e713d846f50fa9dcdb1b373b89a3`, freshly fetched `origin/develop`
  after the external-production refactor (PR #65).
- Publication base: `b620109d93f7ced11628ce380c282ccf7a9c351c`, freshly fetched when publication
  resumed after Catalog PR #68 merged. The earlier rebase preserved Theatre PRs #61/#63; the
  final integration preserves Catalog routes, contracts, schema and SQL checksums.
- Branch: `feat/audio-intake-format-validation`; isolated development worktree, no edits to the
  separate Theatre PR worktree.
- Request: [#64](https://github.com/loveoverflowcom/cantos/issues/64), narrowed by the current
  owner request to explicit selection among **four Opus 20/24/32/40 kbps profiles**, with
  actual-byte format admission before storage. AAC acceptance and implicit defaults are excluded.
- Implementation contract: [Studio owner audio intake v1](../../packages/contracts/studio-audio-intake-v1.md).

## Invariant and boundary

An owner-session upload receives an immutable, private acceptance receipt only after its
actual bytes match the owner-pinned script revision/content digest and canonical profile,
pass bounded Ogg/Opus structure and decoder inspection, and exist durably in private storage.
Technical acceptance is not rights clearance, human-listened QC or release approval.

The implementation is a **development-only owner Studio API slice**, not the proposed external
producer handoff. A configured private filesystem adapter stores media bytes outside PostgreSQL;
PostgreSQL atomically records immutable intents and receipts. Binding audio intake to existing Catalog/episode identities, frozen production briefs, scoped
producer credentials, S3 grants, publication and listener delivery remain separate work. No backend synthesis, encoding, transcoding or mixing was added.

## Versioned admission envelope

The canonical IDs are `opus-mono-20-v1`, `opus-mono-24-v1`, `opus-mono-32-v1` and
`opus-mono-40-v1`. There is no default and no caller-authored policy override. This implementation
supports one Ogg Opus stream, mono, decoder sample rate 48 kHz, at most 32 MiB and one hour.
The upper average coded-payload bitrate is 120% of the nominal encoder target; no lower floor
is imposed on silence/VBR. Ogg framing/tag overhead is excluded. These size, duration and VBR
limits are explicit engineering admission choices, not listening/interoperability findings
or final quality policy from [#51](https://github.com/loveoverflowcom/cantos/issues/51).

Ogg framing checks include CRC, sequence/serial/continuation, exactly one BOS-to-EOS stream,
finite packets, Opus packet frame duration and consistent page granules. Audible duration is
EOS granule minus Opus pre-skip at 48 kHz. Packet-duration disagreement above 125 ms is rejected.
`ffprobe` reads actual stream and packet facts; `ffmpeg` decodes to the null sink solely to reject
bad compressed payloads. Both processes use fixed arguments, one decoder thread, an Ogg-only
format allowlist and file-only input protocol, private random quarantine paths, bounded output,
CPU/memory/open-file limits, process-group cleanup and timeouts. No external playlist/container
is accepted. Raw filenames and MIME labels cannot establish acceptance.

## Checks actually run in this cloud workspace

| Check | Result | Meaning |
| --- | --- | --- |
| `python3 scripts/check_repository.py` | PASS | Document/JSON/link/skill/source-graph diagnostics; not Rust execution |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS | 220 Python tests against the Catalog-integrated snapshot |
| `git diff --check` | PASS | Whitespace diagnostics |
| Synthetic reference encodes, actual `ffprobe` metadata and bounded decode-null | PASS | External tool/reference evidence only |
| Correct-CRC malformed Opus payload decode-null | PASS | External decoder rejected the corrupted packet (exit 183) |
| `cargo xtask verify repository` | NOT_RUN | Cargo unavailable, command exit 127 |
| `cargo xtask verify rust` | NOT_RUN | Cargo/rustc/rustfmt/clippy unavailable |
| `cargo xtask verify contracts` | NOT_RUN | Cargo unavailable |
| `cargo xtask verify audio-inspection` | NOT_RUN | Cargo unavailable; production Rust inspector not executed |
| `cargo xtask verify postgres` | NOT_RUN | Cargo and PostgreSQL server tools unavailable |
| `cargo xtask preflight --changed-only` | NOT_RUN | Cargo unavailable; no aggregate pass claimed |
| `cargo xtask verify web` | NOT_RUN | Cargo/rustc/Trunk unavailable; exhaustive error mappings reviewed only |
| GitHub CI | Not checked | No polling, waiting, dispatch or retry was performed |

`ffmpeg` and `ffprobe` reported version `7.1.5-0+deb13u1`. Four owned two-second 440 Hz
synthetic mono signals were encoded externally at the nominal CBR settings. No narrative,
recording, model, voice asset or generated audio was committed to Git.

| Nominal profile | Whole file bytes | Measured average coded payload bps | Bounded decode-null |
| --- | ---: | ---: | --- |
| Opus 20 kbps | 5,368 | 20,135 | PASS |
| Opus 24 kbps | 6,378 | 24,161 | PASS |
| Opus 32 kbps | 8,398 | 32,215 | PASS |
| Opus 40 kbps | 10,418 | 40,269 | PASS |

These independent tool measurements establish fixture feasibility. **They do not establish that
Rust `inspect_audio`, HTTP acceptance, filesystem tests, migrations or PostgreSQL tests passed.**
No `audio-listened`, deployed object-storage or end-to-end publication evidence is claimed.

## Authored regressions and assurance gaps

Ordinary Rust tests cover the closed profile set, metadata/profile envelope, identity derivation,
empty/spoofed/oversized admission, pure prepare/replay decisions and bounded subprocess failure.
The ignored real inspector suite creates its own signals at runtime and covers all four profiles,
wrong AAC container/Vorbis codec/stereo/bitrate, truncation, changed CRC, chained streams and a
malformed coded packet despite a correct page CRC. The `audio-inspection` xtask gate owns this
suite and its tool prerequisites.

Independent review found the static-alias privacy path, which was fixed before publication.
An intermediate Python regression exposed a shared-fixture ordering issue in the new
architecture test; splitting the cases restored independent inputs. Final diagnostics checked
355 text files and 15 skills, and all 211 Python tests passed. After the develop rebase,
repository diagnostics checked 370 text files and 15 skills, and all 211 Python tests passed
again; the independent rebase review found no introduced Theatre tooling regression.

Storage tests cover descriptor-relative paths, unsafe/symlink/non-private roots, atomic no-overwrite
publication, deduplication, tampered objects and partial failures. Disposable PostgreSQL/HTTP tests
cover current ownership, session/Origin/surface isolation, exact revision/profile/operation
binding, upload rejection, retries, conflicting replay, stale revision, receipt insert failure,
backend termination and restart reconciliation. These tests were authored but remain **NOT_RUN**.

Private staging names are cleaned on normal error paths. A process crash or metadata failure may
leave private unreferenced objects; rollback never deletes a shared content-addressed object.
Reference-aware orphan retention/cleanup, crash recovery, production filesystem isolation and
S3 storage remain deferred. The audio-enabled development host disables all static fallback, including nested public-tree
symlinks. Studio must be served by a separate host with no access to private objects.
The independent read-only review found and the implementation fixed the static symlink
exposure path before publication; its GET/HEAD regression remains NOT_RUN. Exact session-clock
expiry during receipt insertion and lane-cancellation interleavings remain evidence opportunities.
The full Rust formatter, compiler, lint, aggregate and real-database evidence remains required
before this draft can be treated as a verified implementation.

## Catalog integration on publication resumption

Fresh `origin/develop` was `b620109d93f7ced11628ce380c282ccf7a9c351c`. The combined migration
registry now has eleven entries (versions 1–11). Existing 0001–0009 and Catalog 0011 SQL match
that base byte-for-byte; audio 0010 matches the previous reviewed audio snapshot. The authored
upgrade regression checks both 0009-to-combined and already-applied-0011 with the 0010 gap,
including old checksums, manuscript/revision bytes and the original Catalog work snapshot.
Those PostgreSQL regressions remain **NOT_RUN**.

Independent source review found two previously missed shared-contract integrations: the
exhaustive import message mapping needed `AudioRejected`, and the canonical Studio `ApiError`
schema needed `audio_rejected`. Both were fixed with locale/schema regressions. Catalog DTOs,
routes, errors and scoped grants remain preserved; the audio harness now uses explicit named
read grants rather than an all-table grant. The final available diagnostics passed 220 Python
tests and repository checks (404 text files, 15 skills). Rust formatter/compiler, Web and
PostgreSQL checks still could not start (missing Cargo, exit 127). No installation or remote
CI inspection was performed.
