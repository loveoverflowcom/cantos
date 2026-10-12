# Cantos

**Vọng Đài — Theatre of Voices.** An AI-assisted audio drama platform with a creator Studio and a podcast-like Theatre for listeners.

Cantos Studio manages novels, stories and podcasts, their episodes and their audio. It does not
synthesize speech: it supplies the script, casting guidance, standards and an API that external AI
producers follow and send finished audio back through ([ADR 0018](docs/decisions/0018-external-audio-production.md)).
Published audio is stored once and streamed from stored assets; listening does not call a TTS provider.

## Repository status

This repository contains product specifications, architecture decisions, directory guides and
project-management templates, plus an executable Rust [Script IR contract](packages/contracts/README.md)
and validation CLI. A local development Axum/PostgreSQL revision store and minimal live Leptos
Studio consumer now save and read complete immutable exports with actor permissions,
optimistic concurrency and idempotent retries. See the [persistence evidence](docs/evidence/script-revision-persistence.md).
The compatible [editorial handoff](docs/evidence/editorial-handoff.md) adds private preserved
sources, bounded history, owner review records and operator credential tools on that backend.
Bounded manuscript import preserves TXT, Markdown, DOCX and Script IR originals with a minimal
Studio comparison flow; see the [import evidence](docs/evidence/manuscript-import.md).
The bounded adaptation implementation exports immutable source/revision context through
[authenticated HTTP/CLI tools](packages/contracts/adaptation-v1.md), validates host-created proposals,
and supports explicit Studio review/acceptance. Gemini / ChatGPT / Codex generation belongs to
the user-controlled host; Cantos performs no model inference for adaptation. [Adaptation evidence](docs/evidence/ai-script-adaptation.md)
separates actual tool/storage tests from synthetic content and unrun model quality. The [structured Studio editor](docs/evidence/studio-script-editor.md) adds scene/dialogue/character authoring,
validation previews, source comparison and revision history on that same authority.
[Casting and production inputs](docs/evidence/casting-production-inputs.md) add versioned
voice guidance, script-scoped rights/budget declarations, immutable candidates and exact owner
approval on the same backend. An approved snapshot is guidance that an external producer reads;
Cantos makes no TTS or LLM call, reservation, charge or audio.
The in-process text-to-speech runtime (durable jobs, the `cantos-worker` process, eSpeak NG, VieNeu
Nano and Google Cloud TTS adapters, receipts, clip reuse and probe CLIs) was removed by
[ADR 0018](docs/decisions/0018-external-audio-production.md). Its evidence stays under `docs/evidence/` as
history with links pinned to the last commit that contained it. The API for producers to submit
audio, with scoped credentials and storage, is proposed and **not implemented**, so no audio can
enter Cantos yet. Mixing, QC, object storage, Theatre and native mobile playback remain planned.
No runnable Gradle project or production identity deployment is claimed. A standalone, read-only
[WAV PCM inspection CLI](docs/evidence/wav-pcm-inspection.md) reports structure, checksums and
sample peak as JSON; it is the retained admission primitive for submitted audio, not mixing,
loudness measurement or QC approval. A read-only [Script IR diff CLI](docs/evidence/script-diff.md)
compares two versions of one episode by stable ID and reports edited, moved, added and removed
lines, cues and characters plus provenance and rights changes; it is a review aid, not a reuse decision.

Four Studio revision routes (head, pinned revision, save, validation) can also negotiate
MessagePack beside JSON with the same DTOs and error semantics; the Studio ships JSON until the
measured benefit justifies a switch ([wire codec](docs/architecture/wire-codec.md),
[evidence](docs/evidence/messagepack-wire.md)).

The repository is organised by product and kind: `apps/cantos-theatre` (Theatre, for listeners)
targets Web and native Mobile; `apps/cantos-studio` (Studio, for creators) targets Web only. One
backend lives in `services/cantos-api` beside Kanidm deployment notes; shared contracts and design
tokens live in `packages/`. Studio Web and the API are implemented; Theatre Web is a skeleton and
Theatre Mobile remains a guide without a Gradle app. Supported product scope does not imply an
implemented client. See the [repository layout](docs/architecture/repository-layout.md#platform-scope).

`develop` is the first and default branch. Create short-lived feature branches from it and target pull requests to `develop`.

## Product and stack

| Surface | Purpose | Technology |
| --- | --- | --- |
| Cantos Studio | Import, adapt, edit, cast, review and approve episodes; manage works and the audio producers submit | Leptos Web only (`apps/cantos-studio/web`) |
| Cantos Theatre | Discover and listen to published audio drama | Leptos Web (skeleton); Kotlin Compose Multiplatform on Android/iOS (planned) |
| Backend | Domain rules, APIs, production guidance, audio admission and publication | Rust, Axum, PostgreSQL (`services/cantos-api`) |
| Identity | Sign-in and single sign-on | Proposed: Kanidm, pending its own ADR; example config only (`services/kanidm`) |
| Media delivery | Private production assets and approved episode renditions | S3-compatible object storage and CDN |

Start with one modular backend. Audio is produced outside Cantos by AI plugins that read its guidance and submit results through the API.

## Platform scope

| Path | Platform support | Implementation |
| --- | --- | --- |
| `apps/cantos-studio/web` | Supported | Implemented |
| `apps/cantos-studio/mobile` | Out of scope | Not implemented; scope notice only |
| `apps/cantos-theatre/web` | Supported | Skeleton |
| `apps/cantos-theatre/mobile` | Supported | Planned; no Gradle app yet |

Cantos Studio is web-only. Do not implement, scaffold, build or maintain a mobile client unless explicitly requested in a future roadmap.
Shared libraries may serve Theatre Mobile and Studio Web.

## Start here

- [Project brief](docs/product/brief.md) and [business rules](docs/product/business-rules.md).
- [Architecture](docs/architecture/overview.md), [repository layout](docs/architecture/repository-layout.md), [Script IR](docs/architecture/script-ir.md) and [production pipeline](docs/product/production-pipeline.md).
- [UI system](docs/design/ui-system.md) and [native mobile requirements](docs/architecture/mobile.md).
- [Ordered MVP work plan](docs/work-plan/README.md): the next implementation step and its review boundary.
- [GitHub planning guide](docs/work-plan/project-planning.md), [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) and [Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2): 17 implementation issues across five milestones.
- [Contributor workflow](CONTRIBUTING.md), [agent instructions](AGENTS.md), [agent skills](.agents/skills/README.md) and [reusable templates](templates/README.md).

## Directory map

| Path | Responsibility at bootstrap |
| --- | --- |
| `apps/cantos-studio/web/` | Structured Leptos Studio CSR editor and casting/input review |
| `apps/cantos-studio/mobile/` | Out of scope / not implemented; scope notice only |
| `apps/cantos-theatre/web/` | Cantos Theatre Leptos skeleton: builds, shows that nothing exists yet |
| `apps/cantos-theatre/mobile/` | Guide for the native CMP listening app; no Gradle project |
| `services/cantos-api/` | Pure Script IR/revision/production rules, media inspection, local Axum host, PostgreSQL store and migration CLI |
| `services/kanidm/` | Identity-provider deployment notes and an example config; not integrated |
| `packages/contracts/` | Script IR and Studio v1 schemas/fixtures, contract documents and shared Rust wire DTOs (`cantos-contracts`) |
| `packages/ui-system/` | Canonical semantic tokens and Leptos foundation; CMP mapping deferred |
| `docs/` | Product, architecture, design, decisions and the work queue |
| `templates/` | Reusable feature, decision, production and publication records |
| `.agents/skills/` | Canonical agent skills shared by Codex, Claude Code and other agents |
| `.claude/skills/` | Per-skill symlinks so Claude Code discovers `.agents/skills/` |
| `.github/` | Issue forms, PR template and the CI workflow that runs `cargo xtask ci` jobs |
| `xtask/`, `.cargo/` | The `cargo xtask` command surface and its Cargo alias |
| `scripts/` | Primitives invoked by xtask: repository checks, digest and WAV inspection oracles, mutation checks, isolated PostgreSQL runner; plus the story initializer, benchmark and token generator |

## Build, run and verify

Repository-level operations go through [`cargo xtask`](xtask/README.md); the scripts in
`scripts/` and the Cargo/Trunk commands they replace are its implementation primitives. It works
from any directory and checks every prerequisite before it starts.

```sh
cargo xtask doctor                         # tools, gates and surfaces available here
cargo xtask compose-preview --help         # optional Android Studio PNG/semantics capture
cargo xtask preflight --changed-only       # gates selected from your diff against origin/develop
cargo xtask verify repository              # documents, links, skills, Python tests, whitespace
cargo xtask verify rust                    # fmt, clippy -D warnings, workspace tests
cargo xtask verify contracts               # Script IR goldens, wire/WAV contracts, WAV oracle
cargo xtask verify web                     # WASM clippy, both Web clients and the shared component gallery
cargo xtask verify postgres                # disposable PostgreSQL cluster (never an inherited DATABASE_URL)
cargo xtask verify mutation                # bounded Script IR mutation checks
cargo xtask build all                      # cantos-api plus Studio and Theatre Web
cargo xtask run cantos-studio --web --test-cluster   # after `verify postgres --keep`
cargo xtask ci                             # the four CI jobs, as GitHub Actions runs them
```

Use Rust 1.87 (CI sets `RUSTUP_TOOLCHAIN=1.87.0`), the `wasm32-unknown-unknown` target, Trunk
0.21.14 and, for the database gate, PostgreSQL 17 binaries on `PATH` (on a Mac, for example
`PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH"`). Summaries report `PASS`, `FAIL`, `NOT_RUN`
or `UNSUPPORTED`; only explicit `--skip` leaves a gate out, and it is shown as `NOT_RUN`. Theatre Mobile
commands are `UNSUPPORTED` until a Gradle project exists; Studio Mobile is out of scope and rejected. [Contract evidence](docs/evidence/script-ir-contract.md)
names the actual verification boundary. On macOS where temporary symlink tests encounter `/var`
versus `/private/var`, use `TMPDIR=/private/tmp` for the Python unit/mutation runs.

The CLIs remain directly usable, for example:

```sh
cargo run --locked --bin validate-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json
cargo run --locked --bin diff-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json packages/contracts/fixtures/script-ir/0.1.0/accept/provenance.json
```

[API service](services/cantos-api/README.md), [Studio](apps/cantos-studio/web/README.md) and [Theatre skeleton](apps/cantos-theatre/web/README.md) describe migration, test-role
permissions and local serving. [.env.example](.env.example) distinguishes implemented development configuration
from future storage configuration for submitted audio; no dotenv loader or audio QA runtime is provided.

Measure the implemented revision reads against synthetic data in a fresh disposable cluster:

```sh
PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" python3 scripts/benchmark_revision_reads.py --scripts 100
PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" python3 scripts/benchmark_revision_reads.py --scripts 5000
```

The [benchmark protocol](docs/evidence/script-revision-benchmark-protocol.md) records the
chosen local budgets, snapshot isolation between comparisons and shared-buffer cold-start
limitations. The harness stops its HTTP/PostgreSQL processes and retains raw synthetic
samples/plans locally under ignored `target/read-benchmarks/`. It never accepts an existing DB URL.

## MVP completion

The MVP must demonstrate an authorized script imported into Studio, reviewed adaptation and casting, audio produced by an external AI producer and submitted through the API, mixed audio passing QC, approved publication, Theatre Web playback and native Android/iOS listening with background controls, offline playback and synchronized progress. See the [work plan](docs/work-plan/README.md) for individual acceptance gates.

The referenced UI image was not attached to this request. Exact visual matching remains pending receipt of that reference. The lavender/purple, compact Material 3 Expressive direction is recorded in the UI specification.

## Licensing and content

A software license has not been selected. Do not infer redistribution permission from repository visibility. Source works, adaptations, voices, music and sound effects have separate rights records; production or publication approval must be tied to their exact revisions.
