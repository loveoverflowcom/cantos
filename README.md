# Cantos

**Vọng Đài — Theatre of Voices.** An AI-assisted audio drama platform with a creator Studio and a podcast-like Theatre for listeners.

Cantos produces performances with a cast, narration, atmosphere, music and sound effects. Published audio is rendered once and streamed from stored assets; listening does not call a TTS provider.

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
reference-only voice controls, script-scoped rights/budget declarations, immutable candidates
and exact owner approval on the same backend. Reference capabilities and rates are planning
fixtures; that #5 slice produces no TTS/LLM call, reservation, actual charge or audio. The bounded
[durable local/fake worker foundation](docs/evidence/durable-tts-jobs.md) now adds PostgreSQL
runs, jobs, dialogue steps, intent/reservation/observation history, leases and fenced result
selection. Its separate `cantos-worker` process belongs to the same backend package and requires
explicit development/local or synthetic execution. Google commercial dispatch stays disabled;
VieNeu stays research-only. Full production execution, Theatre and native mobile playback remain
planned. No runnable Gradle project or production
identity deployment is claimed. A standalone, read-only [WAV PCM inspection CLI](docs/evidence/wav-pcm-inspection.md)
reports structure, checksums and sample peak as JSON; it is groundwork for audio QC, not mixing,
loudness measurement or QC approval. A read-only [Script IR diff CLI](docs/evidence/script-diff.md)
compares two versions of one episode by stable ID and reports edited, moved, added and removed
lines, cues and characters plus provenance and rights changes; it is a review aid, not a reuse decision.
A [local TTS probe and receipt inspection CLI](docs/evidence/tts-receipt-inspection.md) now
exercise the user-authorized eSpeak NG 1.52.0 path. Real samples from all three supported
Vietnamese voices were generated and validated as native 22,050 Hz mono 16-bit PCM WAV.
The probe saves private frozen-input, intent, receipt and output-hash records; the inspector
checks exact scope and conservative recovery facts. Listening quality is **NOT_RUN**.
This is a sample path, with explicit neutral/default controls and no production dispatch
authorization, durable jobs or asset acceptance. See
[local selection](docs/decisions/0008-local-tts-inspection.md) and
[provider prerequisites](docs/evidence/tts-provider-prerequisites.md).
The user-requested [local neural research slice](docs/evidence/local-neural-tts.md) adds a
bounded VieNeu Nano research adapter while preserving eSpeak. Its pinned model/preset selection
and [rights comparison](docs/evidence/local-neural-tts-research.md) keep production dispatch
disabled. The pinned ARM64 CPU runtime generated six inspected 24,000 Hz mono 16-bit PCM WAVs
from the Đức Trí and Mỹ Duyên presets; required native, PostgreSQL and browser checks passed.
This remains part of #6, with commercial execution and listening acceptance unresolved.

The owner-selected commercial provider, Google Cloud TTS, follows the same pattern: its vi-VN
voices are castable with an explicit neutral delivery, and an opt-in `probe-google-tts` sends
one capped request for one frozen line. Its evidence is mocked only; the live Google sample is
**NOT_RUN** ([Google selection](docs/decisions/0011-google-cloud-tts-adapter.md),
[adapter evidence](docs/evidence/google-tts-adapter.md)). This completes only part of #6 and
does not close the issue.
The [existing provider verification](docs/evidence/tts-provider-verification.md) hardens
conservative HTTP execution, pinned sample rights/budget checks and private local output safety.
Fresh eSpeak samples remain structurally verified; Google live and listening remain NOT_RUN.
The sequential #7 [worker foundation](docs/evidence/durable-tts-jobs.md) starts from the merged
[PR #38](https://github.com/loveoverflowcom/cantos/pull/38), preserving those #6 gates. Completed
worker execution grants neither editorial/publication approval nor accepted delivery assets.

Four Studio revision routes (head, pinned revision, save, validation) can also negotiate
MessagePack beside JSON with the same DTOs and error semantics; the Studio ships JSON until the
measured benefit justifies a switch ([wire codec](docs/architecture/wire-codec.md),
[evidence](docs/evidence/messagepack-wire.md)).

The repository is organised by product and kind: `apps/cantos` (Theatre, for listeners) and
`apps/cantos-studio` (Studio, for creators) each have `web` and `mobile`; one backend lives in
`services/cantos-api` beside Kanidm deployment notes; shared contracts and design tokens live in
`packages/`. Only Studio Web and the API are implemented; the Theatre Web crate is a skeleton and
both mobile directories are guides. See the [repository layout](docs/architecture/repository-layout.md).

`develop` is the first and default branch. Create short-lived feature branches from it and target pull requests to `develop`.

## Product and stack

| Surface | Purpose | Technology |
| --- | --- | --- |
| Cantos Studio | Import, adapt, edit, cast, preview, produce and approve episodes | Leptos Web (`apps/cantos-studio/web`); a mobile client is reserved |
| Cantos Theatre | Discover and listen to published audio drama | Leptos Web (skeleton); Kotlin Compose Multiplatform on Android/iOS (planned) |
| Backend | Domain rules, APIs, production orchestration and publication | Rust, Axum, PostgreSQL (`services/cantos-api`) |
| Identity | Sign-in and single sign-on | Proposed: Kanidm, pending its own ADR; example config only (`services/kanidm`) |
| Media delivery | Private production assets and approved episode renditions | S3-compatible object storage and CDN |

Start with one modular backend. Worker execution can run separately for resource isolation while sharing the same codebase and PostgreSQL job model.

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
| `apps/cantos-studio/mobile/` | Reserved location for a Studio CMP client; guide only |
| `apps/cantos/web/` | Cantos Theatre Leptos skeleton: builds, shows that nothing exists yet |
| `apps/cantos/mobile/` | Guide for the native CMP listening app; no Gradle project |
| `services/cantos-api/` | Pure Script IR/revision/production/job rules, local Axum host, PostgreSQL store, migration CLI and same-package local worker |
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
cargo xtask verify contracts               # Script IR goldens, wire/WAV/TTS receipt contracts, WAV oracle
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
or `UNSUPPORTED`; only explicit `--skip` leaves a gate out, and it is shown as `NOT_RUN`. Mobile
surfaces are `UNSUPPORTED` until a Gradle project exists. [Contract evidence](docs/evidence/script-ir-contract.md)
names the actual verification boundary. On macOS where temporary symlink tests encounter `/var`
versus `/private/var`, use `TMPDIR=/private/tmp` for the Python unit/mutation runs.

The CLIs remain directly usable, for example:

```sh
cargo run --locked --bin validate-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json
cargo run --locked --bin diff-script -- packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json packages/contracts/fixtures/script-ir/0.1.0/accept/provenance.json
```

[API service](services/cantos-api/README.md), [Studio](apps/cantos-studio/web/README.md) and [Theatre skeleton](apps/cantos/web/README.md) describe migration, test-role
permissions and local serving. [.env.example](.env.example) distinguishes implemented development configuration
from future provider/storage configuration; no dotenv loader or audio QA runtime is provided.

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

The MVP must demonstrate an authorized script imported into Studio, reviewed adaptation and casting, resumable production, partial regeneration, mixed audio passing QC, approved publication, Theatre Web playback and native Android/iOS listening with background controls, offline playback and synchronized progress. See the [work plan](docs/work-plan/README.md) for individual acceptance gates.

The referenced UI image was not attached to this request. Exact visual matching remains pending receipt of that reference. The lavender/purple, compact Material 3 Expressive direction is recorded in the UI specification.

## Licensing and content

A software license has not been selected. Do not infer redistribution permission from repository visibility. Source works, adaptations, voices, music and sound effects have separate rights records; production or publication approval must be tied to their exact revisions.
