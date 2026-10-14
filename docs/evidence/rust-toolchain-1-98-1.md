# Rust 1.98.1 compatibility evidence

## Review boundary and source

[Issue 125](https://github.com/loveoverflowcom/cantos/issues/125) is delivered as the P0 production
compiler upgrade plus necessary lint compatibility, not a hot-path optimization or edition migration.
Source baseline is freshly fetched `develop` **d9cff230e4b584b48bfdbbcf7855aa78eb1414c0**; the issue
branch starts directly there, not on the earlier issue 120 feature branch. Historical results and
existing refs are retained. The [active compiler policy](../architecture/rust-toolchain.md) and
[work queue](../work-plan/README.md#compiler-compatibility-review-boundary) keep optional candidates
and independent platform/provider acceptance separate.

The execution host is the dot cloud computer, **x86_64-unknown-linux-gnu**, not a native aarch64/macOS host.
Original isolated baseline was `rustc 1.87.0 (17067e9ac 2025-05-09)` and
`cargo 1.87.0 (99624be96 2025-05-06)`. The separately installed requested production compiler is:

- `rustc 1.98.1 (48a229cea 2026-09-01)`, LLVM **22.1.8**;
- `cargo 1.98.1 (797e8a9bc 2026-08-05)`;
- `clippy 0.1.98 (48a229ceae 2026-09-01)`;
- `rustfmt 1.9.0-stable (48a229ceae 2026-09-01)`;
- native x86_64 GNU and `wasm32-unknown-unknown` installed;
- Trunk **0.21.14**, matching wasm-bindgen **0.2.100**;
- Node **24.19.0**, npm **11.9.0**, integrity-locked Quint **0.33.0**.

`rustup show active-toolchain` with no environment override selects the repository pin.
`cargo metadata --locked --format-version 1` accepts all seven patch-qualified MSRVs as
**1.98.1** and all editions as **2021**. Exact pin and minimum supported version are the same
compiler, not a claim of compatibility with 1.98.0 or older Cargo. `Cargo.lock` stays SHA-256
**678a4b669deb3edcc8595f09684a3e8c569f8df94060e108446d5ca094900303**.
No dependency, wire/schema, authorization, media identity, production-provider or CMP state changes.

The official [1.98.1 release](https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/) and distribution
manifest were checked. Manifest SHA-256 is
`a7c8774a5fd8441c997d94c029776cbc5eb111e9d72ab5d256fa69866644347e`;
Linux compiler/Cargo/Clippy/rustfmt, WASM standard library and aarch64/macOS compiler artifacts are
available. Artifact availability is not a macOS build/run. The approved isolated official Trunk
archive matches upstream SHA-256
`f2b4680cd239693a646a2795e4633c625328d7b2a044fbe749fa3a2fe9e7036b`.
The native xtask ELF `.comment` actually names **LLD 22.1.8** and rustc 1.98.1; no linker-speed
claim or macOS extrapolation follows.

## Executed checks

Full logs stay in ignored `artifacts/agent-runs/issue125-toolchain/`; they are local-only,
not uploaded acceptance artifacts. Commands run from the repository root with the task's isolated
Cargo/rustup/Trunk/cache selection. Rust/private-input tests use an approved task-only
`TMPDIR=/dev/shm/cantos-issue125-tmp` and `RUST_TEST_THREADS=1`; no test is filtered or safety
rule relaxed. Web-only compilation uses an ordinary task-owned temporary directory and an
isolated `XDG_CACHE_HOME`, never a global configuration change.

| Command / claim | Result | Verified scope / gap |
| --- | --- | --- |
| `cargo xtask doctor` | PASS exit, mixed capability rows | Exact compiler/tools observed; missing infrastructure remains UNSUPPORTED, not an executed gate |
| `cargo metadata --locked --format-version 1` | PASS | Seven exact MSRVs/editions; unchanged lock |
| `cargo test --locked -p xtask` | PASS, 158 tests | Pin/components/target, exact doctor token, workflow selection, all existing xtask safety/process tests |
| `cargo xtask ci --job repository` | PASS | Static service manifest, repository/Python hygiene and independent Script IR reference |
| `cargo xtask verify rust` | PASS | rust-format, workspace rust-lint and rust-tests; 1,433 passed, 0 failed, 293 explicitly ignored DB/media/observer cases |
| `cargo xtask verify behavior --setup --conformance` | PASS | Locked model plus independent pure-core private-admission observation, not DB/API/storage/device acceptance |
| `cargo xtask ci --job web` | PASS | WASM lint, actual Studio/Theatre Trunk distributions and gallery/preview build |
| `cargo xtask ci --job rust` | PASS, all seven rows | Format, Protobuf oracle, workspace lint/tests, DBSP, WAV oracle and actual audio inspection |
| `cargo xtask verify dbsp` | PASS | Existing experimental feature lint/tests, no crate upgrade or production consumer change |
| `cargo xtask verify contracts` | PASS, all five rows | Script IR / Protobuf references, contract tests, WAV reference and actual audio inspection |
| `cargo xtask ui-preview leptos verify` | PASS, all three rows | 64 native assertions, warning-denying preview WASM lint and actual gallery build |
| `cargo xtask ci --job postgres` | UNSUPPORTED | initdb/pg_ctl/psql absent; no disposable PostgreSQL execution |
| `cargo xtask verify kani` | UNSUPPORTED, all seven | Kani 0.68.0 / matching nightly / CBMC 6.11.0 bundle absent; no fresh proof |
| `cargo xtask preflight --changed-only --dry-run` | NOT_RUN plan | Toolchain/CI changes broaden selection to every ordinary gate; the changed observer selects behavior |
| `cargo xtask preflight --changed-only` | UNSUPPORTED | All 18 gates selected; PostgreSQL and two Gradle/CMP gates unsupported, other 15 correctly NOT_RUN by prerequisite precheck |
| `cargo xtask preflight --changed-only --skip postgres --skip cmp-ui-preview --skip theatre-cmp-demo` | FAIL overall | 14 PASS, 3 explicitly skipped NOT_RUN, mutation FAIL at its clean-baseline 60-second timeout; zero mutants executed |

The new fixed-array/default/parity/view-expression compatibility changes passed the existing
warning-denying gates. Added regressions preserve no-default generic payloads and exact WAV
complete-frame/earliest-tie values. Existing hex/cursor, golden wire, malformed media and
HTTP/private-public transport tests retain their independent assertions. Source review found no
changed contract or newly reachable malformed-input path.

The Rust CI's DBSP-feature run independently reports **656 passed / 0 failed / 293 ignored**
across 44 suite summaries, with the existing pinned `dbsp = "=0.198.0"`. It does not restore
the retired job-summary consumer. The WAV oracle compares all five owned PCM fixtures against
Python `wave`/`hashlib` and real ffprobe. The dedicated audio-inspection gate executes both
previously ignored actual ffmpeg-generated intake tests: four accepted mono Ogg Opus profiles
with exact byte identity and typed refusals for wrong codec/container/channel/profile/corruption.
These local owned fixtures are not live provider/storage or SQL evidence.

## Quint evidence scope

Retained fresh bundle:
`target/verification/behavior/cantos.media.private-audio-admission-d87bffdfa54d439ca1f0f3b10022cd47`.
The model has PASS typecheck, named scenarios, bounded sampled simulation, required witnesses,
negative controls and freshness. Independent conformance has PASS tool/config/source identity,
**31 positive traces / 265 checkpoints**, seven model-mutant mismatches, two injected-observer-fault
mismatches and final freshness. Its compiler and source identities bind the actual 1.98.1 tools
and pin file. This is **pure-core-synthetic-shell** evidence only. The capability remains draft;
solver/exhaustive proof and DB/API/auth/media/storage/client/device acceptance are NOT_RUN.
It does not model-check publication or listening capabilities that have no executable model.

## Standalone API availability probe

The following owned probe was compiled with `rustc --edition 2021` and ran successfully on the
exact compiler. It is not a Cantos regression, benchmark or new repository command surface.

```rust
fn main() {
    let values = [1, 2, 3, 4];
    assert_eq!(values.array_windows::<2>().count(), 3);
    assert_eq!(
        values.subslice_range(&values[1..3]).map(|r| (r.start, r.end)),
        Some((1, 3))
    );
    let text = "abcde";
    assert_eq!(text.substr_range(&text[1..4]).map(|r| (r.start, r.end)), Some((1, 4)));
    let mut buffer = core::fmt::NumBuffer::<u64>::new();
    assert_eq!(u64::MAX.format_into(&mut buffer), "18446744073709551615");
    std::assert_matches!(Some(1), Some(1));
    assert_eq!(1_u64.strict_add(2), 3);
    let guarded = match Some(1_u64) {
        Some(value) if let Some(next) = value.checked_add(1) => next,
        _ => 0,
    };
    assert_eq!(guarded, 2);
}
```

A separate owned let-chain probe intentionally failed to compile on edition 2021 with the
compiler's explicit edition-2024 requirement; the same isolated probe compiled/ran on edition
2024. The repository stayed on 2021, and no hot-path candidate was adopted from either probe.

## Diagnosed failures and residual acceptance

Earlier failed runs are retained rather than rewritten:

- The first new pin test used TOML value parsing for a complete document; corrected to the
  existing document parser, then all pin tests passed.
- A normal sandbox cannot write the task directory in `/dev/shm`; `/tmp` has a Git ancestor,
  correctly rejected by private-input tests. The approved outside-Git temporary directory
  allowed all 158 tests to run without bypassing that check. The earlier serialized run with
  a nonexistent temp root had 152 PASS / 6 fixture/process ENOENT failures, all retained.
  One earlier parallel port-probe observation also failed; its exact transient cause was not
  established. The probe source is unchanged and the approved serialized/full suites passed.
- New compiler/Clippy diagnostics required the documented minimal compatibility adaptations.
  No warning-denying gate was disabled; later full Rust verification passed.
- Trunk's first build compiled Studio WASM but failed creating its default read-only cache.
  A task-owned XDG cache resolved that environmental failure; the full Web lane then passed.
- The available-gate preflight reaches a genuine **mutation FAIL**: its disposable clean
  `cargo test -p cantos-api --offline --locked --test script_ir` baseline exceeds the existing
  hardcoded 60-second deadline. Zero mutants execute; the precise compile/resource cause is
  not established. Ordinary Script IR tests and the independent reference passed, but do not
  substitute for mutation acceptance. No deadline, test count or target-isolation rule is
  relaxed, and no mutation success is claimed. A directly related configuration fix copies
  the production pin into the outside-repository mutation workspace, with a byte-identity
  fixture regression; this does not resolve or rerun the timed-out heavy gate.
- `proc-macro-error2 2.0.1` emits a future-incompatibility warning about private `proc_macro`
  re-export (upstream Rust issue 127909). It does not fail Rust 1.98.1; dependency versions remain
  locked. Review an upstream-compatible dependency update separately before any later compiler migration.

Before full issue acceptance, an unassigned owner must run the missing native aarch64/macOS,
PostgreSQL, seven fresh Kani proofs and any applicable CMP/platform/browser-runtime acceptance.
The mutation baseline failure also requires diagnosis and an actual successful
`cargo xtask verify mutation`; it remains FAIL, not a skipped gate or accepted core proof.
Use existing xtask commands and the separately pinned verifier model; no installation of Kani,
PostgreSQL service, production credentials, paid live upload or provider call is implied here.
Production identity/storage/delivery acceptance remains independently gated.

Clean/incremental compiler/link benchmark baselines and hot-path allocation/latency/parser
benchmarks are NOT_RUN. No performance optimization is proposed, and the candidate table records
why those code paths remain unchanged. A compiler release announcement, cached build duration,
LLD string or successful asset build is not a throughput benchmark.
A draft PR and user-requested issue closure are submission bookkeeping, not merge, deployment or
complete issue/production acceptance; remaining acceptance belongs to the PR checklist.
