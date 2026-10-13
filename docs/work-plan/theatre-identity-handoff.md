# Theatre identity PR3 → Admin and native resource IO

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
Web waits for acknowledged logout cookie cleanup before starting another login. Native stale
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
