# Theatre account presentation V1

`GET /api/v1/theatre/account/presentation` returns the authenticated listener's generated
pseudonym. It uses the existing Theatre cookie or native Bearer admission, fresh Identity
security-state check and session lease. `X-Cantos-Theatre-Session` fences requests and echoes
the admitted session. Responses are JSON and `Cache-Control: no-store`; unauthenticated,
mixed-channel and foreign-session requests disclose no presentation. Library error envelopes
and existing exact-origin policy apply. No actor parameter or public profile lookup exists.

The [schema](account-presentation.schema.json) contains `version: generated-alias-v1`, `alias`
and `avatar_seed`. These are display facts, never authentication evidence or a verified real
name. Identity signup, mailbox proof, session wire shapes, credentials and login policy are
unchanged. Signup completion still creates no session. There is no migration, profile table,
name form, image URL, upload or user-set profile in this version.

## Stable recipe

The sole dictionary/algorithm owner is
[`contexts::listening::account_presentation`](../../../../src/contexts/listening/account_presentation.rs).
The canonical input is the exact stored, validated `ActorId` spelling (lowercase ASCII grammar).
Concatenate the UTF-8 bytes of `cantos:avatar-alias:v1` and that actor spelling, without a
separator, then SHA-256. Digest bytes 0 and 1 modulo 16 select respectively the first and
last words in the V1 arrays. Their ordering and spelling are frozen; a future revision must
preserve existing names rather than silently replacing V1. Collisions are allowed: the alias
is a pseudonym and never an account key. Names are content and do not change with UI locale.

`avatar_seed` is lowercase hex SHA-256 of `cantos:avatar-seed:v1` concatenated with the same
actor spelling. The raw actor/email never reaches the presentation response or UI. The hash
is a stable display seed, not an anonymization or secrecy guarantee. Both clients consume
this seed rather than duplicating the alias dictionary or SHA recipe. Personal-tonal preview
uses the existing shared FNV-1a(seed) modulo six. Default A pins the existing theme-aware
Lavender/on-Lavender pair. B pins Gold; B/C are diagnostics, not production settings.

The projection is computed, so successful account completion, retries, process restart,
first login and other devices have the same result without an additional write. Consumers
derive Unicode Pair initials through the shared Avatar and bind presentation to the admitted
actor/session generation. Checking, logout, switch and expiry clear it before asynchronous
work; late responses cannot republish it. A display failure keeps authentication authority
with its existing owner and uses an honest unknown-name fallback.

## Evidence boundaries

[`fixtures/vectors.json`](fixtures/vectors.json) contains synthetic actors and independent
Python/hashlib golden outputs. Server tests exercise the production recipe; Web and CMP
consume the same corpus for Unicode initials and tone. The production router journey checks
real Identity cookie/Bearer parity, fencing and no-store when its environment runs. Neither
unit tests nor JVM captures establish signed-in browser, Android/iOS device or deployed
acceptance. The shared Avatar retains authorized-picture priority and initials fallback;
this endpoint supplies no image source, so text is the complete V1 behavior.
