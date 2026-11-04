# Studio entry recovery and form width — issue 498

Part of [#498](https://github.com/loveoverflowcom/cantos/issues/498), not acceptance of the
whole sign-in/reauthentication matrix. The current issue/comments and supplied Space handoff
were read. Base is `origin/develop` at the merge of PR #718 (`9488577d`). Historical 31 October
captures with unknown runtime SHA are reference only.

## Decision and invariant

The primary task is signing in; state and recovery stay beside the form. Studio Web retains
Theatre expression with a quiet entry and one AuthPrimary submit. The state heading stays,
but becomes shorter; supporting copy names the existing form, or administrator/configuration
recovery for 404. A refused session no longer recommends clearing cookies/reloading while an
unsaved draft may exist. Generic claims that a draft exists are removed from error copy;
the shell's existing unsaved-content warnings, including the tab-only MemoryHelp, remain.
Checking, Login, Denied, Unavailable, SignInUnavailable, RetainedOwner and SessionRefused
still have their original typed mappings and gates. No Retry/session-refresh, password replay,
public signup, authentication policy, cookie authority or draft lifecycle is added.

Three layout options were considered: shrink the hero in two columns, use a horizontal brand
strip, or give the form one column until both regions fit. The last avoids retaining non-task
content at tablet widths and reuses the existing container query. Above 64rem of container
space, the hero is narrower and the form is bounded at 32rem. Below that, only the form is
shown; the existing narrow breakpoint removes horizontal form insets and uses the existing
title type role. Long heading/help words may wrap. Removing duplicate hero brand/supporting
copy does not affect the header or the workspace's resource use.

## Components and boundaries

Existing APIs are reused: `text-field` / TextField Floating, `icon-button` / password eye,
and Button AuthPrimary. SignInPanel and the field implementation are unchanged. There is no
new component import, shared API, token, palette, dependency or Studio Mobile target.
Mail/Lock remain a shared-contract/admission gap: TextField currently exposes a trailing
action but no leading icon. This slice does not insert screen-local markup or adopt an
unqualified extension. Both-renderer preview/runtime qualification remains its prerequisite.

Canonical `packages/localization/catalog/v1/catalog.json` owns paired vi-VN/en copy. The
existing resource generator produces app projections; Theatre Web/CMP changes are digest
headers only, with no handwritten Kotlin or Theatre behavior change. No ADR is needed for
this bounded composition/copy choice. Queue: no change in order or prerequisites.
Behavior SSOT: no semantic change; no registered Studio entry capability was found. There
is no state/guard/outcome/retry change or model delta. The resource path conservatively derives
`impact:unknown`; the ignored assurance decision records why modeling is not applicable.

## Evidence and useful limits

The ignored local bundle is `artifacts/ui-evidence/issue-498/`. Its build records retain full
source SHA, dirty build-input patch fingerprint and exact distribution file hashes. Baseline
uses the pinned PR #718 artifact (`da3967da`); its app/package/Cargo/toolchain inputs were
compared with fetched develop and were identical. Candidate builds use Rust 1.99.1 and
`cargo xtask build cantos-studio --web`. A later review restored an unintended substitution in
the unrelated submit-error resource; build/tests/lint were rerun. Earlier visual observations
remain attributable to their captured build; the final matrix recheck binds the final build.

- Studio development build: PASS.
- `cargo test --locked -p cantos-studio`: 475 unit tests and 5 doctests PASS, including typed
  entry distinctions, no-Retry, retained-owner/session guards, secret cleanup and form anatomy.
- Native and wasm32 Studio Clippy, all targets with warnings denied: PASS.
- Production localization generation/drift check: PASS. Localization Python suite: 89 tests,
  two optional Kotlin-compiler probes skipped because their compiler classpaths are absent.
- Browser: actual CSR Studio over a synthetic loopback HTTP error fixture, no successful
  authentication. 401 has one form and no visible policy help; 403 has refusal guidance;
  404 has no credential form; private workspaces stay hidden and Retry stays absent.
- Four matched light/vi-VN before/after scenarios use 320, 390, 768 and 1440 CSS-pixel widths.
  The 768 form grows from 335.25 to 512 CSS pixels (input from 287.25 to 464). Additional DOM
  bounds cover 360, 720, 752, 1024 and either side of the hero breakpoint (1056/1088).
- Dark/English 200% root-text scenarios cover 320/390/768/1440. The inherited unbroken
  English heading overflow is removed; fields grow instead of clipping labels. This is an
  explicit test-host root-font override, not an actual browser-zoom or OS text-size result.
- Pointer reveal preserves selection 14–16 and input focus while changing the existing input
  type. Keyboard Tab reaches the eye; Space changes the action/type, keeps focus on the eye
  and does not submit. Autocomplete stays current-password. Credential value reads are
  redacted by the browser tool, so raw-value equality/length observations are unusable; exact
  value retention is not claimed from that browser run. Native form tests cover state retention.
- All retained screenshots were opened. JPEG MIME/pixel sizes are measured separately from
  requested CSS viewport sizes; full-page text-scale captures include document growth.

Repository/preflight qualification has the same stale Leptos records on the clean fetched
baseline and candidate. Shared token and renderer source digests are unchanged. No evidence
hash or compatibility baseline is adjusted to hide it. Handoff preflight records the actual
repository FAIL and later NOT_RUN gates. Broad backend, audio, Protobuf, mutation and native
preview lanes are explicitly skipped: the conservative resource selector broadens beyond this
Studio-only semantic change and the local filesystem has approximately 1 GiB free. Scoped
Studio checks above remain independent evidence; the inherited repository failure is not PASS.

L1/L2 are source-inspected with existing typed-state tests; L3 is native/unit/static evidence;
L4 covers browser presentation over error responses only. L5 authenticated sign-in, retained
owner/draft recovery and reauthentication acceptance are NOT_RUN. OS Vietnamese IME, real
credential-manager autofill, Safari/Firefox, screen reader, real devices, actual 200% browser
zoom and production remain NOT_RUN. No deploy, self-approval, protection bypass or issue closure.
The original criteria and AC-01–04 are not all satisfied; issue checkboxes are left unchanged.
