# Kani proof ledger

## Theatre Web K1 inventory and feasibility (#363)

This is a source-bound inventory and feasibility record for
[#363](https://github.com/loveoverflowcom/cantos/issues/363), not a production proof campaign.
Rechecked on fetched `origin/refactor/cantos-native-turso` at
`11590218f2752c6a0f9b9ea127f75e3fd0a52807` (merged PR #371), rather than the issue's
historical audit SHA `969d492d41937f9d91084d653976eb85c9d96563`.
`Cargo.lock` SHA-256: `e29a00f17b9820a41dcd78a5544ad2fa8d6e43c9d90a32b0ee3d8bde9cc8ada3`.
K1 removes only the verifier-only module described below; the inventoried production code,
ordinary tests, fixtures and lockfile retain this source identity. Final PR evidence identifies
the resulting commit and actual check outcomes. No Theatre target in this table has an
attributable Kani execution; **K1 makes no `bounded-model-checked` claim**.

Reach describes current source wiring: `LIVE_DEFAULT` is reachable from the default Web
composition (browser effects still require wasm32), `PREVIEW_ONLY` requires the opt-in preview
feature, `TEST_ONLY` has ordinary test callers only, and `UNWIRED` has no runtime entry path.
Re-exporting a symbol or compiling it is not a runtime caller. The execution column concerns
Kani only; it does not describe whether ordinary tests ran.

| Production symbol / owner | Current caller chain | Reach | Harness | Registered | Actual Kani execution / gap |
| --- | --- | --- | --- | --- | --- |
| [`Mode::from_query`](../../apps/cantos-theatre/web/src/live/request.rs) | wasm32 [`main`](../../apps/cantos-theatre/web/src/main.rs) selects `LiveBoundary`; preview variants are feature-gated | LIVE_DEFAULT | None | No | NOT_RUN; mode selection is separate from the probe guards |
| `ProbeState::{begin,cancel,finish}`, `ProbePhase` in `live/request.rs` | Only local tests/doctest and `live/mod.rs` re-export for `ProbeState`; `ProbePhase` also appears in the uncalled browser probe | UNWIRED | **Retired** `proofs::{completion_requires_current_pending_generation,cancellation_never_wraps_or_accepts_previous_completion}` | Never registered | NOT_RUN historically and now; former unwind 2, no assumptions/stubs, zero cover goals; no user-path proof |
| `classify` in `live/request.rs` | [`live::browser::probe`](../../apps/cantos-theatre/web/src/live/browser.rs) calls it, but that probe has no caller; ordinary Rust/Kotlin fixture tests remain | UNWIRED | None (the retired harnesses exercised `ProbeState`, not `classify`) | No | NOT_RUN; correcting the audit's claim that there was no non-test caller at all |
| [`AccountState::{leave,enter,generation,fence,accepts,finish_session,begin_logout,finish_logout,observe_session_change}` / `RequestFence`](../../apps/cantos-theatre/web/src/application/account.rs) | `AccountViewState` delegates its session, invalidation and fence decisions; `AccountWriter` and [`live/runtime.rs`](../../apps/cantos-theatre/web/src/live/runtime.rs) call that aggregate | LIVE_DEFAULT; host-compilable core | None | No | NOT_RUN; current ordinary tests include `exhausted_security_invalidations_purge_without_reissuing_a_generation` |
| `AccountState::{enqueue,dispatch,acknowledge,project,projection}` in `application/account.rs` | Local `account::tests` only; live synchronization uses `LibrarySyncState` instead | TEST_ONLY | None | No | NOT_RUN; do not prioritize these unused control/data paths for K2 |
| [`AccountViewState::{begin_refresh,finish_session,fail_fenced_command,begin_logout,finish_logout,begin_library,finish_page}`](../../apps/cantos-theatre/web/src/application/account_view.rs) | [`AccountWriter`](../../apps/cantos-theatre/web/src/application/account_writer.rs) delegates transitions from `live/runtime.rs` and [`view/live_shell.rs`](../../apps/cantos-theatre/web/src/view/live_shell.rs); aggregate links account, private projection and sync | LIVE_DEFAULT | None | No | NOT_RUN; BTreeMap/Vec/String aggregate needs ordinary tests linking decisions, purge and publication, not a symbolic bulk-state claim |
| [`LibraryProjection::{begin,finish}`](../../apps/cantos-theatre/web/src/application/library.rs) | `AccountViewState::{begin_library,finish_page}` through `AccountWriter` and `live/runtime.rs::{load_library,finish_page}` | LIVE_DEFAULT | None | No | NOT_RUN; generation, cursor, consumed-cursor and presented page data remain unproved |
| [`PublicDiscoveryState::{open,begin,finish}`](../../apps/cantos-theatre/web/src/application/public_discovery.rs) | `LiveBoundary` route/search/retry/more callbacks and `view/live_shell.rs::run_discovery` | LIVE_DEFAULT | None | No | NOT_RUN; actual discovery request admission is a K2 priority, with page/row validation outside any proposed small control proof |
| `PublicDiscoveryState::cancel` in `application/public_discovery.rs` | Local `public_discovery::tests` only; no shell cleanup caller exists | TEST_ONLY | None | No | NOT_RUN; the public discovery state is live, but this method is not wired |
| [`WorkDetailState::{open,leave,finish_work,finish_page}`](../../apps/cantos-theatre/web/src/application/public_work.rs) | `LiveBoundary` route callbacks and `view/live_shell.rs::run_work_request` | LIVE_DEFAULT | None | No | NOT_RUN; `finish_page` requires `PresentedUnitPage`; keep full success/data validation in ordinary tests |
| [`ListenState::{begin,finish}`](../../apps/cantos-theatre/web/src/application/public_listen.rs) | `LiveBoundary` listen callback issues `begin`; `view/live_shell.rs::run_listen` supplies the manifest result to `finish` | LIVE_DEFAULT | None | No | NOT_RUN; manifest admission is not a small scalar proof; a future control-only candidate may cover the `Err` branch, not success delivery |
| [`AuthIntentState::{observe_route,invalidate,begin,accepts,finish}`](../../apps/cantos-theatre/web/src/application/auth_navigation.rs) | [`LiveNavigation`](../../apps/cantos-theatre/web/src/app/live_navigation.rs) observes route changes, admits auth requests and fences returns | LIVE_DEFAULT | None | No | NOT_RUN; secondary K2 epoch/control candidate, not proof of provider/session behavior |
| [`CatalogState::{begin,accept}`](../../apps/cantos-theatre/web/src/application/catalog.rs) | `CatalogSession::{load,accept}` via `CatalogSession::new(MockCatalog)` inside preview-gated `TheatreContext::new`; module itself is `cfg(feature = "preview")` | PREVIEW_ONLY | None | No | NOT_RUN; does not certify production discovery/catalog |
| [`LibrarySyncState` outbox/receipt/reconciliation](../../apps/cantos-theatre/web/src/application/library_sync.rs) | `AccountViewState` sync capabilities through `AccountWriter`; `live/runtime.rs` persists and interprets effects and receipts | LIVE_DEFAULT | None | No | NOT_RUN; BTreeMap/String/serde data needs ordinary property/example/adapter tests; no bulk-state Kani proposal |

### Probe decision: option A

Retire only the two `cfg(kani)` harnesses and their `proofs` module in `live/request.rs`.
The current caller audit confirms the probe guards have no production runtime consumer, so
registering them or adding covers would spend verification effort on an unwired kernel.
Retain `Mode`, all production probe types/methods and `classify`, ordinary transition/exhaustion
and compile-fail tests, and
[`client-boundary/v1/probe-cases.json`](../../packages/contracts/fixtures/client-boundary/v1/probe-cases.json),
which is also consumed by the Kotlin fixture generator. The historical
[NOT_RUN record](../evidence/live-client-integration.md#kani-boundary) remains explicit.
No duplicate kernel, production behavior change or new proof is introduced; source revert is
the rollback. This adopts the issue's recommended option A under the implementation request.

### Feasibility boundary on the selected Mac

| Field | Observed result or declared prerequisite |
| --- | --- |
| Selected host | `uname -sm`: `Darwin arm64`; `rustc -Vv` host: `aarch64-apple-darwin` |
| Observed normal tools | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`; these are not verifier-bundle versions |
| Existing bundle | `KANI_HOME` is unset; default `$HOME/.kani` and its `kani-0.68.0` bundle are absent. No installer proxy/version command was invoked |
| Required model, declared by the existing registry, **not observed here** | Ubuntu x86_64; Kani 0.68.0; bundled `rustc 1.100.0-nightly (8925ea358 2026-08-20)` / `nightly-2026-08-21-x86_64-unknown-linux-gnu`; CBMC 6.11.0; CaDiCaL; x86_64-unknown-linux-gnu / 64-bit usize; library, no default features, unwind 2, normal safety/unwinding checks |
| Host compatibility | **UNSUPPORTED** for the fixed Linux x86_64 model; the selected execution capability is a local Mac shell, with no approved Ubuntu host/bundle for this spike |
| Actual Theatre feasibility spike | **NOT_RUN**: both the supported host/bundle and a Theatre registration are absent; no Linux compiler attempt occurred, so there is no compiler failure to report |
| Actual Theatre xtask invocation / selector | **None / none registered.** The accepted command surface is `cargo xtask verify kani --harness` followed by an existing registered selector; the current registry cannot select Theatre or `AccountState::accepts` |
| Measurements | Theatre build/proof elapsed time and peak memory: **NOT_RUN / not measured**; no ordinary Cargo test result substitutes for them |
| Temporary feasibility changes | None: no temporary harness, registry edit or extracted crate was created |

The capability audit read [`xtask/src/kani.rs`](../../xtask/src/kani.rs),
[`scripts/verify_kani.py`](../../scripts/verify_kani.py), the bundle path and local tool versions.
`Package::{Api,Forge,Identity}`, `Kernel::ALL` (fourteen entries), the parallel Python package
allowlist, fixed cover/unreachable expectations and `affected()` have no Theatre entry.
The existing acceptance policy is
[`cargo xtask verify kani`](../../xtask/README.md#optional-targeted-kani-proofs), never raw
`cargo kani` acceptance. Ordinary registry/adapter tests validate this boundary, not any proof.

The next owner must first obtain authorization for an existing supported Ubuntu x86_64 host
and matching pinned bundle. Then a separately scoped, temporary meaningful harness calling
the production `AccountState::accepts` needs a real typed selector and matching primitive
registration before an exact xtask command can be recorded. Retain its source/lock/config
identities, actual tool versions, command/output, assertion/unwinding/covers and measured costs;
remove all temporary changes. No claim that Leptos compiles under Kani follows from the Mac
host tests. Do not extract a leaf crate unless an actual dependency failure justifies a
separate owner decision with a real consumer under ADR 0001.

K2 registry expansion remains conditional on successful feasibility and stable production
kernels: package/manifest, Python source hash roots, affected-path selection and registry/gate
tests must agree. No such expansion or new CI is part of K1; #336.4 owns the separate D1 runner.
Assignment is unassigned — required before future execution acceptance. Queue order/rationale
is unchanged. This record closes only the inventory/retirement/documentation boundary; actual
supported-host feasibility remains missing acceptance in the draft PR.

## Native Identity kernels registered, not yet run

The #334 [security-core library](../../services/cantos-identity/README.md) registers three
additional kernels in the existing xtask runner. This is registration evidence only;
the historical API campaigns below do not cover Identity.

| Selector / production reach | Model and covers | Actual evidence |
| --- | --- | --- |
| `identity-security-version` / `SecurityVersion::{new,get,successor}` | Full raw `u64`, wider arithmetic oracle, no assumptions/stubs; unwind 2, 6 covers | **PASS / bounded-model-checked on D1**: [run 39663624462](https://github.com/loveoverflowcom/cantos/actions/runs/39663624462) on `c22487a502c6587fe49415cda74c7d12239d9804` and again [run 39666812245](https://github.com/loveoverflowcom/cantos/actions/runs/39666812245) on `281964af7ab2b376cbd13831b5944db0face01ae`, both `ubuntu-24.04` x86_64 with the pinned Kani 0.68.0 bundle; 1 exact harness, 99 checks with 0 failed, 6/6 covers satisfied, lock unchanged |
| `identity-completion` / `check_completion`, checked ID/version constructors | Nonzero full-width IDs, valid storage versions, all account states/purposes/assurances; constructor-domain assumptions only, no stubs; unwind 2, 10 covers | **PASS / bounded-model-checked on D1**: the same two runs and sources; 154 checks with 0 failed, 10/10 covers satisfied, lock unchanged |
| `identity-mailbox-proof` / `ProofTime::new`, `MailboxProof::{issue,plan_confirmation,plan_cancellation,plan_replacement}` and checked bindings | Full raw `u64` clock, nonzero full-width IDs, valid versions/nonempty stored windows; constructor-domain assumptions only; no stubs; unwind 2, 16 covers | **PASS / bounded-model-checked on D1** in [run 39666812245](https://github.com/loveoverflowcom/cantos/actions/runs/39666812245) on `281964af7ab2b376cbd13831b5944db0face01ae`: 1 exact harness, 386 checks with 0 failed, 16/16 covers satisfied, lock unchanged. The earlier run 39663624462 on `c22487a5` was **FAIL** in the evidence parser, not a counterexample: the primitive refused the output with `missing or malformed property blocks`, because 9 descriptions wrap across two lines where rustfmt wrapped a long `assert_eq!` expression. A local rerun on the same bundle retained the raw output, in which the verifier itself reported `0 of 386 failed` and `16 of 16 cover properties satisfied`. The parser now accepts one quoted description that wraps, and still refuses a wrapped value that would absorb a field or block. Neither the model nor its bounds changed |

Reproducible selection: `cargo xtask verify kani --harness identity-security-version --harness identity-completion --harness identity-mailbox-proof`.
The D1 runner for this selection now exists: the manual `native-auth-qualification` job of
[`repository-checks.yml`](../../.github/workflows/repository-checks.yml) installs Kani 0.68.0
with `cargo kani setup` on GitHub Actions `ubuntu-24.04` x86_64 and runs exactly these three
kernels through the [#336 campaign](native-isolation.md#native-auth-linux-qualification-campaign)
`--kani` lane, outside the isolation guard and with the registry's model unchanged. The job's
existence is not evidence: each row stays NOT_RUN until a dispatched run's URL, source SHA and
assertion/unwinding/cover outcomes are recorded here.
Retain the existing Kani/compiler/solver/target pins, safety and unwinding checks. Acceptance
needs the actual #331 D1 GitHub Actions Ubuntu x86_64 `workflow_dispatch` run, exact source SHA,
assertion/unwinding results and all cover outcomes. macOS unit/registry tests and synthetic
verifier-bundle adapter tests are not `bounded-model-checked`; no real Turso, crypto, RPC,
consume-once or durable revocation claim follows from these kernels.

The mailbox-proof proposition concerns transition **plans**, including explicit action, exact
purpose/subject/version/operation/epoch/locator binding, inclusive issuance/exclusive expiry,
terminal consumed/cancelled snapshots and rejection of a replaced locator. Sixteen reachability
goals include the three intent classes, invalid/maximal clock input, refusal and terminal/
replacement cases. The candidate binding and verifier-derived locator are input facts; no
secret, actual HTTP request, account eligibility, clock trust, digest, transaction or receipt
is verified. A stale copied plan must still lose an atomic compare at the shared Turso owner.
The harness does not establish one-winner consumption under concurrency or unknown outcomes.

### Reconciliation with the native auth Quint observers (#106)

`identity-mailbox-proof` reaches every mailbox symbol the
[mailbox observer](../../specs/behavior/observation-protocol.md#native-identity-auth-observation-protocol-v1) calls
(`ProofTime::new`, `MailboxProof::{issue, plan_confirmation, plan_cancellation, plan_replacement}`, the checked bindings
and identifiers) and is kept unchanged. The Quint replay and that kernel are different evidence; neither implies the other.

The session-admission observer's Server decisions, `decide_issue` and `decide_admission`, have **no kernel**, and none is
added: it would not be compatible with the current lane. Both compare `Subject`/`Epoch` values that wrap `[u8; 16]`
(as does their nonzero check), and Kani lowers byte-array equality to CBMC's `memcmp`, whose 16-iteration loop cannot be
discharged at the lane's fixed `--unwind 2` with unwinding checks enabled. Diagnostic only (host Kani 0.67.0, a minimal
crate with the same `[u8; 16]` newtype, not the pinned lane and not about the production crate): `memcmp.unwind.0`
`unwinding assertion loop 0` FAILURE, `VERIFICATION:- FAILED`. Host Kani 0.67.0 cannot compile this workspace at all
(rustc 1.93 nightly against `rust-version` 1.98.1). The Identity crate's identifiers are `u128` internally, so its
registered kernels do not take this path. Covering the Server decisions needs a reviewed lane change (a per-kernel unwind
bound) or `u128` identifiers in the Server decision types; neither is part of #106. `validate_fact`,
`SecurityState::from_response` and every synthetic-shell element also have no kernel, and none is claimed.

## Recorded decision and scope

Ordered assurance commit 7 selects three small production kernels after their
ordinary exact-edge and fixed-seed property tests passed. Full machine-width
revision and byte-offset domains cannot be enumerated practically; mistakes can
corrupt immutable sequence or byte admission. Kani checks those local propositions,
not SQL authority or a complete media pipeline. No production dependency, default
Rust upgrade, migration, wire change or alternate orchestration script is introduced.

Ordered commit 7 is the requested opt-in verifier spike. Its primitive commands
below are historical execution provenance. Ordered commit 10 now implements the
scoped xtask capability described in the phase3 section; commit 11 consolidates
verified evidence. Existing Theatre harnesses remain outside this campaign.

## Executed model

Executed on 2026-10-13, x86_64-unknown-linux-gnu (`usize` = 64 bits), API library,
no default features. Kani **0.68.0**, bundled compiler
`rustc 1.100.0-nightly (8925ea358 2026-08-20)` (toolchain nightly-2026-08-21),
CBMC **6.11.0**, SAT solver **CaDiCaL 3.0.0**. The production build at that historical checkpoint
used Rust **1.87.0**; current production pins **1.98.1**. The primitive commands below are
historical provenance, superseded for repository acceptance by `cargo xtask verify kani`.
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
| `services/cantos-server/src/contexts/catalog/domain/revision.rs` | `9e31570514f79fda5ad9ac4dae4e04158deae3cf490edeb32e3489407fcd4f9f` |
| `services/cantos-server/src/contexts/media/domain/wav_inspection/chunks.rs` | `9507128e47b0d638e7e864e1850a699dd88a009bfb1f2c3b059df1ad2bae6db8` |
| `services/cantos-server/src/contexts/media/domain/lifecycle.rs` | `2f7b823fe1e73170ce0e6587477d075dc51babc14cf2a0afe00d799d5ccd9ddb` |

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
CARGO_BUILD_JOBS=2 cargo kani -p cantos-server --lib --no-default-features \
  --harness catalog_revision_admission_and_successor --unwind 2 --target-dir target/phase2-kani
CARGO_BUILD_JOBS=2 cargo kani -p cantos-server --lib --no-default-features \
  --harness wav_payload_range_is_checked --unwind 2 --target-dir target/phase2-kani
CARGO_BUILD_JOBS=2 cargo kani -p cantos-server --lib --no-default-features \
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
claimed here. The implemented workflow below runs only these registered kernels;
its success does not waive the remaining acceptance above.

## Phase 3: implemented targeted workflow and fresh-develop execution

Ordered commit 10 implements the scoped xtask capability; the primitive commands
above remain historical phase2 provenance. The current public commands are:

```sh
cargo xtask verify kani
cargo xtask verify kani --harness catalog-revision --harness wav-payload
cargo xtask preflight --changed-only --base origin/develop --kani
```

Omission now selects five registered API-library harnesses; repeated typed
selectors choose a subset. Historical Web mock harnesses remain unregistered/NOT_RUN.
Ordinary builds, `verify rust`, full preflight without `--kani`, CI and merged
service lifecycle commands do not execute model checking. Selection and outcome
rules are in [the xtask command contract](../../xtask/README.md#optional-targeted-kani-proofs).
Missing/incompatible installed tools are UNSUPPORTED; explicit skips/stopped rows
NOT_RUN; failed or incomplete proof evidence FAIL. Capability probes and mocked
parser/process tests never establish a proof.

This campaign uses the same Kani/compiler/CBMC/target/solver and symbolic integer
model specified above, with unwind 2, no assumptions/stubs and default safety /
unwinding checks. The actual fixed invocation for each registered fully qualified
harness is:

```sh
"$KANI_HOME/kani-0.68.0/bin/kani-driver" kani \
  --manifest-path "$REPO/Cargo.toml" -p cantos-server --lib --no-default-features \
  --exact --harness "$REGISTERED_FULLY_QUALIFIED_HARNESS" --unwind 2 \
  --solver cadical --output-format regular --target-dir "$REPO/target/assurance-kani"
```

`REPO` denotes the repository root, not an additional required environment key.
xtask supplies each exact name from [the typed registry](../../xtask/src/kani.rs).
KANI_HOME is resolved once to an absolute existing bundle parent. Probes and
proofs invoke the direct bundle driver in cargo-plugin mode, never the upstream
installer proxy, setup or download. Matching child-only toolchain/PATH does not
change ordinary Rust 1.87.0 or the workspace default.

Rust approves and hashes one bounded exact buffer per manifest/Cargo configuration
candidate, including absent root/ancestor/Cargo-home paths. That approved identity
is attached to each active command; Python compares the exact candidate set at
entry, after probing, immediately before proof and on every proof outcome. Each configuration
read consumes at most 1 MiB + 1 byte and refuses oversize input. Ambient model-
changing compiler/flags/wrappers and implicit Kani metadata are refused. Fresh
metadata binds Git revision/dirty/diff hashes, kernel/source/model/configuration
inputs and Cargo.lock before/after. Kani still has no --locked switch. Proof
execution has a 900s timeout and 4 MiB output limit with owned process-group cleanup.
Endpoint hashes require frozen inputs: they do not isolate against concurrent
mutate-and-restore edits, attest external dependency caches/tool binaries, or
place a total-size bound on streamed generic model-source hashing.

The final repaired workflow passed 56 focused xtask tests and 73 synthetic Python
tests; repository 298 and all 18 skills passed. Three targeted review findings were
repaired: approval freshness, relative bundle paths and unbounded Rust configuration
allocation. The paired Python total configuration consumption was capped as well.
The final read-only review found no actionable issue. Initial fake-fixture failures
and intermediate runs remain recorded separately, not erased or called proofs.

After rebasing onto develop `ef5307225abee8ee0c3e014a5bb29f5f8fa7f82e` and
reconciling PR83 Theatre conflicts, `cargo xtask verify kani` executed all three
again on clean local commit `cb7ad37ffabbf12e86a76bb1b80e060db265f75c`, exact tree
`d49c32404328b1fa9557bdd871a6a22a6a2431a3`. The published checkpoint
`f909ca7572251d3b58af751c5703e017c804ce3d` has the identical tree. Raw evidence
is retained in ignored `artifacts/agent-runs/` under these exact basenames:

| Kernel / artifact directory | Checks / satisfied covers / reviewed unreachable | raw.log SHA-256 | metadata.json SHA-256 |
| --- | --- | --- | --- |
| Catalog / `kani-8dc5aebd283c446c94ae304127cbf5a5` | PASS 100 / 6 / 4 | `8bc43be8fa049ed9664472b17440823368ce6c87e0f939f73ec8bb4126e910cc` | `1e34729fc73a4873401379b9b9d5a2ba4c836f1dd22b98440128652990acc0f6` |
| WAV / `kani-5a6abaf1546142a79cc8a0988caff01f` | PASS 20 / 7 / 0 | `55c9a439bc61eaf1ae7f7d53c1131b315f1c8407fe08e315c122b0cfa999243b` | `37381acf6cf560b8a1890be3170952f1e64c1762e489c27b60efe56368ac4fb8` |
| Window / `kani-1906acc96eb04475a3aa6dc22358d884` | PASS 10 / 8 / 1 | `9e0c09bc1a9d8e19602dd42802b3aee6d74e9bf7ad693e010ef65b4579311993` | `c70f0e92c4289f338422451165f80382f9e39796d099b184f9edf1a87f11df89` |

All three retain the kernel source identities above. They each recorded equal
start/end model-input digest
`4b08e7bddfa82c6a38e01d74f1bc0a55e9a0f3cd58bd1f5257af2f75af84824c`,
approved configuration digest
`ea909bb1b2b6b869f4f9340df51560d8387a3e6404f16588deb83478e2b59287`,
and unchanged Cargo.lock
`8cf5d58a6be8481148dae8daf9abec112d6e4ce72740eea56bd87f23108a8e6c`.
Model-input metadata lists the complete scanned API/contracts/schema/build inputs;
it does not claim universal transitive tool/cache authenticity.

Ordered commit 11 changes documentation/verified skill status only. Later merged
PR85 is CMP mock/docs only, with unchanged Rust/API/contracts/manifests/lock/proof
inputs. Those unchanged identities support retention of this exact executed model,
not a new model-checker run merely for a documentation or unrelated CMP integration.
Final fresh-develop/remote commit identities are in the PR and phase3 handoff.

## Ultra review reexecution

The independent review found and repaired the empty `CARGO_HOME` configuration
identity defect described in [the review report](ultra-review.md). Rust approval
and Python identity now use nonempty `CARGO_HOME`, otherwise nonempty `HOME/.cargo`,
and refuse an unresolved home before probes. Both candidate filenames and absent
files stay bound; explicit absolute/relative overrides preserve precedence.

After that repair, actual `cargo xtask verify kani` executed all three production
harnesses again on clean local commit
`3791364cc6802642238997aa1db2551784bd051e`, tree
`e32fb3de5347a98dd32ee4ab75665029aa94a6d6`. Published repair commit
`3367980818be17caaffe45b30eedfd8dc3ffe76e` has exactly that tree. Metadata records
the local commit; this is verified equal-tree binding, not a separately observed
execution at the published commit. Fresh develop
`909169b6fca661d5a2108298548e2a6b5616a145` is an ancestor of both.

Catalog again PASS: 100 checks, 6 covers, 4 expected unreachable checks. WAV again
PASS: 20 checks, 7 covers, no unreachable checks. Submission window again PASS:
10 checks, 8 covers, 1 expected unreachable check. Exactly one registered harness
ran per row; no failures or failed unwinding obligations. The three production
kernel/harness files, manifests and lock are unchanged by this repair. The same
64-bit model, pinned tools, unwind 2, no assumptions/stubs and default safety
checks apply; all earlier limitations remain.

The repair changes a workflow model input, so the complete model digest is now
`0c5069d5ea2edc323cf5d060faa21b2e95dac0914763ffa96a7a423f8df35b75`.
Approved effective configuration is
`ea909bb1b2b6b869f4f9340df51560d8387a3e6404f16588deb83478e2b59287`.
Before/after `Cargo.lock` remains
`8cf5d58a6be8481148dae8daf9abec112d6e4ce72740eea56bd87f23108a8e6c`.
Clean status and diff hashes are SHA-256 of empty bytes. The final review-evidence
commit changes only documentation, which is outside these recorded model inputs;
it does not imply another proof run.

Artifacts are under `artifacts/agent-runs/` (ignored local evidence):

| Exact artifact directory | Raw output SHA-256 | Metadata SHA-256 |
| --- | --- | --- |
| `kani-cc05233125e04e55b294df78a2118faf` | `2b8fa518c2ee7e5d36fc189fa9a9cee437e46ca13cedf0f7e3079466300adb3e` | `096e752c76ebeff1a515e34f272856ebb54651093a9a20b8f29a86ed193ce69c` |
| `kani-cc9dc28e673d463db8dbae05750687cd` | `5c867f64220669500e58584b45c624370cfa5d93e1c179758560907c480e0cce` | `8d90fceb13c70aed838ef97c06f9e06d3ba69052808ca584718b36b6f9c7133c` |
| `kani-7644846a70254b608a0e3d294af89b69` | `e3cf7ad3ab8332195d6730f5bca071c981719942486013aff022c9b5c9222cf3` | `2a755c006fde89af75205bf6946fa032835e451af9e6ab00060f52e2b52e48af` |

The successful proofs do not rewrite full Rust/preflight FAIL, PostgreSQL NOT_RUN
or browser/device IO acceptance. See the final [ledger checkpoint](domain-invariants.md#ultra-review-checkpoint).

## Publication kernel extension

The [Publication core evidence](../evidence/publication-core.md#formal-scope-and-exact-inputs)
adds the fourth registered proposition, `publication-commit`, with the same opt-in verifier
boundary. Actual command: `cargo xtask verify kani --harness publication-commit`.
It checks production pointer CAS/exhaustion and half-open rights time rules over full machine
integer domains: one exact harness,19checks,6 satisfied covers, unwind2 and unchanged lock.
No assumption/stub or meaningful input exclusion is used. Source/hash/raw-output identities and
the sole approved closed-enum unreachable error branch are recorded in that evidence.
This does not prove the entire readiness evaluator, SQL concurrency or storage/authorization.
The earlier three-kernel execution above remains historical evidence with its original inputs.

## Listener order and access-window kernel

Discovery adds the fifth opt-in proposition, `listener-access`. Actual final command
`cargo xtask verify kani --harness listener-access` executed
`contexts::media::domain::listening::kani_proofs::listener_keyset_and_access_window`: 27 checks,
five satisfied covers, unwind2, full-width i64 clocks and u128 identity tie-breakers, no
assumptions/stubs/unreachable exclusions, unchanged merged lock. The production
`after_work`/`access_window_valid` kernels are used by the real listener boundary. This proves
strict order/local expiry only, not all eligibility, cursor randomness, PG, grants or delivery.
See [the exact source/artifact ledger](../evidence/theatre-discovery-results.json) and
[scope/gaps](../evidence/theatre-discovery.md). Earlier three/four-kernel results stay historical.

### 15 October review: exact ordering proposition added, execution pending

The [25-commit review](../evidence/develop-25-assurance-review.md) found that strict
irreflexivity/asymmetry alone also accept an ordering with both directions reversed.
The current harness additionally compares `after_work` against the independent complete
sort key `(Reverse(timestamp), numeric_id)` and binds older/tie/equal cover goals to the
actual production result. It retains full-width `i64`/`u128` inputs, unwind 2, no assumptions
or stubs and five cover goals. Native extreme-value cases are added separately.

This changes the harness source. The historical 27-check PASS above does **not** prove this
new proposition or apply to the changed source. Fresh Rust tests, formatting and
`cargo xtask verify kani --harness listener-access` are **NOT_RUN / BLOCKED** on this host:
`cargo`, `rustc`, `rustup` and Kani are absent; no tool was installed. Do not call the strengthened
source `bounded-model-checked` until the exact configured verifier executes its assertions,
unwinding obligations and all five covers. Production ordering itself is unchanged.

## Private Work artwork numeric admission

The registered `image-bounds` harness executes the actual production
`contexts::media::domain::image::check_image_bounds` function. Full symbolic u64 encoded
length, u32 width/height and u8 bytes-per-pixel are compared to an independently widened
u128 specification with literal limits: encoded 1..8,388,608; each dimension 1..4096;
layout in {1,2,3,4,6,8}; at most 16,777,216 pixels and 67,108,864 decoded sample bytes.
Successful results also equal the independently computed counts. There are no assumptions,
stubs, loop-domain truncations or added unreachable allowances. Thirteen covers retain
minimum, encoded/dimension/pixel/output maxima, exact sixteen-bit output bounds, every
refusal partition and full machine-width maxima.

### Historical 13 October execution

The original ignored raw artifacts became unavailable after the dot snapshot replacement.
The following identities/counts are preserved historical claims, not a rechecked current-tree
proof. A fresh final integrated execution appears below.

Actual canonical command, executed 2026-10-13 UTC:

```sh
cargo xtask verify kani --harness image-bounds
```

The existing runner used Kani 0.68.0, CBMC 6.11.0, CaDiCaL, pinned
nightly-2026-08-21/x86_64-unknown-linux-gnu, `--lib --no-default-features --exact`, and
unwind 2 with normal verifier safety/unwinding checks. Result: **PASS**, one exact harness,
0/9 failed checks, 13/13 satisfied covers, 96.4038 seconds including compilation. The raw
verifier reports `Complete - 1 successfully verified harnesses, 0 failures, 1 total.`

| Bound identity | SHA-256 / Git identity |
| --- | --- |
| Clean executed local source | `98f81365c6c52a9fc743d0ed846abd3b88b49798` |
| Exact executed tree | `bc6191f638c00f1058fa4ee8a938b1feb4ead29e` |
| Registered full source file | `3956b927e14377858fefdaf88fed6d0429a8c467e448a8f7944ed5be39195f86` |
| Cargo.lock before and after | `be0587d1e3c4623b79a7c691a261adf6cdef48fa5b8ef95e682df965c71646a9` |
| Full model inputs before and after | `2d5ab9bb145a7ac7a70ec7dbb62dbb35efe0802bfd0159b832a714593c425328` |
| Approved configuration before and after | `cf0098bad897468f2d51337d289c7aa1e35d3d0363e496ee9127e3c75f902ef2` |
| Raw verifier output | `ad4c2ce3eb68e181c4c226bd49a750a849df0951a743d64d7f1057ac7a2eb058` |

Full raw output, exact tool/command metadata and before/after identities were originally
retained in ignored task record `kani-5ac1cd8ad3444b72ada2fdcaaf80345f`; that raw record is now
unavailable after recovery. Later evidence-only commits or
Git-data publication may have different Git identities; they do not imply a fresh proof.
Exact tree/ordered-parent mappings must remain explicit when publishing through the connector.

This proposition is for the numeric admission kernel on the named 64-bit target. It does not
establish actual decoder correctness, total RSS/CPU, byte verification, owner/session/rights,
SQL constraints/races, storage target/version resolution, HTTP behavior, provider execution,
32-bit/WASM arithmetic or production readiness. Those scopes retain their separate evidence
and pending I/O acceptance in the [artwork implementation record](../evidence/private-work-artwork.md).


### Fresh final integrated execution — 14 October 2026 UTC

The same canonical command actually ran on clean local
`e09c7fd071eaf888386d21be5a6acb42867b36bf`, tree
`6920f130784c726f05b22c16cf4e41428a385f7b`, incorporating final stable develop
`6359e819dcbd5877b50e164a7c67f04e9017d66c`. The exact code tree maps to Git-data commit
`c2f6c378c57b38708e3e3073c0772e038cbacd7c`, with ordered parents
`013a0709b41572399ebb6f2414bff3c343976bc5` then
`6359e819dcbd5877b50e164a7c67f04e9017d66c`. Subsequent evidence-only commits are not reruns.

Artifact `kani-73377143ed8c4f71a97fb2591238fcfe` and outer
`issue104-integration/image-bounds-final-stable.log` record **PASS** in 42.258747756 seconds:
one exact production harness, nine SUCCESS checks, all 13 SATISFIED covers, zero UNREACHABLE
results or allowances. Four checks are acceptance/refinement and exact-count assertions;
the remainder are overflow and compiler-unreachable safety properties. There are no authored
assumptions/stubs, restricted input domains or disabling flags. The invocation retains the
same Kani 0.68.0 / CBMC 6.11.0 / pinned nightly / 64-bit target / unwind 2 and normal safety
checks described above. Pinned-nightly unstable/macro-semicolon warnings remain in raw output.

| Fresh final binding | SHA-256 |
| --- | --- |
| Production image.rs | `3956b927e14377858fefdaf88fed6d0429a8c467e448a8f7944ed5be39195f86` |
| Cargo.lock before/after | `678a4b669deb3edcc8595f09684a3e8c569f8df94060e108446d5ca094900303` |
| All 266 model inputs before/after | `14835e83addfad5660e4dab1cbf87f99cba71a8e1b31e6dfea19e149dbc3b5e0` |
| Approved config/absence before/after | `239b2eea1db2fdc75a682597395b4bcd12ea25c799db596d0221f4202c5f1912` |
| Raw verifier output | `35a52ae612a88fc2d6dcac791f11c19d7e34d1f3131cb973b5f9d8e3606c380e` |
| Metadata | `f0fbdc770eb7afaf78f528536bd777cec883e8908db3b3259a89f3d15f758917` |
| Outer canonical xtask log | `276e5cff10591f1c337a8a4f5bb08be1aeaa4e24ee959162e2a7c314d1472a9f` |

Independent read-only audit matched every model input against both current and committed
bytes, every approved config/presence candidate, source, lock, canonical digests, invocation,
check/cover results and clean executed Git identity. It found no concrete integrity gap or
unsupported numeric claim, and ran no duplicate proof or I/O. Numeric acceptance/rejection
and exact counts on 64-bit x86_64 remain the entire proof scope. Exact error-variant precedence,
32-bit acceptance, actual decoder/resource safety, SQL/storage/auth/HTTP/restart and production
readiness remain outside it; see the separate current [layered evidence](../evidence/private-work-artwork.md).

## Public media kernels registered, not yet run

Issues [#170](https://github.com/loveoverflowcom/cantos/issues/170) and
[#171](https://github.com/loveoverflowcom/cantos/issues/171) add three loop-free production
kernels to the fixed registry. **No Kani execution is claimed for them:** the authoring host is
macOS arm64, while the fixed model pins x86_64-unknown-linux-gnu, Kani 0.68.0 and CBMC 6.11.0.
Their status is `NOT_RUN` until `cargo xtask verify kani --harness byte-range --harness
playback-grant --harness public-cover` runs on that model and its output is recorded here.

| Kernel | Production function | Proposition (unwind 2, no assumptions/stubs) | Covers |
| --- | --- | --- | --- |
| `byte-range` | `ByteRange::normalize` (shared by Studio private reads and the Theatre media route) | For every parser-admissible closed/open/suffix specification and full `u64` length: exact normalized bounds, `first <= last < length`, exact byte count, `Unsatisfiable` exactly for empty objects, empty suffixes and starts at/after the length | 6 |
| `playback-grant` | `grant_expiry`, `decide_media_fetch` | Grant expiry is exactly `min(now + TTL, rights end)` widened to i128 and strictly future; per-fetch admission equals the complete precedence Expired → Unbounded → NotCurrent → Forged → OutsideRights over full i64 clocks | 6 |
| `public-cover` | `cover_outcome` | Every pinned state × decision kind × same/other image × full i64 clock/expiry: exact fallback precedence; a shown cover implies a bound pinned image with a same-image clearance valid now | 6 |

The harnesses were written to have no assertion inside an unreachable branch, so no
`--allow-unreachable` entry is registered; an actual run may still report a compiler-inserted
unreachable check, which must then be reviewed rather than allowed blindly. HMAC correctness,
artwork-history resolution, SQL, storage, HTTP and restart behavior are outside these kernels.
The same propositions are mirrored by ordinary deterministic tests that did run locally:
`fetch_admission_matches_an_independent_widened_specification` (property),
`grant_expiry_is_bounded_by_ttl_rights_and_the_present`,
`cover_outcome_matches_an_independent_precedence_table` (bounded exhaustive) and the existing
`normalized_ranges_never_escape_exact_object` property. Those are `property-tested` /
`example-tested`, not `bounded-model-checked`.

## Forge attempt ledger kernel (#203)

The registered `forge-attempt-ledger` kernel is the first one outside the API library. Its fixed
registration names the package `cantos-forge`, the harness
`domain::attempt::kani_proofs::attempt_ledger_traces_never_overspend_or_blindly_retry`, the source
`services/cantos-forge-functions/src/domain/attempt.rs`, unwind 2, nine covers and no reviewed unreachable
check. The model is otherwise the fixed one above (Kani 0.68.0, CBMC 6.11.0, nightly-2026-08-21,
x86_64-unknown-linux-gnu, CaDiCaL, `--lib --no-default-features --exact`); the registered proof
is **`NOT_RUN`**: no pinned bundle exists on the authoring host.

| Proposition | Domain and bound | Production reach |
| --- | --- | --- |
| The factories refuse exactly zero (`Policy::new`, `Cost::new`); the first reservation is refused only when its cost exceeds the budget; then every four-step sequence of settlements and reservations keeps spend and attempts inside the policy, never wraps, reserves only after a settled (or exactly acknowledged uncertain) attempt, refuses for exactly the stated reason in the stated precedence, and records an outcome once | independent full symbolic `u32` attempt limit, `u64` budget and costs; symbolic outcomes, acknowledgements and choices; no assumptions or stubs; four unrolled steps, no loops; a zero cost is mapped to one only after the exact refusal is asserted | `Policy`, `Cost`, `Ledger::{open,reserve,settle}`, the kernel that `submit`, `regenerate`, `settle` and `Operation::replay` all call |

Nine covers: first reservation refused by the budget; reserved after an uncertain attempt with
its exact decision; reserved exactly up to the budget; refused while open; refused as undecided;
refused on a stale decision; refused at the attempt limit; refused by the budget within `u64`;
refused where the reserved sum exceeds `u64`. The oracle is written with widened arithmetic in the
harness, not copied from the implementation. It proves nothing about the request fingerprint, the
use cases, a store, a provider, a clock or the billable call itself; it is for a 64-bit target.

### Development signal on 2026-10-16 (not the registered model)

On this authoring host only Kani 0.67.0 / CBMC 6.8.0 / nightly-2025-11-21 exist. To get a
signal without installing anything, a scratch crate included the production
`apps/cantos-forge/src/domain/mod.rs` by path (unchanged sources) and ran
`cargo kani --harness domain::attempt::kani_proofs::attempt_ledger_traces_never_overspend_or_blindly_retry --exact --unwind 2 --solver cadical`:
**0 of 296 checks failed, 9 of 9 covers satisfied, no unreachable or failing check**, about 22
seconds. `attempt.rs` SHA-256 `9068b79f21011caafb781051b5de5a12c47286f1930e02e76e84bf0383716cab`;
`Cargo.lock` unchanged. Sensitivity: six deliberate defects injected into a copy (attempt limit
off by one, budget bound too strict, wrapping spend, stale decision accepted, open attempt
reservable, outcome settled twice) each made the same harness FAIL. A different Kani version, a
scratch host crate and the absence of the registered primitive's source/configuration binding
make this **not** evidence for the registered model. Record the registered result here only after
`cargo xtask verify kani --harness forge-attempt-ledger` runs on the pinned bundle and its
retained output is inspected. See the [verification record](../evidence/forge-attempt-verification.md).
