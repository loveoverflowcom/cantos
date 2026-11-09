# Theatre empty-state artwork

App-owned decorative assets for Discovery and Library. Source: the owner-selected
[Theatre v4 handoff](https://drive.google.com/drive/folders/1CT8-icdYTGtCJQCNzb3jP3AwCxMnLFZW),
revision `owner-feedback-2-bold-symmetry-density`, delivered by
[`Cantos-Theatre-Redesign-v4-2026-10-25.zip`](https://drive.google.com/file/d/16A3X5SkhV4lPDiBbGp5K-da_ryY1kk3w/view).
The supplied asset manifest was checked against the actual downloaded bytes before packaging.

`library-expressive-light.webp` and `library-expressive-dark.webp` are opaque lossy WebP
derivatives (one VP8 frame, 512 × 427) of the transparent 1374 × 1145 ImageGen raster from
that handoff; it is not SVG or compiled vector artwork. Its transparency is composited onto
the Light and Dark Theatre surface-container tokens, one file per theme, without other tinting.
[Provenance](assets/empty/provenance.json) retains the source identity and encoding settings.
It replaces the earlier Library glyph only after all Library reads have completed successfully
with no retained rows or continuation cursors.

The works/history SVG pairs are unchanged originals from the handoff. Each has a
240 × 200 viewBox. CSS resolves light/dark from the scoped app preference, including
System, without overriding an explicit Light choice with the OS preference. Images
are decorative (`alt=""` inside an `aria-hidden` wrapper), preserve their aspect ratio
and carry no duplicate accessible name. The existing text and actions explain the state.
The global Library invitation keeps its three section summaries quiet; contextual
history artwork appears only for a completed-empty history section outside that invitation.

The app never fetches Drive at runtime. This directory contains no story covers,
private manuscripts, audio or catalog fixture fallback. These design assets establish
no release permission, public-cover policy or playback acceptance. Native XML and WebP
counterparts remain the CMP owner's responsibility; this Web packaging claims no native support.

| Packaged file | SHA-256 |
| --- | --- |
| `library-expressive-light.webp` | `45b57ebadbf48e3ff2b49673c91e7a174bd8021627006ce1d8e0324a41379ede` |
| `library-expressive-dark.webp` | `bd5447590583e891f2d060469a41413fc7a0bf44110150eaf360a36b5fd4def3` |
| `light/cantos-empty-works.svg` | `1c13660f0c791e8ae52da9ac6dfe837500c0d18cdcf60f5fa5fbdfe42606ad55` |
| `dark/cantos-empty-works.svg` | `8d239077d729f6916fa4188c767632b59f212cb37b82e269253e957f13e7e485` |
| `light/cantos-empty-history.svg` | `b95aeb92c33e6eee439dab2127440712a61ad08efa9c0480016d43a48d09bf84` |
| `dark/cantos-empty-history.svg` | `d968ba6d942ac25ae10c16070a0f22ee425d1eac7982568a34c5567038259ed5` |

Rendered browser, keyboard, screen-reader and actual 200% text acceptance are separate
from source packaging and asset-header checks. Foundation control/expression admission
remains separate under #452; this composition adds no shared API or variant.
