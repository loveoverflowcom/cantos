# ADR 0047: Studio roster provisioning through an Identity-owned operator lookup

- Status: Proposed; not implemented. Nothing below changes current behavior until the owner accepts it
- Date: 2026-10-23
- Decision owner: Cantos owner
- Related issue or PR: #335, #331; gap recorded in the [23 October integration record](../evidence/post-merge-integration-20261023.md)
- Supersedes / superseded by: fills the "Studio operator provisioning" item that [ADR 0045](0045-v1-email-password-sign-in.md) §3 defers; keeps [ADR 0034](0034-theatre-signup-and-provisioned-studio.md) and ADR 0045 §2 unchanged

## Context

Known facts on `refactor/cantos-native-turso` at `473ac64b`:

- The Server alone owns the Studio roster: `CANTOS_IDENTITY_STUDIO_SUBJECTS`, a JSON array of 1–8
  exact 32-hex Identity subject ids. Login and every Studio admission check it; removing a subject
  closes Studio at the next request ([Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md#studio-roster)).
- Only Identity maps an email to a subject (`identity_emails`, `identity_subjects`). The Server
  keeps no email. No HTTP route, RPC, log line or operator command reveals a subject. The Web
  session DTO carries only nonsecret actor/session/deadline fields.
- The `cantos-identity` executable offers only `migrate` and `serve`. `cantos-operator` handles
  actors, development tokens, evidence and sources.
- `cargo xtask run cantos-studio --web` removes any inherited roster and disables Identity mail.
  The supported local lane therefore cannot reach an authorized Studio session. `run cantos-theatre --web`
  uses the Firebase Auth Emulator. Both runs share one persistent `target/native-dev` layout, so a
  Theatre run's account remains in `identity.db` for a later Studio run.
- Tests learn the subject by reading `identity_sessions` with fixture SQL. The 23 October browser run read
  it out of band from a stopped `server.db`. Neither is a provisioning procedure.

ADR 0034 requires Studio admission to be "an identity-provider/account operation plus a separate
server admission configuration" that no client-supplied subject, email or Theatre account can add
itself to. Production Studio onboarding and local Studio development both stop at this gap.

## Decision (proposed)

1. **Identity owns the lookup.** Add one operator subcommand,
   `cantos-identity studio-subject EMAIL`, to the existing executable:
   - It opens `CANTOS_IDENTITY_DATABASE` like `migrate`. Turso's exclusive file owner refuses
     the lookup while `serve` runs, as `cantos-operator` does for the Server.
   - It canonicalizes the argument with the same `EmailAddress` constructor that signup and
     login use, and reads in one snapshot.
   - It prints the 32-hex subject only when that email belongs to an `Active` subject with a
     password credential, i.e. a completed signup.
   - Malformed input and every other case (unknown, pending, `Disabled`, `Deleted`,
     `Tombstoned`) print one generic refusal with one nonzero exit status. Output never echoes
     the email and never lists accounts.
   - It writes nothing, emits no security event and starts no socket or network.
2. **The roster stays the Server's configuration.** The operator copies the printed subject into
   `CANTOS_IDENTITY_STUDIO_SUBJECTS` and restarts the Server, per ADR 0045 §2. Neither the lookup
   nor any Identity state grants Studio authority. Production keeps the existing
   [VPS runbook](../deployment/vps-production.md) authority rules, with a new written step.
   That step names the host, the one or two accounts (ADR 0034) and the user's explicit
   authorization. The step itself is unimplemented.
3. **The local lane gets an explicit, per-run roster.** Add
   `cargo xtask run cantos-studio --web --studio-account EMAIL`:
   - The flag is repeatable up to the contract's 8; ADR 0034 expects one or two.
   - During the existing prepare phase, after both migrations and before any service starts,
     xtask runs the same `studio-subject` command against the layout's `identity.db`.
   - Any refusal stops the run before spawning, with the generic message.
   - The resolved subjects become `CANTOS_IDENTITY_STUDIO_SUBJECTS` for that Server process only.
     Without the flag, behavior is unchanged: an inherited roster is still removed, and a roster
     is never read from the caller's environment or persisted by xtask.
   - `deploy`, `seed`, `verify`, `preflight` and `ci` never accept the flag.

   The documented local flow is: create the account through `run cantos-theatre --web` (emulator
   verification), stop it, then run Studio with `--studio-account`.
4. **No wire, RPC or session change.** No DTO gains a subject. Studio login keeps its 404 when no
   roster is configured, 403 for a roster denial after a verified password, and the session fence.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Identity-owned offline `studio-subject` lookup + explicit roster (proposed) | Data stays with its owner. No new network surface. Lookup authority equals existing file/operator authority. One command serves production and xtask | Needs the Identity process stopped. The roster remains an env value without a durable change audit | Proposed |
| Show the subject to its owner (Theatre account page or session DTO), who sends it to the operator | No operator file access | Changes the frozen session contract. A user-delivered identifier is client-supplied input, which ADR 0034 §3 forbids as admission evidence unless the operator re-verifies it anyway | Rejected |
| `cantos-operator` looks up the subject from the Server's actor registry by actor id | Server-only tooling | The Server has no email. The operator must first learn the actor id and the person must have logged in once. Couples provisioning to session history | Rejected |
| Log the subject on a Studio 403 roster denial | Zero new commands | An attacker-triggered log line, contrary to "statuses and logs never carry request content". The operator still matches logs to people | Rejected |
| Identity RPC that the Server calls to resolve an email | Online and no restart | Adds an email-enumeration RPC and an Identity-side Studio concern, contrary to ADR 0045 §2 | Rejected |
| Move the roster into Identity or a Server table with an admin route | Durable audit and no restart | Admin has no native verifier and fails closed. Larger security change. ADR 0045 §2 places the roster in the Server configuration | Deferred: revisit with Admin verification |
| xtask auto-rosters the first or every local account | Easiest local start | Implicit authority. Inherited or accidental data would become Studio admission | Rejected |
| Keep reading `server.db`/`identity.db` with ad hoc SQL | Works today | Not a supported procedure. Turso is not SQLite (persistence reference). Easy to grab the wrong session's subject | Rejected |

## Consequences

- Product and user impact: an operator can admit the intended one or two Studio accounts without
  touching database files. Developers get a supported authorized Studio session locally.
  Listeners see nothing new.
- Compatibility and migrations: none. Schemas, contracts and existing commands are unchanged.
  The new subcommand and xtask flag are additive.
- Operations, storage and cost: the lookup needs a brief Identity stop. A roster change still
  needs a Server restart. The runbook must record who authorized each roster change, because env
  configuration keeps no history.
- Security and privacy: the lookup reveals whether an email has an eligible account to whoever
  can already open `identity.db`. It adds no remote enumeration. Subject ids are identifiers,
  not secrets, but stay out of Git and chat transcripts beyond what an authorized operator needs.
- Implementation requirements:
  - **Pure core:** one decision mapping a looked-up row to `Eligible(subject)` or a single
    `Refused`. Tests cover every `AccountState`, a pending signup, a missing credential and
    malformed email, and assert identical refusal output.
  - **Identity file tests:** a real `identity.db` with signups completed through the existing
    store APIs. The command refuses while `serve` owns the file.
  - **xtask:** plan tests for flag → env, inherited roster still removed, refusal before any
    spawn, the 8-entry cap and deduplication.
  - **Runtime evidence:** repeat the 23 October browser journey through `run cantos-studio --web --studio-account`,
    with no out-of-band step.
- Deferred questions:
  - Production automation (`cantos-vps` wrapper or Quadlet drop-in) and its recovery.
  - A durable, audited roster after Admin verification exists.
  - Passkey/D3 for Studio.
  - A configurable Studio Trunk port. This is a separate tooling change: on one development Mac
    a system agent holds `*:8081`.

## Validation and revisit conditions

The proposal is satisfied when:

- The pure decision, Identity file tests and xtask plan tests pass in the existing Rust gate.
- `identity_password_turso` keeps passing with its roster supplied from the lookup instead of
  fixture SQL.
- A browser run reaches the Studio writing desk through `xtask run` alone.

Revisit if any of the following happens:

- Admin gains a native verifier.
- Studio needs more than the contract's eight subjects.
- Roster changes must take effect without a Server restart.
- A stronger factor (D3) changes Studio admission.
