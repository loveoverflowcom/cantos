# Resend local synthetic HTTP evidence — 24 October 2026

This record covers a bounded local transport fixture. It accepts no native signup, mailbox,
password-reset, real Resend delivery, browser or device behavior. The implementation and
remaining boundary are in [ADR 0049](../decisions/0049-synthetic-local-mail.md) and the
[fixture runbook](../../infra/local-mail/resend-local/README.md).

## Source and authorization

- Base: fetched `origin/develop@c427f38762f80c67baf15a049559476100ebcf6b`, after merged #438/#439.
- Branch: `feat/identity-resend-local-mail`; separate worktree from the preserved #438 branch.
- The owner approved the exact third-party `resend-local@0.0.7`, y-hiraoka source, isolated
  synthetic-only execution and a separate Draft PR. Shared Firebase CLI remains installed.
- No existing open Resend PR or matching local branch was found. A read-only process check
  found no duplicate installer, build or local mail task before work began.
- Mac arm64; Rust **1.98.1**, Node **22.15.0**, npm **10.9.2**. Commands below ran from the
  follow-up worktree; the two baseline commands ran from a clean detached worktree at BASE.
- Rust commands reused the existing target directory through `CARGO_TARGET_DIR`; no native
  service rebuild was needed. `RUSTUP_TOOLCHAIN=1.98.1` was explicit. No broad cache cleanup ran.

## Approved package and dependency boundary

The downloaded archive is 970,033 bytes and contains 133 files, 3,607,970 unpacked bytes.
Its npm SHA-512 integrity and SHA-1 `4e7df9c08b0d0c8185004e417e5544a0f35a6c75` matched before
installation. Package metadata matches 0.0.7, y-hiraoka and MIT. Each installed release file is
checked against the pinned manifest by `Installed::inspect` before process startup.

The public SLSA statement's archive SHA-512 matches those bytes and its resolved Git dependency
is `41e474ee644049ed75a037c56d0c12c21f938c5d` in `y-hiraoka/resend-local`. The installed graph's
`npm audit signatures` passed: **91 verified registry signatures, 27 verified attestations**.
The statement/digest comparison and full audit output remain in the ignored evidence directory.

Installation disabled lifecycle scripts and used a clean environment, private task npm home/cache,
no npm/provider credentials and no global installation. npm warned that upstream Next.js 15.5.2
is vulnerable; that runtime was not started. The isolated lock overrides Next to **15.5.28**,
following the [official maintenance security update](https://nextjs.org/blog/nextjs-security-update-october-14-2026).
The approved resend-local package bytes remain 0.0.7. This patch is not a complete dependency
audit, an upstream release or production qualification. Dashboard/image/RSC paths are refused.

Observed installed dependencies occupied approximately 381 MiB and task npm cache 548 MiB.
Free disk varied with host activity; no existing cache, SDK image, APK, database or evidence was
deleted. These observations are not a claim about the complete runtime's memory footprint.

## Executed checks

| Check / exact command | Status / result | Actual boundary |
| --- | --- | --- |
| Pinned archive digest, metadata and all installed release-file checks | PASS | Downloaded approved package, no code execution during verification |
| `npm audit signatures --prefix infra/local-mail/resend-local` | PASS: 91 signatures, 27 attestations | Public npm registry/provenance reads, not a Resend provider request |
| `cargo test --locked -p xtask resend_local::tests -- --nocapture` | PASS: 6 tests | Fixed payload, receipt/error decisions, actual loopback HTTP adapter with a deterministic server double, package drift/escape refusal, Node guard tests |
| `cargo test --locked -p xtask` | FAIL: 632 selected; 627 executed; 624 passed, 3 failed, 5 existing ignored | All 9 added Rust tests passed; Node launcher test also executed 3 guard regressions. No installed emulator was started |
| `cargo clippy --locked -p xtask --all-targets -- -D warnings` | PASS | Affected Rust package and test targets |
| `cargo xtask run resend-local` | PASS | Actual installed 0.0.7 with patched Next, guarded loopback listener, real HTTP capture/readback and owned-process shutdown |
| Read-only capture inspection | PASS | Exactly one fixed message to `recipient@cantos.invalid`, matching nonfunctional text, no HTML, attachment or API-key row; mode 0600, private run directory |
| Actual runner with inherited nonsecret `RESEND_API_KEY` canary | PASS: refused by variable name before execution | No new capture/native development directory; the canary value was absent from output |
| Native/provider/browser/device acceptance | NOT_RUN | No usable proof, signup completion, password reset, live API key, real recipient, external attachment fetch, production provider or native device involved |

Full outputs are retained under ignored `artifacts/agent-runs/resend-local-20261024/`, including
the package archive/provenance, dependency installation/audit, unit/clippy logs and capture check.
The final no-skip changed-only preflight and read-only review are reported with the PR's exact
head. This record does not infer their result from the focused passes.

## Failure diagnosis and baseline comparison

The first compile identified a remaining preview-plan field after the mail-choice enum change;
it was repaired before tests or runtime acceptance. The next focused run had two test-harness
failures: a cleared child PATH hid installed Node, and macOS's canonical temporary path differed
from its spelling. Resolving Node before clearing its environment and using the canonical test
root repaired them; the six focused tests then passed. No tool was declared unavailable merely
because of the narrow PATH. The failed test log remains separate from the passing attempt.

The full xtask suite's three failures were reproduced on the exact clean BASE:

| Command at BASE | Result / cause |
| --- | --- |
| `cargo test --locked -p xtask preflight::graph::tests` | FAIL: 5 passed, 2 failed. Graph oracle rejects the existing Server→Identity dev dependency and expects it absent from metadata |
| `cargo test --locked -p xtask services::tests::identity_environment_names_exist_in_the_identity_sources` | FAIL: the source scanner misses `CANTOS_IDENTITY_FIREBASE_PROJECT_BINDING_APPROVED` in its actual Firebase owner |

The graph, scanner, workspace manifests and Cargo.lock inputs are unchanged by this slice.
The five ignored tests predate it: two installed-Firebase boundaries, one real Web release
bundle and two built-Android-APK distribution inspections. No added test is ignored or skipped.

## Layer ledger and deferred acceptance

| Claim | Evidence held | Remaining boundary |
| --- | --- | --- |
| Synthetic mail cannot create Identity authority | Source/type review: no Identity port implementation or service dependency; Identity mail stays disabled in that lane | This is a fixture adapter, not an authentication provider |
| Fixed request, literal loopback, bounded HTTP and no write replay | Unit/example and deterministic loopback HTTP tests; existing parser regressions in package suite | No Resend API behavior or network fault equivalence claimed |
| No external fetch/spawn, unknown body or browser action enters fixture | Real startup negative controls and Node request/body guard tests | Application-level guards, not an OS sandbox/native dependency audit |
| Installed package records the expected synthetic mail | Real local HTTP capture and independent SQLite read-only checks | Upstream hardcodes `delivered`; no real mailbox delivery or proof consumption |
| Existing Firebase proof and production/native contracts preserved | Service source/contracts and production defaults unchanged; explicit Firebase selection remains | Firebase boundary not rerun in this follow-up; prior evidence retains its own source attribution |

Usable native-secret email would require an approved transient-storage design and separate test
scope. Current Web fragment completion and Firebase action completion remain different paths;
CMP native-first parsing still accepts Firebase actions only. No fake mail acknowledgement is
converted into successful verification. Password reset remains unimplemented/out of scope.

Android runtime and signed OS association are not supplied by this transport fixture. Previous
device/disk blockers and association/operator facts remain separate. No deploy, distribution,
merge, live mail, production configuration change, credential creation or issue closure ran.
Queue: no change; this is the explicitly requested bounded tooling follow-up, not a new phase
exit or change to the native verification prerequisite order.

## Independent review correction: resolve the runtime actually loaded

The original record above describes `7b55e50210ecde410e99774a4ae9e1b640d5ac1a`.
The parent's independent review found P2: Rust and the launcher checked the patched sibling
`../next/package.json`, while the real server's module context could prefer an extra stale
copy under `resend-local/node_modules/next` or `dist/app/node_modules/next`. All 133 approved
release files could remain intact. This was a prepared-install validation defect; it is not
evidence that the original clean installed graph or successful smoke used an unpatched copy.

The correction uses Node `createRequire` from the canonical real `dist/app/server.js` path.
It resolves `next/package.json`, the `next` entry and `next/dist/server/lib/start-server`, checks
their canonical locations against the same patched sibling, and validates that resolved
package's name/version before import. Rust preflight invokes this exact helper in metadata-only
inspection mode; the launcher repeats it immediately before importing that same server entry.
No separate Rust approximation of Node's module search remains.

Deterministic fixtures supply a patched sibling and inert 15.5.2-labelled nearer modules whose
sentinels fail if executed. Both nearer locations failed the old check's regression (missing
expected rejection) and pass after the fix without executing those stubs. A further real-CLI
negative check copies all 133 approved release files unchanged and adds each inert shadow in
turn: both are refused by Rust preflight before any capture/process or sentinel execution.
No genuinely vulnerable Next code was loaded to demonstrate the defect.

| Refreshed repair check | Actual result |
| --- | --- |
| `cargo test --locked -p xtask resend_local::tests -- --nocapture` | PASS: 6 Rust tests, including 7 Node guard/resolution tests |
| `cargo test --locked -p xtask` | FAIL: 632 selected, 627 executed; 624 passed, same 3 clean-base failures, 5 existing ignored |
| `cargo clippy --locked -p xtask --all-targets -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo xtask run resend-local` | PASS: actual patched synthetic capture/readback, owned process stopped, capture retained |
| Real Rust CLI with intact approved files + each inert nearer copy | PASS: both refused before capture, no stub execution |

Repair commands used `CARGO_INCREMENTAL=0`, explicit Rust1.98.1 and the preserved existing
target directory. The first filtered repair invocation selected zero tests because that shared
cache still held the clean-base test binary; it is retained as NOT_RUN, not acceptance. The
forced current-worktree regression then selected the intended test and reproduced both failures
before repair. No cache was deleted to obtain a current test executable.

Fresh logs, negative-install capsules and receipts remain under ignored
`artifacts/agent-runs/resend-local-20261024/runtime-resolution-fix/`. The refreshed no-skip
preflight and exact repair SHA are recorded on PR440; baseline failures and subsequent NOT_RUN
layers remain disclosed. Independent re-review of the repair remains pending. No package
installation, production/provider change, usable proof, real recipient or cache cleanup ran.
