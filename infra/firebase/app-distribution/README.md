# Firebase App Distribution operator inputs

App Distribution is CLI-driven; there is no tracked Firebase `appDistribution` section or new
configuration parser. The canonical executable contract is
[`cargo xtask distribute firebase`](../../../xtask/README.md#distribute-firebase);
setup, tester notification, retention and live acceptance remain in the
[Theatre distribution runbook](../../../apps/cantos-theatre/mobile/docs/firebase-distribution.md).

| Existing input | Meaning |
| --- | --- |
| `--app-id` / `CANTOS_FIREBASE_DEMO_APP_ID` | Explicit Firebase **Android App ID**; the flag wins. No default/active Firebase project lookup |
| `--channel demo` | Distribution policy selecting the debug demo; not a Hosting channel |
| `--artifact apk` | Only supported artifact; production/AAB stay unsupported |
| `--groups` / `--testers` | Explicit recipients; distribution can invite/notify them |
| `--release-notes` / `--release-notes-file` | Bounded release notes; otherwise a demo-labeled summary |
| `CANTOS_FIREBASE_DEMO_CERT_SHA256` | Optional expected debug-signing certificate fingerprint |

An App ID is not a Hosting site, Hosting target, project alias or Functions codebase. Distribution
keeps the Firebase CLI `appdistribution:distribute --app ...` argument/result contract, its private
`artifacts/distributions/<run-id>/` run record and tester-file redaction. It never consumes the
Hosting `.firebaserc`, `firebase.json` or Functions selector. Credentials remain operator-managed
Firebase login/ADC, never tracked here. No recipient list or real app identifier is committed.

`--dry-run` is pure planning: no Gradle/Firebase subprocess, artifact write, upload or invitation.
A real invocation builds only the supported Android demo graph, never Web/Functions/API/edge.
The repository move does not widen #184's supported artifacts or replace its live acceptance.
