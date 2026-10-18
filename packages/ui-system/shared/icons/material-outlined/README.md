# Offline Material Icons Outlined subset

These 34 unmodified SVG files come from
[Google Material Design Icons](https://github.com/google/material-design-icons/tree/49d4db35df873165d6bd6ba09b063c7dafbac2f4)
at revision `49d4db35df873165d6bd6ba09b063c7dafbac2f4`. Source paths and SHA256 hashes are in
[`manifest.json`](manifest.json). Google distributes them under the included
[Apache License 2.0](LICENSE). The repository's own software license remains a separate decision.

`python3 scripts/ui_icons.py` verifies the pinned original bytes and generates native Leptos SVG
nodes. It replaces the SVG root with `currentColor`, the shared size class,
`aria-hidden="true"` and `focusable="false"`; child geometry is preserved. It does not inject
HTML, use a remote CDN/font, accept arbitrary SVG markup or substitute emoji.

Add icons only for concrete consumers. Keep an official original, its pinned upstream path/hash
and license; extend the closed `IconName` API through generation. `auto_mirror: true` in the manifest marks the glyphs Material publishes as AutoMirrored
(`arrow_back`, `menu_book`, `logout`, `keyboard_arrow_right`, `queue_music`); the CMP generator emits
it as `autoMirror` and the vector builder applies it. Adjacent text or the parent
control owns the localized accessible name. A standalone meaningful graphic needs a different
explicit labeled contract; this decorative API must not be used as its sole announcement.

## Action semantics and consumers

The registry is a bounded shared asset source, not product adoption. The generator emits both
Web `IconName` (including `ALL` and `as_str()`) and CMP `CantosIcons`. CMP rejects unknown
names even when a pending action temporarily hides its glyph.

| Intent | Glyph | Intended consumer |
| --- | --- | --- |
| Sign in / sign out | `login` / `logout` | Studio and Theatre account actions |
| Create account / verify email | `person_add` / `mail` | Theatre registration |
| Home / contextual Back | `home` / `arrow_back` | Host-owned navigation |
| Cancel / choose | `close` / `check` | Explicit cancel or selection |
| Save / retry | `save` / `refresh` | Draft and recoverable actions |
| Archive / delete | `archive` / `delete_outline` | Explicit draft management |
| Generate voice / preview | `mic` / `play_arrow`, `pause` | Forge mock and listening controls |
| Sync / settings | `cloud_sync` / `settings` | Explicit synchronization and settings |
| Player transport / options | `fast_rewind`, `fast_forward`, `speed`, `timer`, `star`, `queue_music` | Theatre CMP player controls |
| Account / chevron | `account_circle`, `person`, `keyboard_arrow_right` | Theatre CMP shell and account rows |

`arrow_forward` remains progression; `expand_more` remains disclosure. Neither disclosure nor
a selected/success check represents waiting. Buttons keep the visible action label and localized
pending reason, with a separate stationary indeterminate ring on both renderers. The ring has no
numeric progress meaning, independent hit target or motion. AuthPrimary alone keeps its scoped
paired bubble; regular actions retain their existing shape.

The existing `icon`, `button`/AuthPrimary and `icon-button` previews exercise this registry.
The Select candidate and its native fallback remain under #221; this asset addition does not
admit that candidate, add option icons, or migrate preference/auth/Forge consumers.
