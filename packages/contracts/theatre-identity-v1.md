# Theatre Identity v1

PR3 implements an opt-in Web BFF on the existing development host. Native protocol/adapters are
compiled groundwork; native token exchange and resource admission remain disabled/deferred.
See the [architecture](../../docs/architecture/theatre-identity.md) for the trust boundary.

| Route | Admission / response |
| --- | --- |
| GET `/api/v1/theatre/auth/login` | Same-origin navigation; redirects to exact Theatre Web issuer, `openid`, code + S256 |
| GET `/api/v1/theatre/auth/callback` | Exact browser/state/nonce/redirect; consumed before one exchange; opaque cookie only |
| GET `/api/v1/theatre/session` | Theatre cookie; `{actor_id, session_id}` as nonsecret display/fencing facts |
| DELETE `/api/v1/theatre/session` | Exact Origin; works for expired sessions; local revoke/cancel before remote revoke; 204 and cleared cookies |
| Library / Progress / operation receipts | Existing [Library contract](listener-library-v1.md), sealed Listener principal; Web cookie admission |

Session DTO is [`cantos_contracts::theatre_identity::Session`](api/src/theatre_identity.rs), with
required strings and no unknown fields. `actor_id` is an opaque internal actor; `session_id` is a
random UUID unrelated to the credential. Neither is proof. No token, provider claim or refresh
credential appears in the response. Auth-route failures use the existing `ApiError`; Library
admission failures keep the Library error contract. All responses use no-store/no-referrer.

Browser provider credentials remain server-side. Direct bearer is refused on an enabled Web BFF;
native tokens need a separate future verified resource boundary, not Web cookie substitution.
Absent Theatre configuration retains default-deny account admission and existing guest routes.
No automatic guest import/merge. Login/logout never emits history, moves playback or imports
mock state. Client intent IDs/bodies are stable; A's pending intents cannot be submitted asB.

Native client is fixed `cantos-theatre-native`, scope `openid`, callback
`com.cantos.theatre:/oauth2/callback`. System-browser adapters parse exact URI, reject fragments,
unknown/duplicate parameters and correlate state before the single code exchange effect. The
nonce/verifier stay in the pending in-memory attempt. Cancellation/process restart needs a new
login. A code callback is not an authenticated account; native token verification/registry/API
transport and host mounting are deferred acceptance.
