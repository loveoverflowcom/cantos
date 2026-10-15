# Official Cantos Logo Kit v2 subset

The nine unmodified SVGs come from the official kit linked by
[issue #127](https://github.com/loveoverflowcom/cantos/issues/127). The imported
[guide](Logo-Guide.md) and [provenance](provenance.json) preserve source locations,
kit version, SHA-256 checksums and the precise rights boundary. Wordmark-only,
PNG, preview artwork, export scripts and font binaries are not runtime needs and
are not imported.

The kit does not supply a separate license for the Cantos mark or a general
redistribution grant. This is the project owner's requested repository integration,
not a claim that the mark is freely licensed. The supplied [OFL](OFL.txt) concerns
the Noto source fonts. Its document-output exception is not a trademark license.
All lettering in these SVG documents is already outlined; no font is needed.

Web embeds the original SVG document as a local data URI, retaining vector paths
and isolating the monochrome mask IDs. CMP maps the same checked shapes into a
local vector painter through the existing generated-asset pattern, with its own
isolated transparent layer for the true monochrome knockouts. Neither renderer
loads Drive, a CDN, fonts or remote images. The symbol's colored rounded square is
official artwork, not a replacement page background.

Keep the original aspect ratio and guide clear space. Lockups already include
16 SVG units of clear space around the symbol. A symbol-only consumer owns the
surrounding spacing; the guide suggests one quarter of the drawn symbol height.
The guide's suggested minimum sizes are recommendations, not rendered acceptance.
Use the dark asset on a dark surface and mono on a contrasting light surface.
Samples, semantic tests and inspected captures must qualify both renderers before
any new app adoption. Source checks do not establish appearance or accessibility.
