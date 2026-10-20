# Cantos Web Bearer owner

> Implemented transport, pending migration: the [#295 owner decision](https://github.com/loveoverflowcom/cantos/issues/295)
> selects API-issued HttpOnly/Secure cookies for Theatre and Studio Web on the custom domains.
> [ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md#owner-selected-web-cookies-native-bearer-retained-295)
> owns the new target; this package still implements the Bearer behavior below. Preserve its
> account/session and late-result fences during migration. Native Bearer is unchanged.

`cantos-web-auth` is consumed by **both** Leptos Web apps. It imports contracts, never app or
service code. A small shared package prevents the same refresh family from being independently
rotated by competing fetch/XHR owners and gives both surfaces the same tested failure rules.
Studio and Theatre still have separate document-scoped owners, API prefixes and token purposes.
[Bearer v1](../contracts/bearer-v1.md) owns server authority and wire; [ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md)
owns the selected topology. This package does not select/configure an issuer or grant permission.

## Implemented client boundary

- Account access/refresh credentials live in document RAM only. No account token is placed in
  navigation, localStorage, sessionStorage, logs, provider requests or media URLs. Protected
  JSON, Protobuf, XHR and private byte reads omit cookies and use the expected session fence.
  Fetch redirects are refused. Owner and producer credentials remain separate.
- `core` admits exact surface secrets and lifetime order, fences intent and token generation,
  refuses account changes during retained-draft reauthentication and preserves signed auth_time
  across rotation. `refresh` shares one future/result, including unknown outcomes; interruption
  never starts another refresh fetch with a spent credential. Browsers may retransmit a request
  after a pre-header TCP failure; server replay-family revocation remains mandatory. A late waiter cannot clear a newer refresh flight.
- Unknown refresh abandons local admission, revokes with the old refresh/session pair and asks
  for controlled authorization. Unknown exchange cancels its original flow; no exchange,
  refresh or business mutation is automatically replayed. Failed cleanup stays in RAM and
  blocks new admission/authorization until an acknowledged retry. Browser termination can lose
  this cleanup queue; original server authority/expiry remain bounded, with no durable Web
  credential recovery or background worker claim.
- Logout invalidates the local intent before awaiting the API. Cleanup is pinned to the original
  family, so a late A cleanup cannot revoke B. Late success/401/body/refresh results are rejected.
  Durable server revoke and best-effort upstream revoke retain the Bearer v1 distinction.

## Authorization, reload and drafts

A trusted click reserves a fresh popup **before** awaiting any network work. The opener keeps
its verifier, expected account, route, draft text and intent in RAM. The popup navigates through
full issuer pages and the fixed `/auth/bearer/complete` route. Only its TTL-limited protocol
record (flow/state/verifier, origin, surface, created time) uses its own tab's sessionStorage.
The callback validates/consumes that record, scrubs the query from history before product effects
and sends only the one-use handoff to its same-origin opener. The opener verifies exact message
origin **and WindowProxy source**, then exchanges using its RAM verifier. No tokens cross that
message boundary. Cross-realm WindowProxy is handled as the WebIDL value, without an invalid
cross-realm `instanceof Window` test. Orphaned valid returns cancel their original flow and show
localized return/restart instructions; they never mint ownerless client credentials.

Blocked/closed/cancelled windows, storage refusal and a 300-second deadline yield explicit
retry/cancel states. They do not redirect guests in a loop or infer login from a return URL.
Reload/new tab loses credentials: the person explicitly signs in again; a valid issuer SSO may
complete that authorization. Studio's named sensitive-action reauthentication sends a separate
`reauthenticate: true` intent pinned to the owner. Refresh/iat or an SSO token is never claimed
as recent authentication; the API requires actual signed issuer evidence.

The opener stays mounted during authentication, preserving Studio drafts and canonical Theatre
return/focus context. A different account cannot adopt a retained Studio draft. Studio pane
history contains only closed nonsecret pane IDs. Deliberate reload/close still uses the existing
unsaved-work browser warning and discards RAM drafts if confirmed; no manuscript backup is
silently persisted. A new tab has no private draft. Credentials and drafts cannot be recovered
from display/cache state alone.

## Protected resource inventory

| Consumer | Implemented transport / boundary |
| --- | --- |
| Theatre session / Continue, Saved, History | scoped Bearer and expected family; response/body/intent checks; JSON only |
| Studio revision head/history/save/validation | Bearer plus UUID fence; existing per-route JSON/Protobuf codec and ambiguous-write receipts retained |
| Studio catalog/content/brief/audio metadata/delegations | owner Bearer; producer status/upload keeps its distinct work capability |
| Artwork upload | cross-origin XHR with `withCredentials=false`; same-origin uses fetch Omit because XHR would still send cookies. Owner Authorization/fence/receipt ownership retained; fetch has no fabricated byte-progress percentage |
| Private artwork preview | authenticated metadata + bytes, exact identity/hash/format checks, owned revocable object URL |
| Original source (import/adaptation/inspector) | authenticated fetch, session echo and SHA-256, ephemeral local Blob download; no API download anchor |
| Web audio / Range | no production Web HTMLMediaElement or private-audio byte consumer exists in current source; existing player is an explicit mock. No playback/Range acceptance follows from this migration. |

API cookie issuance remains temporary **server compatibility** for old clients; these Web apps
have no cookie fallback. The old Studio development-token cookie form is explicitly unavailable
in the Bearer client. Enrollment configuration/submission remains the existing separate flow;
this work does not activate it or create accounts.

## Verification and activation

Focused host tests run with `cargo test -p cantos-web-auth`; ordinary gates are `cargo xtask
verify rust`, `cargo xtask verify contracts`, `cargo xtask verify web`, then final-diff preflight.
`cargo xtask verify web-bearer [--dry-run]` is an explicit browser fixture only: it builds the
actual two apps against a fixed loopback mock API, serves mock identity pages and drives an
installed Chromium through CDP with an isolated disposable profile. No dependency is installed.
It records hashes, scenario outcomes and synthetic captures in ignored `artifacts/agent-runs/`.
It is excluded from build/run/preflight/CI/deploy and never changes live identity or hosting.
See [the command contract](../../xtask/README.md#verify-web-bearer).

Mock API/issuer journeys establish client interaction/wiring only. The default-deny identity
gateway, exact new callback reconciliation, real Kanidm SSO/prompt/auth_time/revocation, actual
cross-site Firebase/VPS deployment, other browser engines and native secure-store/media
acceptance remain separate gates. No live origin, credential, account, domain or gateway change
is performed by this package or its fixture. Phase 3 consumes the same API contract for CMP;
it must not infer native or real issuer acceptance from Web mocks.
