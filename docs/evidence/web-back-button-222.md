# Web BackButton candidate — #222

Source-bound implementation and partial browser evidence, not product acceptance or a new design
policy. [#222](https://github.com/loveoverflowcom/cantos/issues/222) and
[PR #225](https://github.com/loveoverflowcom/cantos/pull/225) own execution status. This is session
2 of the requested sequential #221 → #222 → #223 repair on one branch/PR; #223 remains next.

## Bug contract and ownership

Starting HEAD: `5a5d099324554c49db4ed3e602b0d9fa741d2f45`; base:
`0d6725a52afc288afb096bb090e6fec4b31c6d22`. The worktree was clean. Existing history, other
worktrees and the main clean `develop` checkout were preserved. Issue #222 had no comments.
The original user screenshots were not supplied as image bytes to this session.

Reproduced the existing BackButton in the local Leptos gallery on Chrome: its neutral icon circle
overlaps the full-button hover layer, while the label has no trailing inset. Source confirms
`padding: 0`, a separately painted 48-unit icon and the generic Button hover/pressed gradient.
The reference is the observed existing component, not an approved pixel/design oracle.

Invariant: one native Back target owns the decorative arrow, label, inset and continuous
hover/pressed/focus region; its geometry never changes the host's return callback, contextual
destination or disabled/loading policy. Owner: existing `back.rs`, Button, Web foundation and
preview workbench. No domain/core, router, identity, backend, contract or CMP layout change.
L2 domain extraction is inapplicable to this presentation repair.

Intensity is Medium-Low: contextual return remains secondary to auth/account content and CTA.
Reuse canonical `size.backControl` (48), `size.icon`, `space.sm`, `space.controlInline`,
`shape.authPressed` (16), shared hover/pressed opacity, focus and motion roles and the existing
Web control corner mapping. Canonical tokens remain v0.6.0 with digest
`25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741`.
No proposed design-v3 geometry was promoted to policy or copied into a second token source.

## Implemented candidate and admission

`BackButtonVariant::Unified` opts the existing BackButton into one native-button state layer,
token padding, minimum 48-unit width/height and a transparent decorative arrow. The corner uses
the Web control role, so multiline labels do not produce a large oval; the pressed outer corner
uses `shape.authPressed`. Focus retains the shared visible focus roles. Logical start alignment
and arrow mirroring cover the RTL source path; no runtime RTL claim. Reduced motion removes the
shape transition. `IconTonal` remains the default for all existing product callers.

The existing catalog retains all 32 APIs and 198 baseline states, plus #221's four and #222's
three diagnostics (`hover`, `unified-web`, `forced-colors`). Generated metadata is regenerated;
qualification hashes and compatibility import sites are unchanged. `Unified` is Web-only;
the generated CMP metadata does not implement or qualify that presentation on CMP.

Recipe: `?v=1&component=back-button&state=enabled&fixture=standard&theme=dark&density=comfortable&locale=en&text=100&frame=390&motion=system&mode=matrix&playback=live`.
The workbench renders real Account/Sign in/icon-only controls and the unchanged IconTonal
baseline, with localized standard/long-Vietnamese fixtures and enabled/disabled/loading states.
Hover/focus/press require real input. Callbacks only log host intents; they never navigate.

The [admission owner](../../.agents/skills/cantos-ui-design/references/preview-catalog.md) requires:
“If any required check is `NOT_RUN`, `UNSUPPORTED`, failed or blocked, leave the component
unqualified and outside real app adoption.” Therefore Auth, Membership and Account consumers
are unchanged. This is a staged repair, not completion of #222's product acceptance.

## Browser observations and limits

Mac Chrome became accessible during this session. The earlier lock blocker in
[the #221 record](web-select-221.md) describes that session, not the current Mac state.
Used `cargo xtask ui-preview leptos run`; first sandboxed launch incorrectly reported a busy
port because loopback binding was restricted. A read-only host check found no existing preview;
the authorized host launch served 8083. Only this session's supervised processes were stopped.
No API, identity, database, cloud or provider was started.

Chrome 155.0.0.0, headed, local Noto fonts reported `loaded`. Browser facts for the desktop
matrix: viewport 1585×741 CSS px, DPR 1; device-mode viewport 390×844 CSS px, DPR 2 also read
through the existing observer. Device Mode was changed from Mobile to Desktop for stable native
pointer observations. Native AX activation during Mobile emulation occasionally targeted a
different same-named sample; only the observed target/intent was credited, and pointer/focus
checks were repeated with known screenshot coordinates or keyboard input.

- Before: gallery light, Vietnamese, normal text; opened capture shows separate icon and label
  hover paints. After: same gallery fixture and real pointer input; one continuous candidate
  hover region with inset. The unchanged baseline remains alongside it for comparison.
- Opened gallery captures at actual 320, 390, 768 and 1280 CSS px. Vietnamese text wraps at narrow
  widths without clipping the candidate label; icon-only remains a separate named native target.
  The workbench's fixed frame intentionally has horizontal scrolling when wider than its host;
  it is not the oracle for full-page reflow. One 320 fixed-frame capture shows that clipping and
  must not be used as a successful reflow result.
- Actual Chrome page zoom 200% was displayed by the browser, and the gallery candidate's
  label, padding and visible focus were opened/inspected. Zoom was restored to 100% afterward.
  Separate injected recipe text 200%, long Vietnamese, dark, reduced motion showed wrapping.
  Its initially oval focus prompted the final Web-corner fix; repeated keyboard focus showed
  the bounded corner instead. Injected font scaling is not browser zoom.
- Desktop matrix English/dark: click, held Space and Enter produced exactly three host intents.
  Tab/Shift+Tab returned to the same named target. The trusted-input observer measured
  176.88×48 CSS px and invariant bounds through a held Space press, with radius 16px during
  the held phase; normal radius 24px. The Vietnamese standard target measured 152.16×48 CSS px.
  Reduced-motion long-text observation measured 286×245.98 CSS px, radius 24px and no transition.
- Native AX exposes disabled/loading samples and their reasons. Native string tests separately
  assert busy/disabled/name/description anatomy. No screen-reader proof or exhaustive DOM claim.

Opened PNGs and raw exported browser metadata live in ignored
`artifacts/agent-runs/20261016-ui222/`; screenshots also appear in the work chat. Chrome DevTools
captured viewport PNGs through its UI; exact task-owned downloads were copied without deleting
or overwriting other user files. The export explicitly reports `source_sha=unrecorded` and
`source_dirty=unrecorded`; it cannot attribute the build itself. The run log plus saved source,
diff and built-asset fingerprints separately pin this session's `5a5d099` + dirty implementation.
Pixels were opened, not merely enumerated. No screenshot or downloaded artifact is committed.

## Independent verification and continuation

L1: existing component/style/preview ownership and opt-in rollout reviewed. L2: native Button
state/name/callback contract retained; no new domain decision. L3: native anatomy and recipe
examples, generator parity, formatting and WASM lint/builds establish `example-tested`,
`statically-checked` and `compiled` evidence. L4/L5: named Chrome inputs and opened captures
establish bounded `interaction-tested` and `screenshot-inspected` observations only.

`cargo xtask ui-preview leptos verify`: PASS, 91 UI native tests + 5 doctests, WASM Clippy and
preview build. The added variant initially failed the compile-time anatomy test before the API
existed; a later 198-state-count test correctly failed after adding three registrations and was
updated to retain the exact frozen baseline plus explicitly enumerated extensions.

Final full changed-only preflight: UNSUPPORTED for both CMP gates (installed supported Gradle
absent), independent gates NOT_RUN due prerequisite precheck. Full logs and final available-gate
results belong to PR #225. The available-gate preflight passes format, scoped native/WASM
lint/tests, both Web builds and preview, with repository/CMP explicitly NOT_RUN via `--skip`.
Native totals: 239 Studio + 221 Theatre + 91 UI, plus 10 doctests. UI generator tests: 95 PASS;
token parity PASS. `ui_preview_catalog.py --check` and repository gate remain FAIL on the exact
same 20 findings as the existing clean-base log (six stale Leptos qualifications and existing
import admission failures). One earlier repository run also saw the not-yet-created evidence
link; the complete-docs rerun removed that transient finding. Neither failure authorizes
refreshing qualification hashes. Contracts/audio inputs are unchanged;
the prior session's missing ffmpeg/ffprobe limitation remains separate. GitHub CI not checked.

Deferred acceptance: forced colors, complete system-theme/reduced-motion/cancel/reversal matrix,
screen reader, runtime RTL, browser-engine coverage, reviewed design/reference admission and
the real Auth/Membership/Account flows remain NOT_RUN. CMP prerequisites and reviewed
qualification remain unmet. Acceptance reviewer is unassigned — assignment required before
acceptance. No performance/cache timing claim; no asset/provider/publication behavior changed.

Next: session 3 continues #223 on the same branch/PR. Keep this opt-in boundary and the Select
candidate intact. Resume browser qualification with the existing recipe and exact inputs; only
after required admission opt the real BackButton callers into Unified and inspect their
contextual return flows. Do not close #222 or remove Draft based on these partial observations.

Residual risk: incomplete renderer/design/accessibility/product admission; no consumer adoption
or navigation-flow acceptance. Source tests and local captures do not accept the whole issue.

## Cantos self-review

Read-only review scope: this session's diff from `5a5d099`, traced through the existing Button,
foundation cascade, recipe/model/preview, generated catalog, tests and unchanged Auth/Membership
callers. No domain/persistence/publication/provider pass applies. Review found a diagnostic
`force-full` override that would suppress Unified's shape transition; the implementation repair
added it to the existing auth shape override. This diagnostic path is source-reviewed/build-only,
not a runtime motion acceptance claim. Normal/reduced evidence inputs are unchanged.

| Claim | Owner / requirement | Evidence / disposition |
| --- | --- | --- |
| Host callback, one native name/target, blocked state retained | BackButton/Button; issue #222 | anatomy examples and actual named Chrome events; held in that scope |
| One padded state region, stable target and wrapped corners | Web foundation; issue #222 | opened previews, measured trusted-input bounds; held for observed scenarios |
| Existing callers unaffected; no admission bypass | UI admission policy | default IconTonal, unchanged app imports/qualifications; held |
| Complete visual/accessibility/product acceptance | issue #222 and preview admission | required evidence gap; gap-required, adoption remains blocked |

No remaining confirmed source defect in the declared repair scope. Assessment for product
adoption: changes-requested because of the required evidence gap; this does not block delivery
of the explicitly unqualified candidate in the requested Draft PR. Residual risk remains the
unrun scenarios and missing qualification above; no remote review or approval was posted.
