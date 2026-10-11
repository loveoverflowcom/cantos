# ADR 0012: Shared token source with separate UI renderers

- Status: accepted for the user-authorized local foundation slice; runtime review pending
- Date: 2026-10-11
- Decision owner: Cantos maintainers (requested four-session implementation boundary)
- Related issue or PR: none created; local-only delivery
- Supersedes: active token ownership in decision 0009; historical restructure evidence unchanged

## Context

After PR27, Studio is implemented, Theatre is a Leptos skeleton and both mobile directories are
reserved. `packages/design-tokens` owns only a Studio v0.1.0 file. The reviewed design reference
now provides one paper/lavender palette, font roles, specs and licensed official outlined icons.
The request needs reusable Web UI without merging the older client-shell branch or bringing in
backend/domain dependencies.

VOT Workspace develop `f3fea641a5fd226b883c4e32224ad8c97b8323f0` has its actual system at
`ui-system/`, with peer Leptos/CMP renderer folders. Its canonical source is CSS and its native
consumer parses a limited mapping. Reuse explicit ownership and composable API principles;
its palette, rounded icons and large layer graph do not fit Cantos's requested boundary.

## Decision

Rename to `packages/ui-system`: `shared` owns one versioned bounded JSON semantic schema and
licensed offline originals; `leptos` owns one `cantos-ui` crate and generated CSS/Rust; `cmp`
documents a deferred native mapping. JSON generation remains Python standard-library code.
The crate pins the existing 0.7.8 version and has a real Studio theme consumer plus a bounded
development example. No native module is fabricated.

The functional core validates token types/themes/aliases/contrast and deterministically renders
outputs; the imperative shell reads/writes files. UI components render anatomy and dispatch
callbacks. A non-empty disabled reason is a type barrier. The app owns durable state, permissions
and effects. No production rule is extracted or duplicated by this UI change.

## Alternatives

| Option | Decision |
| --- | --- |
| Keep Studio token package and copy a Theatre palette | Reject: creates drift between two apps |
| Copy VOT CSS/Gradle architecture | Reject: no native consumer and different visual identity |
| Add a general token framework or new frontend stack | Reject: no concrete need beyond the existing stdlib generator and Leptos |
| One JSON source, generated scoped CSS and typed Rust | Select: current pipeline can validate both themes and preserve role compatibility |

## Consequences and validation

Old semantic CSS names remain available; v0.1.0 bytes move to an immutable legacy snapshot.
The Studio command forwards to the new generator and Studio links shared CSS directly.
Values now follow the reviewed source: intentional palette/body/type/shape changes require later
runtime inspection. The source does not authorize redistribution of embedded artwork/fonts.
Details and risks are in [migration](../architecture/ui-system-migration.md) and
[evidence](../evidence/ui-system-foundation.md). Revisit after component/app integration shows a
real missing role or native consumer, not simply to match VOT's folder count.
