# Contributing to Cantos

Cantos combines an audio-drama production Studio with a Theatre listening experience. Read the [project overview](README.md) before proposing a change. The native Server/Identity foundation exists; the [native business port](docs/architecture/server-turso-porting.md#checkpoint-boundary) is implemented, with production/data-conversion acceptance separate. Documentation, implementation and executed acceptance are separate.

## Branch and pull request workflow

Resolve `BASE_BRANCH` once: use the user's explicit base; otherwise the work item's accepted
integration/staging base; otherwise `develop`. `BASE_REF=origin/$BASE_BRANCH` is the common
remote ref for branching, diff/review, preflight and clean-baseline reproduction; PR creation
uses the matching `BASE_BRANCH`. Record its fetched SHA. Never move another worktree or discard
unrelated edits; use an isolated checkout when necessary. `develop` remains the default integration branch.

```bash
# Example when the user/work item has not selected another base:
BASE_BRANCH=develop
BASE_REF="origin/$BASE_BRANCH"
git fetch origin "$BASE_BRANCH"
git switch -c feat/scene-casting "$BASE_REF"
git diff "$BASE_REF"...HEAD
gh pr create --base "$BASE_BRANCH" --head feat/scene-casting --body-file <scratch-file>
```

Use a short, descriptive branch name such as `feat/scene-casting`, `fix/player-progress`, or `docs/production-workflow`. Keep unrelated changes in separate pull requests. For a large feature, define its dependencies and divide it into changes that each have an observable outcome.

Use the pull request template to explain the resulting behavior, verification, compatibility, and production impact. Do not treat a passed repository check as evidence that an unimplemented application builds or runs.

## Plan and track work

Use the GitHub issue forms for implementation tasks, bugs, design work, and production quality control. Use the [document templates](templates/README.md) when a decision or production run needs a durable record. Link related issues and documents so prerequisites and unresolved questions remain visible.

An implementation task should specify:

- The creator or listener outcome and a bounded scope.
- Observable acceptance criteria, including relevant error and recovery behavior.
- Dependencies, contract changes, and compatibility expectations.
- The evidence needed to verify the change.

Do not invent labels, reviewer identities, provider accounts, or approval policies. Establish these explicitly when the project needs them.

## Architecture and contracts

Keep the initial architecture modular without introducing microservices prematurely. Put code where the [repository layout](docs/architecture/repository-layout.md) says: listener apps in `apps/cantos-theatre`, creator apps in `apps/cantos-studio`, the backend in `services/cantos-server`, canonical contracts in the service that provides them (`services/<service>/contracts/`), and semantic tokens or renderer primitives with concrete consumers in `packages/`. Separate domain rules, provider adapters, production orchestration, storage, APIs, and app presentation through explicit interfaces.

Treat the versioned Script IR as the boundary for exchanging narrative data with Narrative Forge. Reuse concepts and documented contracts rather than importing another project's internal modules. Record schema compatibility, migration behavior, and stable identities for works, episodes, scenes, and dialogue segments before changing persisted data.

Every consumer owns its own representation of a service-owned contract: Rust and Kotlin code read the same canonical schema or `.proto` as a build input and generate or hand-write their own types, checked against the owner's fixtures ([ADR 0046](docs/decisions/0046-service-owned-contracts.md)). Name the source of truth and generation command; do not import another consumer's or the service's types. Keep platform audio playback native on Android and iOS.

## Production changes

Make pipeline work resumable and verifiable. State which input revision a job processes, how retries avoid duplicate generation or publication, and which dialogue or scene assets a change invalidates. Report provider cost impact and preserve the ability to preview corrections before publication.

Never include private manuscripts, credentials, signed storage URLs, or unauthorized voice/music assets in commits, logs, issue attachments, or fixtures. Use original or permitted examples. Publication must wait for the applicable rights, QC, and approval checks documented for that production run.

## UI changes

Follow the project design direction: Material 3 Expressive, soft lavender/purple colors, compact adaptive layouts, expressive rounded controls, accessible typography, and restrained borders and shadows.

Attach the relevant reference and before/after evidence to design work. Cover keyboard and touch input, contrast, focus behavior, reduced motion, and applicable narrow/wide layouts. Document which tokens and components should be shared between Leptos and CMP and which interactions depend on native platform behavior.

## Verification

Follow the foundation's [verification cadence](.agents/skills/cantos-engineering/references/verification-cadence.md):
focused checks while developing, affected module/renderer checks at coherent checkpoints and
preflight on the final diff at handoff. Keep targeted evidence for high-risk invariants.
Intermediate edits/commits do not each trigger preflight; reuse attributable results while
their inputs remain valid. The handoff plan already includes repository/format and selected
suites, so do not run their standalone equivalents again merely to satisfy another checklist.

Implementation handoff and PR creation use the foundation's [local handoff policy](.agents/skills/cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion):
verify the pure core first, record independent layer results and deliver verified layers when
requested even if infrastructure acceptance is pending. Record deferred checks with their
reason, next command, criteria and owner; keep mocked and real evidence separate. Do not wait
or poll for GitHub Actions. Required checks, repository merge protection and production,
publication and phase-exit acceptance keep their existing meaning.

Repository-level build, run and verification commands go through [`cargo xtask`](xtask/README.md).
Scripts in `scripts/` and raw Cargo/Trunk commands are its primitives; do not add a parallel
entrypoint. From any directory in the repository:

```bash
cargo xtask doctor
cargo xtask preflight --changed-only --base "$BASE_REF"
```

Preflight compares your branch, staged, unstaged and untracked files with the resolved `BASE_REF`, runs
the gates they affect and broadens to every gate when the impact is uncertain. Use Rust 1.98.1 and
see [the command contract](xtask/README.md) for the individual `cargo xtask verify` suites
(`repository`, `rust`, `contracts`, `web`, `turso`, `mutation`) and `cargo xtask ci`, which
runs the same jobs as GitHub Actions. If a gate cannot run on your machine, it is reported as
`UNSUPPORTED` and preflight fails; `--skip <gate>` records it as `NOT_RUN`. State every skipped or
unsupported gate in the pull request. On affected macOS temporary-path setups, run with
`TMPDIR=/private/tmp`.

For runtime changes, run the relevant stack-specific checks once the corresponding implementation and tooling exist. Include meaningful integration evidence for job retry/resume, partial regeneration, publishing, streaming, and native background playback when those behaviors change. Record any environment limitations instead of claiming an unrun check passed.

## Service runtime ownership

Follow the [service inventory](services/README.md) and [native decision](docs/architecture/native-turso-refactor.md).
Server and Identity each own a local embedded Turso file and private migration lifecycle. Native
build/test/run never starts PostgreSQL, Kanidm, Podman or an emulator. `cargo xtask run` supervises
the two native processes; `services list|validate` checks inventory, while legacy lifecycle commands refuse.
Production OCI/Quadlet packaging follows [#348](docs/deployment/native-oci-release.md), separate
from native acceptance and host activation. Preserve DB/WAL/media, backups and other tasks' artifacts.
No deploy, migration of user data or installed-service cleanup follows from a source change.
