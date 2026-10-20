# Production Rust toolchain

## Compiler and compatibility boundary

[`rust-toolchain.toml`](../../rust-toolchain.toml) owns the exact production Rust **1.98.1**
compiler pin, minimal profile, matching rustfmt/Clippy and `wasm32-unknown-unknown` target.
All seven member manifests declare the same patch-qualified MSRV. Edition **2021**, the
workspace resolver and dependency versions remain unchanged. The pin is also the minimum
supported compiler; a separate older compiler is not claimed to be supported.

From the repository root, `rustup show active-toolchain` installs/selects this configuration.
All four [CI jobs](../../.github/workflows/repository-checks.yml) use that operation before their
existing `cargo xtask ci --job ...` gate. Trunk stays **0.21.14** and is installed using the
same repository compiler rather than an unrelated moving `stable` channel. Avoid a conflicting
`RUSTUP_TOOLCHAIN` environment variable or directory override; `cargo xtask doctor` diagnoses
an exact compiler mismatch. No application, schema, wire codec, authentication, publication,
storage identity, native CMP or external-producer boundary changes with this upgrade.

The [Rust 1.98.1 release](https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/) fixes vtable
miscompilation in 1.98.0. Selecting the point release is preventive; it is not evidence that
Cantos encountered that defect. The compiler upgrade itself makes no throughput or allocation
claim. Historical run logs retain their original compiler versions.

## Independent verification tooling

[`xtask::config`](../../xtask/src/config.rs) tests the exact pin, profile/components/target and
all seven declared MSRVs/editions. Doctor checks the complete compiler version token, not a
minor-version or patch-prefix match. Workflow tests reject a moving compiler override.
The private-admission [Quint observer](../../scripts/verify_behavior_conformance.py) reads the
production pin, requires exact Cargo/rustc output, and binds the pin file to its source identity.
Wrong patch, nightly, future version and an unqualified channel cannot satisfy its guard.
The disposable Script IR mutation workspace copies the same pin, rather than depending on
an ambient rustup override outside the repository.

[Kani](../../xtask/README.md#optional-targeted-kani-proofs) is intentionally separate:
Kani 0.68.0 / CBMC 6.11.0 / bundled `nightly-2026-08-21-x86_64-unknown-linux-gnu` with
`rustc 1.100.0-nightly (8925ea358 2026-08-20)`, CaDiCaL and unwind 2. Production rustup selection
must not replace that proof compiler. The seven registered kernels remain unchanged. A production
build, doctor capability row or ordinary test does not establish a Kani proof or Quint conformance.

A toolchain/CI input selects every ordinary preflight gate. Required acceptance still includes
real PostgreSQL, both Leptos Web clients and their gallery, applicable contract/media fixtures,
DBSP's existing experimental lane and platform acceptance. Missing tools stay `UNSUPPORTED` or
explicitly skipped `NOT_RUN`; no failure is waived. Native aarch64/macOS runtime requires that
platform, not merely a downloadable distribution artifact on Linux. No paid live storage or
production credentials are needed for compiler verification.

## Required lint compatibility

Rust 1.98 Clippy flags fixed-size `chunks_exact(2)` and the manual generic `PatchField`
and `LinkPatch` defaults. The upgrade uses `as_chunks::<2>().0.iter()` in the existing hex/pair helpers,
retaining full-pair iteration and the previous discarded-remainder behavior. Forge already
validates the exact 64-character digest length; its fixed array makes the pair destructuring
infallible. `PatchField` derives the same `Missing` default without requiring a default payload;
explicit no-default-payload regressions accompany the existing omission/null/value wire
fixtures. WAV sample decoding keeps complete frames and first-tied peaks with fixed sample arrays;
a separate regression pins those values. Cursor evenness uses the same fixed-divisor predicate,
view closures keep identical Boolean expressions in tail blocks, and single-element test slices
borrow their unchanged values. These are compiler/lint compatibility adaptations, not throughput optimizations,
and no warning is globally disabled.

## Optional stable-API candidates

The P0 compiler upgrade deliberately changes no business algorithm. Candidates are a later,
separately measured review boundary, not an automatic syntax migration. The
[Rust 1.98 release](https://blog.rust-lang.org/2026/08/20/Rust-1.98.0/) describes the new formatting
and borrowed-slice helpers; a stabilized API's existence does not prove benefit in these owners.

| Candidate | Actual owner / consumer | Decision in this upgrade | Required oracle and measurement before adoption |
| --- | --- | --- | --- |
| `NumBuffer` / integer `format_into` | `services/cantos-api/src/adapters/inbound/http/audio_transport.rs`; private S3 Range requests | Unchanged: no measured formatting bottleneck | Exact 200/206/416/HEAD, ETag/If-Range and Range header tests; same bounds/cancellation/auth, allocation counts and warm latency distribution against current formatting |
| `array_windows`, `subslice_range`, Copy ranges | Forge `domain/ogg_opus.rs`; API `contexts/media/structure.rs` | Unchanged: page boundaries, CRC and typed refusals outweigh a speculative syntax gain | Differential owned Forge/API fixtures; malformed/truncated/chained/overflow cases; parser throughput/allocations with identical input and profile |
| `assert_matches!`, strict arithmetic | Protobuf scan and Catalog/Media/Listening/Production domain tests | Unchanged: existing exact typed-error assertions/checked arithmetic retain their semantics | Error-variant and unchanged-state boundaries; no panic for untrusted overflow, golden wire bytes and existing independent oracles |
| `cfg_select!`, Cargo configuration include | xtask platform and Forge adapters | Unchanged: no concrete duplication needing another configuration layer | Exact platform command/cleanup/refusal tests and unchanged orchestration ownership |
| Let chains / match guards | Edition-sensitive Rust consumers | No edition or syntax migration | Compiler-check on the actual edition; any edition 2024 drop-scope/lifetime change is a separate RFC/PR |
| `substr_range`, `strip_circumfix`, `NonZero::from_str_radix` | No selected concrete consumer | Deferred: no invariant currently requires migration | Identify an owner, semantic regression and genuine readability/performance benefit first |
| Algebraic float operations | Media/QC/duration/offset evidence | Excluded from deterministic state and evidence | Approximate numeric consumers would need a separate error contract; no reordering of deterministic values |

Standalone compiler probes on Rust 1.98.1 / edition 2021 compiled and ran `core::fmt::NumBuffer`,
integer `format_into`, `array_windows`, `subslice_range`, `substr_range`, `std::assert_matches!`,
`strict_add` and an `if let` match guard. Borrowed helpers return the new `std::range::Range`,
not the legacy `std::ops::Range`; a migration must preserve that type boundary explicitly.
An isolated let-chain probe was rejected on edition 2021 and accepted on edition 2024; this
confirms the exclusion, not a repository edition migration. These probes demonstrate API
availability/syntax only, not Cantos semantic or performance acceptance.

No Range-formatting or Ogg benchmark baseline/new result is claimed because neither code path
changes. Build/link measurements must name the target, source, profile, clean/incremental state,
warmup and sample count; a cached build timing alone is not an optimization claim. Linux LLD
and WASM linker behavior must be observed on their own targets, never generalized to macOS.

See [issue 125](https://github.com/loveoverflowcom/cantos/issues/125) for the wider candidates and
acceptance checklist. A submitted draft is not a merged, deployed or fully accepted upgrade.

The [fresh compatibility record](../evidence/rust-toolchain-1-98-1.md) records executed scope,
retained failures and remaining acceptance separately from this policy.
