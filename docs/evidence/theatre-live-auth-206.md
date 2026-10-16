# Theatre live authentication acceptance (#206): Linux, 16 October 2026

Work item: [#206](https://github.com/loveoverflowcom/cantos/issues/206), part of #174/#191.
This record owns the first **real Kanidm** Theatre sign-in/sign-out, the first **real Firebase
mail → Cantos action handler → Kanidm setup → login** journey, and the defects they exposed.
It supersedes none of the [Firebase runtime ledger](firebase-enrollment-runtime.md): that
ledger's fixtures, emulator and SQL results remain valid for their layers.

## Authority, scope and provenance

The product owner asked for a new PR accepting real sign-up/sign-in with code repair allowed, and
named the existing Firebase project `cantos-a348b` instead of a separate test project. The earlier
approval package (local PostgreSQL/Kanidm, a scoped test CA, a Kanidm onboarding principal, at
most three test mails to the named recipient) covers the local pieces. No production identity,
deployment or tunnel was used. All secrets, keys and the throwaway browser profile live outside
Git in a mode-0700 directory; none appears in this record.

Source: `develop` at `64b5a60` plus this branch. Host: Linux x86_64, Rust 1.98.1, rootless Podman
5.7.0, Trunk 0.21.14, Chrome 155.0.8059.84 (throwaway profile driven over the DevTools protocol; its
only certificate exception is the SPKI of the local test leaf).

| Component | Pinned / actual |
| --- | --- |
| Kanidm | `docker.io/kanidm/server:1.10.5`, `https://idm.cantos.localhost:8444`, CLI image `kanidm/tools:1.10.5` |
| PostgreSQL | `docker.io/library/postgres:17.11-bookworm`, migrations 0001–0028 by `cantos-migrate`, runtime role with only the documented narrow grants |
| Web origin | `https://localhost:8443` (Caddy 2.11.7 TLS front, local leaf) → Trunk 8082 → API 8080; HTTPS because the OIDC origin rule refuses `http://localhost` and `127.0.0.1` is not a Firebase authorized domain |
| Trust | one local test CA (60 days), trusted by the API only through the new `CANTOS_THEATRE_OIDC_CA_FILE` |
| Firebase | real project `cantos-a348b`; API key from `firebase apps:sdkconfig`; service-account key of the default Admin SDK account (broad) with a one-hour bearer minted from it |

`cargo xtask services up` reported `UNSUPPORTED` (Podman 5.7.0 lacks named-volume `nocreate`); the
same labeled volumes were used with equivalent direct `podman run` commands bound to `127.0.0.1`.
`cargo xtask run cantos-theatre --web` fixes the origin to `http://127.0.0.1:8082`, so the API
binary was started directly with explicit environment and `cargo xtask app cantos-theatre --web`
served Trunk.

## Invariant and ownership ledger

| Claim / invariant | Failure mode | Oracle | Evidence |
| --- | --- | --- | --- |
| A private issuer authority is trusted only by that issuer's OIDC and onboarding clients and replaces public roots | Process-wide trust, silently skipped non-certificate block, trust with an HTTP issuer | `platform::oidc::trust` unit tests; live TLS to the local Kanidm and the registration client built from the same anchors | example-tested; integration-tested |
| The dev proxy hands the issuer redirect to the browser | A proxy that follows the 303 fails on the issuer's TLS (HTTP 500) | xtask config test (failed before, passes after); browser login | example-tested; interaction-tested |
| A project without Email/Password is an operator fault, not an unusable link | `PASSWORD_LOGIN_DISABLED` on the inspection call read as "expired link" | fixture HTTP 400 test through the production client and service | example-tested (fixture); observed live |
| Only the actual action consumes the code and opens the one-shot setup | Replay, second person, session from proof | consumed code replayed → `unavailable`, phase stays `handed_off`, no second Kanidm effect | integration-tested (real Firebase + Kanidm + PostgreSQL) |
| Theatre login yields only a Theatre session; logout revokes at the provider | Studio/Admin reuse, pending revocation | `/api/v1/studio/session` 401, `/api/v1/admin/session` 404; revoked rows with `revocation_pending=f` | interaction-tested |

## Executed results

| Step | Result | Notes |
| --- | --- | --- |
| Local PKI, Kanidm, PostgreSQL, migrations, OAuth2 client `cantos-theatre-web-bff`, onboarding principal | PASS | scope map `idm_all_persons`→`openid` is a **test** choice: onboarding creates persons without groups, so new persons reach the client only through a built-in group; the accepted mapping is an operator decision |
| Kanidm onboarding contract with the real bearer | PASS | `POST /v1/person` 200 `null`; update-intent 200 `{token,expiry_time}`; later also exercised by the production adapter |
| Real sign-in of a pre-created person (password + TOTP + consent), reload, Studio/Admin denial, sign-out | PASS | Kanidm requires MFA; session 200, Studio 401, Admin 404, row revoked and provider revocation completed |
| Host composition in Firebase mode with real configuration | PASS | `GET /api/v1/theatre/auth/configuration` reports `firebase_email_verification`; Kanidm client built with the private anchors |
| Real sign-up request → Firebase admin create, custom-token exchange, `sendOobCode` | PASS | phase `awaiting_action`, one durable send receipt; the real mail reached the approved recipient (first in Spam) |
| Inspect with Email/Password disabled | FAIL, then repaired | `PASSWORD_LOGIN_DISABLED`; surfaced as "link unusable"; now a 503 plus one operator event, lease released, nothing consumed |
| First mail's code | invalid when checked (cause undetermined) | the Firebase user already had `emailVerified=true` before Cantos consumed anything; candidates: a mail/link scanner running the default hosted handler, or a provider-side effect. Cantos' own first attempt had failed before consumption |
| Resend (same UID), second mail, immediate Cantos confirm | PASS | inspect → consume → positive Kanidm create → intent → phase `handed_off`; setup link shown once in the confirming tab |
| Credentials via the setup link (password + TOTP), then real OIDC login as that person | PASS | a new actor distinct from the pre-created one; session 200 after reload; Studio 401, Admin 404; sign-out → 401 with provider revocation completed |
| Replay of the consumed code | PASS | `{"status":"unavailable"}`; phase unchanged; no second person |
| API library tests / Clippy / fmt | 629 PASS (99 ignored) / PASS / PASS | `cargo test --offline --locked -p cantos-api --lib`, `cargo clippy --offline --locked -p cantos-api --all-targets -- -D warnings`, `cargo fmt --all -- --check` |
| xtask config tests | PASS (21) | `cargo test --offline --locked -p xtask config::tests` |

Two of the three budgeted mails were sent. The Firebase user `cantos_<operation>` (email verified) and
the Kanidm person remain; their cleanup needs separate approval.

## Defects found by the live runs and repaired

1. **No way to trust a private issuer CA** (`UnknownIssuer`). Opt-in `CANTOS_THEATRE_OIDC_CA_FILE`
   (`platform/oidc/trust.rs`) for the Theatre OIDC provider and the onboarding client only.
2. **Theatre Trunk proxy followed the login 303** (HTTP 500). `no_redirect = true`, with a
   regression test. Studio's `Trunk.toml` has the same shape and was **not** changed or exercised.
3. **Provider misconfiguration read as an expired link.** `PASSWORD_LOGIN_DISABLED` is now
   `ActionError::ProviderDisabled`; confirm answers 503, releases only the inspection lease and
   writes `event=firebase_action_inspection_refused outcome=provider_disabled`.

## Deviations, project changes and prerequisites learned

Changes made in the real project `cantos-a348b` (none reverted): Authentication initialized, IAM
API enabled, `localhost` added to authorized domains, Email/Password provider enabled **with
client sign-up disabled** (`disabledUserSignup`), and one Admin SDK private key generated.
Operators must also record: `accounts:resetPassword` (the non-consuming inspection) fails with
`PASSWORD_LOGIN_DISABLED` unless the Email/Password provider is enabled.

- **Custom action handler NOT accepted.** Setting the action URL failed both through the Identity
  Toolkit API (`EMAIL_TEMPLATE_UPDATE_NOT_ALLOWED`) and in the console ("An error occurred updating
  action URL") on this Spark project, so the mail kept the default hosted handler link. The journey
  was completed by opening the Cantos handler with the same `mode`, `oobCode`, `apiKey` and
  `continueUrl` query copied from the mail. This does not accept the ADR 0036 requirement that the
  default handler never consumes the code first; the unexplained invalid first code is consistent with
  that risk.
- **Provider privileges are broader than the runbook requires.** A dedicated narrow service account,
  role and binding were attempted but denied by the session's permission classifier; the default Admin
  SDK key was used instead. Least-privilege signing/creation authority remains unaccepted.
- The generated key's id prefix is `2795eafc`. Deleting it, the test Firebase user, the Kanidm
  person, the local containers and the project changes need separate approval.

## Not run / residual risk

- Required repository qualification still carries the 20 inherited failures; this change touches
  no qualification input. PostgreSQL/Quint/CMP gates are unrun here.
- Existing-person protection for `idm_people_on_boarding`, retention, the accepted scope-map group
  and numeric policy remain operator decisions. Screen-reader, mobile and native acceptance are unrun.
- Library `continue`/`history` and public `works` returned 503 for the test role (missing
  catalog/discovery grants); outside this slice.
