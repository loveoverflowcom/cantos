# Firebase App Distribution of the Theatre demo APK — issue 184, Phase A

This records the implementation boundary and evidence of `cargo xtask distribute firebase` for
[#184](https://github.com/loveoverflowcom/cantos/issues/184), based on
`develop@87b49a3` plus this change. **One live distribution has been run (2026-10-16, see below); the tester-device
install check is still `NOT_RUN`.** The command contract is
the [xtask README](../../xtask/README.md#distribute-firebase); the operator steps are in
[Firebase distribution](../../apps/cantos-theatre/mobile/docs/firebase-distribution.md).
It distributes the **demo** mock host only. It is not production evidence: production APK/AAB stays
`UNSUPPORTED` behind the same blockers recorded in the
[#122 isolation record](issue122-cmp-sandbox-isolation.md).

## Review boundary

- Delivers: the opt-in `distribute firebase` command with a pure plan and offline `--dry-run`,
  prerequisite and Firebase-destination checks before the build, one reused Gradle build, validated
  and staged APK bytes, one never-retried CLI invocation, a closed remote-outcome vocabulary, an
  intent-then-outcome per-run receipt, redaction, and documentation.
- Does not deliver: production APK/AAB, an optional manual CI workflow, a stable demo signing key,
  `--no-build`/arbitrary artifact paths, automatic recovery, or any live Firebase acceptance.
- `Closes #184`: no. Phase A's live acceptance is pending, so the PR says `Part of #184`.

## Claim ledger

| Claim | Oracle | Level reached | Result |
| --- | --- | --- | --- |
| `distribute` is explicit; ordinary gate, CI and build plans cannot reach it | a test walks every gate, CI step and build plan for a Firebase/distribute command | example-tested | held |
| channel, variant and artifact are distinct; unavailable combinations are `UNSUPPORTED` before any tool; bad flags exit 2 | request and CLI-parse tables | example-tested | held |
| the dry run probes nothing, builds nothing, writes nothing, and shows unresolved configuration | a `ToolProbe` that panics if consulted plus a recording runner; real invocation of the plan | example-tested + observed | held |
| `--app-id` beats `CANTOS_FIREBASE_DEMO_APP_ID`; no default app; an App ID is not a package/project name | request tests | example-tested | held |
| recipients: trim, deduplicate, no empty entry, union, never implicit; addresses never in argv, output or receipt | request and coordinator tests inspecting argv, report and receipts | example-tested | held |
| notes: mutually exclusive, bounded, multiline/Unicode preserved byte for byte, generated demo summary | request and coordinator tests | example-tested | held |
| a missing tool, SDK, `apksigner` or incompatible CLI stops everything before any run file or build | per-prerequisite coordinator tests asserting no run directory, no Gradle command, no upload | example-tested | held |
| destination failure (unregistered app, wrong app, wrong package, auth, timeout) stops before build and upload | coordinator table over captured/real-shaped CLI results | example-tested | held |
| zero/several/split/foreign Gradle output, wrong package or version, release build, unsigned, multi-signer, configured-certificate mismatch are refused with the exact reason | metadata and tool-report tests; real `aapt2`/`apksigner` reports as fixtures | example-tested | held |
| altered staged bytes are never uploaded; intent is recorded before the write and its failure prevents it | coordinator tests with a rehash mismatch and a failing persist | example-tested | held |
| only exit 0 plus a recognized release is success; a release link alone is not; changed output, timeout, interruption and oversized output are *unknown*; nothing is retried | adapter and coordinator tests; one distribute call asserted | example-tested | held |
| a failed final receipt after remote success still prints a recovery pointer and fails | coordinator test | example-tested | held |
| tokens, signed download links and addresses are redacted; only console/tester links on allowlisted hosts are stored | redaction and no-leak tests | example-tested | held |
| the bounded capture helper times out, interrupts, caps output and stops the whole group | real `sh` children | integration-tested (local processes) | held |
| run directories are private and unique; receipts replace atomically; staging copies exact read-only bytes | temp-directory tests | integration-tested (local files) | held |

Non-vacuity: ten hand-made mutants of the invariants above (version gate, destination check, "link
alone is success", altered-bytes check, empty recipients, email redaction, ignored intent-persist
failure, package check, ignored prerequisite failure, empty-client acceptance) were each caught by a
failing test, then restored.

## Real local build and installed-tool inspection (host-capable)

Host: Linux, Gradle 8.14.3, OpenJDK 17.0.20.1, Android SDK platform 35 / build-tools 35.0.0.
`cargo xtask theatre-demo android-build` built the debug variant (`PASS`, 89 tasks executed). The
ignored host test `installed_android_tools_accept_the_built_demo_apk` then ran the real adapters
(metadata → run-owned staged copy → real `aapt2` and `apksigner` → identity validation); it makes no
Firebase call:

| Fact | Observed |
| --- | --- |
| package | `com.cantos.theatre.demo` |
| versionName / versionCode | `0.1.0-demo` / `1` |
| debuggable | `true` (debug variant) |
| size | 30 993 493 bytes |
| SHA-256 | `9eddd476d0793cc92022c73324af26a936f107bd16fd94686d032941c93dcf51` |
| signer | one; Android Debug certificate, SHA-256 `b0f5604fde71614bd8b9578d5022f2498571245ee1f4e16e290071836c934c0e` |
| schemes (v1, v2, v3) | true, true, false |

The certificate is this workstation's debug key: another machine, Dot or CI signs with a different
one, which prevents in-place updates over an installed build. The hash identifies this build only;
it is not a release or production artifact.

## Firebase CLI contract

The adapter targets `firebase-tools` 15.x and was written against **15.34.0** (the installed
version). Behavior established from its sources and read-only calls:

- `appdistribution:distribute` uploads, updates notes, then distributes; it prints the release links
  after the upload, before the later stages. Success is the overall exit status.
- `apps:sdkconfig ANDROID <id>` exits 0 with an empty `client` list for an app that does not exist
  (observed live with a nonexistent ID), so the destination check requires the response to name the
  requested app and registered package.
- The authenticated account's Firebase project had no registered Android app when this was
  checked, so the registered-app response and the distribute outputs in
  [`xtask/fixtures/firebase-tools-15.34.0`](../../xtask/fixtures/firebase-tools-15.34.0/README.md) are
  reconstructed from the CLI sources, not captured. They are labeled so in that directory. The
  adapter is fail-closed: a text it does not recognize yields an *unknown* outcome.

## Live acceptance

Run on 2026-10-16 by the operator's request: a demo Android app `com.cantos.theatre.demo` was
registered in the operator's Firebase project, a stable demo keystore was generated outside the
repository (`~/.config/cantos/demo-signing`, mode 0600, never committed) and supplied through the
opt-in Gradle properties `cantosDemoStoreFile`/`StorePassword`/`KeyAlias`/`KeyPassword`, and
`CANTOS_FIREBASE_DEMO_CERT_SHA256` pinned its certificate. Command:

```sh
cargo xtask distribute firebase cantos-theatre --channel demo --app-id <demo app id> \
  --testers <operator address> --release-notes-file <file>
```

Result: all nine stages `PASS`; receipt state `confirmed`, release `0.1.0-demo (1)` created and
distributed; APK SHA-256 `3711febcfe137d11ae1eb1a3bdc282054c8f270448f60923a47ee974a220a94a`,
signer SHA-256 `f18e09c6592b0ef700e6211df9c275770f4ec06ace09bbe038f6258dd15bd4c2` (the stable demo
key, matching the pin). The CLI transcript matched the adapter's recognized lines; the signed
download link was redacted in the saved diagnostics.
An earlier attempt the same minute failed at the destination lookup with an authentication
refusal, before any build or upload, and the identical lookup then succeeded; the cause was not
established (suspected transient credential refresh right after app creation).

Still `NOT_RUN`: a tester downloading, installing and opening the build on a **named** Android
device. Owner: the operator who received the invitation. Acceptance: the build installs and opens,
and the device is named in the PR.

## Residual risk

- The operator account (project owner) worked for both the destination read and the upload; minimal
  IAM roles, notification fan-out to a group and tester installation are unverified.
- A same-user process could replace the staged read-only file between the final hash and the CLI's
  read; the 0700 run directory and read-only mode narrow this, they do not remove it.
- The CLI may write `firebase-debug.log` into the private run directory; it is not scrubbed.
- Execution requires a POSIX host; `--dry-run` works anywhere.
- Under this workstation's default temporary directory a stray `/tmp/.git` makes 17 existing
  `services` tests refuse their private inputs ("outside any Git checkout"). That is a
  pre-existing environment effect unrelated to this change; with `TMPDIR` outside a checkout the
  whole xtask suite passes.
