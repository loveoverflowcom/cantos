# PR43 integrated CMP preview acceptance

On 2026-10-12 the requester explicitly authorized review, local execution, fixes and merge of
[PR43](https://github.com/loveoverflowcom/cantos/pull/43). This record accepts the bounded
shared UI-system **desktop preview implementation**. It grants no new component adoption,
Android/iOS acceptance, playback capability or production qualification: **0/29 qualified**.

## Scope and freshness

Original three-dot review: merge-base `7f416a1e1bac5bbf338d461463770bffd983b5c1` to PR head
`84eb0ca251a89a2f62d9bf9fc636df849dc7a95e`, all 39 changed paths. Develop/PR42
`342d65fdbd3764f76b772ee0a398a9e209115b65` was integrated in merge commit
`858c4ee35c5059b2e79afa781986441c3d398ba0`, retaining the three original commits. Both conflicting
generated catalogues were regenerated from the merged JSON source; the existing Theatre import
baseline follows its already-reviewed move to `apps/cantos-theatre` without admitting a new API.
The regression fix is `532a07dee1b0d6dd632405cf81462fde9259f9c1`, tree
`eb55da365b15e838baa4c6c645bb59b9738067bb`. Later changes in this acceptance handoff are docs only.

The [session-3 inventory](cmp-ui-system-readiness.json) remains historical evidence for the
original PR head. Its source hashes are not attributed to this integrated execution. Current
`python3 scripts/ui_preview_catalog.py --digests` output for the tested code is:

```json
{
  "tokens_sha256": "e9713d5ad276be53bf3a63b6ce72ad04d1dbf0b5918cbb759a39ea7c2c1f39a2",
  "source_sha256": {
    "cmp": "a6a20b9237624f01246ee89e11dd88446324dcaf8e177284f0e2a78266a0302c",
    "leptos": "393b0e9b5aeae2a6ea1bd2852f5dbd6981f07d2182390e98cd297bacc442fe18"
  }
}
```

Review covered CMP common/JVM APIs, production callers and tests; Leptos diagnostic specimens
and SSR tests; the admission generator/checker; generated outputs and their source; all changed
evidence and work-plan claims. Backend/worker/persistence/contracts implementation and mobile
product code are unchanged relative to current develop. Durability, publication and listening
passes are not applicable to this preview-only diff. No new dependency or canonical token value
was introduced. The unrelated dirty speech-clip-reuse checkout was preserved in place.

## Defect and final disposition

**[P2] Equivalent rebuilt Select options close an open menu** — class: confirmed defect;
category: logic; confidence: confirmed-from-source, then reproduced on the JVM renderer.
`Select` keys its local menu state by the option values. `DisabledReason` previously had reference
equality, so a parent recreating the same disabled option on an unrelated recomposition reset
that state. BASE did not key menu state by options, making this an introduced regression.

The fix gives the immutable validated reason structural `equals`/`hashCode` by text. Its private
constructor and nonblank factory remain intact; no data-class `copy` construction bypass was
introduced. Independent re-review found no actionable issue in the fix.

Production-wrapper regression
`InteractionComponentTest.select_preserves_open_menu_for_equivalent_rebuilt_options_but_closes_on_availability_changes`
first **FAIL**ed at line 190 when the popup disappeared after an unrelated parent update. After
the fix it **PASS**ed, including real reason changes, Disabled-to-Loading interruption and zero
spurious callbacks. Full CMP suite: **75 executed, 0 failures/errors/skips** (25 common,
39 desktop semantics/composition, 11 capture methods). Final preview assessment:
**no-actionable-findings**, complete-for-declared-scope; this is not a production certificate.

## Assurance ledger

| Claim | Owner / target | Required by | Fresh evidence / disposition |
| --- | --- | --- | --- |
| Nonblank names/reasons and stable-value choices reject invalid construction | ComponentState / CMP common | foundation types-as-proofs; CMP conventions | Private constructors/init checks and actual contract tests; held |
| Disabled/loading callbacks are blocked; readonly rich text retains selection | Actions/Fields/Choices / CMP common/JVM | shared UI state/accessibility rules | Production-wrapper semantics/input tests, exact callback outcomes; held for JVM, OS IME deferred |
| Finite bounded range requests close over native ticks and complete actual changes | Choices / CMP common/JVM | work item 016 | Core tables plus production semantics/keyboard tests; held |
| Equivalent options preserve menu; actual availability changes interrupt it | Select / CMP common/JVM | controlled-state contract in CMP README | Failing-first regression and same-scenario PASS; held after fix |
| Nested scale, native tab/dialog focus, wrapping layout and opt-in announcements | Theme/Navigation/Layout/Feedback / CMP JVM | UI-system rules and work item 016 | Actual compositions, semantics, keyboard and inspected pixels; held within the named renderer |
| Both catalogues, named tokens and guarded app adoption stay consistent | shared source/checker / Web+CMP | preview admission contract | Regeneration, repository tests, unchanged token digest, qualifications remain empty; held |
| New app adoption requires both runtime/accessibility/reference qualifications | shared admission boundary | preview admission contract | No adoption added and 0/29 qualified; held as a fail-closed boundary |

## Actual commands and environment

All commands run in the isolated managed worktree on this Mac, one heavy lane at a time, with
Rust/Cargo **1.87.0**, Gradle **8.14.3**, full Microsoft JDK **17.0.19+10-LTS**, pinned
Kotlin/compiler **2.2.20**, Compose **1.9.0**, stable Material3 **1.8.2**, Trunk **0.21.14**,
and Homebrew Python **3.14.8**. Rust 1.87 with rustfmt/clippy/WASM was provisioned through
rustup; the official Gradle archive was verified against its published SHA256
`bd71102213493060956ec229d946beee57158dbd89d0e62b91bca0fa2c5f3531` and extracted under
`/private/tmp/cantos-pr43-tools`. Existing JDK/caches were used. No TLS or OS security preference
was changed. `TMPDIR=/private/tmp`, Cargo two jobs and debug-info-free local build profiles bound
resource use. Ignored raw logs/XML are under `target/pr43-acceptance/`.

| Command | Result / evidence | Local log |
| --- | --- | --- |
| `cargo xtask ui-preview cmp verify` before fix | FAIL, one new regression; not passing evidence | cmp-failing-first.log; failing-first-results/ |
| `cargo xtask ui-preview cmp verify` after fix | PASS, 75 executed; example-tested, semantics-tested and JVM interaction-tested | cmp-fixed.log; CMP build/test-results/desktopTest/ |
| `cargo xtask verify rust` | PASS rust-format, rust-lint (workspace), rust-tests (workspace) | rust.log |
| `cargo xtask verify contracts` | PASS script-ir-reference, contract-tests, wav-reference | contracts.log |
| `cargo xtask verify web` | PASS web-lint (Studio/Theatre), web-build (both), ui-preview | web.log |
| `cargo xtask verify repository` | PASS repository; 195 Python tests, links/JSON/skills and diff checks | repository.log |
| `cargo xtask preflight --changed-only --base origin/develop` | PASS, all 9 selected gates, no skips | preflight.log; preflight-final.log |
| `cargo xtask ui-preview cmp run` | MainKt launched; supported UI binding blocked as described below | cmp-runtime.log |
| `cargo xtask ui-preview leptos run` | Local Chrome interaction observations below; owned runtime stopped with exit 0 | leptos-runtime.log |

Preflight selected repository, rust-format, rust-lint and rust-tests for Studio/Theatre/UI,
web-lint, both app web-builds, ui-preview and cmp-ui-preview. The final committed documentation
is checked again with the same plan before publication; the CMP result may be Gradle UP-TO-DATE
because the fresh 75-test execution above already covers the unchanged renderer inputs.

PostgreSQL and mutation are NOT_RUN: this UI-only delta changes neither persistence/HTTP host nor
Script IR validation/encoding. Remote CI is not checked and is not substituted for local evidence.

## Rendered and native observations

All **26/26 fresh JVM PNGs** were opened and inspected. They live in the ignored
`packages/ui-system/cmp/build/reports/cantos-preview/`. The SHA256 of the sorted
`<filename> <sha256>\n` set is
`cf474edcd57ad3a592574798edbb7119dc04d21b095398e1a62731b7ae35e951`; source was the tested fix
above. Root sizes: 1140×840 and 372×840, density 1, host fontScale 1, preview scale 1×/2×,
light/dark and compact/comfortable as named in each file. The diagnostic surface has low
expressive intensity and caller-owned synthetic Vietnamese copy. No new production locale key
or reference/font resource was added.

Observed: composed action/icon foregrounds, field focus rings, raw `1,25`, blank errors and nested
2× theme; complete long Vietnamese tab labels, loading/checkbox labels and negative-range text
at narrow 2×; reading/grid/recovery/Select specimens without observed overlap. No actionable
pixel defect was found. These are screenshot-inspected and bounded cross-theme/viewport results.
The narrow whole-preview image shows its upper tools only; the token image shows initial roles.
Neither establishes lower-page reachability or all 73 effective token roles.

The supported xtask launch reached the real desktop MainKt process, which appeared in app
inventory. CUA binding by observed ID `net.java.openjdk.java`, name `MainKt` and actual Java
executable path each returned **Invalid app**. No OS pointer/keyboard/resize input was sent.
SIGINT stopped only the owned xtask process; xtask/launcher/MainKt PIDs 58609/58638/58922 were
confirmed absent afterward. This is one observed shutdown, not a proof of every lifecycle path.
JVM injected input/focus assertions remain distinct from OS-window observations.

The counterpart Leptos gallery was also launched through xtask and exercised on local Chrome,
CSS viewport 2031×847. Native text input retained `Người dẫn chuyện · Ỗ, Ữ`, with UTF-16 selection
23–23 and one Enter request. Switching to readonly and attempting replacement preserved the
text and Enter count. Dialog text `Nguyễn Thị Minh Phương · Ỗ, Ữ` survived Escape and reopen;
Escape and CloseButton reported their distinct reasons and AX focus returned to the opener.
These are bounded browser interaction observations, not active Vietnamese IME composition.
The additional Tab-loop probe was inconclusive (no identified focused element; black capture),
so no browser Tab-containment or pixel result is claimed. The task-created tab was closed and
SIGINT stopped owned xtask/trunk PIDs 72296/72334 with xtask exit 0.

Residual risk: OS-window input/focus/resize and real Vietnamese IME remain blocked; screen-reader
walkthrough, motion/reduced-motion runtime, Noto delivery/metrics, full same-state reference and
complete state/token matrices remain NOT_RUN. Android/iOS product apps remain unsupported guides.
Those gaps continue to block production component qualification, while this bounded preview
implementation can be merged with its local regression/gates recorded separately.
