# Cantos source map and snapshot limits

> **Scope.** Where a prompt author starts reading for each kind of task, and which facts must
> be re-read every time instead of trusted from memory or from this file. The map is a set of
> starting hints: confirm each path at the ref you pinned.

## Fixed scope

Repository: <https://github.com/loveoverflowcom/cantos>. Prompts serve Cantos engineering.
Story ingest, radio adaptation and external audio production run through their own
entrypoints and the [story workspace contract](../../../../docs/production/story-workspace.md),
not through engineering prompts.

Cantos Studio is Web-only for creators; Cantos Theatre has Web and native CMP clients for
listeners. Check the [platform matrix](../../../../docs/architecture/repository-layout.md#platform-scope)
before changing scope; never add Studio Mobile.

## Where to start reading

| Question | Starting sources (verify they exist at the pinned ref) |
|---|---|
| working rules, base, architecture, skill index | [`AGENTS.md`](../../../../AGENTS.md), [`CONTRIBUTING.md`](../../../../CONTRIBUTING.md), [skill index](../../README.md), [documentation map](../../../../docs/README.md) |
| engineering method, invariants, evidence, checks | [`cantos-engineering`](../../cantos-engineering/SKILL.md) and the references it routes to |
| one work item or PR | [`cantos-work-item`](../../cantos-work-item/SKILL.md), [work queue](../../../../docs/work-plan/README.md), the live issue |
| review or failure | [`cantos-code-review`](../../cantos-code-review/SKILL.md), [`cantos-diagnostics`](../../cantos-diagnostics/SKILL.md) |
| native Turso and Identity | [native refactor record](../../../../docs/architecture/native-turso-refactor.md), `services/cantos-server`, `services/cantos-identity`, their manifests |
| auth client and RPC | [Identity auth v1](../../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md), [Identity RPC v1](../../../../services/cantos-identity/contracts/identity/v1/README.md), [`packages/web-auth`](../../../../packages/web-auth/README.md) |
| Web | [`cantos-leptos-web`](../../cantos-leptos-web/SKILL.md), [`cantos-ui-design`](../../cantos-ui-design/SKILL.md), `apps/cantos-{theatre,studio}/web` |
| CMP | [`cantos-cmp-mobile`](../../cantos-cmp-mobile/SKILL.md), [Theatre Mobile guide](../../../../apps/cantos-theatre/mobile/README.md), `services/cantos-server/contracts`, `apps/cantos-theatre/conformance` |
| Quint and stateful behavior | [`cantos-behavior`](../../cantos-behavior/SKILL.md), [`specs/behavior`](../../../../specs/behavior/README.md), registered capability and conformance observer |
| tests and toolchain | [`xtask/README.md`](../../../../xtask/README.md), workspace/package manifests, the toolchain pin, the actual tests |
| infrastructure and deployment | [`infra/`](../../../../infra/README.md), [service inventory](../../../../services/README.md), the current runbook and the scope's issue |

Read code at one revision: a PR head can differ from its branch's HEAD and from the base.

## Re-read instead of trusting a snapshot

These facts change often; a prompt states them only after reading the current owner:

| Before claiming | Read |
|---|---|
| which store serves a context (PostgreSQL versus embedded Turso, one database per service) | the native refactor record, the composition root and default Cargo features; a candidate adapter is not a migrated runtime |
| that legacy PostgreSQL or Kanidm code is gone | actual usages and composition; production fallback, test double, explicit preview, migration history and stale docs are different cases |
| how authentication, sessions and permissions split between Identity and Server | the [v1 owner amendment](../../../../docs/architecture/native-turso-refactor.md#owner-amendment-v1-authentication-and-parallel-integration) and the auth contracts; an authentication fact is not a session or a permission |
| Web cookie versus native Bearer transport | `packages/web-auth`, the Web auth migration in `cantos-leptos-web` and the Theatre Mobile guide; never revive an older all-Bearer decision from memory |
| which sign-in factors v1 requires | the amendment and its ADR; passkey, recovery, phone or Google are not default prerequisites unless the user asks |
| CMP live versus preview behavior | the Theatre Mobile guide; native composition is separate from desktop preview mocks |
| whether native dev/test needs Podman | the service inventory and xtask; production OCI packaging is a separate concern |
| which entrypoints still stop before a prerequisite | the entrypoint itself; the foundation already supports progressive handoff |

## Never hard-code into every prompt

Issue or PR numbers, toolchain or Turso versions, harness/capability names, package names,
host or VPS paths, database URLs, feature flags, CI status or release state. Read the current
source for the scope. When the user asks to remove legacy code in source and accepts unwired
seams, put that decision in the prompt and add no production-cutover prerequisite to the source
change; still separate source changes from deleting live data or services.
