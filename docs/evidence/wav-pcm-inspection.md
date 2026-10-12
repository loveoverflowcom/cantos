# WAV PCM inspection evidence

Review boundary: [issue #21](https://github.com/loveoverflowcom/cantos/issues/21), a standalone,
read-only `inspect-wav` CLI that reports WAV integer PCM structure, checksums and sample peak as
JSON. It is groundwork for [#9](https://github.com/loveoverflowcom/cantos/issues/9), not mixing or
QC, and it neither completes nor unblocks #9. Base: `297f6fd` on `develop`.

Delivers: a pure inspection module, the CLI, code-built good and defect fixtures, CLI tests and
an independent differential oracle. Does not deliver: conversion, resampling or normalization;
float, compressed, RF64/BW64 or RIFX decoding; loudness, true peak or clipping verdicts; audio
profiles or numeric targets; QC records, workers, providers, storage, Studio UI or publication.
Script IR, adaptation, the web editor and migrations are untouched.

## Invariant and ownership

The report is a pure function of the input bytes. Every reachable structural error is listed with
a stable code, declared sizes are compared with the bytes present and never drive an allocation,
and frame count, duration and sample peak exist only when the structure is valid. A damaged file
therefore never yields a number that reads like a measurement of the whole file.

[`wav_inspection/`](../../services/cantos-api/src/contexts/synthesis/domain/wav_inspection/mod.rs) has no HTTP, SQL, Script IR,
adaptation, provider, clock or file-system dependency. [`inspect-wav`](../../services/cantos-api/src/bin/inspect-wav.rs)
is the imperative shell: argument parsing, a bounded read, JSON output and exit codes. The module
lives in the existing `cantos-server` package beside `validate-script` and reuses its `sha2`,
`serde` and `serde_json` dependencies; no crate, dependency or `Cargo.lock` change was needed.
The only edit to an existing source file is `pub mod wav_inspection;` in `lib.rs`.

No ADR: the slice selects no audio tool, profile, delivery format or numeric target, which remain
open for #9's recorded decision after listening tests. #9 may adopt, extend or replace this probe.

## Report contract `cantos-wav-inspection-1`

| Field | Meaning |
| --- | --- |
| `inspector` | Report layout version; it changes whenever a field or code meaning changes |
| `file` | Byte length and lowercase hex SHA-256 of the whole file |
| `structure` | `valid` when `errors` is empty, otherwise `invalid`; structural only, never a QC verdict |
| `errors` | Every structural error in detection order, each `{ "code": …, …details }` with file byte offsets |
| `warnings` | Deviations that leave every byte accounted for; they do not make the file invalid |
| `chunks` | Complete chunks in file order: identifier, header offset and declared size |
| `fmt` | `fmt ` fields exactly as declared (even when inconsistent), with the extensible header when present |
| `data` | The first complete `data` payload: first-sample offset, byte length and its own SHA-256 |
| `measurements` | Valid files only: `frames`, `duration_seconds` and `sample_peak` (`null` when there are no frames) |
| `not_assessed` | Always `integrated_loudness`, `true_peak`, `clipping`, `listening_quality`, `publication_readiness` |

`sample_peak` reports per channel the largest magnitude of a stored code, its dBFS level and the
first frame that reaches it, plus the overall maximum. The 0 dBFS reference `full_scale` is the
magnitude of the container's most negative code (128, 32768, 8388608 or 2147483648); 8-bit PCM is
unsigned around 128. Digital silence has magnitude 0 and `dbfs: null`. Sample peak is not true
(inter-sample) peak. `duration_seconds` is a correctly rounded quotient; `frames` and
`sample_rate_hz` remain the exact facts. Extensible files with fewer valid bits are measured
against their container width.

Accepted encodings are `WAVE_FORMAT_PCM` and `WAVE_FORMAT_EXTENSIBLE` with the PCM subformat, at
8, 16, 24 or 32 bits per sample. Error codes: `truncated_riff_header`, `not_riff`,
`unsupported_container`, `not_wave`, `riff_size_too_small`, `riff_size_exceeds_file`,
`trailing_bytes`, `truncated_chunk_header`, `invalid_chunk_id`, `chunk_overrun`,
`missing_pad_byte`, `chunk_limit_exceeded`, `duplicate_chunk`, `missing_chunk`,
`data_before_fmt`, `fmt_too_short`, `invalid_extension_size`, `unsupported_format_tag`,
`unsupported_subformat`, `zero_channels`, `zero_sample_rate`, `unsupported_bits_per_sample`,
`invalid_valid_bits`, `block_align_mismatch`, `byte_rate_mismatch` and `partial_frame`. The only
warning, `unpadded_final_chunk`, covers an odd-sized final chunk whose pad byte is absent while the
RIFF size also excludes it; Python's standard `wave` module writes odd-sized data this way. A pad
byte lost to truncation remains the `missing_pad_byte` error. Missing chunks are claimed only
after the walk covered the whole RIFF body.

Limits: the chunk table holds at most 1,024 chunks, and the CLI refuses inputs above
`--max-bytes` (default 1 GiB, at most the 4 GiB + 8 bytes a RIFF size can describe) before
reading them. Exit codes: `0` valid, `1` invalid (report still printed), `2` usage error,
unreadable or non-regular input, size limit or output failure (message on stderr, no report).

## Claim and oracle ledger

| Claim / failure mode | Cheapest adequate oracle | Evidence |
| --- | --- | --- |
| Reference bytes, metadata, checksums and peaks are exact | Builder output equals Python `wave` bytes (SHA-256 pinned from `sha256sum`); golden JSON with 1e-12 float tolerance | differentially-tested |
| Peak magnitude, first frame and duration follow the signal | 1 kHz sine at 48 kHz peaks at frame 12; 20·log10(0.5) from Python `math` | example-tested |
| Each width decodes signs and the 8-bit center | Extremes at 8/16/24/32 bits; 24-bit −1 sign extension | example-tested |
| Metadata chunks change the file checksum, not the data checksum or measurements | Extensible 20-in-24-bit file with JUNK/LIST versus its plain twin | example-tested |
| Each defect yields exactly its errors and no measurement | 36 code-built defect fixtures with exact error lists, including several defects at once | example-tested |
| The evaluator never stops at the first defect | One file with trailing bytes, a duplicate `data` and a byte-rate mismatch | example-tested |
| Declared sizes cannot force allocation or out-of-bounds reads | `u32::MAX` RIFF and `data` sizes on a 76-byte file; every strict prefix of two valid files; every byte of the reference XORed with three masks | example-tested (bounded exhaustive); not fuzz-tested |
| Typed accessors and CLI output agree with the library report | Accessor-versus-JSON comparison; CLI stdout parsed and compared with `inspect_wav` | example-tested |
| The CLI refuses bad usage and oversized or unreadable input without a report | Missing, duplicate and invalid options, missing file, directory, limit boundaries | example-tested |
| Arbitrary stdlib-written PCM agrees with independent readers | Seeded files from Python `wave`; Python integer decoding and hashlib; ffprobe metadata | differentially-tested (local ffprobe) |
| Assertions kill injected defects | `cargo mutants` over the module and CLI | mutation-tested (see record) |

## Verification record

Environment: Ubuntu 26.04.1 LTS on Linux 7.0.0-38 x86_64, Rust 1.87.0, Python 3.14.4, ffprobe
8.0.1, cargo-mutants 27.1.0. Fixtures are synthetic and generated at test time; no recording,
provider audio or WAV file is committed.

| Result | Exact command | Scope |
| --- | --- | --- |
| pass | `cargo +1.87.0 fmt --all -- --check` | formatting |
| pass | `cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings` | lint |
| 133 passed, 0 failed, 40 ignored | `cargo +1.87.0 test --workspace --locked` | all Rust tests; 14 in `tests/wav_inspection.rs`; the 40 ignored are the existing PostgreSQL suites, not run here |
| 5 of 5 files agree, ffprobe present | `cargo build --locked --bin inspect-wav`, then `python3 scripts/reference_wav_inspection.py` | 8/16/24/32-bit, 1–6 channels, seed 20261010 |
| 214 mutants: 189 caught, 4 missed, 21 unviable | `RUSTUP_TOOLCHAIN=1.87.0 cargo mutants --file 'apps/server/src/wav_inspection/*.rs' --file apps/server/src/bin/inspect-wav.rs -p cantos-server --jobs 4 --timeout 180 -- --test wav_inspection` | module and CLI |
| 2 of 2 caught | the same command with `--re 'replace > with >= in (pcm_layout\|dbfs)'` after adding the zero-channel and typed-silence assertions | re-run of two survivors |
| pass | `python3 scripts/check_repository.py`, `python3 -m unittest discover -s scripts -p 'test_*.py'` (68 tests), `git diff --check` | repository checks |

The two remaining survivors are not killed by tests. `data.offset < fmt.offset` → `<=` is
equivalent because two complete chunks never share an offset. `take(max_bytes + 1)` → `take(max_bytes)`
changes only a file that grows between the size check and the read, a race no test drives.
Mutation logs stay under the ignored `artifacts/wav-inspection/`.

## Not covered / residual risk

- No real drama, TTS or provider audio was inspected; evidence uses synthetic signals only.
  Nothing here is `audio-measured` loudness or `audio-listened`.
- No coverage-guided fuzzing ran; it needs a nightly toolchain and an ADR. The bounded prefix and
  byte-flip sweeps are robustness examples, not a fuzz campaign.
- Reading is in memory: peak memory is about the file size, bounded by `--max-bytes`. A file that
  grows between the size check and the read is refused only after up to `max-bytes + 1` bytes are
  read; that race is not tested.
- `dbfs` uses the platform `log10`; results may differ in the last bits across platforms, so
  consumers should compare with a tolerance or use the exact `magnitude` and `full_scale`.
- The strict policy (trailing bytes, duplicate `data`, data before `fmt ` are errors; only the
  consistently sized unpadded final chunk is a warning) may reject files other readers accept.
  #9 should confirm it against the formats real providers and tools emit.
- The report is not yet bound to a QC record, artifact identifier, profile revision or approval.
