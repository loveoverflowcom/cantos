# Cantos Studio mobile (reserved)

This directory reserves the location for a Kotlin Compose Multiplatform Android/iOS Studio client.
**No Gradle/Xcode project exists, no screen is designed and nothing is built or verified.** The MVP
mobile priority is listening in [Cantos Theatre](../../cantos/mobile/README.md); whether Studio
needs a mobile client, and for which tasks (for example review and approval), is a later product
decision.

Optional developer tooling is already available through
[`cargo xtask compose-preview cantos-studio`](../../../xtask/README.md#compose-preview), once
this product has an actual Gradle project and preview composables. The
[CMP preview workflow](../../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md)
keeps Studio selection and host-only evidence explicit; it does not implement the mobile client.

When it starts, it must:

- consume the Studio v1 contracts in [`packages/contracts`](../../../packages/contracts/README.md)
  and keep authority, validation and approvals in the backend;
- use the shared Material 3 Expressive token roles in [`packages/ui-system`](../../../packages/ui-system/README.md)
  at Studio's denser, task-first intensity;
- follow the [CMP skill](../../../.agents/skills/cantos-cmp-mobile/SKILL.md) and
  [UI system](../../../docs/design/ui-system.md); and
- not import Theatre code. Extract a shared Kotlin module only for code both apps use.
