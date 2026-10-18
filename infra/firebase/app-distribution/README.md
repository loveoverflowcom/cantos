# Firebase App Distribution operator inputs

App Distribution is CLI-driven; there is no tracked Firebase `appDistribution` section or new
configuration parser. The canonical executable contract is
[`cargo xtask distribute firebase`](../../../xtask/README.md#distribute-firebase);
setup, tester notification, retention and live acceptance remain in the
[Theatre distribution runbook](../../../apps/cantos-theatre/mobile/docs/firebase-distribution.md).

| Existing input | Meaning |
| --- | --- |
| `--app-id` / `CANTOS_FIREBASE_DEMO_APP_ID` | Explicit Firebase **Android App ID**; the flag wins. No default/active Firebase project lookup |
| `--channel demo` / `--channel production` | Selects the demo or real production composition, both debug APKs for QA; not a Hosting channel |
| `--artifact apk` | Only supported artifact; AAB remains unsupported |
| `--groups` / `--testers` | Explicit recipients; distribution can invite/notify them |
| `--release-notes` / `--release-notes-file` | Bounded release notes; otherwise a demo-labeled summary |
| `CANTOS_FIREBASE_DEMO_CERT_SHA256` | Optional expected demo debug-signing certificate fingerprint |
| `--app-id` / `CANTOS_FIREBASE_PRODUCTION_APP_ID` | Separate explicit production-composition Firebase Android App ID |
| `CANTOS_FIREBASE_PRODUCTION_CERT_SHA256` | Required expected certificate for real production-composition distribution |
| `--api-origin` / `--native-issuer` | Explicit HTTPS API origin and optional native identity issuer, inspected in the packaged production manifest |

An App ID is not a Hosting site, Hosting target, project alias or Functions codebase. Distribution
keeps the Firebase CLI `appdistribution:distribute --app ...` argument/result contract, its private
`artifacts/distributions/<run-id>/` run record and tester-file redaction. It never consumes the
Hosting `.firebaserc`, `firebase.json` or Functions selector. Credentials remain operator-managed
Firebase login/ADC, never tracked here. No recipient list or real app identifier is committed.

`--dry-run` is pure planning: no Gradle/Firebase subprocess, artifact write, upload or invitation.
A real invocation builds the selected Android host, never Web/Functions/API/edge. The production
lane inspects actual sandbox exclusion before upload; build/distribution do not replace native
device/login/playback acceptance. An agent requires explicit app and recipient authorization
for the remote write.
