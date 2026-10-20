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
  --timeout-seconds 60
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

## Receipt and limits

The command emits a sanitized JSON receipt and the existing xtask status summary. It records
the source SHA, raw tracked/untracked file bytes and modes plus index metadata identity,
guard source digest, executable digest, selected test name/deadline/count, each negative
control and network mode. Git metadata is read without working-tree conversion filters;
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
those tests. The explicit guarded test is not automatically
selected by ordinary gates, preflight or CI, since no production native E2E consumer exists yet.
Kani remains the separate pinned [proof lane and ledger](kani.md). No Kani, Turso, RPC, HTTP,
browser/device, migration, footprint or production acceptance follows from this guard.
