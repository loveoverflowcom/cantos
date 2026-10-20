# CMP admission compatibility recheck — PR 304

After renewing the stale Leptos records, repository verification exposed six stale CMP
records on the existing source. This separate recheck preserves Button, TextField, Surface,
Badge, BackButton and Logo admissions. It does not use Web observations to qualify CMP.
Historical issue145 records and the admission guard remain unchanged; no variant is admitted.

## Source and execution

Base `develop@c2a8b6f`; no handwritten CMP implementation or test was changed by this repair.
Canonical tokens SHA-256: `25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741`.
CMP renderer SHA-256: `8ed0c550c092e217bae37b24f3cbdc37d1570d70e4e4e209f680fadcc058d3ba`.

`RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview cmp verify`: PASS on Linux,
installed Gradle8.14.3/JDK17, Compose Desktop JVM renderer. The first invocation reported
`desktopTest UP-TO-DATE`, so prior captures and XML were preserved locally and the declared
XML output directory moved aside. A second invocation through the same xtask ran
`desktopTest` freshly: 215 tests, zero skipped/failures/errors.
No cache, assertion or renderer input was weakened. XML timestamps are19 October2026
17:10–17:11 UTC (20 October local time). Raw logs are `cmp-verify.log` and `cmp-fresh.log`
under ignored `artifacts/agent-runs/pr304/`.

## Semantics, interaction and opened pixels

| Existing component | Production oracle and observed scope |
| --- | --- |
| Button | `AuthAccountComponentTest`: one intent before loading blocks repeated activation, all auth variants retain native role and refuse disabled callback. `AuthAccountCaptureTest`: native focus on the real control, complete intrinsic layout and long Vietnamese wrapping. Opened `cmp-auth-primary-light-390-focus.png` and both account boards. |
| BackButton | `back_only_dispatches_its_host_intent_and_has_one_named_focusable_target`: callback, accessible name, one native target and blocked state. Arrow visible in both opened account boards at390px. |
| TextField | Exact raw Vietnamese/error preservation, readonly composition, disabled/loading SetText refusal in `AuthAccountComponentTest`. Native focus/error capture at390px/text2 opened; label/help/error wrap and single-line value scrolling remain distinct. |
| Surface | Paired primary-container foreground in both themes asserted by production component tests; opened account boards retain auth/profile/membership/quiet settings hierarchy and complete footer. |
| Badge | All five tones at390px/text2 in both themes opened. Capture tests assert every label is displayed, no click action, no height overflow or ellipsized line and final character retained. |
| Logo | `LogoRendererTest` asserts decorative versus named-image semantics, real ink colors, transparent holes on two caller backgrounds and aspect-fit geometry. Opened both named tone captures, transparency diagnostics and Studio square-slot diagnostic. Green/orange test backgrounds are geometry/transparency oracles, not proposed product color pairings. |

Opened account images: `cmp-auth-account-light-390-text1.png` and
`cmp-auth-account-dark-390-text1.png`. Field: `cmp-auth-field-light-390-focus-error.png`.
Badges: `cmp-auth-badges-light-390-text2.png`, `cmp-auth-badges-dark-390-text2.png`.
Logo images: `cmp-logo-symbol-light.png`, `cmp-logo-symbol-dark.png`,
`cmp-logo-mono-transparent-background-0.png`, `cmp-logo-mono-transparent-background-1.png`,
`cmp-logo-studio-aspect-fit-square160.png`.
All listed images were opened with `view_image`. Immutable local copies, hashes, per-suite
counts and renderer digest are in `artifacts/agent-runs/pr304/cmp-fresh-captures/` and
`cmp-inputs.json`; these ignored artifacts are not PR-hosted images.

## Boundary and remaining acceptance

Evidence is `example-tested`, `semantics-tested` and `interaction-tested` on Compose Desktop
JVM, with `screenshot-inspected` and `cross-theme-inspected` observations at the listed sizes.
This is a bounded existing-admission review against the source/token/component contract;
no missing historical PNG or exact design-pixel oracle is substituted. No Android/iOS device,
TalkBack/VoiceOver, live listener API, native background audio, publication or identity flow
was exercised. Those acceptance obligations remain with the native/application owner.
