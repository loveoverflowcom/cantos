# Android startup diagnosis

Use this procedure for [#754](https://github.com/loveoverflowcom/cantos/issues/754).
A dark Recent apps thumbnail does not identify a cold-start failure, a crash, or the
APK that produced it. Keep the installed artifact, build result, Compose renderer
result and Android runtime result separate.

## Identify the installed artifact first

Choose an authorized test device and the package actually reported by its owner:
debug is `com.cantos.theatre.dev`; release is `com.cantos.theatre`. Keep the evidence
directory outside Git (for example under `artifacts/agent-runs/`). With the installed
Android SDK tools on `PATH`:

```sh
SERIAL='replace-with-test-device-serial'
PACKAGE=com.cantos.theatre.dev
EVIDENCE="$PWD/artifacts/agent-runs/android-startup-device"
mkdir -p "$EVIDENCE"
adb -s "$SERIAL" shell getprop ro.build.version.release
adb -s "$SERIAL" shell getprop ro.build.version.sdk
adb -s "$SERIAL" shell getprop ro.product.model
adb -s "$SERIAL" shell dumpsys package "$PACKAGE" > "$EVIDENCE/package.txt"
adb -s "$SERIAL" shell pm path "$PACKAGE"
```

Pull the exact `base.apk` path returned by `pm path`, then record its hash, package,
version and signer using `shasum -a 256`, `aapt2 dump badging` and
`apksigner verify --print-certs -v`. Preserve any split APK inventory too.
The source revision is meaningful only when the artifact's version/build provenance
binds it; local version `0.1.0` alone does not identify a commit. Follow the
[artifact lineage runbook](firebase-distribution.md#local-build-and-audit).
Record the installer/distribution origin, or `UNKNOWN`; do not infer it from HEAD.

## Record launcher startup and the first destination

Use a dedicated test session with playback stopped. Do not uninstall, clear app
data, clear global logcat, or modify another package to reproduce this report.

1. Start a bounded screen recording before launch. For example, run
   `adb -s "$SERIAL" shell screenrecord --time-limit 20 /data/local/tmp/cantos-754-cold.mp4`
   in one terminal on the admitted test device. Use a fresh filename for each case.
2. In another terminal, force-stop only the chosen package, then **tap its launcher
   icon**. Capture from the launcher through the first meaningful content or recovery.
   An explicit `am start -W -a android.intent.action.MAIN -c android.intent.category.LAUNCHER
   -n "$PACKAGE/com.cantos.theatre.app.TheatreActivity"` can provide timing diagnostics;
   label it separately from a launcher-tap recording.
3. Pull the recording. Capture the app and Recent apps screenshots from this same
   session with `adb -s "$SERIAL" exec-out screencap -p > "$EVIDENCE/app.png"`.
   Note whether the process survived and whether any crash/ANR occurred. Keep bounded,
   time-attributed logcat locally; scrub account data, URLs and credentials before
   quoting the minimal failure signature in GitHub.
4. Repeat cold start in light/dark and offline. Resume via Home → launcher without
   force-stop and verify that the optional Compose splash does not replay. Exercise
   Activity recreation/app restore and the existing pending-link/Back behavior.
   Restore any test appearance/network settings afterwards.
5. Qualify Android 12+ and a pre-31 compat target separately. Missing targets remain
   `NOT_RUN`. Neither an APK build nor a Desktop Compose screenshot replaces this row.

Classify the first failing stage: wrong/stale artifact; system splash/resource;
local owner initialization; first Compose content; or destination loading/recovery.
Do not add a minimum timer, network gate or second splash Activity to make a logo
visible. `CantosLocalLaunch(localReady = true)` intentionally renders its content
directly; a catalog read still pending does not make local readiness false.
See the [Android SplashScreen contract](https://developer.android.com/develop/ui/views/launch/splash-screen)
and [compat API](https://developer.android.com/reference/androidx/core/splashscreen/SplashScreen).

## Bounded source investigation on 2026-11-06

Base: `develop@9a249e40c831954ee3d40c5d5521b9fb86e0cbe9`.
The original phone, installed APK, signer, OS and cold-launch recording are **UNKNOWN**.
No production startup fix is established by this investigation.

- `cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:9`
  built and inspected a real-only debug APK successfully. The explicit unavailable
  loopback origin avoids production requests; it is a diagnostic configuration.
  With `cantos.sourceRevision` set to the base SHA, its version is
  `0.1.0+9a249e40c831954ee3d40c5d5521b9fb86e0cbe9-dev`, version code 1.
  APK SHA-256: `22260cdef4f51dbaa2c2f39ac457b861a89d23db8584f2ae1d0601bf010499c6`.
- Packaged resources contain the starting theme, canonical light/night backgrounds,
  both native symbol variants, the API-31 platform attribute mapping from compat,
  and `postSplashScreenTheme = Theme.Cantos`. The Activity installs compat before
  `super.onCreate`. This is resource/source evidence, not a rendered splash observation.
- The installed AVD pointer was stale. A separate task-owned AVD using installed
  API-35 default arm64 image revision 2 and emulator 35.4.9 could not reach Android:
  `Failed to read vbmeta partitions`, missing `/dev/block/by-name/super`, then
  `InitFatalReboot`. GPT inspection found a `super` partition in the image; the mount
  failure remains an emulator/environment blocker, before APK installation.
- `ReadyLaunchPresentationTest` exercises the actual ready-handoff →
  `CantosTheatreApp` → `TheatreShell` with explicit HTTP/player doubles. It checks
  guest content while HTTP is pending, actionable recovery after failure, and
  destination/player retention over Compose remount and theme changes. It does not
  exercise Android owners, SplashScreen, saved-state recreation, storage or native audio;
  the fixture has no account or Library owner. Final gate results belong to the PR/issue
  handoff, not this source-bound investigation record.

The next owner needs the original installed artifact and a booted, admitted device
to run the matrix above. Keep #754 open until that evidence distinguishes the cause
and verifies any resulting production repair.
