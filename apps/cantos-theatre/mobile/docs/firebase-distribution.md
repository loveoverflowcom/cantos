# Firebase distribution of the Theatre demo APK

> **Scope (Phase A of [#184](https://github.com/loveoverflowcom/cantos/issues/184)).** An explicit,
> opt-in `cargo xtask` command builds the Theatre Android **demo** APK and hands it to Firebase App
> Distribution for QA testers. It distributes the isolated mock host (`com.cantos.theatre.demo`,
> debug variant). It is **not** a production release: production APK/AAB distribution is
> `UNSUPPORTED` until a real production Android host exists and its release graph is shown to exclude
> sandbox code ([#122](https://github.com/loveoverflowcom/cantos/issues/122)). Studio is Web-only.
> The command contract is in the [xtask README](../../../../xtask/README.md#distribute-firebase).

Distribution is a **remote write** and may invite or notify the selected testers. Nothing else in
xtask (`build`, `run`, `verify`, `preflight`, `ci`, `build all`) can reach it.

## One-time Firebase setup (operator)

xtask creates and changes nothing in Firebase. Prepare a **demo** Firebase project yourself:

1. Register an Android app whose package name is exactly `com.cantos.theatre.demo`. Do not reuse a
   production project or app; a demo APK uploaded to a production app is not a supported flow.
2. Open the App Distribution page for that app and press *Get started* (the CLI cannot onboard it).
3. Create a tester group (for example `cantos-internal`) and add the testers. Its **alias** is what
   `--groups` takes.
4. Copy the app's **Firebase App ID** (`1:<project number>:android:<hex>`). It is not the package
   name and not the project ID, and it is not a secret.

## One-time workstation setup

- `firebase-tools` **15.x, at least 15.34.0** on `PATH` (the release the adapter's output contract
  was checked against). xtask never installs it.
- Authenticate it yourself: `firebase login` on a workstation, or Application Default Credentials
  (`GOOGLE_APPLICATION_CREDENTIALS`) for a service account. xtask reads, prints and stores no
  credential and never logs in.
- The same installed toolchain as `theatre-demo android-build`: Gradle 8.13–8.14.3, JDK 17+, and an
  existing Android SDK (`ANDROID_HOME`/`ANDROID_SDK_ROOT`) with platform 35 and build-tools 35.0.0
  (`aapt2` and `apksigner` are used to inspect the result).
- Permissions: the account needs App Distribution release/distribute rights (for example Firebase
  App Distribution Admin) **and** read access to the app's registration, because the destination
  check runs the read-only `firebase apps:sdkconfig ANDROID <app-id>` (for example Firebase Viewer).
  This is the expected role set from Firebase's IAM documentation; the live acceptance below is
  what confirms it for your project.

Nonsecret configuration (export beforehand, or pass `--app-id`):

```sh
# The flag --app-id wins over this variable. There is no default app.
export CANTOS_FIREBASE_DEMO_APP_ID=1:1234567890:android:0a1b2c3d4e5f67890

# Optional: reject an APK signed with any other debug certificate (64 hex digits SHA-256).
export CANTOS_FIREBASE_DEMO_CERT_SHA256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
```

## Run it

1. Review the plan. It starts no tool, reads no credential and writes nothing, so it works on a
   machine without Android or Firebase tooling; missing configuration is shown as `UNRESOLVED`:

   ```sh
   cargo xtask distribute firebase cantos-theatre --channel demo --groups cantos-internal --dry-run
   ```

2. Distribute, intentionally. Name the recipients explicitly; there is no "all testers" default:

   ```sh
   cargo xtask distribute firebase cantos-theatre --channel demo --artifact apk \
     --groups cantos-internal --release-notes-file release-notes.txt
   # or explicit people and inline notes
   cargo xtask distribute firebase cantos-theatre --channel demo \
     --testers qa@example.com --release-notes "CMP demo QA"
   ```

   With neither notes flag, a small demo-labeled summary of the version and source commit is used.
   File notes must be a regular UTF-8 file and are limited to 16 KiB. The local reader consumes
   at most 16 KiB plus one oversize-detection byte, including during `--dry-run`; on Unix it
   opens nonblocking before rejecting special-file descriptors. Invalid notes stop before tool
   probes, builds or Firebase calls. Symlinks to regular files remain usable.

The result is a summary table with one row per stage, the run directory, and, on success, the
Firebase console and tester links. Add `RUSTUP_TOOLCHAIN=1.98.1` as for the other xtask commands
if your default toolchain differs.

## Reading the result

| Distribution row | Meaning | Do |
| --- | --- | --- |
| `PASS` | The service accepted upload, release notes and distribution (exit 0 and a recognized release). | Check installation on a named device; acceptance is service-side only. |
| `FAIL` … `REJECTED` | A recognized refusal (authentication, permission, app not onboarded, invalid request) before any upload was accepted. | Fix the cause and run again. |
| `FAIL` … `PARTIAL` | The release exists, but the notes or the distribution to testers/groups failed. The release identity is in the receipt. | **Reconcile in the Firebase console first**, then decide. |
| `FAIL` … `UNKNOWN remote outcome` | Timeout (20 minutes), interruption, lost or oversized output, or output the adapter does not recognize. The write may have happened. | **Reconcile in the Firebase console first.** Never replay blindly. |
| `UNSUPPORTED` | A tool, SDK, CLI version or channel/artifact combination is not available here. Nothing ran. | Provide it yourself; xtask installs nothing. |
| `NOT_RUN` | The stage was not reached because an earlier one failed. | Fix the failing stage. |

A printed release link or an "uploaded" line is **not** distribution success: the Firebase CLI
prints it before it updates notes and distributes. A repeated binary can create, update or reuse a
release, and replaying the command can notify testers again. Stopping the local process never
undoes an upload.

Everything xtask observed is in `artifacts/distributions/<run-id>/receipt.json` (ignored by Git,
private to the owner): source commit with a dirty flag, tool versions, channel and the actual
Gradle variant/task, package/version, staged APK SHA-256, signing fingerprint, effective App ID,
group aliases, tester **count**, notes digest, stage results and the release identity with its
durable console and tester links. Tester addresses, the Firebase SDK configuration, tokens and the
signed binary download link (valid for about an hour) are never written. The directory also holds
the exact staged APK and, if the CLI failed, possibly its `firebase-debug.log`: keep both out of
shared CI artifacts.

## Signing

A signed **debug** APK is acceptable for this demo channel. Debug keystores are generated per
machine (the Mac, a Dot, CI each differ), and Android refuses to update an installed app signed with
a different key, so testers may have to uninstall first. For in-place updates, supply a stable key
through the existing Android signing mechanisms and set `CANTOS_FIREBASE_DEMO_CERT_SHA256` so xtask
rejects any other certificate. xtask never creates, commits or replaces a key, and the
certificate fingerprint is recorded in every receipt.

## Acceptance checklist (operator run)

Normal tests and `preflight` never perform these. Record the outputs in the PR.

- [ ] Real local demo build and installed-tool inspection: certificate, version, hash recorded.
- [ ] One intentional end-to-end distribution against the chosen demo app and recipients:
      command, summary, `receipt.json` and links.
- [ ] A separate check that a tester downloaded, installed and opened the build on a **named**
      Android device.
- [ ] If any of the above cannot be done, it stays `NOT_RUN` with an owner and the missing
      prerequisite. Never simulate a passing distribution.

## Not in this phase

Production APK/AAB distribution (needs a genuine production module, application identity, release
signing and an isolated Firebase app; an AAB also needs a Firebase ↔ Google Play link), an optional
manual CI workflow (`workflow_dispatch` calling this same command), `--no-build`, arbitrary artifact
paths, an interactive wizard, automatic recovery, and any Firebase SDK, Crashlytics, Google Services
plugin or `google-services.json` in the app itself.
