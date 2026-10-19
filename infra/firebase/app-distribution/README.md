# Firebase App Distribution operator inputs

This CLI-only product uses no Firebase SDK, Hosting configuration or Functions codebase.
The [xtask contract](../../../xtask/README.md#distribute-firebase) and
[distribution runbook](../../../apps/cantos-theatre/mobile/docs/firebase-distribution.md) own behavior.

| Input | Meaning |
| --- | --- |
| `--channel development` / `--channel release` | real-only `.dev` debug or optimized release APK; independent of server environment |
| `--app-id` | overrides the channel's explicit Firebase Android App ID |
| `CANTOS_FIREBASE_DEVELOPMENT_APP_ID`, `CANTOS_FIREBASE_RELEASE_APP_ID` | separate operator registrations for `.dev` and `com.cantos.theatre` |
| `CANTOS_FIREBASE_DEVELOPMENT_CERT_SHA256`, `CANTOS_FIREBASE_RELEASE_CERT_SHA256` | required expected signer for the corresponding live lane |
| `CANTOS_RELEASE_KEYSTORE`, `CANTOS_RELEASE_KEYSTORE_PASSWORD`, `CANTOS_RELEASE_KEY_ALIAS`, `CANTOS_RELEASE_KEY_PASSWORD` | existing operator release key; never created, printed or recorded |
| `--api-origin`, `--native-issuer` | explicit API origin and optional HTTPS native issuer |
| `--groups`, `--testers` | explicit recipients; real distribution may invite/notify them |
| `--release-notes`, `--release-notes-file` | bounded notes; otherwise a real-only source/version summary |

Legacy demo/demo-qa/production channels and old environment names are never silently remapped.
Missing registrations/credentials/recipient authority remain operator prerequisites. AAB distribution
is unsupported. Dry runs have no subprocess/network/write. Local/CI builds never upload, and an
uncertain live outcome requires Firebase console reconciliation before any retry. Keep credentials,
SDK configuration, tester files and firebase-debug logs private and outside shared CI artifacts.
