# Firebase enrollment: selected mode and operator acceptance

[ADR0036](../decisions/0036-firebase-enrollment-proof.md) accepts code and isolated tests for
Firebase mailbox proof, a dedicated UID per enrollment and one-shot Kanidm setup delivery.
**Live activation remains unaccepted.** No real project, secret, grant, account, mail recipient,
action template or deployment is created by configuration parsing. See the separate
[runtime evidence](../evidence/firebase-enrollment-runtime.md).

## Implemented, disabled composition

The existing API host selects one provider with `CANTOS_THEATRE_REGISTRATION_PROVIDER`:
`smtp` preserves the existing sequence; `firebase` composes the new workflow. No provider value
is selected automatically from credentials. Absent registration configuration remains off.
Explicit Firebase selection requires `ENABLED=true` and all inputs below; `ENABLED=false`
without other inputs remains off. Partial/orphan/mixed inputs fail startup. There is no SMTP,
mock, emulator, tenant or arbitrary endpoint fallback.

All following suffixes use `CANTOS_THEATRE_REGISTRATION_`:

| Input | Required meaning; no deployment default |
| --- | --- |
| `ENABLED`, `PROVIDER` | Explicit `true`, `firebase`; keep disabled until acceptance |
| `POLICY_APPROVED`, `ISSUER_ACCESS_APPROVED` | Operator declarations for the exact deployment policy and effective Kanidm target protection, both `true` |
| `TOKEN` | Existing Kanidm onboarding bearer from the operator's secret store; never a browser credential |
| `NETWORK_KEY`, `PEER_POLICY` | Existing nonzero 32-byte hex HMAC secret; only `direct_peer` is supported |
| `VERIFICATION_SECONDS`, `RESEND_SECONDS`, `WINDOW_SECONDS`, `MAX_GLOBAL_REQUESTS`, `MAX_NETWORK_REQUESTS` | Explicit bounded durable lifetime/cooldown/mail budgets; resends consume the same budget |
| `SETUP_SECONDS`, `MAX_INFLIGHT`, `ATTEMPT_WINDOW_SECONDS`, `ATTEMPT_MAX_GLOBAL`, `ATTEMPT_MAX_NETWORK` | Explicit bounded setup lifetime and process-local admission policy |
| `FIREBASE_PROJECT_ID`, `FIREBASE_API_KEY` | Exact verified project/key association; API key stays server-side in this design |
| `FIREBASE_SERVICE_ACCOUNT_FILE` | Absolute existing secret-store file, regular and at most64KiB; service-account JSON with exact project, client email and RSA PKCS8 private key; only read at startup |
| `FIREBASE_ADMIN_ACCESS_TOKEN` | Existing short-lived OAuth bearer with reviewed Auth-create authority; the API performs no OAuth refresh or CLI-login import |
| `FIREBASE_PROJECT_BINDING_APPROVED`, `FIREBASE_ACTION_HANDLER_APPROVED`, `FIREBASE_PROVIDER_ACCESS_APPROVED` | Explicit `true` declarations after the corresponding independent acceptance; declarations alone prove none of them |

Existing Theatre OIDC issuer/client secret and exact Web origin must also match. Firebase
requires the retained `/oauth2/openid/cantos-theatre-web-bff` issuer. The action origin and API
are the existing same-origin BFF origin; cross-origin/Hosting authentication is not introduced.
Tenant, emulator-host, endpoint and independent action-origin inputs are refused. Provider
network origins are fixed Google HTTPS endpoints with certificate validation, no redirects,
no inherited proxies and fresh HTTP/1 connections; test-only loopback constructors do not exist
in production builds. Credentials, OOB codes and setup links are never persisted in receipts.

Migration0028 requires operator-reviewed grants for the runtime role: SELECT/INSERT on
`theatre_firebase_enrollments` and `theatre_firebase_send_starts`, UPDATE only
`phase,last_sent_at,person_id,block_reason` on enrollments, and USAGE on the send receipt
sequence. Transition receipts are written by the fixed-search-path trigger; grant no direct
INSERT/UPDATE/DELETE on those receipts, no table DELETE/TRUNCATE, and no migration authority.
Tests grant only this narrow set in a new disposable database; they change no deployed role.

## Flow and recovery contract

1. Generic start persists a fresh operation/UID and durable send budget before background
   work. The response contains no operation, provider credential, setup link or session.
2. A positive password-free, unverified Firebase create acknowledgment is persisted before
   token exchange. Existing UID/email conflicts are blocked; no lookup/adoption/reset/delete.
3. A server-only custom token obtains the sending ID token. A fresh database fence after that
   wait precedes `VERIFY_EMAIL` sending. `continueUrl` is the configured Web origin with only
   `?firebaseEnrollment=OPERATION_UUID`; it contains no proof/setup credential.
4. Configure a **custom** Firebase action handler on that origin. The default handler must not
   consume the code before Cantos. The Web client captures the standard `mode`, `oobCode` and
   same-origin `continueUrl` query in RAM and scrubs it before application I/O. It ignores the
   URL's API key as authority. Explicit Confirm performs non-consuming purpose/email inspection,
   then consumes the same code and requires exact dedicated UID/email. Provider status/ID tokens
   never replace action possession.
5. Fresh durable fences after waits precede acknowledged-new Kanidm create, setup intent and
   one-shot handoff. Only this confirming response receives the pinned `/ui/reset?token=...`
   link and derived login. No raw/encrypted recovery copy exists. Genuine OIDC and `/session`
   confirmation are still required for login; Studio/Admin remain separate.
6. Explicit Cancel on an unsubmitted email action purpose-checks and consumes that same action,
   then records Cancelled without Kanidm creation. Lifecycle/Back/Escape merely discard local
   possession. An already winning confirmation cannot be rolled back. Cancellation requests
   need connectivity; a generic accepted response is not proof that a race was won.

Resend keeps the same UID and **absolute** deadline; any still-valid provider code for that
operation may win. No latest-code invalidation is claimed. Cancellation/expiry permanently
terminates that operation/UID. A fresh successor can hit `EMAIL_EXISTS` while the old Firebase
record is retained; this requires authorized reconciliation, never automatic recreation or reset.
A timeout, process loss, malformed success or lost local commit after a mutating effect leaves a
non-replayable phase. A lost setup response/navigation needs reconciliation, not a new intent.
No background recovery/cleanup is supplied; PII/budget/audit retention needs operator review.

## Acceptance before enabling

- Verify the owner's requested project `cantos` by exact ID/display name and inspect existing
  Auth users/template consumers with separately authorized read access. Do not switch projects
  if it is unavailable. Existing CLI authentication, which can read project metadata, is not
  server bootstrap authority. A failed login must be repaired by the operator, never silently.
- Independently establish API-key/project association and service-account effective authority.
  Google's read-only [key lookup](https://docs.cloud.google.com/api-keys/docs/reference/rest/v2/keys/lookupKey)
  requires existing `apikeys.keys.lookup`/OAuth permission; it is not a request to grant one.
  Do not put a key in a logged URL/argv. Review Auth create/signing and Kanidm existing-person
  protection; application witnesses do not limit a stolen bearer.
- Name action/API/Kanidm origins, approved recipient and mail-volume budget. Changing Auth
  action templates/authorized domains, creating credentials/grants/accounts, real mail and
  deployment each need their concrete authorized target. Code approval provides none of them.
- Before the **initial action GET**, ensure the edge/CDN has no query-string logging, analytics,
  referrer leakage or caching. JS scrubbing cannot undo ingress/history leakage. Serve no-store
  HTML/action responses and no-referrer; GET/scanner requests must not provision.
- Run the local layers below, then the separately authorized real journey: Firebase mail →
  explicit action → one-shot Kanidm setup → user credentials → genuine OIDC → API session →
  reload/logout and direct Studio/Admin denial. Check actual BFF cookie/origin behavior.

## Independent local commands

```sh
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test --locked -p cantos-api --lib theatre_registration
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify postgres --suite theatre-firebase
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask build cantos-theatre --web
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight --changed-only
```

The scoped PostgreSQL gate verifies Firebase SQL and existing SMTP compatibility in a new
cluster; it does not replace full `verify postgres` or retain an app fixture. The optional
ignored Auth-emulator test uses an **already installed** `firebase-tools` auth/server.js module
named by `CANTOS_TEST_FIREBASE_AUTH_MODULE`, Node and a fresh loopback-only `demo-*` project:

```sh
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test --locked -p cantos-api --lib platform::theatre_registration::firebase_action::emulator_tests::installed_auth_emulator_exercises_exact_action_consumption_and_resend_limits -- --ignored --exact
```

Use the full printed Rust test path with `--exact`, or omit `--exact` for the unique suffix.
The test child clears inherited credentials and blocks outbound sockets. It sends no real mail
and uses emulator-only fake tokens; production accepts neither those tokens nor an emulator URL.
The installed emulator ignores key-to-project association, lacks realistic OOB expiry and keeps
sibling resend codes. Its results cannot accept project binding, IAM, delivery, signatures,
revocation or production resend/expiry semantics. If ID tokens ever become admitted evidence,
add a separate exact RS256/rotating-key/time/revocation verifier; this flow admits no ID tokens.
