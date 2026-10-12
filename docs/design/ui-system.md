# Cantos UI System

Status: product requirements plus an implemented [UI foundation](../../packages/ui-system/README.md).
The live Leptos Studio save/read surface consumes [canonical v0.2.0 tokens](../../packages/ui-system/shared/tokens.json), with earlier
[bounded browser observations](../evidence/script-revision-persistence.md#browser-observations).
The [structured Studio editor](../evidence/studio-script-editor.md) now consumes those same roles;
Theatre/CMP journeys and exact reference fidelity remain unimplemented.

## Direction and reference

Cantos uses Material 3 Expressive principles with a soft lavender/purple identity, rounded controls, clear hierarchy, and purposeful motion. Studio prioritizes reading and editing scripts; Theatre prioritizes choosing and listening to episodes. The brand is shared, while density and navigation follow each task.

The original reference was unavailable during initialization. The authenticated 2026-10-11 design
handoff now supplies identical light/dark paper/lavender palettes, Noto Sans/Noto Serif family
names, component specs and official outlined icons. The [shared contract](../../packages/ui-system/shared/README.md)
records adopted values and bounded foundation choices. Embedded artwork/fonts are not
redistributed. Exact fidelity, interactions and font rendering need actual browser inspection;
static boards do not establish an HTML screenshot comparison.

## Semantic tokens

Keep a versioned token source with platform mappings. Leptos consumes CSS variables; CMP consumes its theme and component wrappers. Share semantic names and relationships, not web pixel values or Rust UI code. Any generated token files must identify their source version.

| Token family | Roles | Rules |
| --- | --- | --- |
| Brand | `brand.primary` | Lavender/purple; retain the chosen brand color as source evidence. Exact values are proposals until selected and checked. |
| Action | `color.primary`, `color.onPrimary`, `color.primaryContainer`, `color.onPrimaryContainer` | Main action, selected control, and supporting emphasis; use paired foreground/background roles. |
| Content | `color.surface`, `color.surfaceContainer`, `color.onSurface`, `color.onSurfaceVariant` | Low-chroma reading surfaces and distinct foreground hierarchy; light/dark themes require separate checks. |
| Structure | `color.outline`, `color.outlineVariant`, `color.focus` | Functional grouping, input boundaries, and visible focus; avoid decorative borders. |
| Status | `color.error`, `color.warning`, `color.success` on their matching `*Container` backgrounds | Pair color with text/icon; never use color alone to identify QC or production state. |
| Type | `type.headline`, `type.title`, `type.body`, `type.label`, `type.metadata` | Fonts must support Vietnamese diacritics and long character/work names. Use tabular figures for comparable time/cost data. |
| Space | `space.xs`, `sm`, `inset`, `md`, `section`, `lg`, `xl`, `xxl` | Adopted rhythm:4/8/12/16/20/24/32/36 in platform-appropriate units; reduce chrome before text or touch targets. |
| Shape | `shape.control`, `shape.container`, `shape.overlay`, `shape.pill` | Pills suit filters, speed, and compact actions; scripts and data tables need stable rectangular reading areas. |
| Elevation | `elevation.base`, `elevation.overlay` | Prefer surface tone and spacing; use elevation where it communicates overlap. |
| Motion | `motion.feedback`, `motion.navigation`, `motion.emphasis`, `motion.reduced` | Specify intent; choose supported framework mechanisms during implementation and verify each target. |

Dynamic system colors must not silently replace Cantos's identity. Shared values belong in the
canonical source, with generated CSS/Rust, contrast checks and a separate deferred CMP mapping.
Studio and Theatre vary density/layout, not palettes. The [foundation evidence](../evidence/ui-system-foundation.md)
separates source/compile checks from pending runtime appearance, motion and interaction review.

Platform scope follows the [repository matrix](../architecture/repository-layout.md#platform-scope):
Studio is Web only; Theatre supports Web and native Mobile. Shared Leptos/CMP component diagnostics
do not authorize a Studio mobile app, mock screen or scaffold.

## Studio: text-first production

The primary journey is import → inspect adaptation → correct script → cast voices → preview → produce → inspect QC → approve and publish. Preserve script position, selections, and unsaved edits across pane changes and recoverable errors.

| Region | Main content and actions | Adaptive behavior |
| --- | --- | --- |
| Work navigation | Works, adaptations, episodes, acts, scenes; explicit current version | Wide: compact rail/tree. Narrow: navigation sheet or dedicated screen with a reliable return path. |
| Script editor | Speaker, dialogue, emotion, pronunciation notes, sound cues; clear draft state | Main reading area gets priority. Wrap dialogue and preserve stable line/scene identity; avoid nested cards around every utterance. |
| Inspector | Character casting, consistent voice profile, pronunciation, generation settings | Wide: secondary pane. Narrow: contextual panel/screen, preserving the selected dialogue. |
| Timeline/preview | Scene order, dialogue clips, cue placement, playback, partial regeneration | Collapsible region; narrow screens show the selected scene instead of shrinking a full desktop timeline. |
| Production status | Stage, affected version, cost estimate/actual cost, retry, QC findings, approval | Persistent compact status with a detailed job view. Production continues independently of an open browser. |

Keep approval and publication controls explicit and permission-aware. Preview, generation, and publication are separate actions. Show the source revision for audio, identify stale clips after edits, and make the scope of a regeneration visible before it starts. A cost estimate may be unavailable; show that state instead of inventing a value.

Use command labels such as “Generate selected dialogue” and “Publish episode” instead of vague actions. Destructive replacements and publication controls should identify the affected episode/revision and preserve recoverable history.

## Theatre: compact listening

Discovery emphasizes cover art, title, genre, synopsis, duration, and episode availability. Avoid oversized banners, excessive gradients, and repeated nested cards. Episode lists must remain useful with long titles, missing covers, and no listening history.

- A compact mini-player preserves the current episode across navigation, with play/pause, title, position, and an entry to the full player.
- The full player presents transport, seek, speed, sleep timer, bookmark, download, and episode context with accessible labels. Group secondary actions without hiding essential playback state.
- Episode/library views distinguish played, in progress, downloaded, downloading, unavailable, and failed states using text or icons as well as color.
- Seek controls expose elapsed and total time in readable text; waveform artwork must not be the only way to seek or inspect progress.
- Bookmarks and resume prompts explain the actual position. Version changes must not silently move a listener into unrelated dialogue.
- Autoplay preferences and download state are explicit. Respect a user's pause, sleep timer, and interrupted session.

Web and mobile share vocabulary and state meaning, while navigation, safe areas, touch, back behavior, and native media integration adapt to the platform. Mobile playback requirements are in [mobile.md](../architecture/mobile.md).

## Components and interaction states

New shared components follow the user-requested
[two-preview admission contract](../../packages/ui-system/shared/README.md#preview-registration-and-adoption):
declare actual examples and concrete state scenarios in both package-local Leptos and CMP apps,
inspect named canonical tokens and the design reference, and pass the required renderer checks
before first real application use. Registration/compilation alone is insufficient. Unsupported
or unrun targets remain explicit and unqualified; current import compatibility never grants new approval.

Document anatomy, variants, and states once for buttons, icon buttons, filter chips, navigation, text fields, episode rows, player controls, job status, QC findings, and confirmation dialogs. Cover idle, hover where relevant, focused, pressed, selected, disabled with reason, loading, error, empty, and success. Disabled actions need a discoverable reason when permissions, QC, or missing inputs block progress.

Use native semantic HTML and input behavior on the web before custom ARIA patterns. Actions are buttons; navigation is links. Keyboard focus is visible and unobscured; dialogs contain focus and restore it to the initiating control. Support tab traversal, meaningful headings, editor input methods, undo where available, and a keyboard-accessible alternative to drag-only reordering.

Avoid single-key global player shortcuts while typing in the script editor. Announce significant async state changes without repeatedly interrupting screen-reader users. Keep generation/error status outside volatile editor text.

## Accessibility and motion acceptance

These are project targets to verify, not claims of compliance:

- Normal text contrast at least 4.5:1; large text at least 3:1; meaningful controls and state graphics at least 3:1 where the applicable criterion requires it.
- Default native touch targets of at least 48×48 dp on Android and 44×44 pt on iOS. Web controls intended for touch use generous hit areas; compact visuals do not imply compact hit areas.
- Browser text zoom at 200% and narrow reflow at 320 CSS px, plus native large font settings. Avoid fixed text heights, clipping Vietnamese diacritics, and hidden actionable labels.
- Screen-reader labels include action, item, and state. Playback controls remain operable without relying on cover art or waveform visuals.
- Reduced-motion settings remove unnecessary travel, overshoot, and continuous animation while preserving immediate state feedback. No animation delays an action.
- Motion stays interruptible and does not move a target away during interaction. Choose and profile framework-supported animation behavior; a still image cannot demonstrate smoothness.

## Future validation matrix

Record the viewport/device, theme, text scale, input method, build revision, observed result, and evidence when each check is run.

| Surface | Representative conditions | Required evidence |
| --- | --- | --- |
| Studio Web | Wide desktop, compact window, 320 CSS px reflow; light/dark; long Vietnamese script and names | Render captures plus import/edit/cast/preview/QC/publish keyboard walkthrough. |
| Theatre Web | Narrow phone and desktop; 200% text zoom; missing cover; long episode list | Render captures and accessible player/seek/bookmark/resume interaction. |
| CMP Android | Small/large windows, safe areas, large fonts, TalkBack, physical touch | Device/emulator captures and full listen/download/interruption flow. |
| CMP iOS | Compact/regular widths, safe areas, large text, VoiceOver | Device/simulator captures and full listen/download/interruption flow. |
| Shared states | Empty/loading/offline/failed/retry/expired access/stale render/publishing conflict | State-specific captures; preserved input and a usable recovery action. |
| Motion | Navigation, mini-player expansion, async feedback; reduced motion | Runtime recording/profiling on supported targets; verify interruption and state clarity. |

Initialization performed no UI checks. The revision slice adds light/dark token contrast tests
and real Safari keyboard/focus, compact-window and 200% zoom observations for its minimal
surface only. Instrumented exact 320 CSS px, screen-reader/IME and native platform checks remain
separate gates; desktop observations never verify Android or iOS.
