# Contributing to Cantos

Cantos combines an audio-drama production Studio with a Theatre listening experience. Read the [project overview](README.md) before proposing a change. The repository starts with planning documents and templates; implementation status and supported commands must be documented as the runtime is added.

## Branch and pull request workflow

`develop` is the default and integration branch. Create focused branches from its latest revision and open pull requests back to `develop`. A `main` branch is not required for this workflow.

```bash
git fetch origin
git switch develop
git pull --ff-only origin develop
git switch -c feat/scene-casting
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

Keep the initial architecture modular without introducing microservices prematurely. Put code where the [repository layout](docs/architecture/repository-layout.md) says: listener apps in `apps/cantos-theatre`, creator apps in `apps/cantos-studio`, the backend in `services/cantos-api`, and shared contracts, semantic tokens or renderer primitives with concrete consumers in `packages/`. Separate domain rules, provider adapters, production orchestration, storage, APIs, and app presentation through explicit interfaces.

Treat the versioned Script IR as the boundary for exchanging narrative data with Narrative Forge. Reuse concepts and documented contracts rather than importing another project's internal modules. Record schema compatibility, migration behavior, and stable identities for works, episodes, scenes, and dialogue segments before changing persisted data.

Rust backend and Kotlin mobile code may share generated API contracts where practical. Document the source of truth and generation process when selecting that tooling; do not assume Rust types can be consumed directly by Kotlin. Keep platform audio playback native on Android and iOS.

## Production changes

Make pipeline work resumable and verifiable. State which input revision a job processes, how retries avoid duplicate generation or publication, and which dialogue or scene assets a change invalidates. Report provider cost impact and preserve the ability to preview corrections before publication.

Never include private manuscripts, credentials, signed storage URLs, or unauthorized voice/music assets in commits, logs, issue attachments, or fixtures. Use original or permitted examples. Publication must wait for the applicable rights, QC, and approval checks documented for that production run.

## UI changes

Follow the project design direction: Material 3 Expressive, soft lavender/purple colors, compact adaptive layouts, expressive rounded controls, accessible typography, and restrained borders and shadows.

Attach the relevant reference and before/after evidence to design work. Cover keyboard and touch input, contrast, focus behavior, reduced motion, and applicable narrow/wide layouts. Document which tokens and components should be shared between Leptos and CMP and which interactions depend on native platform behavior.

## Verification

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
cargo xtask preflight --changed-only
```

Preflight compares your branch, staged, unstaged and untracked files with `origin/develop`, runs
the gates they affect and broadens to every gate when the impact is uncertain. Use Rust 1.87 and
see [the command contract](xtask/README.md) for the individual `cargo xtask verify` suites
(`repository`, `rust`, `contracts`, `web`, `postgres`, `mutation`) and `cargo xtask ci`, which
runs the same jobs as GitHub Actions. If a gate cannot run on your machine, it is reported as
`UNSUPPORTED` and preflight fails; `--skip <gate>` records it as `NOT_RUN`. State every skipped or
unsupported gate in the pull request. On affected macOS temporary-path setups, run with
`TMPDIR=/private/tmp`.

For runtime changes, run the relevant stack-specific checks once the corresponding implementation and tooling exist. Include meaningful integration evidence for job retry/resume, partial regeneration, publishing, streaming, and native background playback when those behaviors change. Record any environment limitations instead of claiming an unrun check passed.
