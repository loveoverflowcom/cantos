# ADR 0049: Explicit synthetic local HTTP mail

- Status: Proposed runner design; implemented bounded fixture, runtime acceptance recorded separately
- Date: 2026-10-24
- Decision owner: Cantos owner; exact third-party package and separate Draft PR authorized in the task
- Related issue or PR: Follow-up to [#438](https://github.com/loveoverflowcom/cantos/pull/438); does not complete [#422](https://github.com/loveoverflowcom/cantos/issues/422)
- Supersedes: Implicit local Auth Emulator startup only; native Firebase proof capability and production selection remain unchanged

## Context

The owner requested Resend local instead of automatically starting Firebase's local emulator,
then approved `resend-local@0.0.7` from y-hiraoka and a separate transport/runner PR. Current
Identity uses `lettre` SMTP or a distinct Firebase action adapter, not `resend-rs`. The approved
third party supplies an HTTP API and plaintext SQLite capture, not SMTP or mailbox proofs.
Only synthetic, nonfunctional messages are authorized in this fixture.

## Decision

Add a narrow HTTP preview adapter under the local runner. It takes no caller-provided mail,
credential or verification link and returns only `SimulatedReceipt`. It implements no Identity
proof or mail port. `run resend-local` starts, probes and stops the verified package. Explicit
`run cantos-theatre --web --local-mail resend-local` starts it with the application composition,
while Identity mail remains disabled. Local run defaults to disabled mail; the existing
Firebase action/proof lane requires explicit `--local-mail firebase`.

Keep the package exactly 0.0.7, check its release file hashes, and lock the local Next dependency
to the official 15.5.28 security patch. The fixture is private loopback only, with an empty child
environment, synthetic request guard, blocked outbound effects and bounded supervision.
Retain each run's synthetic capture. Never uninstall shared Firebase tooling or remove native
DB/WAL, keys, media or evidence as part of this change.

## Alternatives

| Option | Benefit | Limitation / decision |
| --- | --- | --- |
| Existing run-owned SMTP sink | Already exercises the implemented Identity transport | Not a Resend HTTP emulator; retained for existing tests |
| Resend local synthetic HTTP capture | Exercises the approved package and actual HTTP adapter without usable secrets | Selected bounded local slice; cannot test signup completion |
| Send native proofs to its SQLite inbox | Could use the existing Web fragment completion path | Deferred: plaintext proof storage is outside authorized test scope |
| Replace Firebase action proofs with HTTP delivery status | Superficially simple | Rejected: mail acknowledgement is not proof consumption or authentication |

## Consequences

- Production defaults, Identity/Server contracts, proof validation, sessions and migrations do
  not change. Native-first Android/iOS acceptance and Web proof completion are not established.
- Local users must select the Firebase proof lane explicitly when they need it. Resend local
  is a transport fixture and leaves signup unavailable; it cannot silently supply success.
- No live provider, account, API key or new credential is used. The public fixture marker has
  no authority. Captured `delivered` is simulation, not receipt by a real mailbox.
- Package preparation is explicit; ordinary checks need Node for guard tests but never install
  or launch this package. The lock and runtime assets live under `infra/`; `xtask` orchestrates.
- No password-reset feature, live template/handler change, association deployment or cleanup is
  authorized or implemented by this decision.

## Validation and revisit conditions

Validate fixed-message HTTP capture and readback, malformed/rejected/uncertain acknowledgements,
no automatic retry, package drift/path refusal, request and outbound negative controls, explicit
selection and absence of native-file effects in the standalone plan. Record an actual installed
fixture run independently from units and production/provider/device evidence.

Revisit before usable proof messages enter any mailbox, before changing Identity provider or
CMP link semantics, and when the approved upstream package or patched runtime lock changes.
The [fixture runbook](../../infra/local-mail/resend-local/README.md) owns exact software provenance
and local operational limits.
