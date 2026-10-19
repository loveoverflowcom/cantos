# Firebase App Distribution operator inputs

The executable owner is [`cargo xtask distribute firebase`](../../../xtask/README.md#distribute-firebase).
The [release/distribution runbook](../../../apps/cantos-theatre/mobile/docs/firebase-distribution.md)
owns exact package registration, signer inputs, dry-run, staged-byte/hash and mapping checks,
recipient authorization, unknown-outcome recovery and independent device/live acceptance.

`development` selects `com.cantos.theatre.dev` debug; `production` selects optimized
`com.cantos.theatre` release. Each needs its own matching Firebase Android App ID and certificate
pin. Development variables use `CANTOS_FIREBASE_DEVELOPMENT_*`; production uses
`CANTOS_FIREBASE_PRODUCTION_*`. Operator release key inputs use `CANTOS_RELEASE_*`.
No key, registration, recipient list or real app identifier is tracked here. Retired `.demo`/QA
inputs are never used as fallback. Firebase login/ADC stays operator-managed.

Distribution uses the Firebase CLI without a runtime SDK or google-services plugin. App IDs are
not Hosting sites/targets, project aliases or Functions codebases; `.firebaserc`/Hosting config
never chooses an App Distribution destination. AAB distribution remains refused. Build and CI
package unsigned optimized output plus an isolated unshrunk audit; neither uploads or installs.
