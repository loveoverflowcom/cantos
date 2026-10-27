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
> [proposed, not-yet-implemented handoff](services/cantos-server/contracts/producer/handoff/v1/README.md).

## Scope and source of truth

Read [README.md](README.md), the [documentation map](docs/README.md), relevant owning docs and
[current queue](docs/work-plan/README.md) before implementing. Use the map's lifecycle rules:
archived handoffs and source-bound evidence are not active plans; proposed ADRs are not accepted
policy. Inspect actual code before treating a module, schema, command or API as implemented.

The current user's instructions take priority. The Server now composes embedded Turso
`server.db`; Identity exclusively owns `identity.db`. Business schema/SQL porting remains
incomplete: follow the [checkpoint](docs/architecture/server-turso-porting.md#checkpoint-boundary).
Retained PostgreSQL epoch-2 schema, checksum history and test oracles are migration inputs,
not an active runtime workflow. Never edit registered migration SQL or infer business acceptance
from successful owner startup. Preserve Rust/Axum/Leptos/CMP and native playback. Keep docs in
English unless requested otherwise; retain correct Vietnamese names and representative content.

## Repository layout

`apps/cantos-theatre/{web,mobile}` owns listeners and `apps/cantos-studio/web` owns creators
(Web only). `services/cantos-server` owns one native HTTP process, actors, sessions and business
metadata in `server.db`; `services/cantos-identity` owns credentials/security in `identity.db`
and returns facts over authenticated private RPC. `services/manifest.toml` is the process inventory.
`services/cantos-forge-functions` is the external producer CLI/finite worker; real provider,
Functions/Drive execution and API handoff remain separate #203 gates. `packages/` imports neither
apps nor services. Apps call HTTP, never import service code or each other. Each service owns its
canonical contracts (`services/*/contracts/<surface>/<module>/<version>`) and each consumer keeps
its own representation; there is no shared contract crate ([ADR 0046](docs/decisions/0046-service-owned-contracts.md)). `infra/` owns assets;
`xtask/` owns commands. [Layout](docs/architecture/repository-layout.md) owns detailed placement.

Technical names remain `cantos-theatre`/`theatre`; listeners see **Cantos**, creators **Cantos Studio**.
Studio/Theatre/Admin keep separate routers, DTOs, credential purpose/origin and access policy.
The deprecated `/api/v1/*` Studio alias shares its handlers; it never grants another surface.
Provider authentication is removed and Admin fails closed. Keep producer/brief/upload authority
separate from user sessions; never use a Studio credential for Theatre, Admin or producer access.

### Platform scope

`services/cantos-identity` contains the default dependency-free
[security-state and mailbox-proof decision library](services/cantos-identity/README.md) and an opt-in
`persistence` shell using the shared Turso owner for instance/subject security state, disable
events, mailbox-proof lifecycles and bound receipts. Proof confirmation grants no setup or login
authority. Its opt-in `service` feature builds the `cantos-identity` executable: Theatre signup with
a mailbox proof, Argon2id passwords and `AuthenticatePassword` over an OS-peer-authenticated private
Unix-socket RPC ([draft contract](services/cantos-identity/contracts/identity/v1/README.md)); the result is an
authentication fact, never a session. The [v1 owner amendment](docs/architecture/native-turso-refactor.md#owner-amendment-v1-authentication-and-parallel-integration)
and [ADR 0045](docs/decisions/0045-v1-email-password-sign-in.md) own factor scope and parallel integration.
Studio passkeys, recovery and restore rotation remain deferred. Web cookie and CMP/native
Bearer consumers exist; full browser/device acceptance is separate. The Server consumer
(Turso sessions, v1 email/password) is [Identity auth v1](services/cantos-server/contracts/auth/identity-auth/v1/README.md). Checked snapshots and a
passed completion decision or mailbox transition plan are not authentication facts. New persistence/auth work for
#331–#336 branches from latest `origin/develop` and targets `develop` after PR #400; the retired
refactor branch and pinned earlier observations are historical context.

Cantos Studio is web-only. Do not implement, scaffold, build or maintain a mobile client unless explicitly requested in a future roadmap.
`apps/cantos-studio/mobile` may retain a scope notice only; never add it to workspace/Gradle,
xtask, CI, build/test automation or Compose previews. Cantos Theatre still supports Web and
Android/iOS Mobile at `apps/cantos-theatre/{web,mobile}` (native playback). Shared libraries may
serve Theatre Mobile and Studio Web. Consult the [platform matrix](docs/architecture/repository-layout.md#platform-scope)
for scope versus implementation status.

## Architecture boundaries

- Begin with a modular monolith; add a service, crate or generic abstraction only for a concrete consumer and reviewed need.
- Domain rules and versioned Script IR must not depend on HTTP, UI frameworks, a specific AI provider or Narrative Forge internals.
- Preserve durable metadata, revision history, producer receipts and release pointers in the authoritative store: Server-owned Turso (business port incomplete). Store verified received media in private object storage until review/publication.
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

Resolve the base once: explicit user base, else the work item's accepted integration/staging
base, else `develop`. Fetch its latest remote HEAD; use the same `BASE_REF` for branching,
diff/review, preflight `--base`, clean-baseline reproduction and PR creation. Follow
[the workflow](CONTRIBUTING.md#branch-and-pull-request-workflow). Keep changes cohesive and avoid
unrelated cleanup. Update the work queue when order/rationale changes; GitHub owns execution status.

Use [templates](templates/README.md) for a concrete proposal or production record when relevant. Clearly distinguish proposed design, implemented behavior and verified behavior. Never commit secrets, real provider credentials, private manuscripts, downloaded media or generated audio to Git.

**Credential custody:** The [Google Secret Manager operator guide](docs/deployment/google-secret-manager.md) owns project `cantos-a348b`, the nonsecret [inventory](infra/google-secret-manager/inventory.toml), least-privilege IAM, version pins, rotation and local/CI/VPS boundaries; `cargo xtask secrets inventory|status|import` is the operator-only helper ([contract](xtask/README.md#secrets)), never a build/CI side effect. Only nonsecret names, mapping and instructions may be tracked; no keys, JSON credentials, plaintext or ciphertext credential payloads belong in Git. The guide and synthetic tests are not evidence of live GCP provisioning, imports or runtime binding. Agents request missing credentials through an approved secure local route, not chat or issue bodies.

## Agent skills

Canonical skills live in [`.agents/skills/`](.agents/skills/README.md) and are shared by Codex, Claude Code and any other agent; `.claude/skills/<name>` entries are symlinks to them. Read the skill index, then load the skill that owns the task:

- [`cantos-engineering`](.agents/skills/cantos-engineering/SKILL.md) for any domain, backend, persistence, contract, test or verification change. It owns the required order, decoupling, immutability, functional core, the evidence vocabulary and the completion report.
- [`cantos-behavior`](.agents/skills/cantos-behavior/SKILL.md) for catalog lookup, Quint stateful expectations, AI scenario generation, non-vacuity and counterexamples across API/Leptos/CMP; models do not establish implementation conformance.
- Domain owners: [`cantos-script-ir`](.agents/skills/cantos-script-ir/SKILL.md), [`cantos-production-pipeline`](.agents/skills/cantos-production-pipeline/SKILL.md), [`cantos-publication`](.agents/skills/cantos-publication/SKILL.md), [`cantos-listening`](.agents/skills/cantos-listening/SKILL.md).
- UI: [`cantos-ui-design`](.agents/skills/cantos-ui-design/SKILL.md) (Material 3 Expressive and shared UI policy) with [`cantos-leptos-web`](.agents/skills/cantos-leptos-web/SKILL.md) or [`cantos-cmp-mobile`](.agents/skills/cantos-cmp-mobile/SKILL.md).
- Task entrypoints: [`cantos-work-item`](.agents/skills/cantos-work-item/SKILL.md) to deliver a work-plan item, [`cantos-code-review`](.agents/skills/cantos-code-review/SKILL.md) for a read-only review, [`cantos-ui-inspector`](.agents/skills/cantos-ui-inspector/SKILL.md) for a report-only UI audit, [`cantos-ui-fix`](.agents/skills/cantos-ui-fix/SKILL.md) for one bounded UI repair, [`cantos-logic-fix`](.agents/skills/cantos-logic-fix/SKILL.md) for one bounded invariant/logic repair. Both fix entrypoints compose existing owners, use minimal changes and independent layered evidence; neither adds rules or permission. Review/inspection does not authorize fixing. [`cantos-diagnostics`](.agents/skills/cantos-diagnostics/SKILL.md) investigates a failing gate, test, build or runtime log: it reads output summary-first under a context budget, separates environment/pre-existing/flaky failures from regressions, keeps full output in ignored `artifacts/agent-runs/` and reports a diagnosis without repairing.
- Task prompts: [`cantos-prompt`](.agents/skills/cantos-prompt/SKILL.md) authors self-contained `CANTOS_PROMPT_INPUT_V1` prompts for another agent or work chat (one prompt, split, review-to-prompt, resume) and edits nothing. A received prompt, from that skill, the external Cantos Prompt plugin or a pasted work chat, goes to the entrypoint its `TASK` names through the shared [receiving procedure](.agents/skills/cantos-prompt/references/prompt-contract.md#receiving-a-prompt): its `AUTHORIZED_ACTIONS` are the user's grant when the user submits it, its snapshot is revalidated against HEAD, and blockers are classified while independent work continues. Plain-language requests need no template.
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

Native command truth is [xtask](xtask/README.md), `xtask/src/cli.rs`, `retired.rs` and `gate.rs`.
`run` supervises Server + Identity over private files and a socket; explicit migrations precede
serve. `services list|validate` remains available. Plain `seed theatre` refuses while the native
`catalog_http` fixture is absent. Its explicit [prepared-sample lane](docs/deployment/theatre-seeded-catalog.md)
uses `--sample-input JSON --sample-media-source DIR --sample-root DIR` for six private Works,
twelve draft chapters and the six supplied covers in isolated files outside Git. Mock audio
stays unavailable; public-cover policy and Web/CMP acceptance remain separate. `--sample-serve`
is a test-hosted loopback Server router, with no Identity. `--sample-root DIR --reset` archives
only a marker-bound, owner-unlocked sample root and never deletes it. Kanidm, PostgreSQL/DBSP verification, provider enrollment,
container lifecycle and legacy VPS/API-release commands are hidden retired refusals before effects.
Use `verify rust`, `verify turso` and scoped native qualification; no old stack installation is
part of cleanup. Preserve historical PG tests/schema and source-bound evidence.

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
HttpOnly/Secure Web cookies on the custom domains; both Web apps now consume the
[Identity auth v1](services/cantos-server/contracts/auth/identity-auth/v1/README.md) cookie endpoints through
[`packages/web-auth`](packages/web-auth/README.md) (email/password, no OIDC popup or Bearer fallback),
while browser and real Server/Identity acceptance remain pending. Native keeps its opaque Bearer.
Exact-origin CORS, CSRF and separate surface authority remain required.
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
Retire Firebase Hosting only after separately authorized cutover/recovery acceptance and the
agreed retention window; preserve Functions, App Distribution, API and storage owners; Firebase Auth source retirement is separate.

Cloudflare Pages Direct Upload of one already built Web artifact is the explicit, opt-in
`cargo xtask deploy cloudflare <cantos-theatre|cantos-studio> --web --account ID --project NAME --branch BRANCH --environment production|preview --artifact DIR`
([contract](xtask/README.md#deploy-cloudflare), [runbook](docs/deployment/cloudflare-pages.md),
[ADR 0043](docs/decisions/0043-cloudflare-pages-direct-upload.md), proposed). It never builds, is a
remote write that publishes a PUBLIC `*.pages.dev` preview or replaces a live project: `--dry-run`
shows the plan, and an agent runs the real command only on the user's explicit request naming the
account, project, branch, environment and artifact. Creating the token, projects or domains and any
cutover are operator steps; agents never create credentials. Source-bound preview observations
are recorded in the runbook; production/domain cutover acceptance remains separate.

`cargo xtask deploy vps` and legacy `api`/`all` release routes are retired refusals, including
CircleCI selections; no current workflow receives the old VPS context. The
[native OCI contract](docs/deployment/native-oci-release.md) owns candidate publication,
qualification/admission and the optional digest-pinned, operator-declared host inventory plan.
The plan is offline and never locks, pulls, activates, restores or changes a host. Native
First-native/legacy Quadlet cutover and actual health/reboot/recovery acceptance remain separate #348 work; the same-schema NATIVE_READY upgrade lane is described below.
The [historical VPS runbook](docs/deployment/vps-production.md) and
[PostgreSQL prefix-upgrade recovery record](docs/deployment/postgres-prefix-upgrade.md) retain
legacy recovery context; they are not current release instructions. Preserve their historical
schema/checksum evidence and all real host/database/WAL/media resources. Deployment authority
never grants identity or cleanup authority; keep credentials and one-time links out of logs/Git.

Repository-level operations go through `cargo xtask`; scripts and external tools are implementation primitives, not competing orchestration interfaces. Read the [command contract](xtask/README.md) before running or adding a check, and never add a parallel build/test/run entrypoint. Add meaningful crash/retry and publication-gate coverage when implementing those flows. UI changes require observations on relevant viewports/platforms, realistic Vietnamese text, focus/text scaling and accessibility; an image alone does not verify interaction or motion.

The explicit `cargo xtask verify native-isolation --test-binary PATH --test NAME` lane runs
one prebuilt exact Rust test with private run directories, an empty PATH and credential,
executable and supported Linux network negative controls. It never builds or provisions;
macOS reports network isolation and overall acceptance as `UNSUPPORTED`. Its regression tests
belong to existing repository/Rust gates. Read the [guard boundary](docs/assurance/native-isolation.md)
before using a receipt: it is not two-binary native E2E, authentication or Kani evidence.
`--identity-binary PATH` binds a prebuilt `cantos-identity` child by digest and passes only that
path as `CANTOS_TEST_IDENTITY_BINARY`.

The explicit `cargo xtask verify native-auth-qualification [--kani] [--report PATH]` is the
manual #336 Linux campaign ([boundary](docs/assurance/native-isolation.md#native-auth-linux-qualification-campaign)):
builds first, then guarded exact native auth tests, the native auth models with pure-core
conformance and, with `--kani`, the Identity kernels, each layer reported separately. Gates,
preflight and ordinary CI never select it; its D1 run is the dispatch-only job of
`repository-checks.yml`. The candidate journey alone needs a prebuilt
`CANTOS_TEST_IDENTITY_BINARY` and never builds the executable.

The table maps affected scope to checks at the applicable milestone, not a sequence to run
after every edit. Preflight already includes repository/format and selected suites; do not
run their standalone equivalents again when covered by that handoff run.

| Change | Required check at the applicable milestone |
| --- | --- |
| Release artifact contract / CI promotion | `cargo xtask release self-test` for contract, DAG and mocked-adapter regressions; Python tests are in repository and CLI/deploy tests in Rust. Explicit `release oci-qualify` runs real exact images only on disposable matching Linux ([boundary](docs/assurance/native-oci-qualification.md)); ordinary gates never start it. Host activation remains separate. |
| Repository hygiene (documents, skills, scripts) | `repository` gate, included in handoff preflight; `cargo xtask verify repository` for a focused hygiene check when needed. Includes complete [canonical localization catalog](packages/localization/README.md) validation and app-owned Rust/Kotlin drift, plus the historical pre-ADR fixture. Actual Web/CMP/native formatter acceptance remains separate. |
| Completed handoff or a pull request | `cargo xtask preflight --changed-only` on the final diff, or cite a still-valid run covering it (broadens to every gate when impact is uncertain) |
| Rust code, Script IR or contracts | `cargo xtask verify rust` and `cargo xtask verify contracts` (Server and client conformance, independent goldens and the `tools/proto-codegen` drift check of every consumer's binding) |
| Identity pure security core | Existing `rust` gate includes `cantos-identity`; metadata-based preflight selects its lint/tests. Focused primitive: `cargo test --locked -p cantos-identity`. Optional `verify kani --harness identity-security-version --harness identity-completion --harness identity-mailbox-proof` needs actual D1 Ubuntu x86_64 evidence; macOS tests are not Kani evidence. |
| Forge worker/process boundary | `cargo xtask verify forge` (Rust producer plus Node wrapper tests; no provider or cloud calls) |
| Embedded Turso foundation and Identity security persistence | `cargo xtask verify turso` (Identity `persistence` clippy/tests, shared-store file-backed tests and engine capability report; no server, container or cloud service). Also lints and tests the `service` feature (the executable, real Turso files, real Argon2id, a run-owned loopback SMTP sink) with Identity's own RPC server binding, and lints ordinary `cantos-server` targets (the real Identity journey requires a prebuilt Identity executable). Focused primitives: `cargo test --locked -p cantos-identity --features persistence`, `cargo test --locked -p cantos-identity --features service`. |
| Registered Quint models/tooling | `cargo xtask verify behavior` (model-only by default; `--conformance` adds the registered pure-core observers: private audio admission and the native Identity mailbox-proof and session-admission capabilities, with a disclosed synthetic shell; explicit `--setup` for the locked development tool) |
| Script IR validator/encoder | additionally `cargo xtask verify mutation` |
| Studio or Theatre Web | `cargo xtask verify web` |
| Persistence, migrations or HTTP | Scoped real-file/context/HTTP checks through native Rust/Turso owners; preserve archived PG oracles as porting inputs and report unported coverage |
| Prepared sample catalog | Command/parser regressions stay in the existing Rust gate; explicit `cargo xtask seed theatre --sample-input JSON --sample-media-source DIR --sample-root DIR` exercises actual provided PNGs, file-backed Turso and private Studio HTTP. It is never selected by build/preflight/CI and does not establish public catalog, browser/device or audio acceptance. |
| Local PNG comparator/tooling | `cargo xtask visual-compare --self-test` (core plus optional pinned-Pillow tests); compare existing attributed pairs with `cargo xtask visual-compare` and open the images; metric PASS is not UX approval |

Use Rust 1.98.1 (`RUSTUP_TOOLCHAIN=1.98.1`, as in CI). Native checks require no PostgreSQL,
Kanidm, emulator or container runtime. `run` starts only its owned native processes with private
files and explicit migrations; `app` starts only a frontend. Report each gate as printed:
`PASS`, `FAIL`, `NOT_RUN` or `UNSUPPORTED`, keeping browser/device/deployed acceptance separate.
On affected macOS temporary paths, use `TMPDIR=/private/tmp` for Python/mutation runs.

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

The corrected 23 October owner task selects Firebase only for verification email/actions;
see the [native amendment](docs/architecture/native-turso-refactor.md#owner-amendment-firebase-verification-provider)
and [proof-bridge boundary](docs/architecture/firebase-verification.md). Local mail is explicit:
Theatre `xtask run --local-mail firebase` starts the isolated installed Auth Emulator and uses
the operation-bound sealed-proof adapter. Local defaults keep mail disabled. The separately
approved [Resend local fixture](infra/local-mail/resend-local/README.md) accepts only a fixed
nonfunctional HTTP message; it leaves Identity mail disabled and proves no mailbox ownership.
Keep shared Firebase CLI for Hosting/Distribution; no broad emulator/tooling cleanup is implied.
Keep native proof secrets out of continuation URLs. Preserve durable create/send/consume fences,
exact-request keyed replay binding and terminal expiry/uncertain outcomes; do not poll
`emailVerified`, restore provider login or infer browser/production acceptance from readiness.
Ordinary tests/builds do not start the emulator; the explicit ignored boundary test is separate.
The existing Rust gate includes xtask's pure Node.js 22+ launcher guard tests; no installed
Resend package is needed. Only explicit `cargo xtask run resend-local` starts that fixture.

For [#348](https://github.com/loveoverflowcom/cantos/issues/348), production distribution is
GHCR registry digests plus Podman Quadlet for the two native executables. This supersedes only
the native refactor's production no-container clause; native dev/E2E/Turso/Kani remain
container-independent. Target latest `develop` after PR #400. The implemented
[`release oci-verify`](docs/deployment/native-oci-release.md) is offline and plan-only;
its Python tests run in release self-test/repository and its CLI tests in the Rust gate.
Explicit `--registry-readback` adds scoped GHCR manifest/config observation; dry-run remains
offline, with no authfile access, and neither mode establishes native/image-pair acceptance.
The local/Actions producer (`release oci-build`, unadmitted candidate only) retains a sanitized
publish-attempt journal before remote writes; failed/unknown publication requires reconciliation.
`release oci-verify --host-inventory PATH --host-inventory-digest SHA256` adds an offline declared
readiness plan, never activation authority. Actual
publish/admission and host activation require separate evidence. `deploy vps` is a retired
refusal. The explicit `release native-upgrade` lane is host-local rootful Linux, existing
NATIVE_READY and same-schema only, with operator-admitted exact images, cold backups, a durable
recovery marker and image-only rollback; first-native/legacy cutover and live qualification
remain separate. Its file/FakeHost regressions belong to release self-test/repository; they
never establish live VPS or exact-image qualification. Preserve legacy runtime/principals, PG/Kanidm/ingress and recovery until real native
acceptance and separately authorized cutover; missing inventory is `UNKNOWN`, never permission
to bootstrap. Build/publish, inventory, activation, restore and cleanup are separate scopes.

The separate `release native-adopt` lane supports only the reviewed existing
[`/root/cantos-native` v23-to-v24 profile](docs/deployment/native-existing-host-migration.md).
It preserves the declared paths/proxy policy, requires exact historical two-image rehearsal and
separate operator adoption approval, retains complete cold native pairs and refuses restore
after a start intent. `release native-rehearse` is the disposable Linux synthetic upgrade/restore
primitive in the manual OCI workflow; its report is independent of current-schema qualification,
producer admission, generation state and live readiness. No production operation is implicit.

A service is a runnable process or independently managed infrastructure, not necessarily a Rust
crate. Keep deployment/runtime assets under [infra/](infra/README.md), separate from application
code and `xtask` orchestration; the [layout](docs/architecture/repository-layout.md) distinguishes
current paths from future consumers. Follow [the service inventory](services/README.md) and
[ADR 0021](docs/decisions/0021-podman-service-runtime.md). Podman is the standard OCI runtime;
Docker Engine/Desktop/daemon and Compose are not dependencies (`docker.io` remains an allowed
registry). Native `cargo xtask run` owns Server + Identity; `services list|validate` starts nothing.
Legacy container lifecycle commands refuse. Production Podman/Quadlet is separately scoped by
#348; never install/start a machine, alter connections or delete resources during source cleanup.
