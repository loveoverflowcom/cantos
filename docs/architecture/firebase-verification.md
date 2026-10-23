# Firebase verification boundary

**Status:** owner-selected verification provider target (23 October 2026); isolated local
emulator lifecycle and configuration guards implemented. Native/Firebase proof bridge and
production adapter **NOT_IMPLEMENTED**. This is the corrected task's explicit fallback,
not acceptance of Firebase-backed native signup. No live mail, cloud setup or deployment.

## Authority and inspected sources

Native Identity owns subjects, password credentials, security state, mailbox proof expiry and
single-use decisions in `identity.db`. Server alone issues application sessions after native
password authentication. Firebase supplies an external verification action, never a Cantos
login, actor binding, setup session or authenticated session. Matching an email never links accounts.

Reviewed native base `7057ab41720b9bb341b27bae8a08f0ea7211b6ae`, issues
[#242](https://github.com/loveoverflowcom/cantos/issues/242),
[#334](https://github.com/loveoverflowcom/cantos/issues/334),
[#335](https://github.com/loveoverflowcom/cantos/issues/335),
[ADR 0045](../decisions/0045-v1-email-password-sign-in.md) and the
[native amendment](native-turso-refactor.md#owner-amendment-firebase-verification-provider).
The older issues' PG/Kanidm journeys are historical; their possession, explicit-confirmation,
enumeration, one-winner and uncertain-outcome requirements still apply to native auth.

Official API documentation checked on 23 October 2026:

| Source | Technical finding |
| --- | --- |
| [Firebase Auth REST](https://firebase.google.com/docs/reference/rest/auth#section-send-email-verification) | `VERIFY_EMAIL` sends an action for a Firebase account using its ID token. `accounts:update` consumes the OOB code. Neither operation is a generic SMTP interface. |
| [Identity Platform sendOobCode](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/sendOobCode) | `returnOobLink=true` returns a link **without sending mail**. It cannot expose the same native random proof through Firebase delivery. Privileged target-project access requires IAM; syntax alone never proves API-key/project association. |
| [Custom action handlers](https://firebase.google.com/docs/auth/custom-email-handler) | The handler receives `mode`, `oobCode` and optional `continueUrl`. The continuation is browser input, not authenticated native operation evidence. Template/handler configuration must be approved separately. |
| [Auth Emulator](https://firebase.google.com/docs/emulator-suite/connect_auth) | Demo projects have no live resources; unsigned emulator tokens must stay outside production. Its private REST inventory permits retrieving OOB actions without real mail. |

These are API capabilities, not production project/IAM/template observations. No actual project
binding, action handler or credential was configured or tested.

## Why the existing Mailer cannot implement this safely

[`ProofMail`](../../services/cantos-identity/src/mail.rs) carries a native random-secret link;
[`IdentityService`](../../services/cantos-identity/src/application.rs) commits only its digest,
then delivers the secret. Confirmation checks that exact secret and atomically commits native
proof consumption, subject, verified address and password. Secret material is absent after a
restart. A provider reporting acceptance does not change a proof's pending state.

Firebase generates a different code for a Firebase account. Sending the native link as a
`continueUrl` would disclose the native proof to a separate provider/handler and would not bind
its use to consumption of the Firebase action. Polling `emailVerified` loses action freshness,
purpose and operation binding. A decoded Firebase token is not native authentication evidence.
None of these shortcuts is implemented.

Historical `services/cantos-api/src/platform/theatre_registration/{firebase_action.rs,
firebase_bootstrap.rs,firebase_enrollment/}` was inspected at
`develop@1c970a311b01090c86372c60129451e08a8fb468`. Reusable ideas: bounded fixed-origin HTTP,
no redirects/proxies, sanitized typed rejected/uncertain outcomes, exact action purpose/email
inspection and UID validation after consume. The old bootstrap signs a dedicated UID custom
token only after privileged creation; a collision is not authority to adopt an existing account.
The old enrollment's PG fences and Kanidm setup issuer are **not** copied.

The #242 emulator fixture's empty child environment, pinned internal `createApp` launcher,
outbound socket denial and private action inventory are reused in xtask. This is explicitly
emulator-only: synthetic custom tokens accepted there prove neither production signing/IAM
nor native signup. The emulator does not qualify production expiry or API-key/project binding.

## Smallest viable bridge — proposal requiring security decisions

Keep the existing Web signup state machine and native login/session endpoints. Extend the
Identity-owned proof protocol and persistence rather than implement `Mailer::send` with a
continuation containing a native secret:

1. Add an additive Identity migration for provider enrollment/action receipts. Bind each to
   native instance epoch, operation, purpose, exact mailbox, dedicated Firebase UID and approved
   project. Persist positive-create provenance; never infer ownership from an email/UID lookup.
2. Reserve each create/send/consume effect durably before dispatch. Record rejected, accepted
   and uncertain outcomes separately. Crash/lost-response fences survive restart; neither a
   timeout nor a lookup of a verified account authorizes another consuming attempt.
3. Send a Firebase `VERIFY_EMAIL` action using a server-held credential for that dedicated UID.
   Continue with public locators only. Native secrets, passwords and setup capabilities never
   enter `continueUrl`, provider logs or the initiating browser's start response.
4. Capture the incoming action in the existing Web owner, scrub URL/history before app I/O,
   require explicit confirmation and a chosen native password. Server forwards a distinct
   typed Firebase action to Identity; do not reinterpret the existing 32-byte native secret.
5. Inspect exact purpose/mailbox; under a native one-winner claim recheck binding, expiry,
   cancellation and current proof. Consume the exact code and validate the resulting dedicated
   UID/mailbox. Persist a sealed provider-consumption receipt. Only its matching native proof
   transition can create a native subject/credential. The result grants no session.
6. Native password login remains a separate step. Initiator A cannot poll for mailbox owner
   B's credential-setup authority or session. Existing native or Firebase accounts are never
   adopted/reset/linked by email. Scanners and GETs perform no confirmation effects.

Unresolved decisions, all owned by Identity/auth review (unassigned until acceptance):

| Decision / observed blocker | Prerequisite / next check | Acceptance risk |
| --- | --- | --- |
| Provider UID lifetime vs Firebase email uniqueness and native resend replacement | Specify durable positive-create provenance, collision refusal, UID reuse/retention and cleanup authority; test concurrent operations for one mailbox with real emulator transport | Blind reuse/adoption or broken resend; no live account cleanup is authorized |
| Firebase consume succeeds but native commit/response is lost | Approve claim/receipt reconciliation and fail-closed terminal states; fault-inject every dispatch/commit boundary using real Turso | Provider state alone cannot recreate native setup authority; restart may require a new safely bound action |
| Native proof authorization representation | Review a sealed operation-bound provider receipt vs encrypted challenge custody with explicit key/TTL/restore policy; extend the owned RPC/HTTP contract and its negatives | Do not store plaintext native proofs or add a parallel unauthenticated completion route |
| Production project/action binding | Independently approve exact project, API-key association, service-account project/IAM, HTTPS handler and origin allowlist before activation | Syntactically valid config or emulator behavior is insufficient production evidence |

These are protocol/ownership decisions, not a request to supply cloud credentials now.

## Implemented local composition

`cargo xtask run cantos-theatre --web --open` checks an already installed Node.js and
`firebase-tools` **14.2.2** Auth module before building. It derives that module from the installed
`firebase` executable, or accepts the explicit absolute `CANTOS_FIREBASE_AUTH_MODULE` path to
`lib/emulator/auth/server.js`. It never invokes Firebase CLI, login, npx or an installer. Hosting
and App Distribution retain their separate 15.x CLI contract; no global package is upgraded.

xtask creates one private disposable directory, unique `demo-cantos-*` project, in-memory state
and ephemeral `127.0.0.1` port. The emulator has an empty environment, denied outbound TCP/TLS
with startup negative controls, suppressed provider output, bounded readiness and supervision
in the existing owned process-group lifecycle. Shutdown/failure removes only that new emulator
directory after stopping its child. Existing DB/WAL/media/build/evidence are preserved.

The console prints the **inventory endpoint**, not actions. Only a trusted local developer/test
process should read `/emulator/v1/projects/<this-run-project>/oobCodes`; keep codes/links in RAM,
out of terminal logs, traces and screenshots. There is no public application inbox or bundled
test helper. Loopback is local-host isolation, not authentication against other local processes.
Do not open the emulator's default action link before a future Cantos confirmation handler:
it consumes the code. Current inventory stays empty under native signup because the bridge
does not exist. The external adapter test creates its own synthetic account/action privately.

The actual Theatre origin is `http://127.0.0.1:8082/`, derived from the app configuration.
HTTP link parsing needs **both** the `local-development` Identity feature (selected only by
xtask run builds) and `CANTOS_IDENTITY_LOCAL_DEVELOPMENT=1`. Only explicit nonzero-port loopback
origins are allowed. Ordinary `service`/OCI builds reject that flag and retain HTTPS-only parsing.
Local composition rejects inherited Google/Firebase/SMTP configuration by variable name and
selects `CANTOS_IDENTITY_MAIL_PROVIDER=disabled`. Every signup remains `Unavailable` before
reading/writing a pending proof: this is a visible acceptance gap, never mock success.

## Production, images and VPS

`CANTOS_IDENTITY_MAIL_PROVIDER=firebase` or `firebase-emulator` currently refuses configuration
before credential reads or database access. Standard emulator overrides are rejected. `disabled`
refuses mixed SMTP settings; existing explicit `smtp` remains available outside local run.
There is **no production Firebase adapter in this slice**, including the OCI image. Missing
credentials cannot activate anything. Do not interpret image build/publish as mail readiness.

The Identity Containerfile selects `service` without `local-development`, defaults mail to
disabled and accepts no Firebase secret build argument. Build/publish never runs Identity serve
or an email API. Actual image-layer scanning is a separate NOT_RUN check until an image exists.
The manual native OCI publisher retains only its registry credential; no Firebase secret is added.

Future activation must use the VPS's existing operator-owned Podman secret/read-only runtime
file mechanism, explicit provider/project/HTTPS action validation, fixed Google endpoints and
bounded redacted transports. Reject SDK emulator overrides, unsigned emulator tokens, arbitrary
endpoints and inherited proxies in production. No provider credentials belong in Git, image
layers, Actions/release artifacts or nonsecret env templates. Native VPS activation is still a
separate #348 boundary; retained legacy Quadlets must not be repurposed by this source change.

## Acceptance still required

External emulator action creation/retrieval/inspection/consume/replay and lifecycle evidence
is independent of native auth. Required native Firebase journey remains **NOT_RUN**: genuine
pending proof, explicit one-time confirmation, chosen password, login, reload and logout;
wrong/expired/mismatched/replayed action, A/B, resend/races, rejection/timeout and crash recovery;
real Turso/RPC and real browser acceptance. Existing native tests do not qualify this new bridge.
Production config/credential/template/IAM integration, actual image scan and live mail are
NOT_RUN. Live mail is not required for this PR. No issue is closed by this preparation.
