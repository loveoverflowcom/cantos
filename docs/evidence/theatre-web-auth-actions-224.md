# Theatre Web auth action hierarchy

Scope: [#224](https://github.com/loveoverflowcom/cantos/issues/224), on the shared
action-icon/Forge branch. This record distinguishes the existing live composition's
navigation repair from icon candidates in the explicit no-network auth diagnostic.
It does not establish shared-component qualification or real identity acceptance.

## Intent and ownership

The listener should see the current enrollment step and one next action, with a reliable
contextual return and a quiet escape to Home. Auth/account has Medium expression under the
[UI system](../design/ui-system.md#authaccount-expression-scope); body copy and status remain
readable, and a navigation choice does not compete with the email/setup action.

Theatre's `features/auth.rs` and `features/signup.rs` own presentation. The existing signup
state, live transport, `LiveNavigation`, proof scrubbing, request epochs, return snapshots
and provider/session boundaries keep their authority. No backend, DTO, credential,
registration, cancellation request or session rule changes. No new dependency or ADR.

Component lookup: `button`/`Button` AuthPrimary, AuthSecondary and Text; `back-button`/
`BackButton` existing compatible presentation; `text-field`/`TextField` existing AuthTonal;
`icon`/`IconName` from the generated Material Outlined registry. No new shared control,
palette, wrapper, copied SVG or Select implementation. The #225 candidate Back/field/menu
variants remain outside adoption. The #228 icon candidates are passed only by the existing
`?mode=auth-preview` root; default live consumers retain their prior icon mapping.
Both-renderer admission and subsequent product icon adoption remain outstanding.

## Phase and action audit

All rows keep the named Home link in the top brand, replacing the footer Home button.
The link uses the existing Home callback and its cleanup. Every auth route retains exactly
one top contextual Back; the duplicate fallback Back and unconditional full-width Cancel
are removed. Register uses this Back for return-to-sign-in instead of another login button.

| Auth step / signup phase | Next action | Separate Cancel | Back / destination |
| --- | --- | --- | --- |
| Login, guest or blocked session | Sign in; create account is secondary; existing blocked reasons and cleanup barrier | none | origin destination |
| Register, availability checking/unavailable/failure | blocked Create account, or explicit configuration refresh | none | Login with the same origin |
| Register, Editing / SubmitFailed | Send verification email; form and entered email retained on failure | none | Login; existing local cleanup |
| Register, Submitting | Send remains busy/disabled with reason; no repeated dispatch | none | Login; invalidate late completion |
| Register, EmailAccepted | Explicit new request opens editing; never auto-resends | none | Login |
| Register, AwaitingConfirmation, SMTP proof | Confirm email and continue | none: Back already requests the same cancellation | Login; existing SMTP cancellation |
| Register, AwaitingConfirmation, Firebase proof | Confirm email and continue | quiet Text Cancel, with explanation; explicit Firebase revocation differs from Back | Login; discard local possession only |
| Register, Verifying | Confirmation remains busy; proof already consumed | none: cannot revoke an already-started operation | Login; invalidate late completion |
| Register, SetupReady | Open the one-shot setup link | none: no proof remains | Login; existing setup-link cleanup |
| Register, SetupLinkSent / SetupOpened | Informational result; top Back is the one return-to-sign-in action | none | Login |
| Register, Firebase VerificationFailed | Reconciliation help; top Back returns to sign-in; no confirm/retry-new shortcut | none | Login |
| Register, SMTP VerificationFailed / LinkUnavailable / CaptureFailed | Explicit new request opens editing | none | Login |
| RegistrationComplete reconstructed URL | unconfirmed status; no success/entitlement claim; same phase actions if retained | only the same unconsumed Firebase case | original Home/Account/Membership destination; a distinct sign-in action may remain |

Entry contexts are Home, Account, Membership and Membership-from-Account. `AuthRoute::back`
still resolves Login/RegistrationComplete to the typed origin and Register to Login with that
origin. Absent/malformed deep-link context still falls Home. Browser Back/Forward, Escape,
return-snapshot restoration and signed-out/session behavior are unchanged. The top Home link
keeps a safe escape even while auth hides the ordinary global navigation.

Preview icon map: sign in → `login`; create account → `person_add`; send/confirm email →
`mail` (an intent, never `check`/verified); configuration/session recovery and new request →
`refresh`; setup progression → `arrow_forward`; explicit cancel → `close`. Existing Back uses
`arrow_back`. Icons remain decorative and adjacent labels stay visible. Pending controls use
the shared progress cue/reason, preserving their names instead of displaying a success glyph.

## Focus, form and lifecycle

The form's native submit handler and shared non-submit click action remain intact. No custom
Enter/Space listener or second submit dispatcher is introduced. Signup phase changes select
the input, next action or status as appropriate; the initial render and preference changes do
not steal focus. The status can receive programmatic focus. A delayed focus callback resolves
only inside its own still-connected panel, so leaving/remounting cannot focus another route's
element with the same ID. Actual browser input is required to accept these mechanics.

## Evidence and acceptance boundary

The regression first failed against the incoming source: the Editing state rendered a Cancel
action despite having no proof to revoke. The fixed native test checks Editing, SMTP/Firebase
capture and proof consumption. A separate table drives legal signup transitions through all
phases and all four failure variants, asserting action identity/count, focus target and secret
absence in both locales with and without a contextual sign-in Back. Auth serialization tests
cover all four origins, unavailable fallback, the Home escape, icon isolation and reconstructed
completion routes. Native string serialization is structural `example-tested` evidence, not DOM
or browser evidence.

Source review covers the five changed Web source/style paths and the unchanged navigation,
signup state, cancellation adapter and browser-history consumers. No settlement, publication,
media, provider, PostgreSQL or contract invariant is changed. Performance is unmeasured; no
startup/cache/bundle-size improvement is claimed. Token v0.6.0 and its contrast records are
unchanged. The brand link consumes existing control-size/shape/focus roles.

Execution results and exact commands are retained in the PR and the ignored
`artifacts/agent-runs/extra3-224/` handoff. Final affected checks passed 602 native tests and
10 doctests, including 253 Theatre tests and four new regressions, native/WASM lint with
warnings denied, Rust format, Studio/Theatre debug builds and the shared gallery build.
The independent preflight explicitly skips repository and the two CMP gates; those steps
remain `NOT_RUN` in that run. Unchanged CMP inputs retain session-1 evidence at `9448785`
(207 shared tests, 416 Theatre tests and one existing live-origin skip), not a new device run.
The full preflight still reports repository `FAIL`: the exact same 22 qualification findings
as the retained clean-develop baseline. No checker, qualification hash or baseline was changed.
Broad API/contracts checks remain `NOT_RUN` outside the selected affected-package plan; no API
or contract source changed. The combined final review retains that outstanding gate. No GitHub
CI result was read or manually triggered.

### Browser observations

Target: headed Chrome 155.0.8059.80 on the Mac, through native CUA input. The existing
`cargo xtask app cantos-theatre --web` launcher served this worktree on 127.0.0.1:8082.
Only `?mode=auth-preview` was driven; there was no API, provider, account creation or email.
Source: incoming `db42c6c` plus the Web source diff in this PR. Captures and source fingerprints
are retained under `artifacts/ui-evidence/extra3-224/`; images were opened through CUA.

| Observation | Actual evidence / limit |
| --- | --- |
| Native Enter | Synthetic email entered; Enter reached Submitting once, field became read-only, CTA disabled with busy reason and focus moved to status |
| Recovery and keyboard | Invalid-email fixture retained the entered value and focused the input; Tab/Enter resubmitted once; accepted fixture focused the new-request action; Space returned to the editable field |
| Cancellation | Firebase capture showed Confirm plus quiet Text Cancel; Tab/Space on Cancel reached Login with the same Account origin; no transport ran in the diagnostic |
| History and escape | Browser Back returned to a clean registration form without proof; Forward restored Login with Account origin; contextual Back reached Account; Escape from Register reached Login; keyboard account → Register worked |
| Layout | Opened Register captures at actual emulated 320/390/768/1280 × 844 CSS px in light/VI and dark/EN. These are browser viewport sizes, not gallery frame widths; DevTools fit-to-window display scaling is visible in captures. No clipped action label or horizontal page overflow was observed |
| Target sizes | Read-only DOM measurement at 320/390 confirmed viewport dimensions, document width below viewport width, Back height 48 CSS px and CTA height 60 CSS px; this does not replace touch input |
| Zoom and focus | Browser toolbar explicitly reported 200%; entered email, full CTA label and keyboard focus ring stayed visible in dark/EN; restored to 100% afterward |
| Reduced motion | DevTools tab-only emulation returned matchMedia=true and computed transition=0s/animation=none for Back and submit at 390×844; restored the override. No continuous/interrupted-motion or native-device claim |
| Language/theme | Changed through the existing Account/locale controls; DOM inspection exposed vi-VN/en and the light/dark root. This is not full preference persistence or #221 admission |

The snapshots show a single email primary, contextual Back, quiet brand Home link, text-based
status and a decorative mail icon without a verified/success claim. At 320 the top brand wraps
below Back while text and CTA remain within the viewport; at 768 it stays one-column; at 1280
the story panel returns. All input used public synthetic values. An autofill suggestion opened
briefly; no saved value was selected and no autofill acceptance is claimed. Native coordinate
input in the emulated viewport was unavailable (`noWindowsAvailable`); actual interactions
above ran in the ordinary browser viewport, while emulated widths supplied layout evidence.
Programmatic read-only DOM measurements/scrolling were entered through DevTools, not reported
as trusted input. No before/after pixel pair or reference fidelity is claimed.

The user-approved cleanup removed only generated native Cargo `deps`, `build`, `.fingerprint`,
`examples` and `incremental` under this task's `target/debug`. No process held those paths.
HEAD, working diff and all pre-existing artifacts were checked identical before/after;
2,355,503,104 bytes of free space were recovered. No runtime data, other checkout, stash,
root executable, evidence or WASM output was removed.

The five selected Library v2 designs remain unavailable on Mac: current official helpers
reached the app but reported `Library prepare_materialize is not available`. No file bytes
or board pixels were materialized/read; no generic download bypass was used. The tool
failure is technical, not a denied permission or an uncertain external write.

Remaining acceptance: named reference inspection and attributable before/after pairing;
both-renderer shared admission before live icon migration; the full narrow/theme/locale matrix
for Verifying/Setup/Error/Complete and blocked-session states; actual touch, autofill, continuous
motion and named screen-reader walkthroughs. No real issuer/session acceptance was attempted.
Unobserved rows remain `NOT_RUN`. This partial implementation is not grounds to close #224.
