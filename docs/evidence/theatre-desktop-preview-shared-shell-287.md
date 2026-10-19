# Theatre Desktop Preview over the shared shell (#287)

Source: branch `feat/theatre-desktop-preview-shared-shell-287`, based on `develop` after #286 (`ab4c81b`).

## Claim and owner

The Desktop Preview mounts the same `CantosTheatreApp` over the same `TheatreShell`, projection,
controllers and navigation as Android and iOS; only external ports are mocks, composed in
`PreviewOwners`. `:preview-sandbox` is JVM-desktop only. Owners: [mobile README](../../apps/cantos-theatre/mobile/README.md#desktop-preview),
[architecture](../../apps/cantos-theatre/mobile/docs/architecture.md#desktop-preview-composition).

## Evidence ledger

| Claim | Evidence level | Command / result |
| --- | --- | --- |
| Shell, auth, library, playback flows run on the real owners | example-tested, mock-integration-tested | `:preview-sandbox:desktopTest` 87/87 before the package move; rerun at handoff below |
| Shared-class tests no longer depend on the sandbox | example-tested | `:shared:desktopTest` 375 tests green (includes moved presentation/layout/copy/playback tests) |
| xtask: Desktop-only commands, retired verbs refused, gate renamed with alias | example-tested | `cargo test -p xtask --locked` 552 passed |
| iOS resources moved, no `CantosDemo` references | statically-checked | `python3 -m unittest` iOS/logos tests; Xcode build **NOT_RUN** (no macOS/Xcode host) |
| Android release forbids sandbox classes | statically-checked only | inspector inventory now the sandbox JVM classes; `theatre-host android-release` **NOT_RUN** (no Android SDK) |
| Audio, device, process death, live identity | not covered | the decoder stand-in produces no audio; "Restored" is a seed on an in-memory database |

## Known gaps

- `repository` gate: 16 pre-existing Leptos preview-qualification failures, identical on `develop`.
- `:shared` still contains the unreferenced legacy `AppContainer`, `CatalogController`, `ListeningController`
  and `ProgressPort`/`CatalogPort`; removal (and the release inspector's required `AppContainer` class) is a follow-up.
- Seed-driven guest selection is made by opening the unit and settling paused, because the runtime clears
  any session when a guest is established.
- Workbench event log records workbench actions and replayed intents; user taps are observed via the projection.
