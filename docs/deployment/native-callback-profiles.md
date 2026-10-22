# Theatre native callback profiles (retired)

**Retired for the CMP client; kept only so existing links resolve.** The Theatre Android and iOS
apps no longer sign in through a system browser. Native sign-in is first-party Identity v1:
email/password JSON on the API origin ([Identity auth v1](../../packages/contracts/identity-auth-v1.md)).
The apps declare no OIDC issuer, browser callback or URL scheme, so there is no per-build callback
profile (`production` or `development`) to derive, register or admit for them:

- Android's manifest has only the `MAIN`/`LAUNCHER` entry (no `BROWSABLE` filter, no `<data>` scheme);
  iOS `Info.plist` has no `CFBundleURLTypes`. `cargo xtask theatre-host` inspects the built
  manifest and refuses one that declares either, for debug and release.
- Do not add a provider redirect, set `CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK` or run the
  old rollout steps on behalf of a CMP build. The `--native-issuer` build option no longer exists.

The Server's legacy provider-path routes and their closed `callback_profile` admission are
compatibility surfaces owned by [Bearer v1](../../packages/contracts/bearer-v1.md) and the Server and
Kanidm READMEs, not by this page. When they are retired, their owner removes the remaining links
here. The former runbook text is available in Git history for this path.
