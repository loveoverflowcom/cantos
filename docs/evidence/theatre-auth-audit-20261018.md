# Theatre sign-in/sign-up audit: 18 October 2026

Evidence record (not acceptance). Source: `develop` at `1f8aae9`. Host: Linux, local Chrome 155
(throwaway profile), local Kanidm 1.10.5 and PostgreSQL 17.11 containers, fresh epoch-2 database.

## Local (Bearer, Web `https://localhost:8443`)

| Check | Result |
| --- | --- |
| Sign-in popup → Kanidm (password + TOTP) → Bearer exchange → "signed in" account view | PASS (real Kanidm, interaction-observed) |
| Sign-out | PASS: family `revoked=t` |
| Sign-up UI with registration unconfigured | PASS: "not open yet", no fallback |
| `cargo test -p cantos-api --lib theatre_registration` | 135 passed, 28 ignored |
| Firebase emulator lane | NOT_RUN: no `initdb`/`pg_ctl` on this host |
| Real Firebase sign-up | NOT_RUN: would send real mail; no recipient authorised |

Prerequisite found: the Kanidm client must register
`<web origin>/api/v1/theatre/auth/bearer/callback` in addition to the legacy callback, otherwise
Kanidm answers `invalid_origin`. An epoch-1 database makes the API fail with `LegacyDatabase`.

## Production (read-only probes, then one refused deploy)

- VPS ran `4d2153c` (35 commits behind): Bearer routes 404, registration `registration_unconfigured`.
- Hosting bundle `cantoss-theatre.web.app` is pre-Bearer and built for `https://api.cantos.cc`;
  a browser sign-in is refused by the API with 403 (`Sec-Fetch-Site: cross-site`), and the legacy
  redirect names `api.cantoss.duckdns.org`. **Production sign-in is not working.**
- `cargo xtask deploy vps cantos-api` for `1f8aae9` built/uploaded, then the host refused (exit 66):
  `oidc-callbacks.env` lacks `CANTOS_THEATRE_OIDC_BEARER_CALLBACK`; a separately authorised
  `identity --reconcile-existing` is required first. Containers and `state/current` unchanged.
- Hosting preview not deployed: without the API and callback it could not log in.

Next (needs operator authority): reconcile existing Kanidm clients, redeploy API, then Hosting.
