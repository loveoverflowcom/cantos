# Native Identity Web cookie journeys on macOS

Source-bound local qualification for part of #295 and #335. This record does not close either
issue, qualify production domains, or accept the unrelated native business/publishing work.
The [Identity v1 contract](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) owns the rules.

## Provenance and boundary

- Final base: `origin/refactor/cantos-native-turso` at
  `443e7251f61aa89020c2afdfcbfb861ff5a807bf`. Initial fresh base after the user requested
  another fetch was `84eea4c3843908f4f21da9760381c129e21d5263` (including #389/#391/#392).
  The final remote readback found merged #393 overlapping Web-auth/CORS. The task work was
  checkpointed, then merged with that tip without reset/force-push; the README conflict retains
  both the upstream boundary notes and this evidence. Affected checks are repeated on the
  integrated source; no combined result is inferred from the earlier browser run.
- Branch: `test/335-web-identity-browser-acceptance`. One isolated Mac checkout:
  `/Users/manhblue/Documents/Codex/2026-10-23/task-4/cantos`. Other checkouts, stashes and data
  were not changed. A clean base source archive has an independent index for read-only comparison.
- Final tested source is that base plus this PR's scoped changes. The seven changed Rust files
  are bound by `source-manifest-integrated.json`, SHA-256
  `38d58b4daf6f33fd9deb1799967cd2a2c5c35e2085ead8bf244bb29fecd39a10`.
  Earlier results at the initial base retain their separate `source-manifest.json` digest
  `4e6af19dd463077131634c65c935ef2ba7aff84b4b7ebcbd7b8bf6e450f444f3`.
- macOS arm64; Rust/Cargo 1.98.1, Trunk 0.21.14, Leptos 0.7.8 CSR/debug, Node 22.15.0.
  Installed Playwright 1.62.1 drove headless Google Chrome **155.0.8059.85** in fresh contexts.
  No certificate bypass, security-setting change, synthetic successful fetch or stored browser
  profile was used. This one-off driver is not a registered repository/CI browser gate.
- Actual origins: Server `http://127.0.0.1:8080`, Theatre `http://127.0.0.1:8082`, Studio
  `http://127.0.0.1:8081`. Theatre exercised direct credentialed cross-origin requests; Studio
  used its existing development transport. These are same-site loopback **HTTP** observations.
- Native processes used this checkout's `target/debug/cantos-server` and `cantos-identity`,
  with authenticated private RPC at `/private/tmp/cantos-dev-501-a7ab84ce/identity.sock` (0600).
  Files were `target/native-dev/server/server.db` and `target/native-dev/identity/identity.db`,
  each under its own private directory. No development session or authentication bypass was used.
- The supported Theatre signup/complete endpoints enrolled fresh `example.test` addresses using
  a run-owned loopback SMTP sink. Generated passwords and mailbox proofs remained in process
  memory. Completion created no session; actual product forms then performed login. No mail
  left the Mac. Cookie values, tokens and passwords were not logged, traced or committed.

The full submitted `CANTOS_PROMPT_INPUT_V1` was read through Library (9,236 bytes, all 290 lines).
Local Library materialization remained blocked when the transfer helper's skill disappeared from
the runtime catalog; no cloud path or guessed download URL was used as a local file.

## Reproduced defects and repairs

| Defect | Before evidence | Narrow owner and repair | Regression |
| --- | --- | --- | --- |
| Cross-origin consumers could not read the limiter's actual wait (now fixed upstream in #393) | `each_cookie_surface_exposes_retry_after_without_exposing_credentials` failed: Theatre exposed-header list lacked `retry-after` | The task reproduced the defect before its fix; integrated #393 supplies the identical surface exposure change, so this PR now adds the HTTP regression without a duplicate Server source edit | Same test passes for Theatre and Studio, preserves status/value/credentials/exact origin, rejects foreign Origin and exposure of secrets/other surface fence |
| A mounted guest ignored another tab's login | Real production browser: peer login completed but guest performed no session re-read and kept its guest view | `owner::Client::external_change` invalidates the intent even without a local session and notifies views; it does not queue a revoke or adopt the broadcast | `a_peer_cookie_change_notifies_a_guest_without_adopting_authority`: failed before, passes after; same two-tab browser scenario passes after |
| A pending guest session read could accept pre-change authority; Theatre then failed to start a replacement observation | Owner test returned `Ok(Some(SessionView))` instead of `Changed`; delayed real A response after B login left Theatre without a fresh B observation | Existing lifecycle invalidation fences the owner read/login; Theatre's `begin_cookie_observation` supersedes the pending UI lookup, clears private state and preserves its own logout barrier. Runtime settles pending cleanup before the authoritative lookup | Owner pending-read regression failed before, passes after. Pure Theatre generation/old-completion/logout regression passes. Identical delayed real-response browser scenario passes after |

No cookie policy, session lifetime, credential fallback, CSRF admission, database schema or native
Bearer behavior changed. No new domain engine, dependency, framework or service was added.
Queue order and rationale are unchanged; no ADR is needed for these repairs to the existing contract.

## Actual browser assertions

All successful login/session assertions below used production Theatre/Studio WASM consumers and
the real native Server/Identity pair. H's raw negative fetches are explicitly narrower transport
observations. The deterministic E race delays an **unmodified real Server response**, rather than
inventing a successful response. Screenshots are not used as the oracle.

| Scenario / named assertions | Result | Evidence and limits |
| --- | --- | --- |
| A — production Theatre login and protected Library | PASS | Product form POST login returns 200; session DTO has only the five documented nonsecret fields; account logout control becomes available. Actual production Continue consumer returns 200 with the matching `x-cantos-theatre-session`; requests have no Authorization header |
| B — reload requires server confirmation | PASS | Reload emits real GET identity/session, receives 200 for the same session and restores the protected account UI |
| C — real-time renewal and invalidation | PASS | Wait for the actual 600-second idle policy; product account refresh within the renewal margin emits POST session/renew before expiry. Actor/family remain equal, idle deadline increases and stays within the absolute deadline. Wrong-fence renewal remains 409; after product logout, renewal is 401 |
| D — logout clears cookie and denies protected work | PASS for listed assertions | Product DELETE returns 204; cookie metadata disappears; a protected Library fetch returns 401; peer tab loses the protected account control. Late A completion is covered separately by E and the host cancellation regressions |
| E — late A response after switching to B | PASS | Hold a real A session response in a second tab, perform product logout/login B, then release it. Require a fresh Server-confirmed B read and zero protected sends stamped with A's old fence |
| F — BroadcastChannel, Web Lock and two-tab renewal | PASS for listed assertions | Guest peer login triggers authoritative re-read; peer logout clears protected UI; second-tab reload restores through the Server. Holding the actual surface Web Lock prevents production login POST dispatch until release, then login succeeds. After the real idle window, simultaneous product refresh in two tabs keeps one actor/family and absolute cap, and both tabs re-confirm the renewed session. The final-base run made one renewal call (the initial-base run made two); the second authoritative read can already see the extended deadline. Peer logout then removes both protected views |
| G — Studio/Theatre purpose and roster boundary | PASS for negative scope; authorized Studio NOT_RUN | Theatre cookie absent on Studio session request; cookie-less Studio session is 401; product Studio login with no configured roster is 404 and creates no Studio cookie. This does not qualify an authorized Studio account/workspace or reverse-direction isolation |
| H — Origin, preflight, 401/409/429/503 | PASS for listed transport/429 UI assertions; outage recovery partial | Real Studio Origin cannot read Theatre; undeclared preflight header is blocked; allowed Origin reads 401; wrong session fence reads 409 and its current nonsecret echo. Five wrong credentials return 401; the next product login returns 429, exposes actual Retry-After (899 seconds in that run), and holds its submit control without authenticating. Initial-base paused Identity produced readable 503 and preserved the cookie; final-base controlled outage/recovery is NOT_RUN, as detailed below |
| I — HTTP development cookie policy | PASS | Only `cantos_id_theatre`, host `127.0.0.1`, path `/api/v1/theatre`, HttpOnly=true, SameSite=Strict, Secure=false; Set-Cookie has no Domain attribute. Browser JS cannot observe it even from an eligible API cookie path whose session request succeeds. No credential prefix is found in browser storage and Theatre produces no Studio cookie. HTTPS Secure acceptance is separately NOT_RUN |

For H, the first probe incorrectly expected 404 for a cookie-less Studio session read. The actual
401 was correct: the contract's unconfigured-roster 404 applies to login. The corrected probe
checks both independently. No production expectation was relaxed. Browser APIs do not permit
forging duplicate Origin/request-method headers; the existing CORS Rust HTTP tests cover those
malformed forms separately.

## Setup, commands and local artifacts

Builds used `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_NET_OFFLINE=true TMPDIR=/private/tmp`. Initial free space was
about 10 GiB; usage was inspected during the run. No target/data cleanup was used to create space.

Existing orchestration:

```sh
CANTOS_IDENTITY_SMTP_HOST=127.0.0.1 CANTOS_IDENTITY_SMTP_PORT=25261 \
CANTOS_IDENTITY_SMTP_TLS=loopback-plaintext CANTOS_IDENTITY_SMTP_FROM=noreply@example.test \
cargo xtask run cantos-theatre --web
CANTOS_API_ORIGIN=http://127.0.0.1:8080 cargo xtask app cantos-studio --web
```

The one-off fixture/driver used the already installed Playwright module, not a new npm dependency:
`/Users/manhblue/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright`.
The browser executable was `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`.
The fixture implements only loopback test mail capture and the real HTTP signup/complete calls;
it supplies no authentication/session response. Local fixture/driver sources and sanitized logs
are retained under this checkout's ignored `artifacts/agent-runs/web-identity/`:

- `smtp_fixture.py`: in-memory disposable enrollment, SMTP 25261/control 25262.
- `browser_protected.cjs` / `browser-cookie-final.log`: A/B/D/F/I and actual protected consumer,
  including eligible-path HttpOnly and no-Domain assertions; `browser-cookie-integrated.log` repeats them on the final base (`browser-protected.log` is earlier).
- `browser_race.cjs` / `browser-race-integrated.log`: final-base E and actual Web Lock blocking (`browser-race-after.log` retains the initial-base result).
- `browser_renew.cjs` / `browser-renew.log`: real-time C and post-renew logout.
- `browser_multi_renew.cjs` / `browser-multi-integrated.log`: final-base simultaneous renewal and post-renew logout; `browser-multi-renew.log` retains the initial-base observation.
- `browser_negative.cjs` / `browser-negative-integrated.log`: final-base bounded G/H checks without outage (`browser-negative-final.log` is the initial-base result);
  `browser_extended.cjs` / `browser-extended-corrected.log`: the incomplete outage run.
- `cors-before.log`, `cors-after.log`, `cors-integrated.log`, `cors-unit-final.log`, `peer-before.log`,
  `peer-after.log`, `host-integrated.log`, `clippy-host-integrated.log`, `clippy-wasm-auth.log`,
  `clippy-cors.log` and the
  final preflight/build/baseline logs distinguish the verification layers.

The exact browser primitive was `node artifacts/agent-runs/web-identity/<driver>.cjs` after the
existing xtask services were ready. All owned supervisors and the fixture are stopped at handoff, while private database files and experiment sources are retained. These are local experiment sources, not a portable
CI browser entrypoint. No HAR, credential-bearing network dump or downloaded private content
is included. Re-enrollment creates new disposable identities; original passwords cannot be
recovered from the logs or reused after the fixture exits.

## Layered verification

| Layer / command | Result and scope |
| --- | --- |
| L1 architecture and final `cantos-code-review` | Final read-only source review: no-actionable-findings for the scoped repairs; full acceptance gaps remain explicit. Local code-review.md records the assurance ledger |
| L2/L3 production owner/core and Leptos host | `cargo test --locked --offline -p cantos-web-auth -p cantos-theatre -p cantos-studio --features cantos-theatre/preview`: PASS on the final base, 42 / 462 / 286 unit tests plus doctests; scripted Port evidence is explicitly mocked, separate from browser rows |
| L3 host lint | Scoped three-package `cargo clippy ... --features cantos-theatre/preview --all-targets -- -D warnings`: PASS |
| L3 WASM owner lint | `cargo clippy --locked --offline -p cantos-web-auth --target wasm32-unknown-unknown --all-targets -- -D warnings`: PASS |
| L4 CORS adapter | `cargo test --locked --offline -p cantos-server --test browser_cors`: PASS 2; new limiter response is a deterministic test handler wrapped by production CORS, not live Identity |
| L4 malformed CORS cases | `cargo test --locked --offline -p cantos-server --lib adapters::inbound::http::cors`: PASS 5 on the final base; duplicate Origins, exact methods/headers, wrong surface and no-Origin behavior |
| L3 Server CORS lint | `cargo clippy --locked --offline -p cantos-server --lib --test browser_cors -- -D warnings`: PASS |
| L5 browser / actual file and RPC boundaries | Per-scenario matrix above; real local processes, real cookie jar, production consumers, no fake successful sessions |
| Web verification and handoff preflight | Final-base preflight FAIL at repository: 31 findings exactly match the clean 443e7251 source archive after root-path normalization (no added/removed finding). Later preflight gates remain NOT_RUN. Initial-base preflight likewise had the same 31 findings as its clean base. Final-base serial `cargo xtask verify web` PASS: Web lint, both app builds and UI preview. Independent format and whitespace checks PASS. The first concurrent build failure is retained separately |
| Kani, CMP/device, deployed/provider/production | NOT_RUN; no claim from these local Web checks |

## Failures, remaining acceptance and resume conditions

- The default sandbox denied local port binding. xtask reported ports occupied, while a permitted
  read-only listener inspection found none. The same owned composition ran with local execution
  permission. No user process was stopped to free a port.
- The first `verify web` ran while Studio's Trunk dev build used the same staging directory and
  failed writing its JS loader (`No such file or directory`). That actual failure remains in
  `verify-web.log`; final serial verification must run after both dev servers stop.
- During H, Identity was briefly SIGSTOPed to obtain the real 503. It then exited with SIGKILL;
  cause was not established. xtask stopped its other children. No successful resume or same-session
  recovery is claimed from that run. A fresh xtask run preserved both database files and the
  remaining browser journeys used that composition. The missing capability is a controlled
  outage/recovery fixture that retains the Server and the browser context; next owner is the Web/
  native test owner (unassigned). Repeat real 503 UI projection and recover the identical session
  without login or revocation before accepting that full row.

| Remaining check | Observed blocker / available evidence | Exact next check and resume condition / owner |
| --- | --- | --- |
| Trusted HTTPS and Secure/host-only policy on real Web/API origins | No trusted local HTTPS was configured; only the legitimate HTTP development rules ran | Configure trusted test TLS without bypassing browser warnings; repeat A/B/D/H/I with the HTTPS surface origins. Web/API operator, unassigned; no production/domain change authorized here |
| Authorized Studio roster, reverse cookie purpose isolation | Existing native dev composition deliberately clears the roster; operator provisioning is deferred | Supported disposable Studio enrollment/roster setup, then product Studio login/reload/protected workspace/logout and both cross-surface credential refusals. Server/Identity/Web owner, unassigned; no planted sessions or DB edits |
| Browser 503 UI projection and same-session outage recovery | Real 503/cookie retention observed, but stopped Identity died before recovery | Controlled run-owned RPC disconnect/reconnect that keeps Server/browser alive; assert unavailable UI, no cookie clearing/revoke, and same session on recovery. Web/native test owner, unassigned |
| WebKit/Firefox and browsers without Web Locks | Only installed Chromium was exercised | Run the same named cases with an available supported driver/engine; no ordering guarantee beyond the existing fallback is inferred. Web test owner, unassigned |
| Signup mail-link UI, IME, unknown-effect login/deadline browser races | Disposable enrollment was real HTTP/mail capture; these UI/timeouts were not driven | Use the captured link through product UI with explicit setup confirmation; add delayed/lost real replies and named cancellation assertions. Existing owner tests already cover unknown outcomes, cleanup and stale completions; browser acceptance remains open. Web test owner, unassigned |

No merge, deploy, issue closure, production account, SMTP provider, DNS, media/catalog persistence,
native Bearer or Studio MFA/passkey/recovery change is part of this record. Residual risk is limited
by the matrix and the explicit unrun rows; local HTTP success is not HTTPS or production acceptance.
