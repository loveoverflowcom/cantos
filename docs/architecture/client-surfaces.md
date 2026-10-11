# Client surfaces and ownership

Status: three audience-specific entry points are implemented below. Theatre screens are
synthetic UI skeletons; the iOS arm64 simulator host builds, while Android SDK37 and native
runtime verification are pending.
See [decision 0010](../decisions/0010-client-shells.md) and [evidence](../evidence/client-shells.md).

| Audience / surface | Directory / entry | Implemented authority | Missing boundary |
| --- | --- | --- | --- |
| Creator / Cantos Studio Web | `apps/web`, `cantos-studio`, existing `index.html` | Existing authenticated import, adaptation review, revision editor, casting and immutable production-input flows | Existing production identity, durable generation/mix/publication gates |
| Listener / Cantos Theatre Web | `apps/theatre-web`, `cantos-theatre`, separate `index.html` | Hash routes, discovery, local search, one work/episode, library and disabled player shell | Published catalog/manifest, account/progress/library, delivery and browser audio adapters |
| Listener / Cantos Theatre app | `apps/mobile/shared`, Android Activity and SwiftUI host | Compose screens, saved navigation/language/theme/query, unavailable playback; iOS arm64 simulator host compiled/linked | Android SDK37/build and native runtime evidence, audio/background controls/downloads/progress |

The Studio directory and executable name remain stable. Its server static-root and session
entry still belong to Studio. Theatre has no Studio proxy, credentials or crate dependency.
UI separation is not a permission boundary: authorization remains in the backend. A future
listener adapter must consume authorized immutable publications, never private drafts/previews.

## Small shared boundary

- `contracts/` owns versioned domain/wire contracts. This change invents no listener DTOs and
  changes no database schema or backend permission rule.
- `contracts/design/cantos-tokens-0.2.0.json` owns semantic palette/density/type/shape roles.
  `scripts/studio_tokens.py` generates two CSS files and a Kotlin object. Historical Studio
  tokens 0.1.0 remain available. Tokens have no HTTP or domain dependency.
- Each renderer owns layout, focus/back behavior and components. Rust and Kotlin share no UI
  framework or routing abstraction. Synthetic display fixtures are separate from manifests.
- Rust/Axum/PostgreSQL remains one modular backend. No extra service, worker or media storage
  is introduced.

## Reference interpretation

Both supplied stored HTML references were downloaded outside Git, their CSS read and their
Theatre web/mobile and Studio renders inspected in Safari. The shells retain paper surfaces,
lavender ink, serif story headings and compact controls. Studio keeps its editor hierarchy;
Theatre keeps listener navigation and an unavailable mini-player. The web sidebar is 216 CSS px,
collapsing to wrapped navigation at narrow widths; mobile uses native top controls/bottom tabs.

This is a skeleton interpretation, not an exact clone or asset-rights assertion. Embedded art,
font files, private download links and original HTML are excluded. Placeholders and system fonts
expose missing assets. Native runtime comparison remains pending.

## Next bounded work

1. With sufficient disk and explicitly provisioned Android SDK37, build Android. The iOS Debug
   arm64 simulator host already builds; inspect both platforms' safe areas, back, large text and
   assistive technology separately from playback acceptance. No native runtime was launched.
2. Continue publication/listener contracts in the existing [queue](../work-plan/README.md).
   Bind catalog/episode state to reviewed DTOs and real retry semantics after public-delivery
   permissions and immutable manifests exist.
3. Add browser and platform-native playback adapters with cancellation, interruption, entitlement,
   expired URL, resume and offline evidence. Never call AI during listener playback.
4. Resolve cover/font distribution rights before importing assets and comparing final renders.

Commands: [Studio](../../apps/web/README.md), [Theatre Web](../../apps/theatre-web/README.md),
[mobile](../../apps/mobile/README.md).
