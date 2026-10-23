# Cantos — Vọng Đài

Cantos is a content catalog, external-audio handoff and listening platform. **Cantos Studio**
is a Web-only CMS for novels, stories, podcasts, chapters/episodes, versioned instructions,
private audio, rights, review and publication. Listeners see **Cantos**, implemented by the
Theatre Web/CMP apps. External producers create, mix and encode audio; serving/playback never
calls TTS or starts generation. [ADR 0018](docs/decisions/0018-external-audio-production.md)
and [Forge ADR 0022](docs/decisions/0022-cantos-forge-producer-cli.md) own that boundary.

## Current reading path

1. Read the [documentation map](docs/README.md), [architecture](docs/architecture/overview.md)
   and [native decision](docs/architecture/native-turso-refactor.md).
2. Read the [Server checkpoint](docs/architecture/server-turso-porting.md#checkpoint-boundary),
   [Identity](services/cantos-identity/README.md) and [shared store](packages/store-turso/README.md).
3. Read [Identity auth v1](services/cantos-server/contracts/auth/identity-auth/v1/README.md) and
   [private RPC v1](services/cantos-identity/contracts/identity/v1/README.md): Web cookie, CMP/native Bearer,
   distinct purpose/origin/actor authority, fresh security-state checks and bounded receipts.
4. Use [xtask](xtask/README.md) for implemented commands and the
   [short work queue](docs/work-plan/current-priorities.md) for remaining review boundaries.

## Implemented source and remaining gaps

Two native processes own separate embedded Turso files: Server owns `server.db`, Identity owns
`identity.db`. The Server has one Studio/Theatre/Admin HTTP composition, actor bindings,
Identity sessions and development sessions. Identity has mailbox signup, Argon2id password
authentication and authenticated private UDS RPC. It returns facts, never application sessions.
Web cookie and native Bearer consumers exist; browser/device acceptance is separate.

**Native business adapters now use Turso; production acceptance remains pending.** Appended
migrations 12–23 cover all 69 Server-owned business tables. The [retirement record](docs/architecture/server-turso/remaining-pg-retirement.md)
maps 17 removed files / 183 legacy PG tests to actual native boundaries and retired mechanisms;
the earlier six files / 68 tests were retired by the merged PR #395. No archived Rust oracle remains. Provider authentication is removed and
Admin fails closed. Retained PostgreSQL schema/checksum history and recovery assets remain
migration inputs. Host activation, data conversion, restore and full browser/device acceptance
remain separate.

Script IR and domain/application cores cover Editorial, Ingestion, Adaptation, Catalog,
Content, Production Brief, Media/Publication and Listening/Library. Their contracts and prior
PostgreSQL evidence describe behavior to preserve during the port, not production acceptance.
Private filesystem/S3 adapters retain immutable original-target resolution and scoped access.
Publication requires rights, QC and approval; private media never implies public delivery.

Read optimization follows [#384](https://github.com/loveoverflowcom/cantos/issues/384): measure
the actual consumer, optimize Turso SQL/index first, then consider selective ArcSwap/Moka cache.
Incremental engines are DEFER; DBSP's implementation/feature/gate are retired.

## Repository

| Path | Owner |
| --- | --- |
| `services/cantos-server` | Native modular backend; [porting limits](services/cantos-server/README.md) |
| `services/cantos-identity` | Native credential/security owner and private RPC |
| `services/manifest.toml` | Typed process inventory; [runtime policy](services/README.md) |
| `services/cantos-forge-functions` | External producer library/CLI/finite worker; live provider/Functions/Drive execution remains separate #203 work |
| `apps/cantos-studio/web` | Creator Leptos UI; no Studio mobile target |
| `apps/cantos-theatre/{web,mobile}` | Listener Leptos/CMP clients, native playback; isolated opt-in previews |
| `packages/` | Shared store primitives, Web session owner and design tokens; imports neither apps nor services (contracts belong to their service) |
| `infra/` | Edge, Hosting, OCI/Quadlet and deployment policy/assets; no domain implementation |
| `xtask/` | Canonical developer orchestration |
| `.agents/skills/` | Canonical method; `.claude/skills` symlinks and `CLAUDE.md` import share it |
| `docs/`, `specs/behavior/`, `templates/` | Current guidance, lifecycle/catalog expectations and records with real consumers |

Apps call reviewed HTTP contracts and never link service crates into WASM or import each other.
Forge is outside Server serving/login. Firebase Hosting, Functions and App Distribution retain
independent boundaries; removing Firebase Auth does not remove those products.

## Development and verification

Use Rust 1.98.1 from `rust-toolchain.toml`, Python 3.11+ for repository checks and the explicitly
prepared tools for the selected target. No PostgreSQL, Kanidm, container engine or emulator is
needed for native checks. Editor exclusions in `.vscode/settings.json` hide generated output;
they never delete data or change Git tracking.

```sh
cargo xtask doctor
cargo xtask services list
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify turso
cargo xtask verify contracts
cargo xtask build cantos-identity
cargo xtask preflight --changed-only --base "$BASE_SHA"
```

`run` builds/migrates the two native processes with run-owned development files; readiness
is not login or business acceptance. `app cantos-studio|cantos-theatre --web` attaches only a
frontend to an existing API. `seed theatre` currently refuses when the native `catalog_http`
fixture is absent. Retired PG/DBSP/provider/VPS commands fail before effects.

Follow [CONTRIBUTING](CONTRIBUTING.md) and [AGENTS](AGENTS.md): resolve the requested base once,
fetch and pin its SHA for branch/diff/review/preflight. Native work targets
`refactor/cantos-native-turso`; the eventual integration PR to `develop` is a separate epic gate.
Verify focused changes during development, then hand off the final diff once. Record actual
PASS/FAIL/NOT_RUN/UNSUPPORTED with independent model, file, HTTP, browser/device and deployed
states. Historical PASS cannot qualify today's implementation.

Production OCI build/admission is described in the [release contract](docs/deployment/native-oci-release.md).
[Retained VPS records](docs/deployment/vps-production.md) describe legacy operations, not an
available native activation command. No merge, deploy, VPS operation or data retirement follows
from documentation cleanup. Preserve DB/WAL, backups, media, accounts and other tasks' outputs.
