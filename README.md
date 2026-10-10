# Cantos

**Vọng Đài — Theatre of Voices.** An AI-assisted audio drama platform with a creator Studio and a podcast-like Theatre for listeners.

Cantos produces performances with a cast, narration, atmosphere, music and sound effects. Published audio is rendered once and streamed from stored assets; listening does not call a TTS provider.

## Repository status

This repository contains product specifications, architecture decisions, directory guides and
project-management templates, plus an executable Rust [Script IR contract](contracts/README.md)
with a validation CLI and a runnable Axum backend with PostgreSQL-backed, authenticated script
revisions. See the [server guide](apps/server/README.md) and the evidence for the
[contract](docs/evidence/script-ir-contract.md) and [revisions](docs/evidence/script-revisions.md).
Web apps, production workers and the mobile player are **not implemented yet**. No runnable Gradle
project is claimed. This slice contributes to issue #1; it does not complete the MVP.

`develop` is the first and default branch. Create short-lived feature branches from it and target pull requests to `develop`.

## Product and stack

| Surface | Purpose | Technology |
| --- | --- | --- |
| Cantos Studio | Import, adapt, edit, cast, preview, produce and approve episodes | Leptos Web |
| Cantos Theatre | Discover and listen to published audio drama | Leptos Web; Kotlin Compose Multiplatform on Android/iOS |
| Backend | Domain rules, APIs, production orchestration and publication | Rust, Axum, PostgreSQL |
| Media delivery | Private production assets and approved episode renditions | S3-compatible object storage and CDN |

Start with one modular backend. Worker execution can run separately for resource isolation while sharing the same codebase and PostgreSQL job model.

## Start here

- [Project brief](docs/product/brief.md) and [business rules](docs/product/business-rules.md).
- [Architecture](docs/architecture/overview.md), [Script IR](docs/architecture/script-ir.md) and [production pipeline](docs/product/production-pipeline.md).
- [UI system](docs/design/ui-system.md) and [native mobile requirements](docs/architecture/mobile.md).
- [Ordered MVP work plan](docs/work-plan/README.md): the next implementation step and its review boundary.
- [GitHub planning guide](docs/work-plan/project-planning.md), [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) and [Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2): 17 implementation issues across five milestones.
- [Contributor workflow](CONTRIBUTING.md), [agent instructions](AGENTS.md), [agent skills](.agents/skills/README.md) and [reusable templates](templates/README.md).

## Directory map

| Path | Responsibility at bootstrap |
| --- | --- |
| `apps/server/` | Axum host, Script IR reader/validation CLI, editorial core, PostgreSQL migrations and tests |
| `apps/web/` | Guide for Studio and Theatre Web |
| `apps/mobile/` | Guide for the native CMP listening app |
| `contracts/` | Versioned Script IR and HTTP schemas, compatibility/validation fixtures and canonical policy |
| `docs/` | Product, architecture, design, decisions and the work queue |
| `templates/` | Reusable feature, decision, production and publication records |
| `.agents/skills/` | Canonical agent skills shared by Codex, Claude Code and other agents |
| `.claude/skills/` | Per-skill symlinks so Claude Code discovers `.agents/skills/` |
| `.github/` | Issue forms, PR template and repository checks |
| `scripts/` | Repository checks, story initializer, independent digest oracle and bounded mutation runner |

## Validate the repository

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
# TEST_DATABASE_URL must point to a disposable PostgreSQL instance; see the server guide.
cargo test -p cantos-server --test persistence --locked -- --ignored --test-threads=1
python3 scripts/check_repository.py
python3 -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

Verify with the Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/reference_script_ir.py
cargo run --locked --bin validate-script -- contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json
```

The PostgreSQL integration suite additionally needs a disposable database; the
[server guide](apps/server/README.md) documents provisioning, migrations, token management and
startup. [Contract evidence](docs/evidence/script-ir-contract.md) and
[revision evidence](docs/evidence/script-revisions.md) name the actual verification boundaries.
On macOS where temporary symlink tests encounter `/var` versus `/private/var`, use
`TMPDIR=/private/tmp` for the Python unit/mutation runs. [.env.example](.env.example)
distinguishes consumed server configuration from proposed future service names. No audio or
mobile test is claimed.

## MVP completion

The MVP must demonstrate an authorized script imported into Studio, reviewed adaptation and casting, resumable production, partial regeneration, mixed audio passing QC, approved publication, Theatre Web playback and native Android/iOS listening with background controls, offline playback and synchronized progress. See the [work plan](docs/work-plan/README.md) for individual acceptance gates.

The referenced UI image was not attached to this request. Exact visual matching remains pending receipt of that reference. The lavender/purple, compact Material 3 Expressive direction is recorded in the UI specification.

## Licensing and content

A software license has not been selected. Do not infer redistribution permission from repository visibility. Source works, adaptations, voices, music and sound effects have separate rights records; production or publication approval must be tied to their exact revisions.
