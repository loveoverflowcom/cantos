# Compose preview tooling

## Outcome and boundary

`cargo xtask compose-preview <cantos|cantos-studio>` captures an existing Android-target Compose
`@Preview` through the official Android CLI and a running Android Studio Quail 2+ instance.
The source must stay within the selected product's mobile Gradle project. No emulator/phone,
application installation, Gradle scaffold or runtime runner is invoked. The
[command contract](../../xtask/README.md#compose-preview) and
[CMP workflow](../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md) own usage.

Both mobile directories remain README guides: **real Cantos rendering is UNSUPPORTED**.
This is developer tooling ready for the first mobile project, not an implementation or acceptance
claim for #14–#17. Robolectric/Compose UI tests remain candidates until Gradle actually provides
their modules, dependencies, source sets and tasks.

## Invariant and ownership

A successful capture must use the selected app's source and create fresh PNG/JSON artifacts;
missing prerequisites, failing CLI calls, missing/invalid output or source changes must never
reuse an old image or count as success. `xtask/compose_preview.rs` owns argument forwarding and
exit-status vocabulary; `scripts/render_compose_preview.py` owns the CLI adapter, per-command
timeout, logs and output validation. The existing repository gate exercises its tests. IDE
rendering is optional and is not added to CI or full preflight prerequisites.

The adapter validates a PNG header and positive pixel dimensions, not full pixel decoding or
visual quality. It preserves one observed semantics JSON object/array without inventing a
schema. It rejects missing/ambiguous JSON and retains raw output. A capture record's
`image_inspected` and `semantics_asserted` stay false; actual inspection belongs in a separate
report. Host evidence never substitutes for interaction, iOS rendering, native audio or readers.

## Provenance and local verification

Initial source: `develop` at `8a9e140cb7198f3a53beca23f71490f180fa0c28`; rebased onto
`209e9e9` (PR #38) before publication. Local host: Linux x86_64.
Rust 1.87.0 was installed into a task-local scratch directory with rustfmt and clippy, without
modifying the machine's shell profile. The mock fixture uses original Vietnamese text and a
one-pixel test PNG; it contains no manuscript, media, credentials or provider calls.

The capture regression suite invokes the **real Python adapter across a subprocess boundary**
with a temporary executable as the Android CLI double. The double supplies version/help/IDE
responses, PNG/semantics output, failures, a timeout and a concurrent source edit. Its outputs
are not Android Studio output or evidence that Compose itself rendered.

| Claim | Evidence |
| --- | --- |
| Correct source/product/PID selection and shell-safe forwarding | Rust CLI/command assertions; real Python adapter against CLI process double |
| Fresh output; no stale image reused | Two captures with the second returning no PNG; first remains unchanged |
| Unsupported prerequisites | Missing Gradle/CLI, legacy CLI help, disconnected IDE; no render invoked |
| Failed captures never pass | Nonzero render exit, invalid/missing PNG, absent/ambiguous JSON, source edited during capture |
| Bounded timeout and diagnosis | Render timeout with retained partial stdout and failure record |
| Path boundaries and dry-run | Cross-product/traversal/symlink rejection; no CLI calls or artifacts on dry-run |
| Repository/skill hygiene | Repository gate: local links, portable frontmatter, metadata, shared symlinks, Python tests, whitespace |

Exact local command results are recorded in the PR. Evidence levels are `example-tested`,
`mock-integration-tested` and, after the actual Rust checks, `compiled`/`statically-checked`.
No CI status is used for handoff; remote CI is not checked.

After rebase, `cargo xtask verify repository` passed (310 text files, 15 skills and 155 Python
tests, including 10 capture tests); `cargo test -p xtask --locked compose_preview` passed both
Rust tests; `cargo clippy -p xtask --all-targets --locked -- -D warnings` and
`cargo fmt --all -- --check` passed. The wider failed/incomplete suite attempts below used the
initial source base; full changed-only preflight was rerun after rebase and remains unsupported.

## Not covered / residual risk

- Android Studio/CLI were not available, so actual PNG rendering and the exact CLI tree/log
  shape are **NOT_RUN**. The first real mobile project must run this capture, open the PNG,
  assert the observed semantics and record CLI/IDE/Kotlin/CMP versions and preview configuration.
- The wrapper asks for the selected project and retains IDE readiness output; it does not
  prove every Gradle input was synchronized. Agents must inspect readiness before claiming a
  preview reflects the current build. A hash check covers the preview file, not every dependency.
- PNGs, semantics and logs can contain private UI data; keep bundles in the ignored artifacts
  directory and use permitted deterministic fixtures. Sanitize before sharing.
- A timed-out CLI process is stopped; an IDE-side request already dispatched may continue.
  Each run has its own output path, so later requests cannot accept that partial capture.
- Interaction, motion, Robolectric tests, Android/iOS devices, native playback and
  TalkBack/VoiceOver walkthroughs are **NOT_RUN**. Native acceptance remains with
  [050](../work-plan/050-cmp-native-listening.md).
- Routing cases R35–R37 were author-walked only; no independent agent evaluation was run.
- The existing xtask process-group test fails at its initial `alive(grandchild)` check on this
  host. A separate shell-child probe reproduced `ps -o stat= -p <pid>` returning no state for
  a process that `os.kill(pid, 0)` confirms exists. The process test/code was not changed or
  skipped to conceal this environment limitation; full xtask test acceptance remains incomplete.
- Full changed-only preflight selects every gate for an xtask change. On this host it reports
  `UNSUPPORTED` for Web/WASM/Trunk and PostgreSQL prerequisites, and other gates `NOT_RUN`.
  Available repository, focused Rust and contract checks are listed independently in the PR;
  there is no claim of full preflight success.
- `verify contracts` passes its Script IR reference gate, then fails two existing WAV CLI tests
  while spawning `inspect-wav` (`PermissionDenied`, OS error 13); `wav-reference` is `NOT_RUN`.
  A whole-workspace Rust verification attempt completed formatting/lint but exited 1 during
  test compilation without a final summary. Neither whole-suite result is reported as passing.
