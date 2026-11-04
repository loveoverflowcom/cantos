# Studio artwork staging — issue 627

Status: presentation implementation with source, native render tests and inspected synthetic
browser evidence on 4 November 2026. Real private-media/API acceptance remains pending.
Part of [#627](https://github.com/loveoverflowcom/cantos/issues/627); this record does not close it.

## Invariant and ownership

A local file, a prepared request, an admitted upload and a selected cover remain distinct facts.
ArtworkForm emphasizes at most one next explicit action without changing its existing guard,
callback, request, ownership, receipt, revision comparison or recovery. A confirmed empty cover
is stated once, compactly; unreadable/forbidden and preview failure remain distinct from empty.

Base: `develop@1f9116b09311d3b4c0bbc2c31ed2540310541db9` (PR #719).
Final rendered source: `e3422c6a29df0eeb42f3bbfeffda5704c0457392`, with clean build inputs; this evidence document was untracked at build time.
The issue's historical capture has unknown runtime provenance and was used as a lead only.
The source delta is Studio Web, its existing copy owner and a reactive variant input in the
existing shared Leptos Button/Studio ActionButton. No new variant, token, image asset, state
model, service contract, migration, dependency or Studio mobile consumer is introduced.

The shared native button remains mounted when its regular variant changes. Static callers
remain source compatible. Auth geometry remains a mount-time selection, as documented by the
[Leptos owner](../../packages/ui-system/leptos/README.md). CMP already accepts its variant as a
recomposable value; its implementation is unchanged. Generated Theatre changes are catalog
fingerprint headers only, regenerated from the canonical catalog.

Behavior SSOT impact: no semantic change. The reverse-index preflight derives only
`impact:unknown` because the catalog path conservatively broadens selection. The ignored
assurance decision classifies presentation/copy and outcome-preserving variant plumbing under
this issue; no model or requirement is changed.

## Choice and copy roles

Three directions were considered: compact empty-first; persistent current/replacement
comparison; and a progress summary with optional metadata. The implementation combines the
compact empty-first case with comparison only when there is current content, plus a short
workflow line. This retains familiar file/step controls and avoids a new wizard or automatic
mutation chain. Replacement uses the existing surface role; current uses spacing without a
second decorative frame. Grid items align at their start so a nearby disabled reason cannot
stretch the primary button.

Essential copy remains visible: format/size, required alt/provenance/rights constraints,
private/not-published semantics, prepared expiry and immutable declarations, server/field
errors, disabled reasons and revision conflict/recovery. Only current image technical facts
and its asset/revision identity are in a native details disclosure. The selection decision
still names the currently loaded Work revision. Required fields remain outside disclosures.

Theatre/shared inventory: Theatre's public cover uses an authorized public image or its textless
AVIF fallback (`features/public_home.rs`, `features/public_cover.rs`). That decorative discovery
fallback is unsuitable for declaring a Studio private cover successfully read or selected.
Studio retains CoverView Shown, PreviewLoading, PreviewUnavailable, Empty, Loading and
Unavailable from the existing owner, plus its separate stage badge. No public fallback bytes
are imported. Existing ActionButton/Button, TextField, Badge, SurfacePulse and clear dialog
are reused. This is source comparison; no live Theatre catalog was exercised.

## Local evidence

Commands run from the worktree with Rust 1.99.1, Trunk 0.21.14 and Python 3.13. Logs and pixels
are local-only under ignored `artifacts/agent-runs/issue-627/` and
`artifacts/ui-evidence/issue-627/`.

- `cargo test --locked -p cantos-studio`: PASS, 477 tests plus 5 doctests.
- `CARGO_INCREMENTAL=0 cargo test --locked -p cantos-studio live::artwork`: PASS,
  29 focused tests after the final catalog and copy additions. The hierarchy regression failed on
  the old implementation before the patch, then passed. Existing tests cover exact receipt,
  wrong owner/work/revision, rejected files, unknown replay/reconciliation, expired intent,
  conflict, immutable bytes and upload not selecting a cover.
- `CARGO_INCREMENTAL=0 cargo test --locked -p cantos-ui`: PASS, 170 tests plus 5
  doctests, including retained reactive variant projection through caller cleanup.
  An earlier full run exhausted disk; only this worktree's incremental directories were
  removed and the complete suite was rerun successfully.
- `CARGO_INCREMENTAL=0 cargo clippy --locked -p cantos-studio -p cantos-ui --all-targets -- -D warnings`:
  PASS. Final Web lint also covers the later preview scale input.
- `CARGO_INCREMENTAL=0 cargo xtask verify web`: PASS for Web lint, Studio build,
  Theatre build and shared UI preview, after final Rust/resource changes, including the localization handoff repair.
- `CARGO_INCREMENTAL=0 cargo xtask build cantos-studio --web`: PASS at final rendered
  source above, after the CSS-only action-height correction; the later complete Web gate also covers
  the final resource-parameter repair.
- `python3 scripts/localization_catalog.py --resources --check`: PASS at handoff; catalog changes were regenerated through the canonical owner.
- `python3 -m unittest discover -s scripts -p 'test_localization*.py'`: PASS, 89 tests,
  2 explicit skips. No new locale or formatter behavior is added.

L1 source review: PASS for existing owner boundaries and unchanged authority.
L2: no domain/type-policy change; UI variant representation alone becomes reactive.
L3: native render/unit tests and static checks above PASS.
L4: existing controlled form and injected facts render correctly; no actual upload flow is
exercised by the fixture's inert callbacks.
L5 browser presentation: bounded results below; real HTTP/auth/private storage is NOT_RUN.

### Browser provenance and observations

Baseline was freshly built from the pinned base with `cargo xtask build cantos-studio --web`.
Candidate was built from the pinned final source above. `before-build.json` and
`after-build.json` retain exact distribution hashes; `matrix.json` records actual CSS
viewport/DPR, document font, locale, cover classification, button variants/availability,
field values and geometry. Every final image listed by the matrix was opened with the image
viewer. The focused large-text image was also opened directly from the browser.

Browser: ChatGPT in-app browser on macOS; engine/version is not exposed by this interface.
Rendering: CSR development build, synthetic debug recipe `?preview=studio-artwork`, loopback
static file serving on 8627/8628, no Server, Identity, HTTP mock or storage adapter. The fixture
SVG/data URL represents synthetic pixels, not policy-admitted media or an authenticated preview.

The 16-image final matrix covers:

| Scenario | CSS viewport | Theme / locale / font scale |
| --- | --- | --- |
| empty, local replacement, prepared, admitted, selected | 1440 × 900 | light / vi-VN / 100% |
| empty, conflict, non-owner, invalid-file | 390 × 844 | light / vi-VN / 100% |
| prepared, unknown | 390 × 844 | dark / en / 100% |
| loading | 390 × 844 | dark / vi-VN / 100% |
| private preview unavailable | 1440 × 900 | dark / en / 100% |
| admitted | 320 × 844 | dark / en / 200% |
| empty | 320 × 844 | light / vi-VN / 200% |
| selected | 768 × 900 | dark / en / 200% |

Observed: one localized confirmed-empty message, compact current area, obvious labeled native
picker, distinct current/replacement containment and only Prepare, Upload or Select emphasized
as their existing facts permit. No primary mutation is emphasized during unknown or non-owner
scenarios. Conflict keeps submitted/current r5/r7 visible and the existing reload action.
All sampled artwork descendants stay inside the artwork region (zero horizontal outliers).
The debug selector/header outside this issue's region can overflow; full-page image width is
therefore not the CSS viewport. The manifest records both rather than treating those as equal.
The injected 200% root font is 32px; this is text scaling, not browser zoom or OS acceptance.

Keyboard/pointer checks: native details opens/closes with Enter while focus stays on SUMMARY;
clear confirmation opens, Escape closes and returns focus to `artwork-clear`; a Vietnamese
alt draft with literal `{time}` survives locale/theme changes. At 320px/200%, the enabled
Select button retains visible focus and readable wrapped text. The native filechooser was
attempted with a synthetic AVIF fixture, but its final selection could not be verified through
the browser interface; no picker end-to-end, touch or upload acceptance is claimed.

## Deferred acceptance and residual risks

Real owner sign-in, local decode through the live panel, prepare/PUT/admission/select/private
preview, persistence, real timeout/unknown/conflict recovery, native picker touch behavior,
Safari/Firefox, screen readers, OS IME, actual browser zoom, measured contrast and motion are
NOT_RUN. Resume on an authorized local/QA backend with a synthetic valid AVIF and registered
rights, using `cargo xtask run cantos-studio --web`; verify each explicit action separately,
replay the same ambiguous operation and confirm revision guards and private preview access.
Next owner is unassigned; assignment is required before full issue acceptance. This PR grants
no deployment, storage publication or production acceptance.

Repository/preflight is FAIL with 65 stale shared-preview qualification/admission findings.
The exact same set is reproduced by `python3 scripts/check_repository.py --base HEAD` on a
clean worktree of the pinned base. Five newly introduced localization literals were repaired
through their resource owner; the final comparison contains zero added findings. The overall
preflight is not represented as passing. Broad unrelated workspace/backend/CMP/audio gates
are NOT_RUN; scoped Studio/UI tests and the complete Web gate above were run independently. No qualification manifest, protection or test is weakened here.
