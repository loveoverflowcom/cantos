# Theatre native callback profiles

**Source support only (#285).** This records the contract and operator steps for a separate
Android debug callback. No provider registration, deployment or device run has happened; every
live step below is an operator action and `NOT_RUN` here.

## Contract

| Profile (wire) | Android application ID | Exact callback | API admission |
| --- | --- | --- | --- |
| `production` (default when omitted) | `com.cantos.theatre` | `com.cantos.theatre:/oauth2/callback` | always |
| `development` | `com.cantos.theatre.dev` | `com.cantos.theatre.dev:/oauth2/callback` | only with `CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK=admit` |

iOS keeps the production callback and bundle ID; no token is copied between variants.

- The client names a **closed profile**, never a URI, in `POST /api/v1/theatre/native/auth/bearer/flows`
  (`callback_profile`, [Bearer v1](../../packages/contracts/bearer-v1.md)). Production clients omit it, so
  older clients and APIs interoperate. Studio and Theatre Web reject the field.
- The API owns both URIs as exact constants. There is no wildcard, prefix, package or request URI.
  A profile the operator has not admitted is a configuration refusal (HTTP 503); it never falls
  back to production and never to a mock login.
- The admitted callback is written to the pending flow (`bearer_flows.redirect_uri`) at begin and the
  authorization URL uses it. Exchange consumes the flow only if its stored callback is still
  admitted and sends **that stored** callback to the provider. State, nonce, S256, single use and
  cancel/refresh fences are unchanged. No schema change.
- Clients (`NativeCallbackProfile`) derive the profile from the final Android application ID
  (`Context.packageName`). An ID outside the table is a callback refusal, not production.
  The Android parser accepts only the process's own exact callback (no fragment, unknown or
  duplicate parameter). The legacy `/native/auth/flows` routes remain production-only.
- Distinct schemes reduce routing conflicts between co-installed builds. They do not replace
  PKCE/state and do not prove the caller is the genuine app.

## Operator registration and rollout (not performed by this change)

1. Provider first: on the existing public client `cantos-theatre-native` add the exact redirect
   `com.cantos.theatre.dev:/oauth2/callback` while keeping `com.cantos.theatre:/oauth2/callback`
   (for Kanidm, `kanidm system oauth2 add-redirect-url`; read it back). Do not create a client or
   issuer and do not weaken public-client/PKCE checks. Record secrets nowhere in Git.
2. Deploy an API containing this change **without** the opt-in. Production behavior is unchanged.
3. Set `CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK=admit` only on the environment that should
   serve debug builds, then restart. Any value other than exactly `admit` refuses startup.
4. The next host-migration step ships `com.cantos.theatre.dev` and consumes this contract.
   Required acceptance there, not here: co-install debug and release, login round-trip, OS
   intent routing for each scheme.

Rollback: unset the variable and restart. In-flight development flows then fail to exchange
(state refused) and the user starts a new login; production flows are untouched. Removing the
provider redirect afterwards is optional. If the provider lacks the dev redirect, the provider
rejects the authorization and the app reports unavailable/rejected.

## Evidence boundary

Unit tests prove closed profile parsing, admission and client/parser behavior. They do not
establish provider acceptance, OS routing or a device login.
