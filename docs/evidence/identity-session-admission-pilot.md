# Identity session admission: bounded production-flow pilot

## Boundary and authority

Base: `origin/develop` at `3e01a96e60fabbef17e5ce39ba79885e087a55d0`, fetched on
2026-10-29. The input snapshot `cf3ab8ed7194f0a19de6b57ded966c94e876ec7e` was
revalidated against that base. This is an admission characterization and method improvement,
with one editing owner and a draft PR. Production behavior, RPC/HTTP/schema contracts and
model lifecycle are unchanged. Queue: no change; no work ordering or acceptance was selected.

Authority remains [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md#freshness-and-revocation-quantified).
The [module inventory](../../services/cantos-server/behavior/identity/native_session_admission.md)
already reports two contract mismatches. Both still exist at this base. The registered
`cantos.identity.native-session-admission` model is **draft**, not authority to settle them.
Characterization assertions below must be reconciled when the owner approves semantics; they
do not approve current behavior, narrow the contract or establish an exploit.

## Construction, entrypoint, decision and effect

```text
native access secret → IdentityAuth::authenticate → SessionStore::access
  → confirm → IdentityClient::security_state → private GetSecurityState RPC
  → SecurityState::from_response → decide_admission
  → SessionStore::revoke → TursoSessions::revoke / finish
  → committed or rolled-back server.db rows → next authenticate
```

The test seed uses the real session adapter's `issue`, fixed time `1700000000000000` µs,
subject `[0x11; 16]`, epoch `[0x22; 16]`, version `3` and fixed synthetic native secrets.
It does not exercise login or claim a new authentication-fact proof. UUID allocation is opaque
to the oracle; the emitted family ID must match every recorded revoke. The existing scripted
Identity peer serves real Protobuf/tonic bytes over a private Unix socket. The production
client's peer check, response decoder, orchestration, pure decision and Turso adapter all run.

| Branch / construction | Reach from this entrypoint | Observation / limit |
| --- | --- | --- |
| committed-revoked family | `access` returns `Inactive` before Identity | exact `Unauthenticated`; no additional RPC or revoke |
| `NOT_FOUND` | `confirm` requests `SubjectMissing` directly | no call to `decide_admission` for this case; effect probe and real rows |
| disabled / advanced / regressed version / changed epoch | well-formed response → real `decide_admission` → corresponding revoke | exact reason, error/result and durable state |
| response naming another nonzero subject | `from_response` returns `MalformedState` → `RpcFault::Protocol` | `Unavailable`, no revoke; the HTTP reproduction returns `503` |
| `RevokeReason::SubjectMismatch` | direct pure-core literals in the existing finite-grid test | parser correlation check excludes it on this RPC path; public fields permit direct construction elsewhere |
| unspecified state / zero version | malformed response refused by the production parser | exact `Unavailable`, unchanged rows, no revoke |
| scripted RPC `UNAVAILABLE` | real client's status mapping | exact `Unavailable`, unchanged rows; does not test an actual outage/deadline |

`admit`, `renew_web` and `refresh_native` share `authenticate`/`confirm`, but the new sequence
tests enter native `authenticate`, not the admission lease or refresh rotation. One additional
test enters the real HTTP library route. Other HTTP/refresh, browser and device claims remain
separate. Branch absence is justified by the parser predicate, not by zero observed hits.

## Reproduction and effect observations

The [four native tests](../../services/cantos-server/src/platform/identity_auth/turso/tests/admission_pilot.rs)
run 19 fixed schedules: the first three rows below each use disabled, advanced, regressed,
changed-epoch and missing-subject facts; the fourth uses four reply/failure classes. There is
no second reducer. `RevokeProbe` forwards every `SessionStore` operation to the real adapter;
it records only actual revoke arguments/results, with no synthetic session state.

| Sequence | Exact request result | Effect and stored state | Next admission |
| --- | --- | --- | --- |
| security difference → revoke commit → matching Identity | `Unauthenticated` | one exact revoke reason, `Ok(())`; revoked row/reason | `Unauthenticated` before another RPC; committed rows unchanged |
| difference → SQL abort → still different after fault removal | `Unauthenticated` | `Unavailable` from the real revoke; complete actor/binding/session/token snapshot unchanged | another fresh read, same reason, successful revoke commit |
| difference → SQL abort → matching Identity after fault removal | `Unauthenticated` first, then `Ok(())` | one failed revoke; complete snapshot unchanged across both requests | **no retry**; current behavior reproduces mismatch 2, not a requirement |
| other subject / unspecified state / zero version / RPC unavailable → recovery | `Unavailable` first, then `Ok(())` | no revoke attempt; complete rows unchanged | a fresh read accepts the same live family; other-subject semantics remain pending |

The named `pilot_fail_revoke` trigger raises `ABORT` **after** the real session update.
`TursoSessions::finish` rolls the transaction back. Comparing complete durable row projections
separately from the port's return value establishes this scoped real-file rollback observation;
an in-memory failure schedule alone would not. This is not crash/restart, power-loss, fsync,
concurrent writers or lost-acknowledgement evidence for these failure schedules.

The [HTTP test](../../services/cantos-server/tests/identity_password_facts_turso.rs)
`another_subject_at_admission_currently_returns_503_without_revoking` adds the actual production
router/status projection over the same external scripted peer and a real file. It asserts
`200 → 503 → 200`, one family, no stored revoke and fresh RPC calls. It is not browser evidence.

## Independent evidence and execution cost

Run records are ignored under `artifacts/agent-runs/identity-session-pilot/`: full command output,
exit code, elapsed wall time and the negative controls' before/mutant/restored source hashes.
The PR head identifies the delivered source; the base above identifies the production symbols
used. Toolchain: rustc/Cargo `1.98.1`, Turso Rust crate `0.8.3`, macOS arm64. Fixed schedules
are sequential, not exhaustive interleaving exploration.

| Layer | Actual command / evidence | Result and scope |
| --- | --- | --- |
| pure core | `cargo test -p cantos-server --lib platform::identity_auth::decision::tests:: --locked` | 6 passed; includes constructor partitions, exact fact age bounds and independent finite admission grid. Initial clean compile: 81 s |
| production shell with external double | `cargo test -p cantos-server --lib platform::identity_auth::turso::tests::admission_pilot:: --locked -- --test-threads=1` | 4 passed / 19 schedules; the expanded all-column snapshot run took 8.25 s test time, 19.10 s wall time including incremental compilation |
| real Turso | same native command; raw stored projections and actual SQL abort/rollback | file-backed commit/rollback of these sequences only |
| sensitivity controls | temporarily remove the normal production revoke call; temporarily bypass RPC subject correlation; run the respective exact native test | both failed their equality assertion (one selected test each, exit 101); 17.43 s and 16.78 s including incremental compilation; original source bytes restored |
| HTTP | `cargo test -p cantos-server --test identity_password_facts_turso another_subject_at_admission_currently_returns_503_without_revoking --locked -- --exact --test-threads=1` | 1 passed; 0.48 s test time, 7.11 s wall time including compilation |
| Quint model / pure-core observer | `cargo xtask verify behavior --conformance --capability cantos.identity.native-session-admission --setup` | model `PASS`; pure-core-synthetic-shell conformance `PASS`, 17 positive traces / 286 checkpoints, separate model and observer sensitivity stages `PASS`; 43.09 s including locked tool setup. Models, IDs, witnesses, controls and observer unchanged |
| Kani | no registered Server `decide_issue` / `decide_admission` harness selected | `NOT_RUN`; existing Identity kernels prove different symbols, not this shell or write. No identifier rewrite, new bound or zero-unwind claim |
| final diff / static gates | `cargo xtask preflight --changed-only --base origin/develop` | exact gate summary belongs in the PR; unavailable/failing gates retain their status |

The first native command inside the default sandbox failed at `UnixListener::bind` with OS
permission denial before semantic assertions. Its log is preserved separately. The successful
run used approved local execution for private sockets and disposable files. It supplies no
Linux network-isolation acceptance.

The model tool identity is Node `24.19.0`, npm `11.9.0`, locked Quint `0.33.0`.
Bundle `cantos.identity.native-session-admission-77fb51082f3e4b6296a84e65e54d6071`
retains independent `report.json` and `conformance.json`, source/tool/input hashes and raw
traces. A model result is not the SQL result in this record, and neither promotes the draft.

## Method returned to canonical skills

The engineering [production-reach example](../../.agents/skills/cantos-engineering/references/verification-strategy.md#production-reach-entrypoint-core-and-effect)
now follows the effect into the next entrypoint; [type selection](../../.agents/skills/cantos-engineering/references/types-as-proofs.md#choose-a-barrier-for-a-concrete-consumer)
names when a barrier, constructor-coupled proof or deterministic replay answers the claim.
The behavior [authoring reference](../../.agents/skills/cantos-behavior/references/authoring-and-evidence.md#follow-the-effect-through-the-next-entrypoint)
keeps native effect observations separate from the registered model observer.

No new type barrier was selected: `SecurityState`'s public fields remain a construction bypass,
and there is no approved consumer change in this pilot. A closed result or a consumed token
cannot make a failed revoke durable. A `503` return alone also cannot preserve the obligation.
For a future Kani proposition, retain constructor rejection tests, meaningful accepted-domain
covers, full-width representation and unwinding assertions; compare per-kernel bounds against
representation-preserving implementations before changing IDs. No Kani run happened here.

## Pending owner decisions and acceptance

| Decision / gap | Current evidence and tradeoff | Prerequisite / next step / owner |
| --- | --- | --- |
| other-subject reply: security fact or malformed RPC? | parser refuses before the pure mismatch branch. Revoking the asked family would contain suspicious replies but permits protocol faults to end sessions; protocol failure preserves outage semantics but differs from contract wording | product/security owner must select and record the contract interpretation; then replace the characterization with an approved regression and reconcile parser/core/model. Owner: unassigned |
| must a failed revoke remain owed after Identity matches again? | exact real-file sequence currently re-admits with no retry. Preserving an obligation needs an explicitly approved durable mechanism/recovery policy; narrowing the obligation changes the contract and needs acceptance | owner decision before production edits; no tombstone, retry journal or contract amendment was invented. Owner: unassigned |
| new failure schedules across restart/concurrency/unknown acknowledgement | this pilot observed ordinary SQL commit/rollback on one live owner only | select approved semantics first; extend the existing real-file tests with owned reopen/interleavings/acknowledgement schedules and run their exact names. Owner: session/store maintainers, assignment required |
| formal and production acceptance | no Server Kani harness; no real Identity executable, browser/device, deployed service or production data used | keep each separate; a draft model PASS and these tests accept none of them |

Prompt intake: `CANTOS_PROMPT_INPUT_V1`; input snapshot versus base revalidated; independent
reproduction/tests/skills completed within the submitted grant. No production semantics chosen;
no merge or deploy is authorized by this pilot.
