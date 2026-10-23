# Theatre client error presentation v1

Implemented client contract for [#298](https://github.com/loveoverflowcom/cantos/issues/298).
This extends the existing shared Dialog/feedback anatomy; it creates no endpoint, credential,
event bus or generic retry engine. Runtime acceptance is separate from source and fixtures.

## Addressing

Theatre resolves an explicit override first, otherwise `https://api.cantos.cc`. An explicit
empty, whitespace or malformed origin is refused before network; there is no environment fallback.
Web `web_api::api_url` owns both resource and auth addressing. Studio keeps its existing origin
policy. Android/iOS use `selectCatalogOrigin`; packaging represents absence with
`__CANTOS_DEFAULT_API_ORIGIN__`, never an empty override. Release keeps HTTPS; the existing
local Web/debug-native loopback exceptions retain their exact host boundaries.

Local `cargo xtask run|app cantos-theatre --web` explicitly injects `http://127.0.0.1:8080`.
Live CMP preview requires an explicit API origin even when only an issuer was supplied. Missing
or invalid identity configuration is a typed account diagnostic and cannot disable public Home.
No client infers an issuer, changes TLS/CORS, or substitutes mock content.

## Taxonomy and actions

| Kind / message key | Meaning | Recovery |
| --- | --- | --- |
| Network / ErrorNetwork | Connection, DNS or TLS failed | Explicit bounded read attempt |
| Timeout / ErrorTimeout | An HTTP or transport deadline elapsed | Explicit bounded read attempt |
| Unauthenticated / ErrorUnauthenticated | The requested action requires sign-in (401) | Sign in; expected guest discovery is not a system error |
| Forbidden / ErrorForbidden | The current account is denied (403) | Close and retain context; no replay |
| ContentUnavailable / ErrorContentUnavailable | Withdrawn, replaced, private or absent release | Inline unavailable state; never silently substitute a release |
| Protocol / ErrorProtocol | Malformed or contract-incompatible response | Explicit read retry; no automatic protocol loop |
| Configuration / ErrorConfiguration | Refused host/build configuration | Close; correct configuration outside the feature |
| Storage / ErrorStorage | Durable data unavailable or corrupt | Inline owner recovery; never generic mutation replay |
| ServiceUnavailable / ErrorServiceUnavailable | Retryable API outage | Explicit bounded read attempt |

The operation is `LoadCatalog`, `LoadWork`, `LoadUnits`, `StartPlayback`, `UpdateLibrary` or
`SignIn`. Scope is Host for configuration, Section for catalog reads, otherwise Action.
Message keys have paired vi-VN/en copy in the existing Web `messages.rs` and CMP `ShellCopy`;
identity configuration detail stays in the existing account copy owner. No raw exception,
SQL, response body, credential or signed URL is UI copy.

A notice offers one task-specific primary and Close. Retry is admitted only by the operation
owner: a write with an unknown outcome must keep its operation ID/payload, look up its receipt
and recheck the account/session fence before a replay. A generic error window cannot retry
`UpdateLibrary`. Existing mutation controls retain their reconciliation protocol.

## Presentation and lifetime

Web composes the shared `cantos_ui::Dialog` and Button; CMP reuses the shell ErrorDialog over
Material AlertDialog. Both error centers keep at most one active modal and three queued notices,
deduplicate kind/operation, and suppress a dismissed episode until successful resolution or an
explicit new attempt. Loading alone does not resolve an episode. The owning request tickets,
route and session fences reject old completions before error presentation. Route/account changes
retire notices for the previous context. Validation remains inline.

Failures with API-sourced cached/retained rows remain inline with stale/offline status. Cached
API data is not a mock. No failed request fabricates an empty collection, account, saved state
or playback success. Existing query/form/rows/player owners survive read recovery.

The shared Web Dialog owns native focus containment, Escape and return focus. Startup errors use
the shell main landmark as a return target when no opener has focus. CMP retains Escape/Back and
its existing return-focus hook. Actual browser, native Back and screen-reader acceptance must be
recorded on the renderer/device used; host semantics tests never establish those results.

## Durable Web journal refusal

A missing journal is distinct from unavailable or malformed storage. An unread, oversized,
corrupt or cross-account journal is held without overwriting its bytes. A mutation cannot send
before its exact pending operation is persisted. A failed persistence keeps the pending intent
and marks outcome uncertain; recovery uses the same receipt-first lane. A delayed storage result
or receipt callback cannot act on a newer account. Storage retry performs persistence/reload and
reconciliation, never a new mutation with a new ID. Public discovery and the player remain mounted.
