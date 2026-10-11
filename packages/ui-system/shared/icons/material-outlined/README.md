# Offline Material Icons Outlined subset

These nine unmodified SVG files come from
[Google Material Design Icons](https://github.com/google/material-design-icons/tree/49d4db35df873165d6bd6ba09b063c7dafbac2f4)
at revision `49d4db35df873165d6bd6ba09b063c7dafbac2f4`. Source paths and SHA256 hashes are in
[`manifest.json`](manifest.json). Google distributes them under the included
[Apache License 2.0](LICENSE). The repository's own software license remains a separate decision.

`python3 scripts/ui_icons.py` verifies the pinned original bytes and generates native Leptos SVG
nodes. It replaces the SVG root with `currentColor`, the shared size class,
`aria-hidden="true"` and `focusable="false"`; child geometry is preserved. It does not inject
HTML, use a remote CDN/font, accept arbitrary SVG markup or substitute emoji.

Add icons only for concrete consumers. Keep an official original, its pinned upstream path/hash
and license; extend the closed `IconName` API through generation. Adjacent text or the parent
control owns the localized accessible name. A standalone meaningful graphic needs a different
explicit labeled contract; this decorative API must not be used as its sole announcement.
