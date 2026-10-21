# Agent instructions — Cantos

> **Current product boundary (ADR 0018):** Cantos Studio is Web-only CMS for
> novel/story/podcast, chapters/episodes, versioned instructions and external audio assets.
> **AI plugins/human producers produce, mix and encode audio outside Cantos**, then use a
> secure private handoff API. No TTS/provider/worker in Cantos API. Theatre only plays releases.
> [Forge ADR 0022](docs/decisions/0022-cantos-forge-producer-cli.md) records the owner-selected
> local/Functions producer and private Drive drafts: Studio controls operations, Forge executes
> outside the API. The local artifact CLI, attempt/recovery core, local attempt store and finite
> worker boundary are implemented; provider/Functions/Drive execution and API handoff remain
> unimplemented. The [worker runbook](docs/deployment/forge-worker.md) separates local source
> from deployed packaging and live acceptance.
> See [current decision](docs/decisions/0018-external-audio-production.md) and
> [proposed, not-yet-implemented handoff](packages/contracts/external-audio-handoff-v1.md).

## Scope and source of truth

Read [README.md](README.md), the [documentation map](docs/README.md), relevant owning docs and
[current queue](docs/work-plan/README.md) before implementing. Use the map's lifecycle rules:
archived handoffs and source-bound evidence are not active plans; proposed ADRs are not accepted
policy. Inspect actual code before treating a module, schema, command or API as implemented.

The current user's instructions take priority. The current fresh PostgreSQL schema is epoch 2;
ordinary deploy/rollback refuses legacy epoch-1 databases. The explicit narrow
`--cutover-epoch2` lane and its same-revision/attempt incoming-tree recovery are owned by the
[controlled cutover](docs/deployment/postgres-schema-cutover.md); populated-data conversion
remains unimplemented.
Retired SQL is auditable through its manifest and original Git history, never silently replayed.
 Preserve Cantos Studio and Cantos Theatre terminology, Rust/Axum/PostgreSQL/Leptos/CMP choices and native mobile playback. Keep docs in English unless the task requests otherwise; retain correct Vietnamese names and representative content.

## Repository layout

`apps/cantos-theatre/{web,mobile}` is Cantos Theatre (listeners), `apps/cantos-studio/web` is Cantos Studio (creators, Web only), `services/cantos-forge-functions` is the producer-side Cantos Forge library/CLI ([ADR 0022](docs/decisions/0022-cantos-forge-producer-cli.md); today register/validate/export local artifacts, attempt/recovery core, local durable attempt adapter, finite worker and Node process boundary, with no real provider call or API handoff; accepted Studio/Functions/appData execution remains unimplemented), `services/cantos-server` is the authoritative Rust/Axum/PostgreSQL modular backend (not the only possible process), `services/kanidm` is identity-provider configuration only, `packages/` holds shared contracts and design tokens, `infra/` owns deployment/runtime assets (`infra/edge` and `infra/firebase/hosting` today), and `xtask/` owns the `cargo xtask` developer command surface, not deployment assets. Forge lives at `services/cantos-forge-functions` with the existing `cantos-forge` crate and CLI identity; its hosting and live-provider acceptance remain separate #203 gates. Apps depend on `packages/` and call the API over HTTP; they never import each other or service code, and `packages/` imports neither. Do not mix Theatre and Studio UI or permissions. Naming is strict: `cantos-theatre`/`theatre` is the technical name (path, package, API group, DTOs), while listeners only ever see **Cantos** (never "Cantos Theatre") and creators see **Cantos Studio**. The API has three route groups with separate routers, DTOs, authentication and data access: `/api/v1/studio/*` (implemented; the pre-split `/api/v1/*` paths are a deprecated alias), `/api/v1/theatre/*` (anonymous public discovery/playback core; delivery acceptance pending) and `/api/v1/admin/*` (opt-in dedicated capability/revocation and audit; live issuer/group acceptance pending). Never serve a Theatre or Admin feature from a Studio handler, reuse a Studio session for them, or add fake data to a reserved group ([decision 0015](docs/decisions/0015-api-surface-boundaries.md)). [Repository layout](docs/architecture/repository-layout.md) records what is implemented, a skeleton or only reserved; do not present a skeleton as a feature.

### Platform scope

`services/cantos-identity` contains the default dependency-free
[security-state and mailbox-proof decision library](services/cantos-identity/README.md) and an opt-in
`persistence` shell using the shared Turso owner for instance/subject security state, disable
events, mailbox-proof lifecycles and bound receipts. Proof confirmation grants no setup or login
authority. Its opt-in `service` feature builds the `cantos-identity` executable: Theatre signup with
a mailbox proof, Argon2id passwords and `AuthenticatePassword` over an OS-peer-authenticated private
Unix-socket RPC ([draft contract](packages/contracts/identity-rpc-v1.md)); the result is an
authentication fact, never a session. Studio passkeys, recovery, restore rotation and front ends remain absent; the
Server consumer (PostgreSQL sessions, v1 email/password) is [Identity auth v1](packages/contracts/identity-auth-v1.md). Checked snapshots and a
passed completion decision or mailbox transition plan are not authentication facts. New persistence/auth work for
#331–#336 branches from and targets `refactor/cantos-native-turso`, as specified by those issues.

Cantos Studio is web-only. Do not implement, scaffold, build or maintain a mobile client unless explicitly requested in a future roadmap.
`apps/cantos-studio/mobile` may retain a scope notice only; never add it to workspace/Gradle,
xtask, CI, build/test automation or Compose previews. Cantos Theatre still supports Web and
Android/iOS Mobile at `apps/cantos-theatre/{web,mobile}` (native playback). Shared libraries may
serve Theatre Mobile and Studio Web. Consult the [platform matrix](docs/architecture/repository-layout.md#platform-scope)
for scope versus implementation status.

## Architecture boundaries

- Begin with a modular monolith; add a service, crate or generic abstraction only for a concrete consumer and reviewed need.
- Domain rules and versioned Script IR must not depend on HTTP, UI frameworks, a specific AI provider or Narrative Forge internals.
- Use PostgreSQL for metadata, revision history, producer submission receipts and release pointers. Store verified received media in private object storage until review/publication.
- Do not invoke AI during listener playback. Preserve immutable publication manifests and cached renditions.
- Keep private drafts, voice assets and previews inaccessible to public delivery. Enforce permissions in the backend.
- Mobile uses CMP with platform-native audio adapters; no WebView audio player.

## Correctness rules

For external audio handoff, reason about **producer scope and revocation, duplicate upload,
lost finalize response, private object integrity, stale briefs, content ownership, wrong
episode binding, rights, QC and publication approval invalidation**. The AI producer owns
voice-provider timeouts, any TTS attempts and costs: there are **no Cantos generation jobs**.
Server acceptance must verify actual bytes/hash/format and persist immutable receipts.

A dialogue/text change can mark older submissions stale but never auto-regenerates audio.
Any imported or externally produced asset requires provenance and rights claims; claims
are not the same as verified rights or human-listened quality.

## Working and review

Branch from `develop` and target PRs to `develop`. Keep changes cohesive and avoid unrelated cleanup. Update the work queue after implementation; GitHub issues/PRs own execution status when linked, and the queue owns recommended order and rationale.

Use [templates](templates/README.md) for a concrete proposal or production record when relevant. Clearly distinguish proposed design, implemented behavior and verified behavior. Never commit secrets, real provider credentials, private manuscripts, downloaded media or generated audio to Git.

## Agent skills

Canonical skills live in [`.agents/skills/`](.agents/skills/README.md) and are shared by Codex, Claude Code and any other agent; `.claude/skills/<name>` entries are symlinks to them. Read the skill index, then load the skill that owns the task:

- [`cantos-engineering`](.agents/skills/cantos-engineering/SKILL.md) for any domain, backend, persistence, contract, test or verification change. It owns the required order, decoupling, immutability, functional core, the evidence vocabulary and the completion report.
- [`cantos-behavior`](.agents/skills/cantos-behavior/SKILL.md) for catalog lookup, Quint stateful expectations, AI scenario generation, non-vacuity and counterexamples across API/Leptos/CMP; models do not establish implementation conformance.
- Domain owners: [`cantos-script-ir`](.agents/skills/cantos-script-ir/SKILL.md), [`cantos-production-pipeline`](.agents/skills/cantos-production-pipeline/SKILL.md), [`cantos-publication`](.agents/skills/cantos-publication/SKILL.md), [`cantos-listening`](.agents/skills/cantos-listening/SKILL.md).
- UI: [`cantos-ui-design`](.agents/skills/cantos-ui-design/SKILL.md) (Material 3 Expressive and shared UI policy) with [`cantos-leptos-web`](.agents/skills/cantos-leptos-web/SKILL.md) or [`cantos-cmp-mobile`](.agents/skills/cantos-cmp-mobile/SKILL.md).
- Task entrypoints: [`cantos-work-item`](.agents/skills/cantos-work-item/SKILL.md) to deliver a work-plan item, [`cantos-code-review`](.agents/skills/cantos-code-review/SKILL.md) for a read-only review, [`cantos-ui-inspector`](.agents/skills/cantos-ui-inspector/SKILL.md) for a report-only UI audit, [`cantos-ui-fix`](.agents/skills/cantos-ui-fix/SKILL.md) for one bounded UI repair, [`cantos-logic-fix`](.agents/skills/cantos-logic-fix/SKILL.md) for one bounded invariant/logic repair. Both fix entrypoints compose existing owners, use minimal changes and independent layered evidence; neither adds rules or permission. Review/inspection does not authorize fixing. [`cantos-diagnostics`](.agents/skills/cantos-diagnostics/SKILL.md) investigates a failing gate, test, build or runtime log: it reads output summary-first under a context budget, separates environment/pre-existing/flaky failures from regressions, keeps full output in ignored `artifacts/agent-runs/` and reports a diagnosis without repairing.
- Story artifact workflows: [`cantos-story-ingest`](.agents/skills/cantos-story-ingest/SKILL.md), [`cantos-radio-adapt`](.agents/skills/cantos-radio-adapt/SKILL.md), [`cantos-audio-produce`](.agents/skills/cantos-audio-produce/SKILL.md) and [`cantos-story-audit`](.agents/skills/cantos-story-audit/SKILL.md) share the [Drive/local workspace contract](docs/production/story-workspace.md). They preserve raw sources, maintain `score/name_map.yml`, coordinate immutable score/audio revisions and recover blocked synchronization through local records and GitHub issues. **Only external agents run audio rendering**, never the Cantos backend. Keep all story data outside Git.

Skills own working method; `docs/` owns product rules. Keep one copy of each skill and update it rather than adding per-runtime instructions.

## Verification

Use the foundation's [verification cadence](.agents/skills/cantos-engineering/references/verification-cadence.md):
focused checks during DEV, affected module/renderer checks at CHECKPOINT, and preflight on the
final diff at HANDOFF, with CRITICAL evidence for changed high-risk invariants. Do not run a
full verification campaign after each edit or commit. Reuse attributable results while their
inputs remain valid; commit/push/PR packaging of the same content needs no duplicate run.

Follow the foundation's [layered verification order](.agents/skills/cantos-engineering/references/verification-strategy.md#layered-verification)
and [local handoff policy](.agents/skills/cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion):
verify the pure core first and deliver independently verified layers through requested
commit/push/PR actions. Missing infrastructure does not hold passed core work pending; report
each layer's evidence, deferred acceptance, blocker and actual failure separately. Do not wait
for remote CI. Required checks and production acceptance retain their gates; use the existing
docs-only fallback when applicable.

Optional Studio identity operator tools are `cargo xtask kanidm reconcile --dry-run` (plan only)
and `cargo xtask kanidm verify` (explicit read-only discovery/JWKS, never login acceptance).
They never bootstrap accounts, grants, secrets, Podman or TLS; live steps require operator authority.

The explicit local enrollment lane is `cargo xtask verify theatre-auth --suite firebase-emulator`
([runbook](docs/deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing));
`--dry-run` only prints a plan. It is excluded from ordinary gates/preflight/CI and requires
separately installed tools. Its recording mock issuer does not establish Kanidm/OIDC or browser
acceptance; the `firebase-web-e2e` lane reports `UNSUPPORTED` until that fixture is implemented.

Synthetic Theatre catalog data for local Web/CMP integration is the explicit
`cargo xtask seed theatre [--reset]` ([runbook](docs/deployment/theatre-seeded-catalog.md)). It
defaults to a new database in a proven disposable cluster retained by `verify postgres --keep`
(including an explicit narrow scope). The ownership receipt preserves verification outcomes;
retention never establishes a passing gate.
The explicit named `--environment` lane reconciles only its journal-owned dataset in an existing
epoch-2 environment and never resets, migrates, grants or drops persistent data; see the runbook
for mandatory API/auth/storage inputs. Seeding is never a gate, and synthetic rights/QC records
and stored signal bytes are not native playback or human-listening evidence.

Tester distribution of the Theatre Android development/release APK is the explicit, opt-in
`cargo xtask distribute firebase cantos-theatre --channel development|production ...` ([contract](xtask/README.md#distribute-firebase)).
It is a remote write that may notify testers: `--dry-run` shows the plan, and an agent runs the real
command only on the user's explicit request naming the Firebase app and recipients. Build, run,
verify, preflight and ci never invoke it; an uncertain remote outcome is reconciled in the Firebase
console, never replayed blindly.

Static Web deployment of one app (Theatre or Studio) to Firebase Hosting is the explicit, opt-in
`cargo xtask deploy firebase <cantos-theatre|cantos-studio> --web --project ID --channel live|NAME ...`
([contract](xtask/README.md#deploy-firebase), [runbook](docs/deployment/firebase-hosting.md)). It is a
remote write that publishes a PUBLIC preview URL or replaces a live site: `--dry-run` shows the plan,
and an agent runs the real command only on the user's explicit request naming the Firebase project,
the channel and the app. Build, run, verify, preflight and ci never invoke it, and it serves static
files only. [ADR 0041](docs/decisions/0041-direct-hosting-api-boundary.md) selects direct Hosting
with an explicit API origin and exact per-surface CORS; installed legacy proxies are transitional.
The [#295 owner amendment](https://github.com/loveoverflowcom/cantos/issues/295) selects
HttpOnly/Secure Web cookies on the custom domains; that migration and browser acceptance remain
pending. Native keeps #253 Bearer. Exact-origin CORS, CSRF and separate surface authority remain required.
Use the [current production runbook](docs/deployment/vps-production.md), not old deployment records.
An uncertain remote outcome is reconciled in the Firebase console, never replayed blindly.

The requested Cloudflare transition follows the [rollout/recovery runbook](docs/deployment/cloudflare-rollout.md)
and #277/#278 promotion/publisher prerequisites. `cargo xtask hosting-report --record PATH
[--dry-run]` is an optional offline operator-declared report; `--self-test` exercises synthetic
records. It never queries/deploys a provider or establishes live acceptance, and adds no gate.
`cargo xtask hosting-smoke` is the explicit read-only static HTTP lane
([contract](xtask/README.md#read-only-hosting-http-observation)): verified release v2 input,
exact static origin, declared deployment ID, sanitized receipt; no publish/API/auth/browser or
rollback action. Its deterministic regressions run in the existing Rust gate, with no new gate.
Retire only Firebase Hosting after separately authorized cutover/recovery acceptance and the
agreed retention window; preserve Auth, Functions, App Distribution, API and storage owners.

Cloudflare Pages Direct Upload of one already built Web artifact is the explicit, opt-in
`cargo xtask deploy cloudflare <cantos-theatre|cantos-studio> --web --account ID --project NAME --branch BRANCH --environment production|preview --artifact DIR`
([contract](xtask/README.md#deploy-cloudflare), [runbook](docs/deployment/cloudflare-pages.md),
[ADR 0043](docs/decisions/0043-cloudflare-pages-direct-upload.md), proposed). It never builds, is a
remote write that publishes a PUBLIC `*.pages.dev` preview or replaces a live project: `--dry-run`
shows the plan, and an agent runs the real command only on the user's explicit request naming the
account, project, branch, environment and artifact. Creating the token, projects or domains and any
cutover are operator steps; agents never create credentials. Source-bound preview observations
are recorded in the runbook; production/domain cutover acceptance remains separate.

Production API deployment to the VPS is the explicit, opt-in
`cargo xtask deploy vps cantos-server --host USER@HOST [--bootstrap|--rollback|--status|--cutover-epoch2]`
([contract](xtask/README.md#deploy-vps), [runbook](docs/deployment/vps-production.md)). It builds the
image locally from committed `HEAD`, then the uploaded `infra/vps/bin/cantos-vps` backs up, migrates,
grants, switches and health-gates with automatic switch-back. It is a remote write: `--dry-run` shows
the plan, and an agent runs it only on the user's explicit request naming the host.
The separate `--upgrade-epoch2-prefix` lane preserves an epoch-2 database and upgrades a restored
sibling by the candidate's exact additive registry suffix (`[1]`→`[1,2]`, `[1,2]`→`[1,2,3]`); read its [operator/recovery runbook](docs/deployment/postgres-prefix-upgrade.md).
It requires separately explicit operator authority and a native Linux x86_64/17.11 rehearsal;
ordinary deploy checks remain strict.
`cantos-vps identity` is operator-run: the operator may explicitly delegate it to an AI agent
for a named host and accounts, including Kanidm account creation, OAuth2 client/group setup and
credential-reset issuance. Once authorized, the agent may complete that scope without repeated
confirmation. Deployment authorization alone does not authorize identity changes. Keep secrets
and one-time credential links out of chat, agent logs and Git; the operator completes personal
password/passkey setup in their own interactive session. CircleCI (`.circleci/config.yml`) only
offers a manual pipeline that calls the deployment commands.
The existing-client `identity --reconcile-existing` lane has no account/reset path; its exact
callback readback, preservation checks and cutover recovery follow the same runbook.

Repository-level operations go through `cargo xtask`; scripts and external tools are implementation primitives, not competing orchestration interfaces. Read the [command contract](xtask/README.md) before running or adding a check, and never add a parallel build/test/run entrypoint. Add meaningful crash/retry and publication-gate coverage when implementing those flows. UI changes require observations on relevant viewports/platforms, realistic Vietnamese text, focus/text scaling and accessibility; an image alone does not verify interaction or motion.

The explicit `cargo xtask verify native-isolation --test-binary PATH --test NAME` lane runs
one prebuilt exact Rust test with private run directories, an empty PATH and credential,
executable and supported Linux network negative controls. It never builds or provisions;
macOS reports network isolation and overall acceptance as `UNSUPPORTED`. Its regression tests
belong to existing repository/Rust gates. Read the [guard boundary](docs/assurance/native-isolation.md)
before using a receipt: it is not two-binary native E2E, authentication or Kani evidence.

The table maps affected scope to checks at the applicable milestone, not a sequence to run
after every edit. Preflight already includes repository/format and selected suites; do not
run their standalone equivalents again when covered by that handoff run.

| Change | Required check at the applicable milestone |
| --- | --- |
| Release artifact contract / CI promotion | `cargo xtask release self-test` for focused contract, DAG and mocked-adapter regressions; Python tests are included in the repository gate and Rust CLI/deploy tests in the Rust gate. Real CI/build/activate acceptance remains separate. |
| Repository hygiene (documents, skills, scripts) | `repository` gate, included in handoff preflight; `cargo xtask verify repository` for a focused hygiene check when needed |
| Completed handoff or a pull request | `cargo xtask preflight --changed-only` on the final diff, or cite a still-valid run covering it (broadens to every gate when impact is uncertain) |
| Rust code, Script IR or contracts | `cargo xtask verify rust` and `cargo xtask verify contracts` |
| Identity pure security core | Existing `rust` gate includes `cantos-identity`; metadata-based preflight selects its lint/tests. Focused primitive: `cargo test --locked -p cantos-identity`. Optional `verify kani --harness identity-security-version --harness identity-completion --harness identity-mailbox-proof` needs actual D1 Ubuntu x86_64 evidence; macOS tests are not Kani evidence. |
| Forge worker/process boundary | `cargo xtask verify forge` (Rust producer plus Node wrapper tests; no provider or cloud calls) |
| Embedded Turso foundation and Identity security persistence | `cargo xtask verify turso` (Identity `persistence` clippy/tests, shared-store file-backed tests and engine capability report; no server, container or cloud service). Also lints and tests the `service` feature (the executable, real Turso files, real Argon2id, a run-owned loopback SMTP sink) and `cantos-identity-rpc`. Focused primitives: `cargo test --locked -p cantos-identity --features persistence`, `cargo test --locked -p cantos-identity --features service`. |
| Registered Quint models/tooling | `cargo xtask verify behavior` (model-only by default; `--conformance` adds the registered pure-core observer; explicit `--setup` for the locked development tool) |
| Script IR validator/encoder | additionally `cargo xtask verify mutation` |
| Studio or Theatre Web | `cargo xtask verify web` |
| Persistence, migrations or the HTTP host | `cargo xtask verify postgres` (PostgreSQL `initdb`, `pg_ctl` and `psql` on `PATH`); optional `--suite schema` verifies the current baseline/refusal/grants and selected current SQL owners, and `--suite theatre-firebase` only Firebase/SMTP registration SQL; neither replaces the full gate |
| Generic DBSP incremental engine (job summary binding retired) | `cargo xtask verify dbsp`; do not reintroduce a synthesis worker to satisfy it |
| Local PNG comparator/tooling | `cargo xtask visual-compare --self-test` (core plus optional pinned-Pillow tests); compare existing attributed pairs with `cargo xtask visual-compare` and open the images; metric PASS is not UX approval |

Use Rust 1.98.1 (`RUSTUP_TOOLCHAIN=1.98.1`, as in CI). Current schema supports stable PostgreSQL 17/18;
production major upgrades remain separately approved. PostgreSQL checks create a new disposable
cluster; never substitute a production or inherited database URL. `cargo xtask run`/`app` start
only local processes they own and never provision databases, identity or providers. Report each
gate as xtask printed it: `PASS`, `FAIL`, `NOT_RUN` (explicit `--skip` or not reached) or
`UNSUPPORTED` (missing tool, or a mobile surface without a Gradle project). Studio runtime
interaction/accessibility evidence is separate from the WASM build; no mobile runtime gate exists.
On affected macOS temporary-path setups, use `TMPDIR=/private/tmp` for Python unit/mutation runs.

For optional Web release asset/hash/compression inventory, use `cargo xtask web-performance
<cantos-studio|cantos-theatre>`; see the [measurement boundary](docs/evidence/web-performance-patterns.md).
Offline bundle sizes and reactive-work counters never establish browser startup, cache hits or
deployed headers. This command adds no required gate or production cache policy.

Static Compose UI capture goes through `cargo xtask compose-preview <cantos-theatre|ui-system-cmp>`;
read the [CMP preview workflow](.agents/skills/cantos-cmp-mobile/references/compose-preview.md).
It needs a real mobile Gradle project and Android Studio Quail 2+ with that project open, but no
emulator or phone. Theatre Mobile has no Android preview wrappers after #287 (the Desktop Preview is JVM-only);
capture requires the official CLI and a ready synced IDE, otherwise it is `UNSUPPORTED`.
Studio Mobile is excluded. The shared `ui-system-cmp` library has an Android preview host. Open the PNG and assert the observed semantics before claiming inspected/tested
evidence; host rendering does not replace Android/iOS runtime or screen-reader gates.

Do not report an app build, mobile test, audio quality result or end-to-end production run unless it actually ran. Record limitations and required follow-ups with evidence.

## Service runtime ownership

For [#348](https://github.com/loveoverflowcom/cantos/issues/348), production distribution is
GHCR registry digests plus Podman Quadlet for the two native executables. This supersedes only
the native refactor's production no-container clause; native dev/E2E/Turso/Kani remain
container-independent. Target `refactor/cantos-native-turso`. The implemented
[`release oci-verify`](docs/deployment/native-oci-release.md) is offline and plan-only;
its Python tests run in release self-test/repository and its CLI tests in the Rust gate.
Explicit `--registry-readback` adds scoped GHCR manifest/config observation; dry-run remains
offline, with no authfile access, and neither mode establishes native/image-pair acceptance.
Local/Actions producers and native activation remain pending; existing `deploy vps` is the
legacy archive lane. Preserve current runtime/principal, PG/Kanidm/ingress and recovery until
real native acceptance and separately authorized cutover. Missing inventory is `UNKNOWN`,
never permission to bootstrap. Build/publish, inventory, bootstrap, activate, restore and
cleanup are separate operational scopes.

A service is a runnable process or independently managed infrastructure, not necessarily a Rust
crate. Keep deployment/runtime assets under [infra/](infra/README.md), separate from application
code and `xtask` orchestration; the [layout](docs/architecture/repository-layout.md) distinguishes
current paths from future consumers. Follow [the service inventory](services/README.md) and
[ADR 0021](docs/decisions/0021-podman-service-runtime.md). Podman is the standard OCI runtime;
Docker Engine/Desktop/daemon and Compose are not dependencies (`docker.io` remains an allowed
registry). Only explicit `cargo xtask services up` starts local infrastructure; the production VPS runs its own
Quadlet units from [`infra/vps`](infra/vps/README.md). Build/test/run
never auto-start it. Native disposable PostgreSQL verification stays independent. Never install
Podman, initialize/start a machine or change its connection without explicit consent. Manage
only exact Cantos-owned labeled resources; stop never deletes persistent volumes. Keep secrets,
TLS and data outside Git, and distinguish configuration from observed runtime and integration.
