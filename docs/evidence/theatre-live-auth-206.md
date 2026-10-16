# Theatre live authentication acceptance (#206): Linux, 16 October 2026

Work item: [#206](https://github.com/loveoverflowcom/cantos/issues/206), part of #174/#191.
This record owns the first **real Kanidm** Theatre sign-in/sign-out and the defects it exposed. It
supersedes none of the [Firebase runtime ledger](firebase-enrollment-runtime.md): that ledger's
fixtures, emulator and SQL results remain valid for their layers, and the real Firebase
sign-up leg below remains **NOT_RUN** until it has an actual entry.

## Authority, scope and provenance

The product owner asked for a new PR that accepts real sign-up/sign-in and permits code repair,
and named the existing Firebase project `cantos-a348b` instead of a separate test project. The
earlier approval package (local PostgreSQL/Kanidm, a scoped test CA, a Kanidm onboarding
principal, at most three test mails to the named recipient) applies to the local pieces below.
No production identity, deployment or tunnel was used. All secrets, keys and the throwaway
profile live outside Git in a mode-0700 directory; none appears in this record.

Source: `develop` at `64b5a60` plus this branch's working tree. Host: Linux x86_64, Rust 1.98.1,
rootless Podman 5.7.0, Trunk 0.21.14, Google Chrome 155.0.8059.84 (throwaway profile, driven over
the DevTools protocol; its only certificate exception is the SPKI of the local test leaf).

| Component | Pinned / actual |
| --- | --- |
| Kanidm | `docker.io/kanidm/server:1.10.5`, `https://idm.cantos.localhost:8444`, tools image `kanidm/tools:1.10.5` |
| PostgreSQL | `docker.io/library/postgres:17.11-bookworm`, migrations 0001–0028 applied by `cantos-migrate`, runtime role with only the documented narrow grants |
| Web origin | `https://localhost:8443` (Caddy 2.11.7 TLS front, local leaf) → Trunk 8082 → API 8080; `https` because `http://localhost` is refused by the OIDC origin rule and `127.0.0.1` is not a Firebase authorized domain |
| Trust | one local test CA, valid 60 days; trusted by the API only through the new `CANTOS_THEATRE_OIDC_CA_FILE`, never by a system store |

`cargo xtask services up` reported `UNSUPPORTED` (Podman 5.7.0 lacks named-volume `nocreate`);
the same labeled volumes were used with equivalent direct `podman run` commands, bound to
`127.0.0.1`. That deviation is part of this record, not a claim about the xtask path.

## Invariant and ownership ledger

| Claim / invariant | Failure mode | Oracle | Evidence |
| --- | --- | --- | --- |
| A private issuer authority is trusted only by that issuer's OIDC and onboarding clients and replaces public roots | Process-wide trust, silent skip of a non-certificate block, trust used with an HTTP issuer | `platform::oidc::trust` unit tests; live handshake against the local Kanidm | example-tested; integration-tested (real TLS to Kanidm) |
| The dev proxy must hand the issuer redirect to the browser | A proxy that follows the 303 server-side fails on the issuer's TLS (HTTP 500) and hides the redirect | `xtask` config test (failed before, passes after); browser login | example-tested; interaction-tested |
| Theatre login yields only a Theatre session | Studio/Admin reuse of the session | live probe of both route groups with the signed-in cookie | interaction-tested |
| Logout revokes locally and at the provider | Pending or missing revocation | session 401 after logout; row `revoked=t, revocation_pending=f` | interaction-tested; integration-tested |

## Executed results

| Step | Result | Notes |
| --- | --- | --- |
| Local PKI, Kanidm, PostgreSQL, migrations, OAuth2 client `cantos-theatre-web-bff`, onboarding principal | PASS | client scope map `idm_all_persons`→`openid` is a **test** choice: Kanidm onboarding creates persons without groups, so new persons reach the client only through a built-in group. The accepted mapping is an operator decision |
| Kanidm onboarding adapter contract (create, update-intent) with the real bearer | PASS | `POST /v1/person` 200 `null`; intent 200 `{token,expiry_time}`, JSON; same two calls as `KanidmIssuer`, issued with `curl`, not the production code |
| Real sign-in: Kanidm password + TOTP + consent, callback, `GET /api/v1/theatre/session` 200 | PASS | Kanidm requires MFA for the test person; the signed-in account view was opened and inspected |
| Reload keeps the session; `GET /api/v1/studio/session` 401, `/api/v1/admin/session` 404 | PASS | |
| Sign-out via the UI | PASS | session 401, local row revoked, provider revocation completed; fresh load shows signed-out view |
| Library `continue`/`history` and public `works` | 503 | the test role lacks catalog/discovery table grants; outside this slice and not a login defect |
| API library tests / Clippy / fmt | 628 PASS (94 ignored) / PASS / PASS | `cargo test --offline --locked -p cantos-api --lib`, `cargo clippy --offline --locked -p cantos-api --all-targets -- -D warnings`, `cargo fmt --all -- --check` |
| xtask config tests | PASS (21) | `cargo test --offline --locked -p xtask config::tests` |

## Defects found by the live run and repaired

1. **No way to trust a private issuer CA.** The API's `reqwest` uses the bundled public roots, so an
   HTTPS Kanidm with a private authority could not be reached (`UnknownIssuer`). Added the
   opt-in `CANTOS_THEATRE_OIDC_CA_FILE` (`platform/oidc/trust.rs`), applied to the Theatre OIDC
   provider and the registration onboarding client only.
2. **Theatre Trunk proxy followed the login 303.** `/api/v1/theatre/auth/login` returned HTTP 500
   through the dev proxy because Trunk followed the issuer redirect itself. `no_redirect = true`
   is now set, with a regression test. Studio's `Trunk.toml` has the same shape and was **not**
   changed or exercised here.

## Not run / blockers

- **Real Firebase sign-up: NOT_RUN.** It needs a project-bound API key, a service-account key for
  custom-token signing, a short-lived admin bearer, a project action-URL change to the local
  origin, and real mail to the approved recipient. The session's permission classifier denied
  writing those credentials to a file, so none was created or read; no Firebase user was created
  and no mail was sent. Next owner: the user, who decides how credentials reach the API process.
- Kanidm existing-person protection for `idm_people_on_boarding`, retention, and the accepted
  scope-map group remain operator/identity review items (see the
  [runbook](../deployment/firebase-enrollment-proof.md)).
- The required repository qualification gate still carries the 20 inherited failures; this
  change touches no qualification input.
- Screen-reader, mobile and native acceptance are unrun.
