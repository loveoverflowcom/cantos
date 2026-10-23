# Post-merge vertical integration, 23 October 2026

Source-bound acceptance review of the integration seams changed by the PRs merged into
`refactor/cantos-native-turso` on 23 October (#389–#407). It does not close an issue, accept
production, a device, a deployed host or native audio, and it replaces no earlier record.

## Provenance and boundary

- Base: `origin/refactor/cantos-native-turso` at `473ac64b` (re-fetched before handoff, unchanged).
  `develop` (`1c970a31`) is a strict ancestor and was not the integration base.
- macOS arm64; Rust 1.98.1; Trunk from Homebrew; Gradle 8.14.3 and the system JDK; the in-app
  Chromium browser pane. `cantos-identity` built with `--features service` from this source.
- Every run used run-owned temporary files and a loopback SMTP sink. No production data, provider,
  deployed host, development session or authentication bypass was used. Passwords, cookie values
  and mailbox proof secrets were not printed or committed.

## Selected journeys

The day's PRs moved Editorial/Ingestion to Turso (#392), retired the PostgreSQL oracles
(#395/#398), changed Web session judgment (#393), qualified Theatre Web cookies in a browser
(#394), bound the surface policy to Identity v1 (#399) and completed the CMP live boundaries
(#388/#390). The existing harnesses already join Identity with Theatre; the Studio half had only
synthetic development sessions and the Studio README recorded no backend or browser run.

| Journey | Why selected |
| --- | --- |
| J1 Studio Identity login → Editorial write → authoritative Turso read → restart → logout | #392 + Identity sessions met only in separate tests; #394 left authorized Studio `NOT_RUN` |
| J2 Theatre enrollment → native Bearer → catalog/playback → Library → server restart | Re-validate #387/#388/#398 on the integrated head |
| J3 CMP production HTTP/auth owners → Library outbox through an Identity outage and recovery | #388/#390 are the only CMP path against the real Server and Identity |
| J4 Studio Web product login → create Work → Server process restart → reload → logout | Browser half of J1; closes #394's authorized-Studio `NOT_RUN` for local loopback HTTP |

## Methods considered

| Method | Coverage | Cost / prerequisites | Separates code from environment |
| --- | --- | --- | --- |
| A Real API + disposable Turso (existing `identity_world`, `theatre_business_*`) | Production router or Server binary, real `server.db`, real Identity executable over its socket, SMTP sink | Identity binary only; seconds | Yes: no network, browser or toolchain beyond Cargo |
| B Browser-driven Studio/Theatre | Production WASM consumers, real cookie jar, Trunk proxy | Hand-started processes, a roster, free loopback ports | Partly: port conflicts and roster provisioning are environment/contract, not code |
| C CMP Desktop Preview over production adapters (existing ignored tests) | CMP auth/Library owners, real HTTP, Identity outage control | Gradle + JDK; ~75 s | Yes, but JVM only: no Android/iOS, no native audio |
| D Layered combination | A for invariants, B and C for the consumer seams | Sum of the above | Used here |

## Evidence per journey

| Journey | Invariant → production path → oracle | Command | Result / tier |
| --- | --- | --- | --- |
| Baseline | Existing cross-process harnesses on the unchanged head | `CANTOS_TEST_IDENTITY_BINARY=… cargo test --locked -p cantos-server --no-fail-fast --test identity_password_turso --test theatre_business_journey --test theatre_business_process --test editorial_recovery_turso --test identity_auth_candidate --test editorial_turso --test ingestion_turso` | PASS 42, 2 ignored (CMP). Real HTTP + real DB + real Identity process |
| J1 (new) | A roster member's Identity-earned Studio cookie is the only Editorial authority: no `Origin` → 403, stale fence → 409 with the current id, same account's Theatre cookie → 401, each with zero revisions; accepted save → revision 1 `accepted_by` the Identity-bound actor; identical replay → identical receipt, one row; Identity stopped → 503, no row; Server reopened on the same file → identical head; logout → session revoked, next write 401, no row | `cargo test --locked -p cantos-server --test identity_password_turso an_identity_earned_studio_session_writes_a_revision_that_survives_restart_and_logout_ends_it` | PASS. Production router, real file-backed `server.db` reopened, real Identity executable. Non-vacuity: disabling the Studio fence (409 → 200, a revision would commit) and mapping an Identity outage to 401 each fail it at the named assertion |
| J2 | `theatre_business_journey` (in-process production composition) and `theatre_business_process` (the `cantos-server` binary, a second process reads the same outcomes) | Baseline command | PASS. Loopback HTTP between OS processes; no TLS, proxy, CDN, device or Ogg decode |
| J3 | `cmp_authorized_library_against_real_identity_and_http` (`LIVE-AUTHORIZED-LIBRARY-OK requests=23`, Identity stopped and restored mid-run) and `cmp_live_desktop_preview_against_real_identity_and_http` (`LIVE-DESKTOP-PREVIEW-OK`); saved/bookmark rows exact, no anomalies | `CANTOS_TEST_GRADLE=… cargo test --locked -p cantos-server --test theatre_business_journey cmp_ -- --ignored --test-threads=1` | PASS. JVM desktop tests against the production router over loopback. Secure storage and Library store are in memory, audio unavailable by design; not Android/iOS |
| J4 | Real `cantos-identity serve` (SMTP sink) and `cantos-server` binaries, roster set to the enrolled subject, Studio Trunk proxy. Product form: login 200 → `GET identity/session` 200 → `GET works` 200; Create Work 201 (r1). Server process killed and restarted, full reload: session 200, the Work listed. Product logout: `DELETE identity/session` 204, workspace hidden; reload → 401. Server-side readback: Work/revision owned by the actor, only the `studio-web` session revoked | Hand-started processes (below) + browser pane | PASS for local loopback HTTP. Not HTTPS `Secure` cookies, cross-site domains, multi-tab or Script editor save |

J4 setup, in order: `cantos-identity migrate`; `cantos-migrate`; `cantos-identity serve` with
`CANTOS_IDENTITY_SMTP_TLS=loopback-plaintext`; `cantos-server` with `CANTOS_ENV=development`,
distinct Studio/Theatre origins and `direct_peer`; Theatre signup and completion over HTTP from the
mailed link; Server stopped; subject read **out of band** from the stopped `server.db` with a
read-only `sqlite3`; rights/source evidence registered with `cantos-operator register-evidence`;
Server restarted with `CANTOS_IDENTITY_STUDIO_SUBJECTS`; `trunk serve --port 8083`.

## Defects, gaps and blockers

No integration defect was confirmed on the selected paths, so no production source changed.
This record adds the J1 regression to the existing real-Identity harness.

| Kind | Finding | Next step |
| --- | --- | --- |
| Contract gap | No supported operator path discovers an account's Identity subject for the Studio roster; ADR 0045 defers Studio operator provisioning. Tests read it with fixture SQL and J4 read it out of band | An owner decision on Studio provisioning (Identity- or Server-side lookup) before production Studio onboarding |
| Tooling gap | `cargo xtask run cantos-studio --web` strips the roster and disables Identity mail, so the supported local lane cannot reach an authorized Studio session; J4 needed hand-started processes | Follows the provisioning decision; no roster flag was added without it |
| Environment | On this Mac a system endpoint agent (`macmnsvc`) listens on `*:8081`, Studio's fixed Trunk port, so `xtask app cantos-studio --web` refuses to start | Free the port or run Trunk on another port with a matching `CANTOS_STUDIO_WEB_ORIGIN`, as here |
| Observation | The browser log marks the session read and logout as `ERR_ABORTED` after their 401/204 responses; the server-side revocation was durable | None; consistent with the client's best-effort abort after reading the status |

## Module versus cross-system acceptance

Module evidence (domain cores, adapters with doubles, a single crate's HTTP router) shows each
owner keeps its rule. The journeys above add a cross-system claim: a credential earned in one
process (Identity) becomes authority for a durable write in another (Server), survives that
process's restart and dies on logout. They still stop short of production: disposable files are
not migration/restore acceptance, loopback HTTP is not HTTPS or deployed CORS, a JVM test is not
an Android/iOS device or native player, and an out-of-band subject lookup is not a provisioning
procedure.

## Not run

HTTPS `Secure` cookies and production domains; Studio Script editor save in the browser;
multi-tab Studio; Android/iOS runtime and native audio; deployed hosts; PostgreSQL data conversion;
real SMTP relay; `xtask preflight` beyond the gates listed in the pull request.
