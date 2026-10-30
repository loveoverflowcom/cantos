# Me preferences self-audit and simplification — PR #646

Codex, 30 October 2026, one automated reviewer. This is the owner's latest **Web-only**
amendment to [the compact preference checkpoint](theatre-preferences-646.md).
`origin/develop` was fetched again and remains `aa0bec8dd770305753aa8212b40bd87ea0afd79a`.
Before: parent `e2c111c8`, staged compact-preference tree
`9174e4f6053e0f731c53c7e3a034e6319c514445`, with no unstaged source at that checkpoint.
After: the two app-owned changes below; final committed source/check ledger is recorded in the PR.

## Contract and component findings

Invariant: remove unnecessary preference chrome while retaining two discoverable, labelled,
full-sized controls, their actual values, existing chooser/focus behavior and root ownership.
The source snapshot in the new prompt predates the current compact rows: ThemeCycleButton is
already absent, both current entries already use Select, and Settings already aliases Me.
The actual mounted files are `features/account.rs` and `styles/account_me.css`, with the latter
scoped over the retained `theatre.css` compatibility styles. No shared-token change is needed.

Local live production Theatre Web `#account`, Chromium/DPR 1, healthy guest over the native
loopback Server/Identity host, no published audio or signed-in profile. Default/open/focused
controls were inspected separately before relationships; supplied drafts are advisory, not a
pixel baseline. Actual account fixture and capture metadata remain explicit below.

| Component | Actual observation and result |
| --- | --- |
| Me layout | Inspected: existing two-column desktop / stacked narrow layout correctly keeps profile before preferences before Membership. Keep its owner and geometry. |
| Guest/profile | Inspected: contrasting profile surface, 40px borderless avatar, two real auth actions, no removed guest-info footer. Light profile fill is `rgb(41, 19, 50)` before and after. Actual signed-in browser state is NOT_RUN; existing SSR phase/signed-in tests pass. |
| Preference container | Inspected: **P3 visual economy finding**, high confidence. An extra plain Surface adds 24px inset on every edge around two already-tonal rows, plus an 8px redundant top margin. The region measures 189px high at 1280px normal text. DOM/styles confirm the cause; it is an app composition concern (`UI-INTENT`, existing semantic spacing/surface roles). |
| Theme | Inspected: all Light/Dark/System choices are explicit, selected System remains identifiable, icon follows the selected mode, value/chevron are readable, row/trigger meet the existing 48px minimum. Keep Select and its focus state. A cycle-only control would make the complete choice less discoverable. |
| Language | Inspected: full Tiếng Việt / English autonyms, independent action and unchanged chooser. English → Vietnamese survives actual page reload through the existing locale store. Keep its root callback and warning mapping. Browser storage-denial/fault injection is NOT_RUN; no durable-success message is added. |
| Membership / neighbors | Inspected: truthful unavailable content and CTA remain unchanged, no duplicated Settings link is mounted. Moving only the preference chrome brings Membership up while preserving its dimensions and text. |

The found defect is unnecessary visual nesting/space, not a domain result or an auth failure.
All six component regions were inspected in the named guest states; authenticated/browser AT,
actual zoom and other states below remain partial/unrun. No whole-screen acceptance is inferred.

## Alternatives and selected repair

| Candidate | Judgment |
| --- | --- |
| A — two independent compact rows | **Selected.** Clear labels/current values, natural reading and Tab order, same 48px targets; robust to VI/EN and enlarged text. Removing the parent card reduces chrome without shrinking the controls. |
| B — one small tonal toolbar | Rejected here: grouping both utilities into one toolbar weakens their independent boundaries; a theme-cycle shortcut hides direct System choice. Existing selectors already supply complete interaction semantics. |
| C — two selectors beside each other on wide layouts | Rejected here: the right column is only 528px at the inspected 1280px viewport. Two labelled icon/value controls would divide that capacity, need earlier reflow and offer less resilience to longer autonyms/200% text. A shorter panel alone is not a better interaction. |

Implementation: replace only the outer preference Surface with the existing labelled semantic
section and set the existing row group's block margin to zero. Keep both tonal rows, canonical
icons, inline labels, selected values, Select IDs/options/callbacks, responsive text reflow and
visible functional focus. No extra card, selector, state owner, resource or global variant.
`account.rs` and `account_me.css` own the repair. No CMP, shared component/token, guest-card fill,
Membership content/state, route grammar, authentication or persistence behavior changes in
this amendment. Earlier authorized CMP work remains intact in the cumulative PR.

## Before/after measurements and interactions

Same local host, 1280 × 800 CSS px, VI, Light, root font 16px, actual guest:

| Measurement | Before | After |
| --- | --- | --- |
| Preferences region | 528 × 189px | 528 × 133px |
| Parent preference padding | 24px | 0px |
| Each real row | 480 × 48px | 528 × 48px |
| Membership top | 369.1875px | 313.1875px |
| Membership size | 528 × 219.984375px | identical |
| Guest profile size/fill | 480 × 333.390625px, `rgb(41, 19, 50)` | identical |

The region is 56px shorter (about 30%); controls retain their height and gain text capacity.
The same 133px group / 48px rows were observed at actual 390 × 844px, with no horizontal
overflow (client width and scroll width both 375px with the visible scrollbar).

Matched 390px Dark/EN default, opened Language popup and post-Escape focus captures were opened
before/after. The existing popup anchors to the actual control; Escape closes it and leaves
`theatre-account-locale-trigger` focused. Selecting Tiếng Việt also returns focus there and
updates document/UI locale; an actual reload retains Vietnamese. Theme System was selected in
the real three-option popup; outside-click closes without changing it; Light and Dark choices
both update their actual values. Existing Web theme state remains root/in-memory and reload
starts System; no new persistence promise is made. There is no duplicate field or Settings entry.

At actual 320 × 800px, root font 32px, System resolved Dark / EN, client and scroll widths were
both 305px. The 241px-wide unframed group reflowed its two rows to keep Appearance, Language,
System and English whole and reachable. Final group height was 574px; each expanded-text row
was 250px. A functional keyboard perimeter remains visible, without a decorative selected ring.
The temporary HTML text style was restored byte for byte (SHA256
`d583467a64e6afef7f87ae36a184b508b68954fa1b49aca0ecf93492e65bc698`). Final normal font was
rechecked as 16px. This is app text enlargement, not browser zoom; viewport overrides and owned
tabs/processes were cleaned up. No content, credentials or enrollment was submitted.

## Opened images and checks

All ten images below were opened with the image-capable browser tool; ignored local directory:
`artifacts/agent-runs/646-preferences/`. Full default captures use the same viewport/state in
each pair; popup/focus captures are actual viewport images with browser-owned scroll positioning.
There is no normalized pixel-match or metric approval claim.

| PNG | SHA256 |
| --- | --- |
| `self-audit-before-light-vi-1280.png` | `26d649b9c0bff8a15b88c3cfa3d223dc54794f34c3c07337e30aa2d902ef416b` |
| `self-audit-after-light-vi-1280.png` | `8911d23a7dd3122a4f899b9f42519b71d7519fff7efbe61a50f83ace56ac9f0f` |
| `self-audit-before-dark-en-390.png` | `138c47e3e21c589acba77025ff74e4f8b5b03ac2a2e7519d95e350c55a30bf38` |
| `self-audit-after-dark-en-390.png` | `957e5001c7e0e3a9706d7c590d88df2bcf713c8c1a1edf8eb012414f36a5e1ec` |
| `self-audit-before-dark-en-390-open.png` | `e833c2378bc6a5b516704fef62e8c036bd024d10c80cfeaf12da9bf137b9c8db` |
| `self-audit-after-dark-en-390-open.png` | `2ddedb756cd73b76653bf94818e93b94a0d0bded64bb572d86039453e79def17` |
| `self-audit-before-dark-en-390-focus.png` | `e147315d3cba424d323914dc76ee940889e8eb6efc24ef6a9c85b3a96635b2fb` |
| `self-audit-after-dark-en-390-focus.png` | `410a3a0c8111292412f970d59b58df57fbfc22dae8efcf1181485f1f700d83e0` |
| `self-audit-after-dark-en-text2-320.png` | `97500f729edd9ec5f6b9ed833b3493019c523bef3c4aff7669dc6367bf61697c` |
| `self-audit-after-light-vi-390.png` | `32c68f417d1b2cca281231d1d961fff3d7b289105b5dcfc513f9474913d27f51` |

PASS: `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-theatre --features preview
features::account::tests`, 16 passed, zero failures/skips, final `account-tests-self-audit.log`.
Existing exact phase/recovery, signed-in composition, single chooser/autonym/value and
preference-before-Membership tests retain their original purpose. Pure markup/CSS economy uses
the foundation's inspected-visual exemption; no test mirrors a wrapper deletion.
The prior compact checkpoint's full preflight PASS remains attributable to its inputs; the
latest Web-only delta receives a separate final-diff handoff preflight, reported in the PR.
Unchanged shared renderer qualification/CMP evidence remains valid; no hashes were refreshed for
this app-only repair. Exact shared source digests remain in the current paired records.

Layer ledger: L1/L2 ownership, typed callbacks and inward boundaries unchanged; L3 focused SSR
checks PASS; L4 existing Select behavior reused; L5 interaction-tested / screenshot-inspected /
cross-theme-inspected within the local guest cases above. Behavior SSOT class: no semantic
change, presentation-only; existing draft capabilities and their outcomes/lifecycles unchanged.
Performance claim is only measured geometry; no startup/cache/bundle/timing claim. No dependency,
shared API, model, backend, durable schema or ADR change is needed.

## Separate handoff test timing repair

The first handoff attempt after this Web delta (`preflight-handoff-all.log`) passed repository,
model, format, references and lint, then reported **FAIL** in workspace tests at
`seed::sample::tests::sample_operation_lock_spans_the_output_directory_lifecycle`: immediate
reacquisition after `drop(first)` returned an error. Later gates were **NOT_RUN**, and required
assurance consequently failed. That output is retained; a later PASS does not erase it.

Before any repair, the exact focused case passed (one test), and the complete parallel xtask
binary passed (708 passed, five ignored) on the unchanged source. Its sample-lock source was
identical to the pinned base; no clean-base full campaign is claimed. This establishes an
intermittent observation, not a Web regression. An independent ignored Rust 1.98.1 fork probe
then reproduced the relevant mechanism on the same macOS host: dropping the parent's File
still left a contender blocked while a forked child retained its inherited descriptor;
acquisition succeeded after the child closed it. See `fork-lock-probe.log`. The exact failing
spawn was not traced, so attribution to a particular parallel test remains an inference.

The [standard File locking contract](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
releases the lock only after all duplicated/inherited handles close (or explicit unlock).
The final assertion's immediate-parent-close assumption is therefore stronger than that API
contract in a suite that spawns processes. The existing test now observes release for at most
two seconds, sampling every five milliseconds and failing with the actual acquisition error
if release never occurs. The before/after-directory-rename live-owner refusal assertions stay
immediate. Production locking, ownership checks, path/symlink refusal and reset behavior are
untouched; there is no unlock shortcut, retry policy or test skip in production.

PASS after this test-only repair: full parallel `cargo test --locked -p xtask --bin xtask`,
708 passed, zero failures, five ignored (`lock-full-parallel-fixed.log`). Final cumulative
preflight is run again with the repair and this report staged; its actual result is recorded
in the PR. The original failing run and all focused/parallel/probe outcomes remain separate.

The next complete attempt (`preflight-handoff-final.log`) passed that lifecycle case but exposed
the same immediate-close assumption in the adjacent archive/reset fixture: after dropping its
database/snapshot locks, archive still reported an owner. This was **FAIL**, with later
gates **NOT_RUN**, not a passing handoff. The fixture now calls the standard `File::unlock()`
on its own two handles after each active-owner refusal assertion, before dropping them and
expecting archive success. This releases only the fixture's shared lock description, not an
independent owner; archive/production code remains unchanged. The lifecycle/drop test retains
its separate bounded eventual-release observation. All database/WAL/snapshot preservation and
active-owner refusal assertions remain in place. The full parallel xtask binary passed again
after this second fixture correction (708 passed, five ignored;
`lock-full-parallel-reset-fixed.log`). The final PR ledger names the subsequent exact-source run;
both earlier failing attempts remain available.

Residual risk: actual authenticated browser layout/data, denied browser storage with error/retry,
browser zoom, screen-reader walk, OS touch/IME/Back, live audio and deployed acceptance remain
NOT_RUN. Existing deterministic CMP refusal/retry and Web warning/source coverage are narrower
evidence, not those browser/device results. No merge/deploy/distribution was performed. The
broader #641/#527 acceptance remains partial as described in the PR.
