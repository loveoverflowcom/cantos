# Theatre enrollment audit and edge hardening: 17 October 2026

Scope: Firebase-mode Theatre enrollment (Firebase mailbox proof, Cantos API as sole orchestrator,
Kanidm for credentials/OIDC) against the production runtime of merged
[PR #231](https://github.com/loveoverflowcom/cantos/pull/231) /
[#234](https://github.com/loveoverflowcom/cantos/pull/234). Source: `develop` at `451d3a0` plus this
branch. This is not activation evidence: registration stays disabled in production. The hardening
change did not send mail, create accounts/keys, change IAM or deploy; the separately authorized
live probes below have their own effect and cleanup boundaries.
Related: [ADR 0036](../decisions/0036-firebase-enrollment-proof.md),
[ADR 0040](../decisions/0040-firebase-email-proof-alternatives.md) (proposed),
[#206 record](theatre-live-auth-206.md), [ADR 0039](../decisions/0039-vps-production-runtime.md).

## Findings

**Confirmed defects (fixed here)**

1. *Quota collapses behind the edge.* Registration keyed attempt and durable mail budgets on the
   socket peer only (`PEER_POLICY=direct_peer`). Behind Caddy the peer is the edge container for
   every user, so one user could exhaust everyone's global/per-network budget. Fixed by the
   `trusted_proxy` policy (below).
2. *Email action code reached the static upstream.* The Theatre host proxied the whole query,
   including the one-time `oobCode`, to Firebase Hosting and set no `no-store`/`no-referrer` for
   it. The edge now strips the code from the upstream URL and sets both headers.

**Security blocker (open, not fixable in code)**

3. *The default `VERIFY_EMAIL` handler plus polling/continuation cannot supply the proof.* See
   [ADR 0040](../decisions/0040-firebase-email-proof-alternatives.md): for `VERIFY_EMAIL`, polling `emailVerified`
   or following `continueUrl` would hand B's setup capability to A. The accepted design
   (custom handler consuming the code) is blocked by the Spark project's refusal to set the
   action URL.
4. *Kanidm management API publicly reachable.* The identity host proxied every path, including
   `/v1/person`, `/v1/oauth2`, credential-intent creation, to the Internet. Bearer-gated, but it
   widened the surface for the onboarding bearer. The edge now answers 404 for those prefixes
   unless the TCP peer is private.
5. *Broad Admin SDK key in use and not retired* (inherited from #206; operator action).

**Architectural mismatch**

6. ADR 0036 requires a custom action handler on the Cantos origin, which the owner's Firebase
   project refused. The controlled option-4 probe now has positive evidence, but does not amend
   ADR 0036; an explicit email-link design and its implementation acceptance remain required.

**Production integration gaps**

7. Runtime grants for registration were absent by design; now an optional, tested block applied
   only when registration is enabled (and revoked otherwise).
8. No Quadlet mount for `FIREBASE_SERVICE_ACCOUNT_FILE`; the production Web origin must be added
   to Firebase authorized domains. Operator work, documented, not done.
9. Stock Caddy has no rate limiter; limits are the application window and durable budgets, per
   process.

**Optional (not done)**

10. Quint registration model is draft/model-only; a scenario for "initiator differs from mailbox
    owner" would only restate the possession rule and was not added. The Web signup state
    machine (`application/signup.rs`) already separates email accepted, awaiting confirmation,
    verifying, setup ready/opened, link unavailable, unknown outcome and capture failure and never
    claims registration complete; it was read, not changed, and no UI observation was run.

## Changes and invariants

| Change | Invariant | Owner |
| --- | --- | --- |
| `PeerPolicy`/`TrustedProxies` pure core, `PEER_POLICY=trusted_proxy` + `TRUSTED_PROXIES` | A client cannot choose its quota identity: the header counts only from a configured proxy network, is read right to left, fails closed when a trusted proxy sends no single usable value; `direct_peer` ignores it; `/0`, names and >16 entries refused | `platform::theatre_registration::domain::peer`, `runtime`, both provider configs, HTTP handlers |
| `theatre-registration-grants.sql` / `-revoke.sql`, conditional in `cantos-vps` | Runtime role can run exactly the Firebase enrollment repository: no DELETE/TRUNCATE, no binding-column UPDATE, no receipt-table or trigger-function access, no DDL | `infra/vps/postgres`, `infra/vps/bin/cantos-vps` |
| Caddyfile: `X-Forwarded-For` replaced, `Forwarded`/`X-Real-IP` dropped, registration body cap, `oobCode` headers and strip, Kanidm management filter | Spoofed forwarding headers never reach the API; email codes are not cached, referred or forwarded; only the private network reaches Kanidm administration | `infra/edge/Caddyfile` |

Unchanged by design: contracts, migrations, capability names, Web client, ADR 0036 accepted
direction. The Firebase credential mechanism (the API mints a short-lived bearer from a
service-account file) already exists and is fixture-tested; the narrow account remains designed,
not created.

## Evidence ledger

| Claim | Failure mode | Oracle | Level | Result |
| --- | --- | --- | --- | --- |
| Peer-policy decision | spoof, `/0` trust, left-entry choice, missing/duplicate/garbage header | `domain::peer::tests` (6 tests) | example-tested | PASS |
| Config | header trust without list, list with `direct_peer`, orphan list | SMTP and Firebase config tests | example-tested | PASS |
| Handlers use the policy | edge users pooled; direct access spoofing | `trusted_edge_partitions_quota_by_client_while_direct_access_cannot_spoof_it`, `direct_peer_policy_ignores_forwarded_headers_entirely` through the mounted router | mock-integration-tested | PASS |
| Grants exact and revocable | over-grant, missing grant, non-idempotent | `registration_grants_are_separate_minimal_idempotent_and_revocable`; 16 Firebase repository tests now run against the deployed grant file | integration-tested (disposable `postgres:17.11` podman container, not the `initdb` runner) | PASS |
| `cantos-vps` enable detection | commented/false/`truex` enabling | ad-hoc shell check of `registration_enabled` (4 cases) | example-tested | PASS (not a repository test) |
| Edge behavior | header spoof, code in upstream URL, caching, oversize body, Kanidm admin reach | pinned `caddy:2.11.7-alpine` against stub upstreams; `caddy validate` of the real file | example-tested (stubs; no TLS, no real Firebase or Kanidm) | PASS |
| Cross-device, lost-response, replay, concurrent confirm, Kanidm uncertain outcomes | duplicate provisioning | existing suites (`firebase_enrollment`, `firebase_action`, HTTP firebase tests) | unchanged; re-run PASS in the registration filter (129 tests) | PASS |

## Follow-up probe: action URL (owner-authorized)

Re-setting only the Auth action URL on `cantos-a348b` to `https://cantoss.duckdns.org/` was
rejected again with `EMAIL_TEMPLATE_UPDATE_NOT_ALLOWED`; read-back confirmed no change and no mail
was sent (details in ADR 0040). Evidence level: integration-tested against the real Identity
Toolkit admin API for the negative result only.

Second probe (`EMAIL_SIGNIN`, one mail to the approved recipient, email-link enabled then restored
by the owner): the send succeeded with a per-request `continueUrl`, and the default hosted handler
forwarded the code to the continue origin (details and limits in ADR 0040). Run by the owner on
their machine from scripts written for this purpose after the automatic permission classifier
refused the agent-run call; the agent saw only the owner-pasted output. The redemption of the
forwarded code was inconclusive (`INVALID_OOB_CODE`, with a confounding user deletion and delay).
Cleanup performed by the owner the same day: the #206 test Firebase user `cantos_4f25019f…` was
deleted and email-link sign-in was restored to disabled; the #206 service-account key, test
Kanidm person and project changes are not touched by this.

### Controlled fresh-mailbox follow-up

The owner selected a fresh mailbox for a bounded follow-up (at most two new mails). Direct lookup
confirmed it absent. With `disabledUserSignup=true` preserved, an admin `EMAIL_SIGNIN` send to the
absent user returned **400 `ADMIN_ONLY_OPERATION`**; no successful mail send was acknowledged.
Email-link was restored to disabled and read back. This is integration-tested evidence of the
negative send result on this configuration, not a code-consumption result.

One dedicated, password-free, unverified test UID was then admin-created and read back for the
fresh mailbox. One mail was accepted at **07:50:20 UTC**, continuing to an already authorized
loopback origin. The temporary receiver requires explicit confirmation, measures arrival to
redemption, performs a replay only after a definite first success, and restores email-link after
completion or timeout. Both actual mails were retrieved through the connected Gmail mailbox
(both landed in Spam), opened in a browser through the default Firebase handler, and consumed
server-side through the diagnostic controller's explicit POST to the receiver.

| Claim | Oracle | Level | Result |
| --- | --- | --- | --- |
| Default `EMAIL_SIGNIN` handler leaves the forwarded code usable in these two journeys | Two first redemptions **200**, two replays **400 `INVALID_OOB_CODE`** | integration-tested (real Firebase + browser + diagnostic receiver, not Cantos routes) | PASS |
| Dedicated UID retained with client signup disabled | Exact response UID/email and admin lookup agree with the precreated UID; `isNewUser=false` both times | integration-tested | PASS |
| Controlled timing | Second handler opening 07:59:09.053 UTC → success 07:59:19.604 UTC (**10.551 s**); first dispatch 41.51 s after arrival only | integration-tested; no first-click timestamp | PASS for second run's requested interval |
| Replay cannot reuse the consumed code | Each first **200** followed by **400 `INVALID_OOB_CODE`**, before cleanup | integration-tested | PASS |
| Cleanup and restoration | Exact run-created UID deleted after both runs; lookup empty; `passwordRequired=true`, `disabledUserSignup=true`; authorized domains/action URL unchanged | integration-tested (direct admin read-back) | PASS |

No user deletion or configuration change occurred between either successful send and redemption.
The local browser-form POST was refused before any provider dispatch in the first run; the
diagnostic controller's POST performed redemption. This does not qualify the Cantos Web UI,
production CSRF handling or a scanner journey. Both new mails are spent; the original probe's
inconclusive result remains inconclusive.

The [ADR follow-up](../decisions/0040-firebase-email-proof-alternatives.md#controlled-follow-up-fresh-mailbox-2026-10-17)
owns the scope and interpretation. Raw local diagnostics remain ignored at
`artifacts/agent-runs/2026-10-17-email-link-controlled/`; no codes, tokens, keys or recipient address
belong in this record or Git.

### Documentation follow-up verification

This follow-up changes only ADR 0040, this record, the enrollment runbook and the current queue,
on source `b5f8ee23620049a31fd4051022939263566b3743`. The diagnostic receiver is ignored local
material; no production implementation, contract, migration or UI source changed.

`cargo xtask preflight --changed-only --base b5f8ee23620049a31fd4051022939263566b3743`
reports **FAIL repository**, with 22 stale preview-qualification findings; **rust-format NOT_RUN**
because the earlier gate failed. A clean-source `cargo xtask verify repository` at that same HEAD
produced the identical 22 findings. This is a pre-existing failure, not a passing handoff gate.
The separate preflight invocation with `--skip repository` runs the remaining docs-only gate;
its **PASS rust-format / NOT_RUN repository (explicit skip)** does not clear the required failure.
Detailed logs and the clean-baseline comparison stay in the ignored run record.

## Not covered / residual risk

- **Live behavior** of every edge change on the VPS, including real Kanidm sign-in and credential
  reset through the management-API filter (the UI's own use of `/v1/*` is unobserved).
- The trusted CIDR is the whole `cantos` bridge; the Studio API and Kanidm could set the header.
- Whether any plan lifts the action-URL restriction; `EMAIL_SIGNIN` now has positive bounded
  forwarding/consumption/replay evidence, but its design amendment and implementation/production
  acceptance remain open (ADR 0040).
- Least-privilege signing credential, remaining #206 key/person/project cleanup, project/key association,
  Kanidm existing-person protection, retention: unchanged operator gates.
- Web UI interaction, accessibility, mobile and screen-reader evidence were not run.
- Preflight gates NOT_RUN here: see the PR description for the exact list and reasons.
