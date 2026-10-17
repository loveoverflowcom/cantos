# Firebase enrollment: selected mode and operator acceptance

[ADR0036](../decisions/0036-firebase-enrollment-proof.md) accepts code and isolated tests for
Firebase mailbox proof, a dedicated UID per enrollment and one-shot Kanidm setup delivery.
**Live activation remains unaccepted.** No real project, secret, grant, account, mail recipient,
action template or deployment is created by configuration parsing. See the separate
[runtime evidence](../evidence/firebase-enrollment-runtime.md).

A first real journey against project `cantos-a348b` is recorded in the
[live record](../evidence/theatre-live-auth-206.md). It leaves these gates **open** before any
activation:

- **Custom action handler.** Setting the action URL failed on the Spark project, so the mail kept
  the default hosted handler. The requirement that the default handler never consumes the code first
  is unaccepted, and the first mail's code was invalid for an undetermined reason.
- **Least privilege.** The default Admin SDK key was used; a dedicated narrow service account, role
  and binding were never created. The API now mints its own Auth-create bearer from the key
  (implemented, fixture-tested; the one-hour limitation is gone), and the
  [service-account design and runbook](firebase-enrollment-service-account.md) proposes the narrow
  account, **not executed**. Signing/creation authority stays unaccepted.
- **Alternatives.** [ADR 0040](../decisions/0040-firebase-email-proof-alternatives.md) (proposed)
  rejects default `VERIFY_EMAIL` polling/continuation as proof. Its controlled `EMAIL_SIGNIN`
  follow-up observed two usable forwarded codes and refused replays while retaining a dedicated
  admin-precreated UID. This requires a design amendment and implementation verification;
  it does not activate the existing `VERIFY_EMAIL` flow.
- **Cleanup.** The owner deleted the original #206 test Firebase user on 17 October; see the
  [audit](../evidence/theatre-enrollment-audit-20261017.md). That does not retire the generated key
  (id prefix `2795eafc`), the test Kanidm person, local containers or project changes; their cleanup
  still needs its own scope. Later probe accounts have separate cleanup records.

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
| `NETWORK_KEY`, `PEER_POLICY`, `TRUSTED_PROXIES` | Existing nonzero 32-byte hex HMAC secret; `direct_peer`, or `trusted_proxy` with a required comma-separated list of proxy CIDRs (refused with `direct_peer`; see the [edge runbook](vps-production.md#theatre-registration-behind-the-edge-not-enabled)) |
| `VERIFICATION_SECONDS`, `RESEND_SECONDS`, `WINDOW_SECONDS`, `MAX_GLOBAL_REQUESTS`, `MAX_NETWORK_REQUESTS` | Explicit bounded durable lifetime/cooldown/mail budgets; resends consume the same budget |
| `SETUP_SECONDS`, `MAX_INFLIGHT`, `ATTEMPT_WINDOW_SECONDS`, `ATTEMPT_MAX_GLOBAL`, `ATTEMPT_MAX_NETWORK` | Explicit bounded setup lifetime and process-local admission policy |
| `FIREBASE_PROJECT_ID`, `FIREBASE_API_KEY` | Exact verified project/key association; API key stays server-side in this design |
| `FIREBASE_SERVICE_ACCOUNT_FILE` | Absolute existing secret-store file, regular and at most64KiB; service-account JSON with exact project, client email and RSA PKCS8 private key; only read at startup. The API signs a JWT-bearer assertion with it and exchanges it at the fixed `https://oauth2.googleapis.com/token` for the `identitytoolkit`-scoped bearer that Auth create needs; the token is RAM-only, renewed 5 minutes before expiry, single-flight |
| `FIREBASE_PROJECT_BINDING_APPROVED`, `FIREBASE_ACTION_HANDLER_APPROVED`, `FIREBASE_PROVIDER_ACCESS_APPROVED` | Explicit `true` declarations after the corresponding independent acceptance; declarations alone prove none of them |

Existing Theatre OIDC issuer/client secret and exact Web origin must also match. Firebase
requires the retained `/oauth2/openid/cantos-theatre-web-bff` issuer. The action origin and API
are the existing same-origin BFF origin; cross-origin/Hosting authentication is not introduced.
Tenant, emulator-host, endpoint, independent action-origin and the **retired**
`FIREBASE_ADMIN_ACCESS_TOKEN` inputs are refused: a static bearer expires within about an hour and
a second credential path would only preserve that failure, so it is not an alternative mode. Provider
network origins are fixed Google HTTPS endpoints with certificate validation, no redirects,
no inherited proxies and fresh HTTP/1 connections; test-only loopback constructors do not exist
in production builds. Credentials, OOB codes and setup links are never persisted in receipts.

Migration0028 requires operator-reviewed grants for the runtime role: SELECT/INSERT on
`theatre_firebase_enrollments` and `theatre_firebase_send_starts`, UPDATE only
`phase,last_sent_at,person_id,block_reason` on enrollments, and USAGE on the send receipt
sequence. Transition receipts are written by the fixed-search-path trigger; grant no direct
INSERT/UPDATE/DELETE on those receipts, no table DELETE/TRUNCATE, and no migration authority.
The VPS ships exactly this set as the optional
[`theatre-registration-grants.sql`](../../infra/vps/postgres/theatre-registration-grants.sql)
(applied only when registration is enabled); the repository's PostgreSQL tests run against that
file in a new disposable database and change no deployed role.

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
- Create the dedicated account, one-permission role and key by the
  [reviewed runbook](firebase-enrollment-service-account.md) (not executed), prove with one
  approved real enrollment that its token and role suffice, then retire the broad default Admin SDK
  key (id prefix `2795eafc`).
- Independently establish API-key/project association and service-account effective authority.
  Google's read-only [key lookup](https://docs.cloud.google.com/api-keys/docs/reference/rest/v2/keys/lookupKey)
  requires existing `apikeys.keys.lookup`/OAuth permission; it is not a request to grant one.
  Do not put a key in a logged URL/argv. Review Auth create/signing and Kanidm existing-person
  protection; application witnesses do not limit a stolen bearer.
- Enable the project's Email/Password sign-in provider (keep client sign-up disabled): the
  non-consuming action inspection otherwise fails with `PASSWORD_LOGIN_DISABLED`, which Cantos now
  reports as a configuration fault (503) rather than an unusable link. A Spark project refused to set
  a custom action URL; see the [live record](../evidence/theatre-live-auth-206.md).
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

The explicit isolated lane below is independent of the ordinary unit/SQL gates. Neither a
passing emulator run nor the existing real-mail record completes the full local Web acceptance.

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

## Isolated local enrollment testing

The implementation boundary for [#242](https://github.com/loveoverflowcom/cantos/issues/242)
is a test-only composition of existing enrollment code. Production configuration, provider
origins, registration activation and identity/session validation are unchanged. No test route,
inbox or emulator configuration is added to the shipped server or Web bundle. No new ADR or
registration service is needed: this exercises ADR0036's existing rules.

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify theatre-auth --suite firebase-emulator --dry-run
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify theatre-auth --suite firebase-emulator
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify theatre-auth --suite firebase-web-e2e
```

The dry run prints the plan without probing tools, starting subprocesses, creating fixture
resources, contacting a network or writing receipts at xtask execution. Cargo may still build
xtask before executing it. These lanes are opt-in; ordinary build/test/preflight/CI never start
the emulator or provision identity infrastructure. The Web lane reports `UNSUPPORTED`: the
owned Kanidm/PKI/Web fixture and complete credential-setup/OIDC browser journey are unfinished.
This is an implementation gap as well as an acceptance gap, not a skipped successful E2E test.

### Source-to-test map

| Boundary | Actual component / fixture | Evidence limit |
| --- | --- | --- |
| Registration HTTP | Existing Theatre registration handlers and router | Generic start response; possession and explicit confirmation remain required |
| Durable state | Existing `firebase_enrollment::Service` and `PostgresRepository`, current migrations/grants in a fresh disposable cluster | Real SQL and one-winner semantics require the integration lane to run |
| Firebase bootstrap | Existing `FirebaseBootstrapClient` request construction, signing and response parsers; `cfg(test)` loopback transport and fixed mock access-token source | Does not exercise real OAuth, IAM or signature acceptance; existing signing/OAuth fixture tests remain separate |
| Email proof | Existing `FirebaseActionClient` against an owned Auth Emulator; inventory delta matches the new action and exact continuation | No real email, template configuration, API-key/project association or realistic provider expiry |
| Kanidm onboarding | Recording mock of the existing `Issuer` port | A setup-ready response proves neither real credential setup nor OIDC/session authentication |
| Recipient/browser support | `services/cantos-api/tests/support/theatre-auth/{recipient,browser}.mjs`, outside application routes and bundles | Helper tests use a small HTTP fixture, not the Cantos Web app or a real issuer |
| Stateful expectations | Existing draft `cantos.identity.theatre-registration` catalog and native application/SQL regressions | No new model or lifecycle promotion; model evidence never establishes adapter conformance |

### Prerequisites and private lifecycle

Prepare PostgreSQL 17's `initdb`, `pg_ctl` and `psql`, the repository Rust toolchain and Node
separately. Set `CANTOS_TEST_FIREBASE_AUTH_MODULE` to the absolute path of an already installed
`firebase-tools` **14.2.2** `lib/emulator/auth/server.js`. The internal module is version-sensitive;
a different globally installed Firebase CLI is not a replacement. The runner installs nothing,
does not log in and never selects a cloud project or developer ADC/service-account credentials.
Missing or incompatible prerequisites produce `UNSUPPORTED`, not an empty successful run.

The lane uses a run-owned `demo-*` project, `.test` addresses, literal loopback endpoints and a
fresh disposable PostgreSQL cluster. It never accepts an inherited production database URL or
reuses an operator's emulator/project. Runtime egress must be blocked before executing the test;
the Linux namespace prerequisite and its forbidden-destination checks are part of the runner.
Other hosts without this isolation report `UNSUPPORTED` rather than weakening it.
The private browser helper uses Chrome's debugging pipe, a fresh profile and separate browser
contexts, with an exact-origin proxy/request guard. Its fixture tests can exercise input and
cookie isolation, but do not demonstrate the actual Cantos UI, local PKI or Kanidm login.

Only owned children and directories are cleaned on normal completion, failure or cancellation.
No global process killing, shared emulator reset, volume pruning or existing-account deletion
is allowed. Never recover a failed run by resending a real action, editing verified flags or
manufacturing a session. After an interrupted run, use only its exact owned resource identity;
do not guess a cleanup target from a port or process name.

Provider/action/setup/session secrets must remain private transient values. Do not enable
Firebase debug output, browser URL logs, screenshots, HAR or traces containing those values.
Normal reports carry statuses/counts/tool/source identity, never proof or setup values. The
recipient must preserve the exact server-selected `continueUrl` binding and must not navigate
the emulator's default `oobLink`, which can consume the code before Cantos confirms it.

### Remaining acceptance

Run the integration lane with its pinned tools before claiming actual emulator/PostgreSQL
coverage. Deterministic existing tests cover purpose/binding failures, provider wait expiry,
cooldowns, replay/races, cancellation and uncertain create/send/consume/setup outcomes; these
are separate from emulator observations. Preserve the emulator's sibling-code behavior rather
than claiming newest-code-only invalidation.

The complete #242 gate still needs an owned real local Kanidm with scoped trust and access,
actual Theatre Web credential setup and genuine OIDC, API-confirmed anonymous-before-login and
authenticated-after-login `/session`, reload/logout, and separate-browser A-starts-for-B
isolation. A mock issuer cannot satisfy these criteria. Assignment is required before accepting
this remaining fixture work. Native/device, real mail/IAM/action-template/project binding,
production activation and deployed cross-site Hosting/cookie acceptance stay separately owned
by #174/#191/#206/#199; this local lane cannot close them.
