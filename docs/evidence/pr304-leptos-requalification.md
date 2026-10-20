# Leptos admission compatibility recheck — PR 304

The repository gate reported six stale Leptos qualifications and sixteen dependent application
admission errors on both the original baseline and the PR. This fresh recheck preserves only
the existing Button, TextField, Surface, Badge, BackButton and Logo admissions. Historical
issue141/issue142 records remain unchanged. No new component, variant or CMP admission is added.

## Source and commands

Base: `develop@c2a8b6f` after PR 303; PR review head before reconciliation: `85d9249`.
The renderer, declarations, tokens, font bytes and component contracts are unchanged by this
repair. The previous renderer digest predates later workspace/renderer inputs. The digest
includes root Cargo manifests and lockfile as well as the gallery, so compilation alone could
not renew admission. New records bind this actual source after runtime inspection.

- Tokens SHA-256: `25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741`.
- Leptos source SHA-256: `04b743cd311e259b98ebef41c52866d7e9eda84cde6b1479d7e7b59ca039fbb2`.
- CMP source SHA-256 (unchanged): `8ed0c550c092e217bae37b24f3cbdc37d1570d70e4e4e209f680fadcc058d3ba`.
- `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos verify`: PASS, native SSR tests,
  WASM Clippy with warnings denied, and Trunk build.
- `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos run`: local gallery on port 8083.
  An initial concurrent Trunk build collided on the output directory; the completed verify
  build supplied the final same-source bundle served and inspected below. No source fix or
  weaker check was used for that local build collision.

## Actual observations

Reviewer: Codex root, 20 October 2026, Linux host, Codex in-app browser. Default viewport
1280×720, explicit 1280×900 and 390×844; viewport override restored after inspection.
Vietnamese fixtures, Compact density, light/dark as named, 100% and scoped 200% text.
Browser version and DPR were not exposed by this tool and are unknown.

| Existing component | Runtime/accessibility oracle and observed result | Opened local captures |
| --- | --- | --- |
| Button | Enter and a separate pointer activation each produced one intent. Loading and disabled controls were disabled. Long Vietnamese label wrapped at frame320/text200; root remained16px and no global horizontal overflow was observed at390px. | `button-dark-390-200.png`; the earlier light capture shows only the scrolled lower portion and is not the complete-label oracle |
| BackButton | Enter on the existing baseline sent one host intent; Tab moved focus. Loading was disabled. Narrow frame320/text200 retained the complete label and arrow. Unified candidate remains unqualified. | `back-light-200.png` |
| TextField | Typed fixture retained visibly through readonly/error/theme/scale. `aria-invalid=true` with help/error IDs, readonly true, loading disabled. Focus outline, persistent label and wrapped help/error inspected at390px/200%; native single-line value scroll remains expected. The DOM tool redacts email values, so value retention uses visible fixture output and pixels. | `field-dark-200.png`, `field-dark-390-200.png` |
| Badge | All five tones had visible icon and full text in both themes at390px, with wrapping and no document overflow. Passive component; no action is claimed. | `badges-light-390.png`, `badges-dark-390.png` |
| Surface | Reading/tonal groups retained distinct surfaces, paired ink, Vietnamese wrapping and quiet grouping in both themes at390px. Passive component; no action is claimed. | `surfaces-light-390.png`, `surfaces-dark-390.png` |
| Logo | Nine variant/tone specimens loaded; original vector aspect ratios283×96 and64×64 retained. Decorative images had empty alt/aria-hidden; exactly one named Cantos symbol appeared in accessibility output. | `logos-dark-390.png`, `logos-dark-1280.png` |

All named captures were opened through CUA image output. The gallery's real four-font diagnostic
reported Noto Sans400/700 and Noto Serif400/700 loaded. Colors, layouts and artwork were judged
qualitatively against their existing source/token/component contract; no design-pixel comparison
or missing historical image was substituted. Capture hashes and source digests are in local
`artifacts/agent-runs/pr304/ui-inputs.json`; logs, DOM and PNGs are ignored local evidence,
not PR-hosted screenshots.

## Evidence boundary and remaining acceptance

These are `example-tested`, `DOM-tested`, `interaction-tested`, `accessibility-checked` and
`screenshot-inspected` observations scoped to the rows above. The admission guard is unchanged.
No screen reader, named OS IME, full animation/cancellation matrix, browser engine matrix,
Android/iOS device, live API, identity or audio was exercised. CMP records remain attributable
to their own unchanged source and are not renewed from Web observations. Production acceptance
still belongs to each application and its native/browser acceptance owner.
