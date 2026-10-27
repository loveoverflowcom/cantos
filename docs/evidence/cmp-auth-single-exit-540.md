# CMP registration: one normal exit (#540 follow-up)

Initial base: `develop@24d5489f6a60826574222c475cb4bab4048175ce`; refreshed base: `develop@cdcdc158a9152d143f03beadf32cc0d98bec200e` (#550, integrated before final checks). This is the updated
[#540](https://github.com/loveoverflowcom/cantos/issues/540) delta after
[#543](https://github.com/loveoverflowcom/cantos/pull/543), not a replacement for its
[source-bound record](cmp-auth-v43-minimal-expressive-540.md). The current issue amendment
governs exits; the older Drive concept's repeated sign-in action is superseded.

## State and destination audit

| Surface / state | Action and owner |
| --- | --- |
| Register, every phase | Header Back is the one normal exit to Login. `TheatreShell.navigate` calls `RegistrationController.leave` before publishing navigation. No `RegisterCancel` or duplicate `ReturnLogin` is rendered. |
| Register, submitting | Leaving cancels local waiting and increments the existing epoch. The mail effect becomes unknown; an old answer cannot overwrite it. Returning does not resend. |
| Register, waiting | Back retains the accepted initiation. Returning still shows waiting, with no cancellation/revocation claim and no resend. |
| Register, failed/cancelled | Existing explicit restart/retry recovery remains; it returns to editing, without sending automatically. Invalid email remains editable in place. |
| Register, owner unavailable | Back still returns to Login. The distinct Discover action remains useful for browsing without authentication. |
| VerifyEmail, confirmation/pending | Back returns to the underlying typed destination and invokes the existing verification `leave`; no new return action. |
| VerifyEmail, retry/invalid/refused/completed | Identical-request retry, choose-password, request-another and signed-out Login / signed-in Account remain distinct recovery or completion outcomes. No session admission is inferred from account creation. |
| Login, pending/cleanup/startup | `CancelLogin`, `RetryCleanup` and automatic startup restore remain unchanged. |

The hierarchy, `FloatingTonal`, scoped `tonalAuthActions`, shared fields/buttons/header and
independent default-masked eyes from #543 remain. The existing shared choice row now honors the tonal auth scope with the existing focus-fill modifier; ordinary choices keep their ring. The common recipe adds unchecked, checked and disabled confirmation specimens, with pixel checks and the ordinary row as a negative control. No tokens, public component API, admission contract or auth protocol changes. No new shared primitive or dependency. Queue: no change;
the requested review boundary and sequential #531 work already belong to their issues.

## Completion input delta

The password rule is now `8–128 ký tự.` / `8–128 characters.` and the mailbox/account/password
confirmation is shorter. Only `theatre.cmp_auth.*` catalog entries change; generated projections
are regenerated, including catalog-digest headers in other consumers. Unicode-scalar/byte
validation, confirmation reset, retry and secret lifetime stay with the existing domain/controller.

IME Next explicitly requests the repeat field. A pointer press on the existing shared eye
temporarily refuses focus transfer before the shared button handles it, preserving the editor's
focus and selection. The eye remains keyboard-focusable outside the pointer gesture, with the
same 48 dp shared control, accessible name and actions. No pointer event is consumed here.
JVM input is synthetic; this does not establish native IME composition or screen-reader behavior.

## Checkpoint evidence

- `cargo xtask theatre-preview verify` on the original worktree before the checkbox follow-up:
  572 shared tests (3 skipped) and 155 preview tests (2 skipped), no failures. This is retained
  checkpoint evidence; the final combined source requires the app gate again.
- `cargo xtask ui-preview cmp verify` in `/tmp/cantos-540-recovered`: 290 tests, zero
  failures/errors/skips. `TonalAuthRecipeTest` checks unchecked/checked confirmation focus in
  both themes, no outside paint, perceivable interior fill, and a default-ring negative control.
  Opened light unchecked / dark checked focused captures and the light default-ring control.
- Diagnostic `scripts/check_repository.py --base origin/develop` on clean initial-base source
  reports 50 qualification/adoption findings. The current token-source receipts predate #548.
  This follow-up also changes CMP source, so its package freshness remains pending; those
  receipts are not silently rebound or represented as current acceptance.
- The final preflight command, its exact checked tree and individual results are recorded in
  the PR. JVM/native/qualification evidence remains separate.

## Verification and acceptance boundaries

The local evidence root is ignored `artifacts/agent-runs/540/` in the issue worktree. It retains
the original failing regression, subsequent diagnostics, command logs and design images. The
Mac disconnected twice. iCloud later made the original Documents worktree dataless, including its Git pointer. Resident source edits were byte-verified and transferred into `/tmp/cantos-540-recovered`; the original worktree and its dataless evidence remain preserved. The interrupted multi-file patch was read back before retrying. The two small evicted source deltas and new state-matrix test were restored from this session's recorded patches, and generated resources were regenerated. No iCloud setting was changed.
The main checkout was clean and fast-forwarded to the initial base above; the recovery branch then integrated the refreshed base. Cleanup removed only verified
empty Kotlin compiler directories, preserving screenshots, reports, receipts, audit locks and
all other worktrees. No caffeinate or iCloud setting was changed.

| Criterion | Source / oracle | Remaining acceptance |
| --- | --- | --- |
| AC-01 | #543 hierarchy retained; Register loses the redundant return action | Native capture matrix remains separate |
| AC-02 | Existing scoped tonal recipe and token pairs retained; confirmation focus uses the same fill without a ring | Production contrast and native focus/AT observations not newly measured |
| AC-03 | `VerifyEmailFlowTest` checks independent eyes, focus, selection, Next and confirmation | Native IME composition/autofill and AT remain unobserved |
| AC-04 | Existing identity/completion suites plus `RegistrationBackFlowTest`; controllers and policy unchanged | Real mailbox/auth and native runtime not claimed |
| AC-05 | Existing UI-system recipe extended for confirmation focus, no public API delta; common preview/pixel checks and canonical generated resources | Freshness of the package qualification receipts remains separate; no digest-only requalification is claimed |
| AC-06 | Core and production-composable suites, final-diff preflight; exact commands/results recorded in the PR | Missing or failed gates are disclosed individually |
| AC-07 | JVM captures at 320/390, VI/EN, light/dark and 2x text are host evidence only | Android matrix `NOT_RUN`: no connected device; official preview CLI unavailable |
| AC-08 | Separate from JVM and Android | iOS `NOT_RUN`: no booted simulator and installed Kotlin/Native 2.2.0/2.4.20 do not match 2.2.20 |
| AC-09 | `RegistrationExitUiTest`: every phase, both locales; real shell pending/late-response regression | Native Back remains in AC-07/08 |
| AC-10 | State/destination audit above; production registration/verification recovery tests | Native navigation/AT remains in AC-07/08 |
| AC-11 | Only actual exit/copy/input gaps changed; existing tonal recipe extended without a fork or global focus rewrite | Device focus/IME remains pending |
| AC-12 | Four assertions fail against unchanged production source; all-phase, pending-leave and recovery regressions | Issue checklist/comment synchronization and final-head results recorded in GitHub |

Quint lookup: the old `cantos.identity.theatre-registration` capability is deprecated and
non-executable; native mailbox/session models are draft backend capabilities. This presentation
delta changes no modeled transition, backend core or model. No new model or model-conformance
claim is made; native Kotlin/controller tests remain the implementation oracle.

Resume native acceptance with a connected Android target and
`cargo xtask theatre-host android-debug --api-origin https://api.cantos.cc`; record installed
artifact/source identity, then exercise the six states at 320/390 equivalent, Light/Dark/System,
VI/EN, 200% system text, IME, Back and TalkBack. For iOS, provide the already installed matching
Kotlin/Native 2.2.20 toolchain, run `cargo xtask theatre-host ios-build --kotlin-native-home PATH`,
then launch the production host and independently inspect safe areas, IME, large text and
VoiceOver. Build/host pixels alone cannot close those criteria. No deployment or distribution
is implied by either resume step.
