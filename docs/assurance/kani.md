# Bounded proofs for critical API kernels

## Recorded decision and scope

Ordered assurance commit 7 selects three small production kernels after their
ordinary exact-edge and fixed-seed property tests passed. Full machine-width
revision and byte-offset domains cannot be enumerated practically; mistakes can
corrupt immutable sequence or byte admission. Kani checks those local propositions,
not SQL authority or a complete media pipeline. No production dependency, default
Rust upgrade, migration, wire change or alternate orchestration script is introduced.

This is the opt-in verifier spike requested by the assurance plan. The underlying
primitive commands below are execution provenance, **not** an implemented Cantos
xtask capability. Ordered commit 10 owns scoped xtask integration and prerequisite /
status / preflight handling. Existing Theatre harnesses remain outside this campaign.

## Executed model

Executed on 2026-10-13, x86_64-unknown-linux-gnu (`usize` = 64 bits), API library,
no default features. Kani **0.68.0**, bundled compiler
`rustc 1.100.0-nightly (8925ea358 2026-08-20)` (toolchain nightly-2026-08-21),
CBMC **6.11.0**, SAT solver **CaDiCaL 3.0.0**. The production build still uses Rust **1.87.0**.
Compatibility was checked against the [official pinned release](https://github.com/model-checking/kani/releases/tag/kani-0.68.0),
[installation guide](https://model-checking.github.io/kani/install-guide.html),
[attributes](https://model-checking.github.io/kani/reference/attributes.html) and
[cover documentation](https://model-checking.github.io/kani/faq.html).
The installer and bundle live in isolated task tooling. The registry installer
requires a newer compiler than 1.87; it was bootstrapped with Kani's own matching
nightly rather than changing the workspace default.

Source base is published commit `c945e29bcb52843b545251956a6b72536486aba9`, plus
only the verifier modules and manifest cfg declaration in ordered commit 7.
The following full-file SHA-256 identities bind the executed source (including
harnesses). These identities bind those sources, not all transitive types,
dependencies, build configuration or verifier inputs. Recheck those inputs and
rerun after a rebase or relevant dependency/toolchain change:

| Source | SHA-256 |
| --- | --- |
| `services/cantos-api/src/contexts/catalog/domain/revision.rs` | `9e31570514f79fda5ad9ac4dae4e04158deae3cf490edeb32e3489407fcd4f9f` |
| `services/cantos-api/src/contexts/media/domain/wav_inspection/chunks.rs` | `9507128e47b0d638e7e864e1850a699dd88a009bfb1f2c3b059df1ad2bae6db8` |
| `services/cantos-api/src/contexts/media/domain/lifecycle.rs` | `2f7b823fe1e73170ce0e6587477d075dc51babc14cf2a0afe00d799d5ccd9ddb` |

`Cargo.lock` before and after execution was SHA-256
`1967688497c7d1cf4a62bbfa79ab8b7cc2646bd619cb82649831051a336ab7e8`.
Kani 0.68.0 does not expose Cargo's `--locked` switch; this equality is the
observed lock-preservation evidence, not a claim of a locked verifier invocation.

All inputs below are independent full-width symbolic integers. **No assumptions,
no stubs, no input exclusions.** Each harness and invocation sets unwind **2**;
there are no business loops/recursion, symbolic allocations or event traces in
these kernels. Default overflow, safety and unwinding checks remain enabled.
The wider integer oracle is a specification, not a copied checked-arithmetic
implementation. Each harness calls the real production symbols directly.

| Harness / production reach | Proposition and independent oracle | Actual result / reachability |
| --- | --- | --- |
| `catalog_revision_admission_and_successor`; `Revision::{new,get,next}` retained by Catalog snapshots | Full `u64` input: factory accepts exactly `1..=i64::MAX`; successor is exact widened `u128(value)+1` below the storage maximum, otherwise exactly `RevisionLimit`. Raw invalid admission is exactly `InvalidRevision`. | **PASS / bounded-model-checked**: 0 of 100 checks failed, 4 unreachable; **6/6** covers: zero, first, max−1, max, max+1, u64 maximum. |
| `wav_payload_range_is_checked`; `payload_range` consumed by `ChunkFacts::payload` | Full `u64` offset, `u32` size, `usize` length: range exists iff widened `u128(offset)+8+size` fits u64 and buffer length; exact endpoints and ordered in-buffer range. | **PASS / bounded-model-checked**: 0 of 20 checks failed; **7/7** covers: nonempty, empty, exact end, header overflow, payload overflow, buffer overrun, maximum payload fitting. |
| `submission_window_admission_and_membership`; `SubmissionWindow::{new,contains}` retained by submission create/restore and consumed by rendition restore / admission | Full independent `i64` creation, expiry, current time: accept exactly nonnegative creation and positive `i128(expiry)-i128(creation)` <= independently pinned 86_400_000 ms (24h); membership exactly `[creation,expiry)`. Exact rejection is `InvalidExpiry`. | **PASS / bounded-model-checked**: 0 of 10 checks failed, 1 unreachable; **8/8** covers: negative creation, subtraction overflow, zero, overlong, maximum lifetime, inclusive creation, exclusive expiry, expiry at i64 maximum. |

The unreachable domain assertions are intentional catch-all error branches:
`Revision::new` returns only `InvalidRevision`, `next` only `RevisionLimit`,
and `SubmissionWindow::new` only `InvalidExpiry`. No meaningful acceptance or
rejection class is unreachable; every named cover was satisfied. Catalog's other
two unreachable checks are standard-library allocation-layout overflow and format
literal shift checks on fixed assertion formatting, not excluded business inputs.
There were no counterexamples or failed unwinding obligations.

## Exact execution and retained output

From the repository root, with the isolated Kani environment loaded:

```sh
CARGO_BUILD_JOBS=2 cargo kani -p cantos-api --lib --no-default-features \
  --harness catalog_revision_admission_and_successor --unwind 2 --target-dir target/phase2-kani
CARGO_BUILD_JOBS=2 cargo kani -p cantos-api --lib --no-default-features \
  --harness wav_payload_range_is_checked --unwind 2 --target-dir target/phase2-kani
CARGO_BUILD_JOBS=2 cargo kani -p cantos-api --lib --no-default-features \
  --harness submission_window_admission_and_membership --unwind 2 --target-dir target/phase2-kani
```

Each run discovered/executed exactly one harness and reported
`Complete - 1 successfully verified harnesses, 0 failures, 1 total.`
Raw verifier output is retained outside Git in the task's verification artifacts:

| Output basename | SHA-256 |
| --- | --- |
| `phase2-kani-catalog.log` | `b3427d89f9513429d56988b4adf43cf61388de6f84ff0d0a5ba7596de2f9b012` |
| `phase2-kani-wav.log` | `8b38d9537ba098465707380da308dc9d785437742cc4f30fc2e66849c37cfeb7` |
| `phase2-kani-window.log` | `cdd0c4a1f86a047b39159826456519f571f561430e87376a9db0b2b41d69c071` |

## Post-rebase reexecution

The phase-end rebase onto fresh `develop`
`4066c462cb3e13bb1738d5ccaed19a08acc1f788` preserved the three kernel/harness
source identities above. Transitive inputs were rechecked and **all three exact
commands were rerun**: catalog 0/100 failed and 6/6 covers; WAV 0/20 failed and
7/7 covers; window 0/10 failed and 8/8 covers. The same expected unreachable
checks and verifier/compiler/solver configuration remain. No unwind-check
disabling flag was used in either campaign.

The merged service-runtime change adds xtask TOML/hash dependencies to the
workspace lock. That upstream change is retained; it was not a verifier lock
mutation. Rebased `Cargo.lock` before/after all three reruns is SHA-256
`8cf5d58a6be8481148dae8daf9abec112d6e4ce72740eea56bd87f23108a8e6c`.
The initial execution identities/results above remain historical evidence.

| Reexecution output basename | SHA-256 |
| --- | --- |
| `phase2-rebased-kani-catalog.log` | `82efbfd494a3600bc83119afb8a5908a7e07b6af0088865335a8906ad0aab14e` |
| `phase2-rebased-kani-wav.log` | `92f60496c8c3853a4aa547a6064471de722c4c930959db70ad75a2740b83e936` |
| `phase2-rebased-kani-window.log` | `0aa74a853536784620dc194586aabf50dd1d98c8f035600f450e492dd16c46f0` |

The final fresh fetch subsequently advanced `develop` to
`782793dcee86b10211e6090b419a8e6fcba967fb` (diagnostics/routing documentation).
A second clean rebase preserved all 18 current skills and routing. The upstream
delta contains no Rust source, Cargo manifest/lock or `.cargo` changes; those
model inputs and the three full-file source hashes remain identical to the
executed post-rebase campaign above. Its results are retained with that exact
source/configuration binding; no additional model-checker run is claimed merely
for the docs-only base advance.

## Remaining acceptance and limitations

These are three local bounded-model-checked propositions. Private typestate,
ordinary examples/property tests and these runs are distinct evidence. This does
not establish SQL clock/current authorization, transactions/constraints, revocation,
producer rights, digest authenticity, whole WAV/PCM parsing, actual audio quality,
publication, DBSP application, retry delivery or concurrency. The range proof is
for a 64-bit target, not a proof of 32-bit/WASM arithmetic (ordinary target builds
remain separate evidence). Feature-gated DBSP is not in this verifier build.

Real disposable PostgreSQL acceptance remains required for the changed adapters
and lifecycle transactions. This executor denied PostgreSQL's Unix-socket startup
with EPERM, including the reviewed escalation; those integration checks are
**NOT_RUN / IO_ACCEPTANCE_PENDING**, never satisfied by this proof campaign.
Run `cargo xtask verify postgres` in a permitted disposable environment and accept
only its actual migration/constraint/transaction/replay/corruption results.
Owner: unassigned — assignment required before integration acceptance.
No remote CI, browser, device, provider, publication or named listening result is
claimed here. Ordered commit 10 must add the scoped verifier gate before advertising
`cargo xtask verify kani`; ordered commit 11 owns the final consolidated ledger.
