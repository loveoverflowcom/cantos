# Native infrastructure isolation guard

## Review boundary

[#336.2](https://github.com/loveoverflowcom/cantos/issues/336) permits independent guard work
before the production Server/Identity composition is available. The implemented command
qualifies the environment around **one explicitly selected prebuilt Rust test executable**.
It starts no database, identity provider, container, browser, mail sender or installer.
Its immediate consumer is the named test supplied to the command; it is not a native runtime
launcher. The [native architecture](../architecture/native-turso-refactor.md) owns the remaining
two-process, authentication, D3 passkey, D4 first-party JSON and D7 acceptance requirements.

```sh
cargo xtask verify native-isolation \
  --test-binary /absolute/path/to/prebuilt-rust-test \
  --test fully::qualified::test_name \
  --timeout-seconds 60 \
  [--identity-binary /absolute/path/to/prebuilt/cantos-identity]
```

Build the owning test normally before invoking this lane. The guard runs libtest with exact
name selection and one test thread; it refuses zero, multiple, ignored or failed tests.
It accepts no shell command, arbitrary argument list, build or tool-setup option.
`--dry-run` prints a `NOT_RUN` plan without reading the executable or creating resources.
The test deadline is 1–120 seconds. A longer future E2E consumer needs a reviewed budget change.

## Controls and ownership

Each real invocation creates a fresh private directory, empty executable PATH and separate
HOME, temporary and XDG directories. The test receives an allowlisted environment rather than
inherited database, application, provider, cloud, proxy or dynamic-loader configuration.
The executable is absolute and already built, so compilation cannot hide tool or network use.
Forbidden names include Podman, Docker, PostgreSQL, Kanidm, Firebase, Redis and Feldera.

The executable control attempts actual resolution/execution with the isolated PATH, then
injects a synthetic executable and requires rejection. The credential control injects a
synthetic prohibited environment key and requires rejection. These negative controls must
fail in the expected way before the selected test can count as guarded evidence.

Linux additionally requires installed `unshare` and `ip`, usable user/network/PID namespaces,
and a fresh network namespace with only loopback. Numeric IPv4/IPv6 egress attempts must be
refused by the kernel; no request bytes or DNS are sent. A deliberately reachable private
loopback destination checks that the network refusal oracle detects a forbidden success.
Missing namespace capability is `UNSUPPORTED`, never permission to fall back to host networking.

macOS runs the executable/credential controls and the selected test as best-effort evidence.
Network isolation and its negative control are `UNSUPPORTED`, and the **overall result remains
`UNSUPPORTED` even when the selected test passes**. No egress probe is sent on the host network.

Subprocess output and deadlines are bounded. Cleanup addresses only process groups/namespaces
created by this invocation, before reaping their leaders, and removes only its private directory.
There is no process-name kill, port kill, inherited database reset or shared-directory pruning.

### Identity child executable

A native auth test starts the real `cantos-identity` executable as a child. `--identity-binary`
names that already built file. The guard validates it like the test executable (absolute,
regular, not a symlink, executable, and distinct from the test executable), passes exactly its
absolute path as the one additional test-only key `CANTOS_TEST_IDENTITY_BINARY`, and hashes it
before and after the run; a changed child fails the run. Without the option that key is refused
like any other unapproved input. No other environment pass-through exists.

The test helpers ([Server](../../services/cantos-server/tests/support/identity_process.rs),
[Identity](../../services/cantos-identity/tests/support/mod.rs)) start exactly the supplied
path. Under the guard (`CANTOS_NATIVE_TEST_DIRECTORY` set) they refuse to build or to fall back
to Cargo's compile-time executable, place their short private run directories inside the
guard's directory and stop their own unreaped child with `kill(2)` and a bounded wait, so no
program on `PATH` is needed. The child digest proves which file was supplied and that it stayed
unchanged; the source-to-build link comes from the build record of whoever built it (the
campaign below), never from the guard.

## Receipt and limits

The command emits a sanitized JSON receipt (schema 2) and the existing xtask status summary. It
records the source SHA, raw tracked/untracked file bytes and modes plus index metadata identity,
guard source digest, executable digest, the Identity child digest (or `null`), selected test
name/deadline/count, each negative control and network mode. xtask refuses a receipt whose
child digest is missing, malformed or equal to the test executable's when a child was supplied,
or present when none was. Git metadata is read without working-tree conversion filters;
changed executable/source inputs invalidate success. Dirty attribution is conservative when
working-tree bytes differ from Git's stored representation.
Raw target stdout/stderr, environment values and caller arguments are suppressed on success
and failure. Save the receipt outside Git when retaining run evidence.

| Claim | Oracle / evidence | Boundary |
| --- | --- | --- |
| No inherited executable or credential prerequisite | Actual missing-executable attempts plus executable/credential injection controls; deterministic Python regressions | Applies to the supplied test's environment; this is not an OS filesystem or syscall sandbox |
| Linux egress isolation | Actual fresh namespace, kernel connection refusal and reachable-destination negative control | Requires a supported Linux host; macOS cannot satisfy it |
| One selected test really passed | Exact libtest selection, test-name/result validation and source/executable digests | A test can still be a mock; the guard does not promote it into production runtime evidence |
| Honest status at the xtask boundary | Typed receipt validation rejects missing controls, zero tests, malformed provenance and status/exit mismatch | Ordinary Rust/Python adapter tests are `example-tested`, not Kani proofs |
| Resource containment | Bounded timeout/output and exact owned-child cleanup regressions | Linux PID namespace contains descendants; host-level privileged or deliberately hostile programs are outside the claim |

This lane does not remove forbidden software from the host or prevent an explicit absolute
path from executing it. It detects PATH dependencies under an empty PATH; a test's source and
production linkage still require independent review. Private HOME does not prevent arbitrary
absolute filesystem reads or access to an existing Unix socket. Native two-binary tests must
own their files/sockets and preserve separate database owners under the runtime contract.

Regressions are registered with the existing repository Python tests and Rust xtask tests,
which the repository/Rust CI jobs select. A CI failure before test discovery does not exercise
those tests. Guarded tests are not selected by ordinary gates, preflight or CI; the manual
campaign below is their only composed consumer. Kani remains the separate pinned
[proof lane and ledger](kani.md). No Kani, Turso, RPC, HTTP, browser/device, migration,
footprint or production acceptance follows from this guard alone.

## Native auth Linux qualification campaign

[#336](https://github.com/loveoverflowcom/cantos/issues/336) composes the existing owners into
one finite command, `cargo xtask verify native-auth-qualification [--kani] [--report PATH]`
([contract](../../xtask/README.md#verification-gates)). Setup and builds run first; execution
never builds, installs or provisions. Each selected test is an exact name from a fixed
allowlist, checked against its source and listed exactly once before it runs under this guard.

| Layer | Evidence | Selected |
| --- | --- | --- |
| `MODEL`, `PURE_CONFORMANCE` | `verify behavior --capability ID`, then `--conformance`, for `cantos.identity.native-mailbox-proof` and `cantos.identity.native-session-admission` | pure-core observers with a disclosed synthetic shell; not DB, RPC or HTTP evidence |
| `IDENTITY_PROCESS_RPC` | real executable, authenticated UDS, Turso file, loopback SMTP, under the guard with the child bound | `mailbox_owner_signs_up_then_authenticates_again_after_a_restart`, `a_caller_that_is_not_an_allowed_peer_gets_nothing_and_causes_no_effect` |
| `ACTOR_TURSO` | file-backed Server Actor Registry tests (`cargo xtask verify turso` does **not** run them) | contract, concurrent registration, reopen and lost-acknowledgement regressions |
| `SERVER_SESSION_CANDIDATE` | the agreed `identity_auth_candidate` target (`turso-candidate`): the real Identity journey; file-backed concurrent refresh, unknown acknowledgement and reopen regressions; and the production orchestration over a scripted Identity (durable revocation on every security change, concurrent first logins), which is mock-integration evidence | required when the target exists; `NOT_RUN` with the prerequisite named when it does not; a broken target shape, zero-selected, build or run failure is `FAIL` |
| `ISOLATION` | every guarded receipt `PASS` on Linux namespaces | a test that passed while the guard reported `UNSUPPORTED` (macOS) counts for its own layer with the detail "passed without isolation", never for this one |
| `KANI` | with `--kani`: `identity-security-version`, `identity-completion`, `identity-mailbox-proof` on the pinned bundle, outside the guard | the Server `decide_issue`/`decide_admission` gap stays (no kernel compatible with fixed unwind 2) |
| `FULL_CANDIDATE` | `PASS` only when every layer above passed on one unchanged, committed revision: the same HEAD, Git status and lockfile before and after, one raw-content snapshot across every receipt, and no receipt or status reporting uncommitted content | a changed source is `FAIL`; missing evidence or a dirty tree is `NOT_RUN`; with `--kani` the command exits non-zero unless it is `PASS` |
| `FULL_SERVER_HTTP`, `BROWSER`, `DEVICE` | no lane in this campaign | always `NOT_RUN` |

Every receipt must bind the digest of the executable that this invocation's Cargo JSON reported
and the source revision recorded at the start. The Identity executable is copied to a
campaign-owned path right after its build, because later test builds re-link
`target/debug/cantos-identity` with dev-dependency features; the first local run caught exactly
that mismatch. The JSON report holds the revision, lockfile and status digests, tool identities,
build commands, artifact digests, per-test receipts and the limitations, and no paths, mail,
credentials or raw test output.

The D1 run is the manual `native-auth-qualification` job of
[`repository-checks.yml`](../../.github/workflows/repository-checks.yml), dispatched with
`native_auth_qualification=true` on the candidate ref. It provisions Node 24.19.0/npm 11.9.0
and the locked Quint tool, Kani 0.68.0 through `cargo kani setup`, and allows unprivileged user
namespaces on the runner, then calls the campaign with `--kani` and uploads the report. The
Kani bundle that `cargo kani setup` downloads is not hash-checked beyond the registry's version
and compiler sentinels; that remains a tool-provenance limit of the D1 run.

Recorded D1 run: [actions run 39666812245](https://github.com/loveoverflowcom/cantos/actions/runs/39666812245),
job `native-auth-qualification` on `ubuntu-24.04` x86_64 at
`281964af7ab2b376cbd13831b5944db0face01ae` (clean tree), with Node 24.19.0, npm 11.9.0, Quint
0.33.0, Rust 1.98.1 and Kani 0.68.0. Its report artifact
(`native-auth-qualification-281964af…`, SHA-256 `7009680aad26a47ba69a71153cb14c1ec12c284b184050ade6da84609be0b918`)
shows every layer `PASS`: `MODEL`, `PURE_CONFORMANCE`, `IDENTITY_PROCESS_RPC` (2),
`ACTOR_TURSO` (4), `SERVER_SESSION_CANDIDATE` (6), `ISOLATION` (12 runs in
`linux-network-namespace`), `KANI` (3 kernels) and therefore `FULL_CANDIDATE`.
`FULL_SERVER_HTTP`, `BROWSER` and `DEVICE` are `NOT_RUN`. The preceding run 39663624462 on
`c22487a5` passed every layer except `identity-mailbox-proof`, which failed in the Kani output
parser ([ledger](kani.md#native-identity-kernels-registered-not-yet-run)). This record is for
that revision only; later changes to any selected input need a new run.
