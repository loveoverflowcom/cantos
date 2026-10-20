# ADR 0044: Shared CMP enrollment with explicit native admission

- Status: Proposed; development implementation only, production activation is not accepted
- Date: 2026-10-20
- Decision owner: Cantos owner
- Related issue: [#296](https://github.com/loveoverflowcom/cantos/issues/296)
- Supersedes: N/A; preserves ADR 0036 and the native Bearer contract

## Context

CMP Register currently explains unavailability. The existing Firebase enrollment service already
owns mailbox proof, durable claims, one-shot Kanidm setup and cancellation. Its Web HTTP routes
require an exact Web Origin. Native and desktop clients must not manufacture that Origin.

## Decision

Propose a separately enabled native HTTP admission surface over that same service. The
`CANTOS_THEATRE_REGISTRATION_FIREBASE_NATIVE_ENABLED=true` flag is off by default and requires
the complete existing Firebase registration configuration. It introduces no second workflow,
database, provider adapter or authentication credential. Native requests reject Origin, Cookie,
Authorization and browser Fetch Metadata; JSON, body bounds, real-peer quotas, durable mail
budgets, proof binding, expiry, consumption and provisioning fences remain mandatory. This
separates browser CSRF admission; it does not authenticate an app or prevent scripted abuse.

Shared CMP owns email editing, explicit send, action import, explicit confirmation/cancellation,
RAM-only setup delivery and return to Login. It never receives passwords or writes a verified
email flag. A receipt or setup link never changes the account controller. Only the existing
native OIDC exchange and API-confirmed session can authenticate the listener.

The first return mechanism is deliberate copy/paste of the exact configured Web action link,
validated against its action origin and continuation. It adds no OS deep-link registration.
The setup button consumes its RAM capability before opening the exact configured Kanidm
`/ui/reset?token=…` URL in an external browser. Returning to CMP is manual, followed by the
existing Login flow. Cancellation/Back fences delayed callbacks; a lost consuming response
requires reconciliation and cannot replay the proof or setup navigation.

The existing JVM Desktop Preview has an explicit live mode with configured API, action and
issuer origins. It mounts the production screen/controller/HTTP adapter. Diagnostic transports
and transient desktop credentials remain in `:preview-sandbox`; native hosts retain `:shared`
only. No dependency or toolchain adoption is required. Static captures never initiate requests.

## Alternatives

| Option | Benefit | Cost / reason |
| --- | --- | --- |
| Hosted registration only | Reuses Web admission | Does not exercise the requested shared CMP email/proof UI |
| Send a Web Origin from native | No server change | Rejected: misrepresents the caller and obscures admission |
| SDK signup or client-created account | Familiar native SDK | Rejected: bypasses API-owned proof/provisioning and creates a second workflow |
| Originless native admission with existing service | Real shared CMP flow and preserved proof fences | Selected for development review; separate activation and abuse review required |

## Consequences

No migration, real mail, credential creation, deployment or provider activation is authorized.
An uncertain response is visibly distinct from a refusal. Email input survives Back; proof and
setup values never enter saved state, logs, diagnostic snapshots or persistent desktop storage.
The API cannot distinguish expired from replayed proof in its public `unavailable` outcome;
the client must not invent a more precise server diagnosis.

## Validation and revisit conditions

Require deterministic admission, reducer/controller and strict wire tests; actual interactive
JVM UI and HTTP observations; then the isolated #242 API/PostgreSQL/Auth Emulator/Kanidm journey.
Android/iOS system-browser, secure-storage, process-death, account-switch and accessibility
acceptance remain with #191/#184. Desktop observations establish no native acceptance.
Review before any production enablement, automatic deep-link return or change to proof delivery.
Platform mechanisms: [Android ACTION_VIEW](https://developer.android.com/training/basics/intents/sending),
[Apple UIApplication.open](https://developer.apple.com/documentation/uikit/uiapplication/open(_:options:completionhandler:)),
[Java Desktop.browse](https://docs.oracle.com/en/java/javase/21/docs/api/java.desktop/java/awt/Desktop.html).
