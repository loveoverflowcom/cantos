# Agent instructions — Cantos

## Scope and source of truth

Read [README.md](README.md), the relevant product/architecture docs and [docs/work-plan/README.md](docs/work-plan/README.md) before implementing a feature. The script revision backend and contracts are implemented; Web, mobile and production execution remain planned. Inspect actual code before treating a proposed module, schema, command or API as implemented.

The current user's instructions take priority. Preserve Cantos Studio and Cantos Theatre terminology, Rust/Axum/PostgreSQL/Leptos/CMP choices and native mobile playback. Keep docs in English unless the task requests otherwise; retain correct Vietnamese names and representative content.

## Architecture boundaries

- Begin with a modular monolith; add a service, crate or generic abstraction only for a concrete consumer and reviewed need.
- Domain rules and versioned Script IR must not depend on HTTP, UI frameworks, a specific AI provider or Narrative Forge internals.
- Use PostgreSQL for metadata, revision history and durable job state. Store media in object storage.
- Do not invoke AI during listener playback. Preserve immutable publication manifests and cached renditions.
- Keep private drafts, voice assets and previews inaccessible to public delivery. Enforce permissions in the backend.
- Mobile uses CMP with platform-native audio adapters; no WebView audio player.

## Correctness rules

For production changes, reason about duplicate delivery, worker crashes, cancellation, provider timeouts, expired job leases, stale revisions, cost reservations and approval invalidation. Never claim exactly-once external TTS generation unless a provider contract proves it; record ambiguous attempts before retrying.

Cache keys must cover all effective generation inputs. A dialogue edit invalidates its speech plus dependent mix/QC/publication artifacts; unchanged dialogue can remain reusable. Any generated or imported content requires recorded provenance and applicable rights.

## Working and review

Branch from `develop` and target PRs to `develop`. Keep changes cohesive and avoid unrelated cleanup. Update the work queue after implementation; GitHub issues/PRs own execution status when linked, and the queue owns recommended order and rationale.

Use [templates](templates/README.md) for a concrete proposal or production record when relevant. Clearly distinguish proposed design, implemented behavior and verified behavior. Never commit secrets, real provider credentials, private manuscripts, downloaded media or generated audio to Git.

## Agent skills

Canonical skills live in [`.agents/skills/`](.agents/skills/README.md) and are shared by Codex, Claude Code and any other agent; `.claude/skills/<name>` entries are symlinks to them. Read the skill index, then load the skill that owns the task:

- [`cantos-engineering`](.agents/skills/cantos-engineering/SKILL.md) for any domain, backend, persistence, contract, test or verification change. It owns the required order, decoupling, immutability, functional core, the evidence vocabulary and the completion report.
- Domain owners: [`cantos-script-ir`](.agents/skills/cantos-script-ir/SKILL.md), [`cantos-production-pipeline`](.agents/skills/cantos-production-pipeline/SKILL.md), [`cantos-publication`](.agents/skills/cantos-publication/SKILL.md), [`cantos-listening`](.agents/skills/cantos-listening/SKILL.md).
- UI: [`cantos-ui-design`](.agents/skills/cantos-ui-design/SKILL.md) (Material 3 Expressive and shared UI policy) with [`cantos-leptos-web`](.agents/skills/cantos-leptos-web/SKILL.md) or [`cantos-cmp-mobile`](.agents/skills/cantos-cmp-mobile/SKILL.md).
- Task entrypoints: [`cantos-work-item`](.agents/skills/cantos-work-item/SKILL.md) to deliver a work-plan item, [`cantos-code-review`](.agents/skills/cantos-code-review/SKILL.md) for a read-only review, [`cantos-ui-inspector`](.agents/skills/cantos-ui-inspector/SKILL.md) for a report-only UI audit.
- Story artifact workflows: [`cantos-story-ingest`](.agents/skills/cantos-story-ingest/SKILL.md), [`cantos-radio-adapt`](.agents/skills/cantos-radio-adapt/SKILL.md), [`cantos-audio-produce`](.agents/skills/cantos-audio-produce/SKILL.md) and [`cantos-story-audit`](.agents/skills/cantos-story-audit/SKILL.md) share the [Drive/local workspace contract](docs/production/story-workspace.md). They preserve raw sources, maintain `score/name_map.yml`, coordinate immutable score/audio revisions and recover blocked synchronization through local records and GitHub issues. Keep all story data outside Git.

Skills own working method; `docs/` owns product rules. Keep one copy of each skill and update it rather than adding per-runtime instructions.

## Verification

Run `python3 scripts/check_repository.py`, `python3 -m unittest discover -s scripts -p 'test_*.py'` and `git diff --check` for this bootstrap. For Rust changes run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked`. Persistence changes also require `TEST_DATABASE_URL` pointing to disposable PostgreSQL and `cargo test -p cantos-server --test persistence --locked -- --ignored --test-threads=1`; see `apps/server/README.md`. Add meaningful crash/retry and publication-gate coverage when implementing those flows. UI changes require observations on relevant viewports/platforms, realistic Vietnamese text, focus/text scaling and accessibility; an image alone does not verify interaction or motion.

Do not report an app build, mobile test, audio quality result or end-to-end production run unless it actually ran. Record limitations and required follow-ups with evidence.
