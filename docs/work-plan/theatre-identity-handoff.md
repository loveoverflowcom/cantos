# Theatre identity PR3 → Admin and native resource IO

Current resumed status is in the [14 October appendix](#resumed-native-identity-handoff--14-october-2026); earlier sections
retain the results and blockers of their original snapshots.

Initial fresh develop was b09c180 (Publication merged); published PR98 exact
`9794ed9ddcd376680a01b37938eabdde564290e1`, tree
`c48d4e0c050a669f09addf46541c0cc82b19f87b`, was normal-merged in ad8b7a3.
PR98 contains PR92 exact277e28c. Subsequent fresh develop
`6b7ee1f6540dc7fb7756eb7c2fb7f5c7863ba6d5` contains merged PR91/PR93/PR95 and was
normal-merged in f9744a6. PR95 exactdc45b46/treeb53ccaff was read before integration;
its actual sealed seam is used. No separate Library/Discovery history was imported into PR3;
those features are now develop base. Private StoragePR90 remains separate. Final develop00eab6d
(PR99/PR100) was normal-merged in22fd352; Mobile v2 account-space changes are preserved. No prior
worktree or published dependency history is rewritten. This worktree is `/private/tmp/cantos-theatre-identity`.

## Implemented versus deferred

| Surface | Implemented code | Deferred acceptance / blocker |
| --- | --- | --- |
| Web backend | Separate Theatre BFF client, actual ES256/issuer/audience/nonce→registry→sealed Listener; durable single-use flows, hashed sessions, introspection, local disable/logout/rotation and pending remote revocation | Live Kanidm/client/scopes/TLS/credentials/SSO and production operations; no activation |
| Library seam | Actual PR95 authenticator/principal/router; current session/actor admission locks and default deny; no new Library domain | Full account sync/physical playback and production workload acceptance |
| Web client | WASM-typechecked same-origin BFF port, bounded fetch, generation/session fences, old projection hide, immutable account-owned intents and receipt-first recovery; pending logout fences new login until cookie cleanup acknowledged | Port unmounted in existing explicit mock UI; actual backend UI journeys/two tabs/CUA/accessibility and durable outbox (#17) |
| CMP common | Actual system-browser PKCE state/effect code, exact callback/state/nonce/redirect, replay/cancel/new-generation fencing, immutable account-owned intent helpers | Native token exchange consumer/resource API, stable durable persistence, lifecycle/OS runtime and Web+native sync |
| Android | Keystore nonexportable AES-GCM + AtomicFile/noBackupFilesDir; SecureRandom/SHA256, external ACTION_VIEW, exact callback parser | Host callback registration/mounting, encrypted crash restore/denial/key invalidation and device acceptance |
| iOS | Keychain AfterFirstUnlockThisDeviceOnly, secure randomness/SHA256, ASWebAuthenticationSession retained/cancelled owner and exact callback parser | Swift/Kotlin bridge/project mounting, Keychain lifecycle and device acceptance |
| Native verifier | Separate exact native issuer/client/nonce ES256 code-response kernel with real signature refusal tests; response is deliberately not Library authority | Trusted JWKS/clock consumer, durable native flow consumption, public-client token exchange, access-token/resource audience+revocation contract and registry→sealed principal composition |
| Admin | No session/role routes or privilege inherited | PR4 separate client/admission/role+audit; migration0022 available after final registry check |

**Native resource IO is a concrete incomplete implementation boundary, not a missing-device excuse.**
PR98's transport authenticates token/introspection/revoke with `client_secret_basic`, while native
is a public client and must have no embedded Web/Studio secret. Kanidm documents public clients and
RFC9068 access JWTs, but this repository has no reviewed public-client resource introspection/
revocation adapter or native exchange/API consumer. The new native code-response verifier proves
only issuer/sub for a bound exchange, never private account authority. Do not configure native
bearer admission or label native sign-in complete. The successor must implement trusted discovery/
JWKS transport, a durable one-attempt native flow, public exchange without secret, separate RFC9068
resource validation/current revoke observation, registry issuance and the sealed Library adapter,
then mount Android/iOS callbacks/OS stores in an opt-in host. This is code work before provider/device
acceptance. No live registration, access grant, secret, TLS or Podman bootstrap is authorized here.

Account policy: no automatic guest import/merge (#34 open). Login/logout does not emit history or
move playback. A→logout→B hides A data immediately and fences both late success/failure; A's original
operation/body remains A-owned, never relabeled asB. Returning asA first reads a receipt for an
unknown outcome. Helpers are memory only; no restart/durable outbox acceptance is claimed.
Web waits for acknowledged durable logout before starting another login. Guarded responses
leave the locally revoked HttpOnly cookie inert until replacement/expiry and emit no global
cookie expiry that could erase a newer tab's session. Native stale
callbacks do not invalidate a newer pending flow. Actual secure-store failure handling must be
accepted on OS targets before restoring credentials or enabling account UI.

Migration **0021_theatre_identity.sql** only. Preserve0015storage/0016publication/0017discovery/
0018registry/0019library/0020Studio. Combined registry is1–14 plus16–21 (20 SQL files); original
checksums are untouched. Admin0022 must remain separate. Shared `platform/oidc` is crypto/HTTP
plumbing, not a shared credential/session purpose. Neither Studio nor Theatre authenticates Admin.

## Next acceptance order

1. Parent Ultra independent combined source/security review after PR4; no review or merge is
   represented as completed by this implementation session.
2. Native resource IO implementation above, typed common exchange consumer and opt-in host wiring;
   preserve default deny until this full path is tested with signed HTTP fixtures and actual PG.
3. Web port→existing account/library context mounting with actual intent/receipt transport; preserve
   anonymous discovery and explicitly labeled mock mode until proven; no UI redesign or player work.
4. Authorized live Kanidm matrix and browser/native OS account-switch/crash/cancel/secure-store
   acceptance; native app build/device/audio and production activation stay separate.
5. Resolve at-rest bearer protection, backups, least privilege, quotas/retention and pending remote
   revocation recovery before deployment. No performance/DRM/global logout promise is made.

See [architecture](../architecture/theatre-identity.md),
[contract](../../packages/contracts/theatre-identity-v1.md) and [evidence](../evidence/theatre-identity.md).

## Independent campaign completion handoff

The code boundary above is historical. Native resource IO now has its own public-client config/
transport, durable single-attempt flow, real ID/access signature witnesses, current introspection,
exact registry issuance, native opaque session, fresh admission and local-first pending revoke.
Migration 0024 also replaces Web issuance with final expiry rollback after all writes/trigger
waits; 0021 bytes stay untouched. Contract fixtures, signature tests and actual SQL/HTTP regression
sources accompany it. Fresh execution belongs to the parent resource lane; older PASS rows do
not establish current integrated acceptance.

The common controller and OS adapters are mounted into iOS with explicit HTTPS config. Android
INTERNET/callback manifest activation remains an automatic-review blocker; the source-only
proposal at `/tmp/cantos-native-android-manifest-proposal.md` is unapplied pending direct human
exception. Provider registration/grants, deployed credentials/TLS/network settings and container
startup were not performed. Complete the permitted source patch, then run common, Android/iOS
build and named OS callback/storage/lifecycle acceptance where tools are available.

Private requests now carry a nonsecret session fence. Mismatch 409 refuses dispatch and requires
identity revalidation, preserving owner intent. Web stale-only cleanup and expected-session
logout cannot revoke a switched valid cookie. Provider reconfiguration never receives a bearer
stored for another exact issuer.

Guarded/stale-only logout and completed callbacks emit no shared cookie expiry headers. A new
login accepts a retained old cookie only when its exact local issuer/client session is positively
revoked/expired; unknown cookies and provider outages remain closed. The actual PG/HTTP mock
regression pauses old A's provider revoke after local commit while two guest callbacks complete,
then checks that the late A response has no cookie effects and B remains admitted. This new
regression is source evidence until the parent build lane executes it.

The durable repair uses at most eight immutable per-flow cookies, with strict overflow refusal
before new allocation and no mutable-cookie fallback. Active expected-A logout locks the proven
flow inventory and A's source flow before associated sessions, atomically cancels pending flows
and revokes already-issued siblings. A stale-only or already-inert A cleanup never cancels newly
intended B. The paused signed-mock regression covers two initially guest callbacks plus a third
binder, late callback refusal after acknowledged logout, and a fresh login. Overflow from more
than eight concurrent anonymous starts requires waiting for 300-second binder expiry. Unguarded
HTTP DELETE is retired. All new regression execution remains assigned to the parent lane.

Recommended order: combined pure/mock/PG gates → Android source exception/application → actual
Web/native interaction and secure-store lifecycle → authorized live-provider matrix → production
at-rest/recovery/retention review. Native playback, durable outbox/guest-import policy and full
Web/native synchronization retain their separate owners and acceptance gates.
Because Web/native issuers are client-specific, shared Library acceptance also needs an explicit
authorized registry link; equal subject strings do not authorize automatic linking.

## Independent integrated acceptance handoff — 13 October 2026

Current implementation source is `2a1a98546a04420638626ff6d05bbce550fb1235`, tree
`2e8178a6607dd212acd6d657c36d5ade486ad13c`. Actual runtime used `0a4ee8f`, tree `bcde33a8`;
the later change is the inherited PostgreSQL test oracle only, with production/UI bytes unchanged.
The [current evidence appendix](../evidence/theatre-identity.md#independent-integrated-acceptance--13-october-2026)
supersedes earlier incomplete-source/provisional gate descriptions for this snapshot.

The mounted readonly Web account/Library consumer and native exchange/resource/registry/session
code are implemented. Shared OIDC uses one proof boundary while Studio, Theatre Web, native
and Admin purposes stay separate. Old A/UUID alone cannot revoke observed B: cancellation needs
a captured unexpired private binder and exact matching receipt, or an already-revoked attributed
retry cohort. Guarded cleanup leaves revoked HttpOnly cookies inert and emits no global expiry
headers. Roots serialize related rotations and grant no cancellation authority by themselves.

Verification is precise: the fourth full preflight was **16 PASS / PostgreSQL FAIL**. The
test-only FK-graph correction retains the expected immutability error `55000`; subsequent full
PostgreSQL verification **PASS** includes 15 Web, 6 native and 18 Studio OIDC tests plus all
inherited suites. Final changed-only preflight selected **8 PASS with no skips**. Integrated Rust,
the 4 Web build/lint gates, canonical account tests, CMP/reference/preview and 9-killed/0-survived
mutation checks passed in their retained runs. Do not call this a single 17-green invocation.

Actual built Web/Axum/PG plus ephemeral signed-provider browser observations are **14 PASS,
0 FAIL, 2 INCONCLUSIVE, 7 NOT_RUN**; all 12 PNGs were opened and inspected. They exercise real
provider chooser/callback, empty readonly Library, A/B fences, delayed 401 cleanup, provider
outage/recovery and local logout with remote pending state. Native-select/text scaling caveats,
200% zoom, nonempty Library/audio, screen reader/IME and live/native OS gates remain explicit.

Final 16-record manifest: `/private/tmp/cantos-identity-campaign/pr101-acceptance-manifest.json`,
SHA256 `e507afad65f29c01f9959b36f52de564171cce26ea0de27bda5e80fa2c216591`.
It pins exact logs, runtime report/ledgers/images, build manifest and owned-process stop records.
Task-owned API/provider and both fresh PG clusters are stopped; unique data/evidence is retained.

Android **mock build PASS** compiled shared protocol/adapters: 61 tasks executed in 52 seconds.
Its INTERNET/callback manifest and dependent host activation remain unapplied after automatic
approval rejection, awaiting the direct human exception. This is not an Android auth-host/device
acceptance or completion of the whole mobile client. iOS simulator framework is **UNSUPPORTED**:
required Kotlin/Native 2.2.20 has no executable `bin/konanc`; no alternate toolchain substituted.
PR101 remains draft while the Android source blocker is unresolved. Live provider/public-client
setup, device/system-browser/secure-store/crash/playback/accessibility acceptance remain NOT_RUN,
and production activation stays closed.

Recommended order remains the held Android source exception, then named Web/native OS and
secure-store lifecycle gates, followed by authorized live-provider setup and production
at-rest/recovery/retention review. Durable outbox, guest-import, client-linking and broad playback
acceptance keep their existing owners; no issue is closed by this handoff. GitHub owns PR execution
status and the queue records only the next recommended work.

## Resumed native identity handoff — 14 October 2026

This appendix supersedes the prior pending Android source exception for the resumed snapshot;
the 13 October ledger/hashes remain unchanged historical evidence. Fresh develop storage
`edbd7c7` was normally merged at `b5ba2c58`, tree `fda73180`. Final reviewed resumed source:
`1e2adae537d6b94ee824a5eb8f290a9dbdfa170a`, tree `ab556046cc5922d933c8792c6e7a037b2d595ec2`. The additive registry is now 23 entries
(1–21,24,25), including inherited storage0015; historical migration bytes/checksums stay intact.
See the [resumed execution record](../evidence/theatre-identity.md#resumed-native-identity-acceptance--14-october-2026).

The trusted user authorized the exact held Android INTERNET/callback/Application/MainActivity
source operation. It was applied successfully and the real Android host now owns/mounts the
opt-in account controller and dispatches `singleTask` callbacks. Operator-supplied validated
nonsecret origins remain required; no live values, OAuth client registration, credential
configuration, app installation or OS network/TLS change was performed.

The native cleanup controller retains its exact token after failed persistence or ambiguous
revocation. Pending/unavailable denial markers with lost/corrupt credentials remain closed;
there is no invented revoke acknowledgement, automatic reset or unsafe new-account escape.
Controlled lost-credential recovery remains unassigned. The production raw-query guard refuses
encoded name aliases before Android URI getters; common tests cover its bounded parsing only.

Actual common/JVM verification passed 207 tests (28 identity), with zero failures/errors/skips.
The same new fixture against the preserved old controller produced four failures before repair,
and fixed source was restored by hash before passing. The original manifest failed both new
static contract tests; the mounted source passed both. The Android gate is **PASS — Theatre mock
Android build** (61 actionable tasks: 20 executed/41 up-to-date, 1m52s); packaged-manifest inspection
confirms permission/callback/singleTask. iOS is **UNSUPPORTED** without installed canonical
Kotlin/Native 2.2.20 `bin/konanc`; no other version substituted. Final full preflight is **PASS:
all 17 gates, no skips**, including 331 repository tests and mutation 9 killed/0 survived.
Full PostgreSQL is **PASS**: 16 Theatre Web, 7 native and 18 Studio OIDC tests plus all inherited
HTTP/process-kill/PostgreSQL-restart/migration/replay/codec/storage/publication suites.

Those Web/native suites also use actual signed-mock-issued credentials whose own session GETs
remain 200 before/after 20 exact Studio rendition bytes/read-access 401 cases, including bearer,
cookie and relabeling variants. Safe JSON has no redirect/read-grant headers; authority/media
counts and Theatre/native introspection counts stay unchanged. A test-local SELECT on empty
Studio sessions makes this real authentication refusal rather than fixture 503, with no
production grant or storage/media IO claim. This is assurance coverage, not a confirmed finding
or fail-before regression.

Final resumed evidence manifest: `/private/tmp/cantos-identity-campaign/resume-20261014/pr101-resumed-acceptance-manifest.json`, SHA256
`ab3f1270317a1db8b6f7640a6df6db048070c8dc92d03135554392b38bb87f67`. Prior Web runtime evidence is historical and unchanged,
not a new native/device result. Device/system-browser/secure-store/OS lifecycle/accessibility,
live provider and native playback acceptance remain **NOT_RUN**; production activation is closed.
Next owners should perform named native OS/secure-store lifecycle acceptance and the authorized
provider/public-client matrix, then review controlled cleanup recovery and at-rest/backup/
retention. Durable outbox, guest-import, explicit issuer linking, broad playback/synchronization
and whole-mobile completion keep their existing scope; no broader issue is closed here.
GitHub owns publication/merge status.
