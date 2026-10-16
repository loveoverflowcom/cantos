# Firebase enrollment proof: isolated feasibility evidence

## Review boundary and provenance

[#206](https://github.com/loveoverflowcom/cantos/issues/206), Mac mini, branch
`feat/206-firebase-enrollment-feasibility`, fetched `origin/develop` baseline
`178def7c65622614212e2f43be39f4587087b671`. PR196's code is merged in that baseline;
its earlier Draft description is historical. #174/#191/#199/#206 remain separate owners.
No other PR, stash, existing worktree, historical migration or qualification hash is edited.

Invariant: only the actual purpose-checked action, consumed against supplied exact pending
facts and fresh local fences, can produce candidate action evidence. Historical provider
status, the initiating browser and a polling receipt cannot acquire it or issue setup/session
authority. Owner: `platform::theatre_registration`; inward pure facts, concrete HTTP shell,
test-only composition. The existing runtime application/DTO/SQL/UI remain unchanged.

[ADR0036](../decisions/0036-firebase-enrollment-proof.md) is **Proposed**. Its A/B/C comparison,
password-free bootstrap candidate and protected setup continuation need owner decisions.
The test-only code is a feasibility experiment, not an accepted production adapter. An
`ActionProof` has no conversion to current application proof, new-person, setup or session.

## Claim and oracle ledger

| Claim | Failure mode | Cheapest independent oracle / evidence scope |
| --- | --- | --- |
| Exact local binding | Another operation/project/UID/email/epoch/expiry is substituted | Pure table tests assert exact `InvalidBinding` field; project association with a real key is not established |
| Purpose before consumption | A reset/recovery action mutates provider state | Exact precheck errors and captured HTTP trace; consume uses the same code |
| Fresh local single winner | Expired/cancelled/in-flight/consumed operation proceeds | Pure transition and fixture-interleaving tests; process-memory fences only |
| Lost response stays uncertain | Consumed action is replayed or replaced with verified status | Consume-failure trace and sticky refusal; no provider lookup/retry |
| Initiator cannot gain proof | A starts for V, V confirms, A polls and obtains setup/session | Status-only initiating receipt before/after actual V action; no authority conversion or runtime release exists |
| Provider input is bounded | Redirect, malformed/oversized body, timeout or upstream error leaks/proceeds | Local HTTP doubles and exact sanitized closed errors; no live provider |
| Runtime remains unchanged | Prototype is accidentally mounted or advertised | `#[cfg(test)]` module inclusion, unchanged application/config/router/contract; source review |
| Historical status is inert | Model grants claim/effect authority from a bool | Three named runs in the existing draft registration catalog; authored, execution unavailable without pinned Quint |

## Local results

Rust1.98.1. Full command output is retained under ignored
`artifacts/agent-runs/issue206/` in the isolated checkout. No fixture is live
Firebase/SQL/browser evidence. The PR carries the final preflight summary and any repository
handoff blockers separately; the results below describe only their named scopes.

| Layer / check | Result | Actual evidence and command |
| --- | --- | --- |
| L1 boundary | PASS, documented/source-reviewed | Separate Cantos Code Review found no actionable defect across the prototype, docs and model; private `#[cfg(test)]` inclusion leaves runtime unchanged. This is not activation approval. |
| L2/L3 pure core | PASS, example-tested | Six pure tests passed before transport existed: exact binding/error variants, purpose/email, terminal/expiry/freshness and initiator polling separation. `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test --locked -p cantos-api --lib platform::theatre_registration::firebase_prototype::core::core_tests`; `prototype-core.log`. |
| L3/L4 candidate HTTP shell | PASS, example-tested/fault-injected | Final `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test --locked -p cantos-api --lib theatre_registration::firebase_prototype`: 15 passed, zero failed/ignored. Six core plus nine transport/fixture tests, including duplicate interleaving, late expiry/cancel, timeout and dropped future after consume dispatch. `prototype-final.log`. |
| Affected API library | PASS, example-tested; named existing loopback doubles | `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test --locked -p cantos-api --lib`: 581 passed, 78 ignored, zero failed. Ignored SQL/media tests did not run. `api-lib-tests.log`. |
| Affected Rust lint | PASS, statically-checked | `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo clippy --locked -p cantos-api --all-targets -- -D warnings`; `api-clippy.log`. No workspace/mobile/release build. |
| Rust formatting/whitespace | PASS | Rust1.98.1 `rustfmt --edition 2021 --check services/cantos-api/src/platform/theatre_registration/firebase_prototype.rs` recursively checked its submodules; `git diff --check`; `RUSTUP_TOOLCHAIN=1.98.1 cargo fmt --all -- --check` passed (`rust-format.log`). This is separate from preflight's unreached rust-format row. |
| Repository Python unit fixtures | PASS, example-tested | `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'`: ran498, OK with5 optional Pillow skips (`repository-unit-tests-local.log`). These include mocked behavior-tool protocol tests, not actual Quint execution or Kani proof. |
| Registered model execution | UNSUPPORTED | `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify behavior --capability cantos.identity.theatre-registration` refused: locked Node/npm/Quint tooling unavailable. Three new runs are authored/source-reviewed, not executed or conformance-tested. No install attempted. |

The full preflight cannot start selected gates because locked Quint and ffmpeg/ffprobe are
missing (`preflight.log`). The available-gates run skips those prerequisites and reuses the
already-passed all-target lint; it reaches an actual **FAIL** in repository validation:
six stale Leptos qualifications and14 consumer-admission failures. The clean primary checkout
at the exact baseline produces the identical20-item failure list (`repository-baseline.log`).
No evidence hash or qualification record was changed. The earlier failure leaves aggregate
Rust tests/DBSP/WAV/format stages NOT_RUN; separate focused results above stay distinct.
The PR records final-diff summaries, explicit skips and the still-open handoff gate.

The initial Python fixture run also failed:37 sandbox `killpg` permission errors, one
consequential fixture assertion, and one real changed-catalog assertion expecting8 runs
instead of11. The catalog inventory expectation was corrected to include the three authored
runs, preserving all five properties/controls and four witnesses. The same suite then ran
with permission to manage only its owned fixture child groups and passed as reported above.
Original failure output remains in `repository-unit-tests.log`; it is not discarded.

Initial full fixture execution was **FAIL**: seven `TcpListener::bind` calls received sandbox
`EPERM`; seven non-binding tests passed. The same local-only operation then received sandbox
escalation approval and passed14/14 (`prototype-fixtures-loopback.log`); the final added
abort regression produced15/15 as recorded above. The original `prototype-fixtures.log`
failure remains retained. No remote provider request occurred in either run. Local in-memory
duplicates/abort tests establish no PostgreSQL race, transaction or process-restart semantics.

The initiator/recipient test checks only typed status/proof separation and replay refusal.
There is no browser or protected handoff implementation here, so it does not accept safe
same-device/cross-device delivery. Exact provider project association and UID provenance are
also external assumptions, not established by comparing supplied local facts.

## Deferred acceptance and residual risk

| Layer / status | Reason / prerequisite / next action | Owner / acceptance criterion |
| --- | --- | --- |
| Product/provider binding — BLOCKED | Explicit decision on bootstrap, never-reused dedicated UID, key/project association, existing identities, resend and setup-mail replacement | Product/identity owner: accept exact contracts and recovery without email-based linking or historical-state proof |
| Durable application/SQL — NOT_RUN | No Firebase claims/migration/runtime integration in this prototype; after decision, implement narrow provider mode and run `cargo xtask verify postgres` on a disposable cluster | Identity implementation owner; real uniqueness, races, fresh-clock waits, compatibility, restart, retention and lost-response reconciliation |
| Emulator/JWT — NOT_RUN | No authorized emulator tool setup, token verifier or bootstrap implementation; supply pinned tools separately and exercise action/bootstrap flow; add signed fixtures if tokens selected | Identity/operator owner; unsigned emulator results never satisfy production token acceptance |
| Web/native — NOT_RUN | No client changes; continuation/origins/query leakage and lifecycle design precede implementation | Web/identity owner, native separately; explicit input/focus/history/account fences and safe same/cross-device action completion |
| Provider/mail/live journey — NOT_RUN | No named project, recipients, authority or accepted delivery design; follow the [operator review plan](../deployment/firebase-enrollment-proof.md) only after specific authorization | Operator/identity owner; real mail → action → approved Kanidm setup → OIDC → `/session` → reload/logout and Studio/Admin denial |

No performance path, cache/audio fingerprint, rights or publication gate changes. All fixtures
are synthetic; zero provider calls, accounts, credentials, grants, mail, deployment or legal-term
acceptances. PostgreSQL business data stays with the existing VPS owner; future Firebase Auth
records and retention are explicit costs of the proposed integration. Fixture agreement does
not establish provider project association, dedicated-UID provenance, latest-resend semantics,
database durability, protected setup delivery, login or production readiness. Missing tools
and unresolved decisions must stay visible in the PR; #206 remains open.
