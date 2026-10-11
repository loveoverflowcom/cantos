# Design tokens

[`studio-tokens-0.1.0.json`](studio-tokens-0.1.0.json) is the versioned source of the semantic
light/dark color roles, logical dimensions and contrast pairs that follow the shared Material 3
Expressive system in the [UI system](../../docs/design/ui-system.md). It moved here unchanged from
`contracts/design/`; its bytes and version `0.1.0` are the same.

`python3 scripts/studio_tokens.py` generates [`apps/cantos-studio/web/tokens.css`](../../apps/cantos-studio/web/tokens.css)
and `--check` fails on drift. Only that Web consumer exists. No Kotlin theme is generated.

## Studio and Theatre

Both products use the same semantic role names and Material 3 Expressive intent, with different
intensity: Theatre is calm and media-first, Studio denser and task-first (see the
[token pipeline](../../.agents/skills/cantos-ui-design/references/tokens.md)). The file name says
`studio` because that is the only token set so far. Theatre has none: add its set, or a
surface-specific layer over shared roles, with the first Theatre UI slice after a design review.
Do not copy Studio values into Theatre CSS.
