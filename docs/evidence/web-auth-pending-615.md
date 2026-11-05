# Web sign-in pending presentation (#615)

Status: implemented presentation repair with scoped local browser evidence, 5 November 2026.
Real Server/Identity authentication, production deployment and complete accessibility acceptance
remain separate. Authority: [issue #615's 5 November owner decision](https://github.com/loveoverflowcom/cantos/issues/615).

## Boundary and provenance

The branch starts at `origin/develop@aa97e0a3c1c573a4ba9c1818fa8e4212cc80511a`.
The PR binds the final source revision and preflight results. Before captures use that base's
runtime code; after captures use this working diff, development CSR builds through `cargo xtask`.
The browser is Codex's in-app browser, DPR 1; its engine/version was not exposed by the
available browser API. No other browser engine or production origin was qualified.

Theatre ran through `cargo xtask app cantos-theatre --web` on loopback port 8082. Studio was
built with `cargo xtask build cantos-studio --web`; its default 8081 port belonged to an OS
process, so a test-only static host served that exact bundle on 8181. Both builds set
`CANTOS_API_ORIGIN=http://127.0.0.1:8080`. A loopback test transport held responses until the
driver released them. It discarded request bodies and recorded method/path only. Synthetic
session facts enabled the reauthentication scenario; this is **mocked orchestration**, never
proof of a real login, cookie retention, roster admission or private API authorization.

Images, DOM measurements, transport and full command output are local-only under ignored
`artifacts/agent-runs/615-signin-pending/`. No credentials, real account data or screenshots
are committed. The fixture address uses `example.invalid`; no real account was submitted.

## Claims and evidence

| Claim | Regression / observation | Scope and result |
| --- | --- | --- |
| Healthy Theatre guests have no redundant status banner; checking is not login submit | `pending_login_feedback_is_absent_for_healthy_guests_and_bootstrap_is_not_submit` failed on the base; state/HTML tests and actual held session read | Example-tested; bootstrap uses a separate shared spinner, primary is disabled without busy/submit copy |
| Only explicit Theatre submit makes the primary busy | `only_an_explicit_sign_in_intent_makes_the_primary_button_busy`, navigation intent regression, held browser request | One spinner in primary; secondary has no spinner/busy state; no visible pending banner |
| Studio has one busy control, with stable entry form | `pending_login_marks_only_the_submit_button_busy_in_entry_and_reauthentication` failed on the base (three busy controls); `AuthDisplay` regression and held browser request | Fields become read-only; only primary is busy; entry stays visible with its original heading |
| Reauthentication remains visible while private workspace is blocked | `explicit_sign_in_keeps_its_form_while_session_observation_blocks_private_work`, actual account-menu submit | Private workspace has `hidden` and `inert`; entry panel stays hidden; account form remains mounted with one spinner |
| Cancellation and late responses retain their fences | Existing navigation/session/sign-in tests; entry cancellation followed by released synthetic success | Returned to sign-in, password empty, private workspace hidden, no spinner; stale session ticket cannot clear a newer form |
| Essential feedback survives | Existing exact failure mapping/form tests; browser 401 refusal, 403 Theatre refusal, Studio 429 with 17-second copy, network disconnect, independent bootstrap | Spinner ends; failure/recovery stays reachable; no password replay |

Both Web unit suites passed after the repair. The PR records the final handoff command and
selected gate results; compilation and string-render tests do not establish browser interaction.

## Opened captures and measurements

The following original PNGs were opened and inspected, rather than inferred from file existence:

- `theatre-before-pending.png`: visible checking banner plus repeated checking reasons under
  the primary/secondary actions. `theatre-after-pending.png`: only the primary spinner,
  legible disabled tone, stable form, clear secondary label and focused password field.
- `studio-before-pending.png`: sign-in form replaced by checking heading/panel.
  `studio-after-pending.png`: original heading/form remain; one primary spinner, no checking
  panel; cancellation remains available below the form.
- `studio-reauth-final.png`: reauthentication stays in the account menu; transient disabled
  reasons no longer push it down; the private workspace is hidden and inert.
- `theatre-320-light-vi.png`, `studio-320-light-vi.png`: actual 320×740 CSS viewport, Vietnamese,
  Light; primary label/spinner fit, password focus visible, no horizontal overflow.
- `theatre-390-dark-en.png`, `studio-390-dark-en.png`: actual 390×740 CSS viewport, English,
  Dark; readable labels, one contained spinner and no horizontal overflow.
- `theatre-network.png`: a real browser transport disconnect ends progress and retains the
  error plus read-retry action. This is a controlled local failure, not a production outage.

At 1280×720 CSS pixels, Theatre's primary stayed at document position `(664.5, 541.5)` with
size `512×60`; Studio entry stayed at `(676, 414.8125)` with size `464×60`. Each form's
height stayed 228 CSS pixels through explicit submit. Reauthentication retained the same
form/CTA positions in the popover's scroll content (`224` / `507.1875` CSS pixels), with a
`281×60` CTA. Focus-induced popover scrolling is measured separately from layout movement.
Keyboard Enter submitted once; a second Enter while pending did not dispatch another login.
The captured opaque foreground/background RGB pairs yield disabled-label contrast ratios of
5.38:1–7.98:1 across the sampled Light/Dark states (`images.json` and per-state DOM records).

## Layer and acceptance limits

- L1/L2: presentation projections reuse existing intent/form/session facts; no service,
  dependency, persistence, contract, session authority or immutable record changed.
- L3: native Rust unit/string-render evidence; exact failure mappings, duplicate-submit and
  stale-response guards remain in their existing owners.
- L4: actual compiled Web apps, trusted browser keyboard/pointer input, synthetic held HTTP
  transport; opened screenshots and DOM/CSS geometry. No invented application state injection.
- L5: real API/Identity, cookie, roster, database and deployed acceptance **NOT_RUN**; #565
  retains real Studio authentication acceptance. Native/CMP was outside this change.

The behavior catalog selects `cantos.presentation.guest-navigation` and
`cantos.presentation.discovery-retired-language` (both draft). Classification: presentation
change with preserved navigation/session outcomes; no model delta. The existing cancellation
ticket guard is retained at the completion boundary. Model checks remain separate from
implementation conformance and runtime acceptance.

Initial preflight probes selected Apple's Python 3.9, which lacks the bounded runner's
`os.waitid` and cannot import the localization checker's type annotations. The final command
selects the already-installed Python 3.14.8 and isolated pinned Node 24.19.0/npm 11.9.0.
No runner guard or tool source was changed; the PR reports the resulting full preflight.

The initial repository gate failed on the integration base's stale paired UI qualifications.
The user subsequently authorized requalification and an unskipped preflight before merging
PR #723. [The new shared UI evidence](ui-requalification-615.md) preserves the original records,
records current renderer checks and opened observations, and admits LoadingState's new import.
The final unskipped gate ledger is recorded in the PR; the earlier skipped run is historical.

The full locale/theme/viewport cross-product, 200% text, reduced motion, screen-reader output,
OS IME, BFCache and other browser engines are **NOT_RUN**. Complete those scenarios with the
current build on the target browser before accepting all of AC-07/AC-10; next owner is
unassigned. Existing reduced-motion CSS and token color roles were reused, not newly qualified.
Theatre's empty-input click remains a no-op without additional validation feedback (existing
gap outside the pending repair); it starts neither a request nor progress.

No ADR was needed: #615 already selects the outcome. Queue ordering did not change.
Rollout is the normal Web artifact release; recovery is the previous Web artifact. There are
no migration, API/schema, cache/audio invalidation, provider-cost or rights/approval effects.
