# Web AuthTonal spacing candidate — #223

Source-bound implementation and partial Chrome observations. This is session 3 of the requested
sequential #221 → #222 → #223 repair in [Draft PR #225](https://github.com/loveoverflowcom/cantos/pull/225).
[#223](https://github.com/loveoverflowcom/cantos/issues/223) owns execution status. None of the
three issues is accepted or closed by this record.

## Bug contract and design reconciliation

Starting head `84e4e3568989ae3a042e64a58c060477f05a8b50`, clean worktree; base
`0d6725a52afc288afb096bb090e6fec4b31c6d22`. Read issue #223 and its empty comment list,
#221/#222, current docs/queue and the Cantos engineering, UI fix, UI design, Leptos Web,
verification and code-review owners. No branch, worktree, stash or history was replaced.

Observed the existing AuthTonal in the actual local gallery with keyboard focus: its 3-unit
outline plus 4-unit offset extends seven units toward a label separated by `space.xs` (four).
The opened light/VI baseline shows the outline immediately below the label and beside help.
DevTools later confirmed the candidate's computed outline independently. Source inspection of
Theatre's app focus selectors found the same outline property, not a second independent paint;
this record does not claim an app-runtime computed-style inspection.

Read the [v2 handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
and [v3 handoff](https://drive.google.com/file/d/17_s9ix7Qtzj76-RwLbCQLA42qob8g3gq/view),
including its later Select addendum, through connected Drive. Both describe proposals, including
a three-unit focus reference; neither supersedes canonical tokens or the later #223 request to
reduce focus weight accessibly. No design PNG/HTML pixel reference was captured or approved in
this session. Comparison against a reviewed design reference remains a required gap.

Invariant: the auth field's external label, native input, descriptions and adjacent action have
separate visual space; a visible keyboard focus never changes control bounds or auth semantics.
Owner: existing TextField/FieldMessages, Web foundation and preview composition. No domain
extraction is needed for this presentation-only change. Intensity is Medium-Low: email entry
and the primary continuation action lead; focus is functional rather than decorative.

## Implementation and rollout boundary

`TextFieldVariant::AuthTonalSpacious` is an opt-in Web candidate. Existing `AuthTonal` and
`Regular` callers retain their presentation. The candidate reuses AuthTonal's 56-unit minimum,
20-unit corner and semantic surface; its label/control/message gap is `space.sm` (eight).
Focus uses the existing `color.focus`, with width and offset `2 × size.border` (two each).
The four-unit outward footprint leaves room below the label without changing layout on focus.
Forced colors explicitly use system `Highlight`. Error container/underline/text and native
readonly/disabled/loading semantics remain independent. No `outline:none`, new palette,
`!important`, global token or auth-state change was introduced.

The existing recipe host adds `component=auth-tonal`, mapped to catalog `text-field`, with
enabled/error/disabled/loading/readonly states. Standard/long VI/EN fixtures render the actual
TextField, descriptions and a local-only AuthPrimary action. Candidate field→CTA gap uses
`space.lg` (24); the compatibility baseline uses `space.md` (16). This is a proposed app-layout
recipe, not a change to Theatre's form. The fluid gallery includes the candidate and unchanged
baseline for narrow viewport observations. Fixed workbench frames still intentionally scroll
when wider than their host; they are not a full-page reflow oracle.

The catalog retains 32 APIs and all 198 baseline states, plus four Select, three BackButton and
three field diagnostics (`auth-tonal-spacious`, `autofill`, `forced-colors`). Metadata is generated
by the existing primitive; CMP metadata does not implement new CMP geometry. Canonical token
v0.6.0 digest remains `25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741`.
Qualification hashes and compatibility imports are unchanged.

The [admission policy](../../.agents/skills/cantos-ui-design/references/preview-catalog.md) says:
“If any required check is `NOT_RUN`, `UNSUPPORTED`, failed or blocked, leave the component
unqualified and outside real app adoption.” Required design/two-renderer/accessibility evidence
is incomplete. Therefore Signup/Auth/Membership, Account/Settings and topbar consumers remain
unchanged. This partial delivery does not fix the visible product issues yet.

## Browser observations

Mac terminal, task-owned preview and Chrome continued working after 18:00 UTC; no work was lost
to the brief connection notification. Served through `cargo xtask ui-preview leptos run`; the
sandbox launch falsely reported 8083 busy, while the authorized host launch served loopback.
No API, identity, database, Firebase, email, account, provider or deployment was started.

Headed Chrome 155.0.0.0, local Noto fonts `loaded`. Exported facts: 1585×741 CSS px, DPR 1.
Device Mode Desktop toolbar separately showed widths 320/390/768/1280 and height 844; its
fit-to-window display scale is not browser page zoom. Opened images showed the fluid gallery's
long Vietnamese label/help/CTA wrapping without clipping at those widths. Native single-line
email text scrolls within the input; this is not multiline email wrapping.

- Light/VI: actual pointer and Tab/Shift+Tab focus on candidate versus the retained AuthTonal
  baseline. Candidate focus leaves visible space below the label and above help. The earlier
  baseline was opened at the desktop viewport; the retained same-page baseline supplies the
  equal-fixture comparison. No pixel-comparator PASS is claimed.
- DevTools diagnostic `:focus-visible` on real `#auth-email`: computed outline solid 2px,
  offset 2px, color `rgb(116, 75, 152)`; height 56px, radius 20px, local Noto Sans 16px. This
  forced pseudo-state is a computed-style observation, separate from real keyboard focus.
- Dark/EN, long fixture, reduced recipe: edited enabled email, observed the acknowledged value
  in both candidate and baseline, then Tab/Enter produced one local CTA intent. Readonly refused
  a typed `x` and retained its value with no readonly input intent. Disabled/loading samples were
  exposed as disabled with a visible reason; their native/busy anatomy is also example-tested.
- Error plus focus retained error container, underline and explanatory text independently of
  the lavender perimeter. Actual Chrome toolbar showed **Zoom: 200%**; opened error/focus pixels
  retained full external label/help/error and CTA. Restored zoom 100% afterward. This is distinct
  from injected recipe text scaling, which was not exhaustively inspected for this field.
- DevTools **Emulate CSS forced-colors: active**, while DevTools remained open, visibly changed
  the preview to system colors. Opened focused error input retained a visible system-colored
  perimeter and error text; the nonfocused baseline had the dashed error cue. Emulation was
  returned to **Do not emulate CSS forced-colors**. This is Chrome emulation, not a Windows
  high-contrast or screen-reader walkthrough. An earlier emulation attempt was not credited.

The existing contrast primitive measured focus against surface/container/error-container:
light 6.41/5.45/5.36:1, dark 8.70/7.59/7.66:1. These token-pair measurements exceed 3:1 but do
not establish every rendered/autofill/system-color combination or complete WCAG conformance.

Opened screenshots appear in the work chat. The DevTools viewport download copied to ignored
`artifacts/agent-runs/20261016-ui223/auth-gallery-light-390.png` was separately opened; it shows
unfocused layout after DevTools took focus, not the keyboard-focus result. Raw exports and
`provenance.json`/`runtime-source.diff` record recipes, source and built-asset fingerprints.
Export source SHA/dirty fields remain explicitly `unrecorded`; separate fingerprints do not
turn them into build-supplied attribution. Later formatting/test-copy/catalog-reason changes
have no visual effect. No screenshots, downloads or private data are committed.

## Layered verification and review

L1: existing field/style/preview ownership and unchanged product imports reviewed. L2: no domain
rule changed; native field and host validation/value authority retained. L3: added native tests
cover required email/autocomplete/name/raw value, readonly/disabled/busy/error associations across
all states; VI/EN recipe tests cover local non-submit CTAs and unique IDs. The first checkpoint
failed before the candidate enum existed. The next `cargo xtask ui-preview leptos verify` passed
93 native tests, five doctests, WASM Clippy and preview build. Later gallery/forced-colors changes
are covered by final handoff checks recorded in PR #225, not by that earlier checkpoint alone.
L4/L5: the named browser interactions/opened images above establish bounded `interaction-tested`,
`DOM-tested` and `screenshot-inspected` evidence, not live auth integration or screen-reader proof.

Final-diff preflight and generator/Python results, including actual failures and unavailable
prerequisites, are recorded in PR #225 and ignored agent-run logs. The existing clean-base
repository failure (20 stale qualification/adoption findings), unsupported CMP Gradle gates and
session 1's missing ffmpeg/ffprobe contract prerequisite remain independently reportable; no
gate or qualification hash is relaxed. GitHub CI is not checked or dispatched.

Read-only Cantos self-review covers the whole branch contribution from `0d6725a5`, not only #223:
Select's pure decisions, native fallback/form facts, Popover platform adapter/listener cleanup,
BackButton default/opt-in and CSS cascade, TextField semantics, preview recipe/fixtures,
generated metadata, tests and consumer imports. No new backend/security/publication/storage
claim earns a domain pass. No remaining confirmed source defect was found in this declared
candidate scope. Required evidence gaps still give `changes-requested` for product adoption.

| Claim | Owner / requirement | Evidence / disposition |
| --- | --- | --- |
| Controlled Select browsing/acceptance and native fallback | Select; #221 | production decision/anatomy examples and bounded Chrome continuation; held within named scope |
| Back name/target/host callback and opt-in geometry | BackButton/Button; #222 | session 2 input/capture record plus source review; held within named scope |
| Field native semantics and scoped label/focus/rhythm | TextField/foundation; #223 | state examples, computed styles, real inputs and opened images; held within named scope |
| Tokens/catalog/default callers preserved | generators and admission policy | unchanged tokens/imports/qualification records, generated metadata; held |
| Complete design/accessibility/renderer/product acceptance | all three issue criteria and preview policy | gap-required; keep candidates unqualified, Draft and consumers unchanged |

Remaining: actual autofill, screen reader, OS IME/touch, other browser engines, complete system/
nested-theme focus/value preservation, full injected-large-text/motion/cancellation matrix and
reviewed design reference/comparator pairing. #221 still needs full viewport-edge/scroll/resize,
touch/fallback/form-validity and preference-persistence flows; #222 still needs its recorded
remaining checks. CMP tooling/qualification remains unavailable. Next acceptance owner:
**unassigned — assignment required before acceptance**. Resume the same recipes after the
required tooling/review is available, then adopt and inspect the real signup and preference/
Back flows. Do not close issues or remove Draft from this partial evidence.

Residual risk: these are preview candidates with incomplete admission; the three product bugs
remain present in existing callers. Builds, token contrast and Chrome observations do not
establish cross-engine, assistive-technology, CMP or production authentication acceptance.
