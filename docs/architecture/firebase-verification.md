# Firebase verification boundary

**Status:** owner-selected verification provider (23 October 2026). The operation-bound
Identity adapter, additive Turso migration, private RPC/HTTP/Web consumer and supervised local
Auth Emulator are implemented. Acceptance is layered below; this is not production activation.
Native Identity owns subjects, passwords, security state and authentication facts. Server owns
application sessions. Firebase supplies email actions, never Cantos login/session authority.

## Capability findings and historical reuse

Official sources checked on 23 October 2026:

- [sendOobCode](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/sendOobCode):
  ordinary `VERIFY_EMAIL` uses a Firebase ID token. Privileged `returnOobLink=true` returns a link
  without sending email; it is not a generic mail transport for a native challenge.
- [Custom action handlers](https://firebase.google.com/docs/auth/custom-email-handler):
  `mode`, `oobCode` and `continueUrl` arrive as browser input. A continuation is not authenticated
  native operation evidence and must never contain a native proof secret.
- [Action confirmation](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/update)
  applies the one-time OOB action; the
  [response](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/SetAccountInfoResponse)
  includes the account ID, email and verification state.
- [Auth Emulator](https://firebase.google.com/docs/emulator-suite/connect_auth): demo projects,
  unsigned local tokens and private OOB inventory support local tests without real email.
  Unsigned tokens/endpoints are excluded from production configuration.
- The reviewed [firebase-tools 14.2.2 implementation](https://github.com/firebase/firebase-tools/blob/v14.2.2/src/emulator/auth/operations.ts)
  resolves `VERIFY_EMAIL` by the code's email (`getUserByEmail(oob.email)`), rather than an immutable
  code-to-UID binding. Deleting/recreating that mailbox may let an old code affect a new UID.
  Production invalidation equivalence has not been established; no safe recovery is inferred.

Historical `develop` commit `1c970a311b01090c86372c60129451e08a8fb468` supplied patterns for
bounded REST parsing, positive enrollment provenance and the installed emulator module.
No PostgreSQL, Kanidm, old enrollment table, OIDC issuer or legacy session adapter was restored.
The existing `Mailer`/`ProofMail` native-secret contract remains the explicit SMTP path. Firebase
uses the distinct inward `VerificationActions` port because its action capability is different.

## Implemented protocol and durable boundaries

1. A well-formed start receives the same public accepted/TTL reply for new, registered, colliding,
   cooling-down and provider-rejected addresses. Existing IP/email budgets remain before effects.
   Dispatch outcomes are internal only. Equal network timing is not claimed.
2. In one Identity-owned Turso transaction, reserve the exact `(project,email)` enrollment,
   native signup operation/proof and a create fence. The native 32-byte random proof secret is
   AES-256-GCM encrypted, with fresh nonce and AAD binding project, operation, proof, email,
   native instance epoch, purpose `TheatreSignup` and the absolute 900-second deadline.
   The encryption key is separately injected, never stored in the DB or sent to Firebase.
3. After commit, create dedicated Firebase UID `cantos-<native-operation-hex>`. Only a positive
   matching create response establishes provenance. Existing accounts/collisions are never
   adopted, linked, reset or deleted by email. Lost create responses remain fail-closed.
4. Reserve send durably before the external call. A custom-token exchange for this owned UID
   supplies the ID token to `VERIFY_EMAIL`; the token never reaches native authentication.
   Continuation carries only public operation/proof locators. Provider acceptance is a send
   acknowledgment, not mailbox confirmation. Rejection and uncertain delivery persist separately.
5. Web captures the action query in RAM and scrubs history before application I/O. Its exact
   current origin/path and action purpose are validated. GETs/scanners do not confirm anything.
   The existing signup state machine collects a chosen password and explicit confirmation.
6. Identity checks budgets, enrollment deadline and action purpose/mailbox, then reserves one
   consume. The exact request is bound with HMAC-SHA-256 under an HKDF-separated runtime key;
   project, operation, proof, command, action code and password are length-framed inputs.
   There is no fast unkeyed password digest in the database. Password login still uses Argon2id.
7. Apply the Firebase action exactly once. Require the expected UID, exact email and
   `emailVerified=true` response, then commit `ReceiptReady`. Acknowledged rejection is terminal
   `ConsumeRejected`; uncertain dispatch/response remains `ConsumeReserved`. Neither is replayed
   automatically or reconciled from a generic `emailVerified` lookup.
8. Only a matching acknowledged receipt releases its sealed native proof internally. The
   existing native transaction atomically consumes the proof and creates subject/email/password.
   An identical command/body may reconcile `ReceiptReady` and replay that native receipt;
   changed password/code/command/locators are refused. Completion returns no application session.
   Native password login remains a separate step, followed by Server session policy.

Migration 4 appends `firebase_signup_enrollments`, unique mailbox slots and append-only
`firebase_verification_events`; migrations 1–3 are unchanged. Database guards freeze bindings,
expiry, positive create provenance and claimed request fingerprints. Provider I/O and password
hashing occur outside database transactions. There is no cross-database transaction.

## Recovery limits: intentionally fail closed

Resend reuses the positively created UID, same operation/proof and original deadline after the
existing cooldown. It does not replace the native operation or extend expiry. Reserved effects
are never blindly retried. An uncertain consume can strand that enrollment even if Firebase
actually consumed it; no poll, new command or restored provider state recreates native authority.

**Expired enrollments retain their mailbox slot permanently in the same project.** On a later
start the expired envelope is cleared and no replacement is issued. Automatic deletion,
UID recreation, deadline extension or enrollment rebinding is not implemented: the observed
email-based emulator consumption cannot prove old-code invalidation. This can strand a mailbox,
including after an attacker-initiated enrollment. It is a required availability/recovery gap,
not production-ready self-service registration. Identity/auth review must establish a provider
invalidation protocol or approve a different delivery capability before accepting production
registration recovery. No live account cleanup or security-setting change is authorized here.

Authorization always checks expiry, including after an acknowledged consume. Expired ciphertext
is cleared when that mailbox is touched; there is no background physical-erasure guarantee for
untouched rows/backups. Runtime-key loss/rotation fails closed for old envelopes/fingerprints;
restore must preserve the appropriate external key and native epoch policy. Key rotation and
operator-assisted enrollment recovery remain unimplemented owner work, never email linking.

## Local development composition

`cargo xtask run cantos-theatre --web --open` checks installed Node.js and `firebase-tools`
**14.2.2** Auth module before building. It derives the module from the installed `firebase`
executable or explicit absolute `CANTOS_FIREBASE_AUTH_MODULE` path to
`lib/emulator/auth/server.js`. Missing/wrong tooling gives an actionable refusal. No installer,
Firebase CLI login, cloud fallback or global version upgrade runs. Hosting/App Distribution
retain their separate tooling contract.

xtask owns a private disposable directory, fresh envelope key, unique `demo-cantos-*` project,
in-memory emulator and ephemeral `127.0.0.1` port. Its child environment is empty, outbound
TCP/TLS is denied with startup negative controls, provider output is suppressed and readiness/
shutdown are bounded by existing process-group supervision. Only these new emulator resources
are removed after stopping the child. Native DB/WAL/media persist under their existing owner.
A new xtask invocation has a new emulator project/key: pending emulator enrollments cannot be
resumed across runs; committed native credentials remain in `identity.db`.

Identity selects `firebase-emulator`, with the actual Theatre origin `http://127.0.0.1:8082/`.
Loopback HTTP requires both `local-development` feature and explicit local composition flag.
Inherited Google/Firebase/SMTP inputs are refused by name before execution. Ordinary `service`
and OCI builds reject local development and emulator selection. No fixed OTP or success mock
exists. The requested composition reached Identity, Server catalog and Theatre Web readiness
on this Mac; this does not qualify unrelated business porting or publication behavior.

Only the inventory endpoint is printed:
`http://<run-loopback>/emulator/v1/projects/<run-demo-project>/oobCodes`.
A trusted local developer/test may retrieve the current run's action into RAM. Keep action
codes/links out of terminal logs, traces, screenshots and persistent browser storage. Do not
open the emulator's default `/emulator/action` link: it consumes the code outside Cantos.
Use the approved Theatre handler with the action query for explicit confirmation instead.
Loopback is developer-host isolation, not authentication against another local process.

## Production configuration, images and VPS

Production selection is explicit `CANTOS_IDENTITY_MAIL_PROVIDER=firebase`, with:

| Input | Contract |
| --- | --- |
| `CANTOS_IDENTITY_FIREBASE_PROJECT` | Exact approved non-demo project ID |
| `CANTOS_IDENTITY_FIREBASE_PROJECT_BINDING_APPROVED` | `true` only after operator review of API key/project/IAM binding |
| `CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER` | Exact approved HTTPS handler URL, no query/fragment/userinfo |
| `CANTOS_IDENTITY_FIREBASE_ACTION_ORIGIN` | Exact handler origin |
| `CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER_APPROVED` | `true` only after actual template/domain/handler review |
| `CANTOS_IDENTITY_FIREBASE_API_KEY` | Runtime-injected project API key; never a build argument |
| `CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE` | Absolute private runtime JSON file, same-project service account and PKCS8 RSA key |
| `CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE` | Absolute private file containing exactly 32 random bytes, outside DB custody |

Syntax/approval validation precedes secret file reads. Files must be regular, not symlinks,
with no world access or group write/execute. Missing/partial/mixed configuration fails closed.
Only fixed Google Identity Toolkit/OAuth HTTPS endpoints are used, with no inherited proxy,
redirects, SDK emulator overrides or unsigned production tokens. Transport responses have bounded
size, JSON content type, deadlines and sanitized errors. No provider response/token is logged.

Use existing operator-owned Podman secrets/read-only runtime mounts for the two files and
runtime secret injection for API key. This document supplies a configuration contract, not an
activated native Quadlet. Do not repurpose retained legacy PG/Kanidm Quadlets. Native VPS
activation remains the separate #348 owner boundary. This task transmits no secrets and changes
no real project, IAM, template, origin, provider setting or deployed service.

The Identity Containerfile compiles the adapter with `service`, without `local-development` or
credentials, and defaults mail to `disabled`. Images contain neither DB nor runtime credentials.
Build/push never runs Identity serve or an email API; the native OCI workflow gains no Firebase
secret. Actual image-layer inspection and production activation are separate acceptance layers.
The recovery gap above remains unresolved even when configuration syntax is valid.

## Evidence and remaining acceptance

The cleanup checkpoint's 148-unit result and real emulator/RPC journey were run before final
hardening. Those old raw logs were moved to recoverable local Trash; the former `artifacts/`
paths are not current evidence. The final affected checks below supersede that checkpoint.
See the PR's exact command/result matrix for current source-bound results.

A real Identity executable + Turso + emulator test exercises pending proof, private action
retrieval, explicit confirmation, exact-request retry/mismatch, password authentication and
process restart. The separate real browser journey on this Mac exercised signup, safe action capture, explicit
confirmation, separate native password login, session restoration after reload and durable
logout after reload against the composed native processes. The developer action handoff was a
disposable loopback test page retrieving the real emulator inventory into RAM and navigating to
the approved handler; it did not fake provider/native success or consume the code itself.
No action/password was printed, stored by that helper or captured in screenshots. The helper
was not added to the application/runtime. This establishes one in-app-browser journey, not
cross-browser/device, production template or real mail acceptance.

Remaining required acceptance: complete dispatch/commit crash matrix,
provider-safe expired enrollment recovery, production
project/IAM/template/handler review and actual image-layer scan. Live email is NOT_RUN and not
required by this source task. No merge, deployment, image push or issue closure is implied.

### Final local checks (23 October 2026)

Commands below use `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
to keep this Mac's builds scoped. Logs are ignored local evidence under
`artifacts/agent-runs/firebase-verification/`; the PR records final packaging/CI status.

| Command / observation | Result and actual scope |
| --- | --- |
| `cargo test --locked -p cantos-identity --features local-development --lib --tests` | PASS: 199 tests (153 unit, 22 application, 2 mail, 15 process, 2 authorization, 5 wire); 2 opt-in tests ignored. Real loopback response loss/5-second timeout, keyed request binding and real Turso fences are included. |
| `cargo test --locked -p cantos-identity --features service --lib config::tests` | PASS: 8 tests with production feature selection; local flag refused. No production credentials supplied. |
| `cargo test --locked -p xtask auth_emulator::tests::installed_emulator -- --ignored --nocapture` | PASS: pinned installed emulator action/lifecycle test, no cloud/mail. |
| Prebuild `cargo test --locked -p cantos-identity --features local-development --test firebase --no-run`; pass its absolute executable as `CANTOS_TEST_FIREBASE_TEST_BINARY` to `cargo test --locked -p xtask auth_emulator::tests::native_identity -- --ignored --nocapture` | PASS: real executable/private RPC/Turso/emulator; no mock authentication. Mixed/missing proof alternatives, mismatched proof/operation/code/password, replay, known-email generic response and restart included. |
| `cargo test --locked -p cantos-theatre --features preview --lib application::signup::tests` | PASS: 18 state-machine/parser tests. |
| With prebuilt `CANTOS_TEST_IDENTITY_BINARY`, `cargo test --locked -p cantos-server --test identity_auth_candidate` | PASS: 1 real native credential/actor/session adapter journey. |
| With the same binary, `cargo test --locked -p cantos-server --test contract_conformance --test identity_password_turso` | PASS: 103 wire and 15 real Identity HTTP/session regressions; these retained regressions use loopback SMTP capture. |
| `TMPDIR=/private/tmp cargo xtask run cantos-theatre --web --open` | PASS readiness; actual browser signup/action/confirmation/separate password login/reload/logout/reload observed. Deliberate Ctrl-C after acceptance stopped all four owned processes. Native DB/WAL retained; emulator/key directory disposed. |
| `cargo test --locked -p xtask` | FAIL: 600 passed, 2 graph failures, 5 ignored. Both graph failures were reproduced on pinned BASE before cleanup: existing Server dev-dependency on Identity is absent from the graph oracle. No weakening of the oracle. |

The changed-only preflight, formatting, Clippy and source/image-boundary scans are recorded in
the final PR matrix, including failures/skips. Compilation/tests do not establish a built image's
layer contents. Actual image build/layer scan, Linux deployment, production project credentials/
IAM/template/handler and live email remain NOT_RUN. No full crash-boundary coverage is claimed.
