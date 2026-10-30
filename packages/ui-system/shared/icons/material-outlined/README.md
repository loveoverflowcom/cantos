# Offline Material Icons Outlined subset

These 42 unmodified SVG files (plus two Cantos-authored glyphs, below) come from
[Google Material Design Icons](https://github.com/google/material-design-icons/tree/49d4db35df873165d6bd6ba09b063c7dafbac2f4)
at revision `49d4db35df873165d6bd6ba09b063c7dafbac2f4`. Source paths and SHA256 hashes are in
[`manifest.json`](manifest.json). Google distributes them under the included
[Apache License 2.0](LICENSE). The repository's own software license remains a separate decision.

`python3 scripts/ui_icons.py` verifies the pinned original bytes and generates native Leptos SVG
nodes. It replaces the SVG root with `currentColor`, the shared size class,
`aria-hidden="true"` and `focusable="false"`; child geometry is preserved. It does not inject
HTML, use a remote CDN/font, accept arbitrary SVG markup or substitute emoji.

`visibility` and `visibility_off` are the exception to the Outlined style: the Theatre v4 design
(issue [#452](https://github.com/loveoverflowcom/cantos/issues/452)) selects the filled Material Icons
originals, `src/action/visibility{,_off}/materialicons/24px.svg` at the same pinned revision. Their
manifest paths and hashes record that style; the bytes equal the upstream files. The Outlined
variants exist upstream, but no consumer selected them.

`theme_system` (issue [#613](https://github.com/loveoverflowcom/cantos/issues/613)) is the one
Cantos-authored glyph: a split-disc sun with right-hand rays, drawn on the 24-unit grid from the
`light_mode` and `dark_mode` ring/ray geometry so the three theme modes share one stroke family.
Its manifest entry records `origin: cantos-authored` and `derived_from` instead of an upstream
path; the hash still binds the file to generation. It is the System mode of `ThemeCycleButton`
and the CMP topbar theme action, never a composition of two small icons.

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
| Copy | `content_copy` | Studio private handoff receipt copy action |
| Save / retry | `save` / `refresh` | Draft and recoverable actions |
| Reorder chapters | `arrow_upward` / `arrow_downward` | Studio chapter earlier/later actions |
| Archive / delete | `archive` / `delete_outline` | Explicit draft management |
| Generate voice / preview | `mic` / `play_arrow`, `pause` | Forge mock and listening controls |
| Sync / settings | `cloud_sync` / `settings` | Explicit synchronization and settings |
| Player transport / options | `fast_rewind`, `fast_forward`, `speed`, `timer`, `star`, `queue_music` | Theatre CMP player controls |
| Account / chevron | `account_circle`, `person`, `keyboard_arrow_right` | Theatre CMP shell and account rows |
| Show / hide a secret field value | `visibility` / `visibility_off` | Theatre password fields' trailing action; the host owns the visibility state and its localized name |
| Studio workspace navigation | `home`, `menu_book`, `upload`, `transform`, `edit`, `mic` | One decorative icon per Studio Web workspace link; the link text owns the name |
| Search action | `search` | Icon-only, docked action of a Theatre search field; the host owns the localized name |

`auto_fix_high` is not in the subset: its upstream SVG uses `polygon` anatomy that the trusted
generator and the CMP path mapping reject, so the Studio adaptation link uses `transform`.

`arrow_forward` remains progression; `expand_more` remains disclosure. Neither disclosure nor
a selected/success check represents waiting. Buttons keep the visible action label and localized
pending reason, with a separate indeterminate ring. The CMP ring turns under full motion and holds a stationary arc
under reduced motion; the Leptos ring is stationary until its own follow-up. The ring has no
numeric progress meaning or independent hit target. The 23 October AuthPrimary anatomy keeps icon and label centered without a bubble;
regular actions retain their existing shape.

The existing `icon`, `button`/AuthPrimary and `icon-button` previews exercise this registry.
The [23 October delta](../../../../../docs/evidence/ui-parity-402.md) records the bounded
Button/Select admission, optional option icons and actual preference/auth consumers.
Browser/device and remaining #221/#228/#229 acceptance retain their own boundaries.

`library_spines` (#641/#613) is Cantos-authored navigation artwork: two upright spines and one
slightly tilted. Its filled contours are the 1.5px outline of the owner-supplied v9.1 SVG,
including rounded corners and spine bands. This preserves identical 24px geometry in the
existing Leptos SVG and CMP filled-path adapters without a global stroke or scale override.
The [owner source](https://drive.google.com/file/d/13kHSMHjhWiGa35BFZmz-MzCqc_OWnKxu/view)
is design provenance, not a Google upstream icon or a personal-profile fact.
