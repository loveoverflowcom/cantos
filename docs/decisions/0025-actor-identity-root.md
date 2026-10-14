# ADR 0025: `ActorId` as the root of actor, owner and listener identity

- Status: proposed; the listener slice is implemented, the Studio slices are not
- Date: 2026-10-14
- Decision owner: Cantos maintainers
- Related: [identity registry](../architecture/identity-registry.md), [Library v1](../../packages/contracts/listener-library-v1.md), [Theatre identity v1](../../packages/contracts/theatre-identity-v1.md)
- Supersedes / superseded by: N/A

## Context

Every principal in Cantos is one row of the platform registry `actors(id)`. Two Rust newtypes
admit that same text with the same grammar (1–64 lowercase ASCII letters, digits, `_`, `-`):

- `platform::identity::domain::ActorId` — the registry identity, serde-guarded (`try_from`),
  used by sessions, the operator, external-identity links, admin audit and Theatre identity.
- `contexts::catalog::domain::OwnerId` — documented as "existing actor identity", but used for
  two different facts: the **owner** of a Studio resource and the **actor performing** a Studio
  command (`actor_id`, `recorded_by`, `created_by`, `reviewer_id`, `published_by`).

Until this change `listening::domain::ListenerId` wrapped `OwnerId`, so a Theatre account was
typed as a Catalog owner, and the listener adapters parsed verified actors with
`OwnerId::parse`. Known facts, from code and migrations:

| Layer | Touchpoint | Identity it actually means |
| --- | --- | --- |
| Domain | `ActorId` (`platform::identity::domain`) | registry actor |
| Domain | `OwnerId` in Catalog `Work`, Media image/artwork/lifecycle/publication, Production brief | Studio owner, and (in commands and records) the acting actor |
| Domain | `ListenerId` (`listening::domain`) | an actor acting as a Theatre account |
| Domain | `media::domain::object_storage::decide_owner_read(actor: &OwnerId, owner: &OwnerId)` | actor compared with owner, both typed as owner |
| Persistence | `actors(id)`; 48 `REFERENCES actors(id)` columns in migrations 0001–0024 named `owner_id`, `actor_id`, `recorded_by`, `created_by`, `reviewed_by`, `accepted_by`, `approved_by`, `revoked_by`, `resolved_by`, `imported_by`, `target_actor_id`, `listener_id` | one registry key under many role names |
| Persistence | SQL `listener_actor_active(text)` (0017) and `listener_lock_actor(text)` (0019) | generic actor facts; Discovery applies the former to the **release owner** |
| Auth | Studio `Store::authenticate` → `String` validated by `ActorId::new`; Studio handlers re-parse it as `OwnerId` | acting actor |
| Auth | `TheatreListenerAuthenticator`, `NativeListenerAuthenticator` → `AuthenticatedListener::from_verified_actor` | listener |
| Wire | `theatre_identity::Session.actor_id`, native exchange `actor_id`; Catalog `owner_id`; content `recorded_by`; brief `created_by`/`owner_id`; lifecycle `reviewer_id`/`created_by`; publication `published_by`; admin `target` | plain strings; no wire type distinguishes them |

The risk is semantic, not syntactic: because both grammars are identical, nothing fails today,
but a listener, an owner and an acting actor are interchangeable values, and a future grammar
change to one type would silently diverge from `actors.id`.

## Decision

Make `ActorId` the single root type for "a row of `actors`". Role types wrap it and never
re-implement its grammar. No database column, SQL function, wire field or serialization changes.

1. **Implemented in this change — listener.** `ListenerId(ActorId)`;
   `AuthenticatedListener::from_verified_actor(ActorId)`; both Theatre adapters validate with
   `ActorId::new` and fail closed as `Unavailable`. The Rust constant applying
   `listener_actor_active` to the release owner is renamed `RELEASE_OWNER_ACTIVE_FILTER`; the SQL
   function keeps its applied name and checksum. A bounded differential test pins that
   `ActorId` and `OwnerId` admit exactly the same strings, so every existing actor row stays a
   valid listener scope.
2. **Proposed — owner.** `OwnerId(ActorId)` with `OwnerId::from_actor(ActorId)` and
   `as_actor()`; `OwnerId::parse` delegates to `ActorId::new` and maps the error to
   `CatalogError::InvalidId(IdKind::Owner)`, so Catalog errors and wire behavior are unchanged.
   `token_id_valid` stops being an actor grammar.
3. **Proposed — acting actor.** Studio command and record fields that name *who acted*
   (`actor_id`, `recorded_by`, `created_by`, `reviewer_id`, `published_by`) take `ActorId`;
   fields that name *who owns* stay `OwnerId`. `decide_owner_read(actor: &ActorId, owner: &OwnerId)`
   then states the comparison in its signature. Studio's session adapter returns `ActorId`
   instead of a re-parsed `String`.

Slices 2 and 3 are one cohesive Studio refactor each, delivered separately, with Catalog, Media
and Production tests unchanged in meaning.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| `ActorId` root with role newtypes (this ADR) | one grammar; roles visible in signatures; no schema or wire change | touches many Studio signatures in slices 2–3 | selected |
| `type OwnerId = ActorId` | smallest diff | erases the owner/actor distinction the types should carry | rejected |
| Keep two parallel grammars | no work | silent drift between `OwnerId` and `actors.id`; listener typed as owner | rejected; the differential test guards the transition only |
| Rename SQL columns/functions to role names | self-describing schema | breaks applied migrations' checksums and every query; no behavioral gain | rejected; names stay, Rust names carry the role |
| Separate listener accounts table | strict Theatre/Studio separation | duplicates the registry, external links and admin revocation | rejected; ADR 0015 already separates surfaces by credential and router, not by registry |

## Consequences

- Product and user impact: none; identifiers, responses and errors are unchanged.
- Compatibility and migrations: none. `listener_id`, `owner_id` and `actor_id` stay text keys of
  `actors(id)`.
- Operations, storage, and production cost: N/A.
- Implementation and recovery requirements: slices 2–3 must keep `CatalogError`/`ImageError`
  mappings exact and keep `OwnerId` without `Deserialize`.
- Deferred questions: whether wire DTOs should gain distinct role fields; account merge/unlink
  (still deferred by the identity registry).

## Validation and revisit conditions

Slice 1 is `type-enforced` (a `ListenerId` cannot be built from an `OwnerId`) and
`example-tested` by `listener_scope_admits_exactly_the_registry_actor_grammar_studio_owners_already_use`
plus the unchanged listening, cursor and Router tests. Slices 2–3 are accepted when the Studio
suites pass unchanged and no `OwnerId::parse` remains on an acting-actor value. Revisit if the
registry grammar changes, if a non-actor principal (for example a producer credential) needs
ownership, or if accounts are split from Studio actors.
