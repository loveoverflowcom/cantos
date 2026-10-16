# Theatre identity activation — issue 191

Part of [#191](https://github.com/loveoverflowcom/cantos/issues/191), based on fetched
`develop@f3619bff1d69246f22be2906570a3933ea922f53`. Issue #191 and
[#174](https://github.com/loveoverflowcom/cantos/issues/174) had no comments at inspection.
This slice supplies native nonsecret build inputs and typed activation diagnostics; it does
not accept live login or signup. The user limited local verification to Android CMP preview
and lightweight checks, excluding full/release/iOS builds and new toolchain installation.

## Invariants and implementation boundary

| Claim / failure mode | Owner / cheapest oracle | Evidence held |
| --- | --- | --- |
| Missing/invalid config cannot create a native account controller | Common `NativeClientConfiguration.resolve`, both host compositions; exact Kotlin cases | Source-reviewed; four new Kotlin tests are NOT_RUN |
| Nonsecret selected Android inputs reach the metadata declaration, with empty opt-out | Gradle providers → manifest placeholders; `test_theatre_android_identity_host.py` | Source example-tested, not merged/packaged artifact evidence |
| Both iOS target configurations forward selected build settings into plist generation | Xcode Debug/Release → source plist expansion → Swift → shared owner; `test_theatre_ios_identity.py` | Source example-tested, not built Info.plist evidence |
| Diagnostics do not retain raw rejected values | Missing/Invalid carry enum fields only; new Kotlin sentinel cases | Source-reviewed; Kotlin execution NOT_RUN |
| Guest Home stays independent; signup does not fabricate success | Existing guest composition, Web BFF and #174 boundary | Source-reviewed; no new live/browser/device acceptance |

The existing client grammar remains exact HTTPS API origin, exact native issuer suffix
`/oauth2/openid/cantos-theatre-native` and fixed `com.cantos.theatre:/oauth2/callback`.
The legacy nullable `create` delegates to the typed resolver. Android preserves typed
manifest literals as supplied invalid inputs. iOS forwards optional Bundle values to one
owner and initializes the logout-barrier directory lazily; unavailable config creates no
controller. The default still uses the explicit demo sandbox. The existing controller, PKCE,
session fences, secure store, callback handling and listener authority are reused.

Apple's [plist build documentation](https://developer.apple.com/documentation/bundleresources/managing-your-app-s-information-property-list)
requires custom keys in a supplied plist: arbitrary `INFOPLIST_KEY_*` settings are ignored.
Both target configurations therefore use the minimal source `Info.plist` through
`INFOPLIST_FILE` and retain generated system values. The source is not a copied bundle resource.
Android uses the documented [manifest placeholder mechanism](https://developer.android.com/build/manage-manifests#inject_build_variables_into_the_manifest).

No new dependency edge, protocol, database, migration, provider registration or accepted ADR
is introduced. No account, secret, grant, TLS setting, Membership or guest-data merge is created.
The account screen's generic unavailable copy is unchanged; typed details are operator-facing.
Settled records and audio revisions are untouched. Performance was not measured; no performance
claim is made.

## Configuration and runbook

Use the [native build-input guide](../../apps/cantos-theatre/mobile/README.md#native-identity-build-inputs-and-operator-diagnostics).
Both app keys are public configuration, never client-secret carriers. Property overrides can
disable an inherited Android environment value explicitly. iOS requires actual Xcode setting
overrides; the framework-only xtask command does not build the Swift app.

Web already has a Trunk `/api/` proxy and real same-origin BFF login/session/logout client.
`cargo xtask run cantos-theatre --web` starts API 8080 and Trunk 8082 with exact Web origin
`http://127.0.0.1:8082`; `cargo xtask app cantos-theatre --web` starts only the frontend.
See the [existing Web runbook](../../apps/cantos-theatre/web/README.md#build-run-and-checks),
including its pending #183 compatibility decision. An operator-prepared migrated local
app-role database and separately supplied `CANTOS_THEATRE_OIDC_ISSUER` /
`CANTOS_THEATRE_OIDC_SECRET` enable the independent Web BFF. Its issuer suffix is
`/oauth2/openid/cantos-theatre-web-bff`; callback is the exact Web origin plus
`/api/v1/theatre/auth/callback`. Native API activation separately reads
`CANTOS_THEATRE_NATIVE_ISSUER`. These commands provision no prerequisite.

| Observation | Operator interpretation |
| --- | --- |
| Native `Missing` | Check build key presence/empty override and selected artifact |
| Native `Invalid` | Correct the named API-origin/issuer field; do not relax HTTPS or issuer matching |
| Native `Available` | Syntax only; inspect backend activation, provider, TLS and current session separately |
| Web session 404 | Identity route absent; check activation and matching API deployment; public discovery can still work |
| Session 401 | No accepted current session; the existing client cleanup/session fences still apply |
| Transport/provider unavailable | Retain unavailable/pending recovery state; never switch to mock success |

When the official Android CLI and a synced Quail 2+ IDE are supplied, the requested preview
uses the existing guest Home wrapper, without contacting a provider:

```sh
cargo xtask compose-preview cantos-theatre \
  --file apps/cantos-theatre/mobile/preview-sandbox/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt \
  --composable TheatreDiscoveryLightPreview \
  --label issue191-guest-discovery-light-vi
```

The [preview workflow](../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md)
requires source/project readiness. Only minimal compilation needed for preview is within this
session's authorization. Open any resulting PNG and assert the observed semantics before claiming
inspection. A preview cannot establish session IO, native callbacks or live login.

## Local evidence and limitations

Mac checkout: `/Users/manhblue/projects/loveoverflow/cantos`; Rust `1.98.1`, Python `3.13.5`.
Raw output is retained outside Git in `artifacts/agent-runs/issue191-ultra1/` and the sibling
`issue191-native-config-*.log` files. Checked dirty-source hashes are retained in the local
run record; PR commits identify the delivered source. No remote CI was checked.

| Check | Result / evidence level | Scope |
| --- | --- | --- |
| Android source regression before wiring | FAIL, example-tested: 6 tests, 2 failures | Required metadata and provider wiring absent in baseline |
| iOS source regression against baseline files in a disposable directory | FAIL, example-tested: 3 tests, 1 failure + 1 error | Missing build keys and optional-value delegation detected |
| Custom iOS plist regression before correction | FAIL, example-tested: 4 tests, 2 errors | Source plist and both `INFOPLIST_FILE` bindings absent |
| `python3 -m unittest scripts.test_theatre_android_identity_host scripts.test_theatre_ios_identity` | PASS, example-tested: 10 tests | Source/manifest/Xcode wiring only |
| `git diff --check` | PASS, statically-checked at the recorded source checkpoint | Changed whitespace only |
| Requested `cargo xtask compose-preview cantos-theatre ...` | UNSUPPORTED | Official Android CLI unavailable; no PNG/semantics captured |
| `cargo xtask verify repository` | FAIL, pre-existing | Unchanged CMP skill is 307 lines, over the 300-line gate; Python suite not reached |
| Initial `cargo xtask verify rust` | FAIL, pre-existing rust-format; rust-lint/tests NOT_RUN | Untouched Rust formatting differences; no app compilation ran |
| Handoff preflight | See PR's exact final-diff summary | Build/test lanes excluded by the user's updated preview-only instruction |
| Four new Kotlin configuration tests / common-JVM suite | NOT_RUN | Source added; compilation/execution excluded by updated instruction |
| Merged Android manifest/APK and built iOS Info.plist | NOT_RUN | No app builds performed; source tests cannot satisfy artifact acceptance |
| Provider login, browser navigation, device/secure-store lifecycle | NOT_RUN | No authorized live provider/account/device acceptance environment |

The inherited hygiene failures are confirmed against the unchanged baseline files, not repaired
as unrelated cleanup. They are actual FAIL results. The remaining unrun scopes are acceptance
gaps, not passing results; this slice does not claim `CORE_VERIFIED` without Kotlin execution.

## Second independent review — 16 October 2026

The second session reviewed the complete three-dot contribution from
`f3619bff1d69246f22be2906570a3933ea922f53` to
`a346af04abd03b604e1cc74630f387282472aa9c`, plus the affected host/controller consumers.
Remote `develop` and the existing PR branch still matched those SHAs when inspected.
Issues #191/#174 and PR #196 had no new comments or accepted signup decision. The review did
not reuse the first session's conclusion as its oracle.

Assessment: **no-actionable-findings** for source correctness in the declared scope;
**insufficient-evidence** for compiler/artifact/live acceptance. Kotlin/Gradle/Android and
Swift/plist/export received separate read-only passes. No functional code defect was confirmed,
so no auth behavior, validation grammar or security configuration was changed. One misleading
resolver comment was narrowed: missing keys take precedence, then the API origin and issuer
are validated. For example, an invalid API value plus an empty issuer yields `Missing(Issuer)`;
this is the existing first-prerequisite diagnostic, not an aggregate report of every error.
The README now states that order explicitly. Comment/documentation changes require no new
behavior test; the original four Kotlin cases remain unexecuted under the user's limit.

| Changed claim reviewed | Disposition / evidence | Acceptance still absent |
| --- | --- | --- |
| Private validated configuration and enum-only refusal diagnostics | Source-reviewed; no constructor/copy/decoder bypass; common grammar matches BASE | Executed Kotlin/compiler evidence |
| Android properties → environment → empty defaults and metadata | Source-reviewed; explicit empty property wins; only `Available` constructs owner | Merged/packaged metadata, OS callback and secure store |
| iOS Debug/Release custom plist, nullable Swift/Kotlin bridge, cleanup | Source-reviewed; shared framework export present; unavailable composition performs no controller/barrier creation | Built plist expansion, native compilation and Keychain/browser lifecycle |
| Existing Web authority and guest/signup boundary | Source-reviewed; no Web/backend edits; #174 remains independent | API/provider-confirmed session/read/logout acceptance |
| Python regression claims | Source wiring assertions only; no compiler or artifact promotion | Runtime negative cases retain their separate gates |

### Android preview investigation

The supported Theatre command above was retried in this session with label
`issue191-ultra2-guest-discovery-light-vi` and `--timeout-seconds 30`; xtask reported
`UNSUPPORTED capture` (exit 1). No preview PNG or semantics was produced or inspected.
Discovery went beyond PATH: Studio/SDK, user-local/cache/Downloads/Application Support,
Homebrew and task-local temporary tools were checked. The installed IDE at
`/Applications/Android Studio.app` reports version `2024.3`, build
`243.24978.46.2431.13363775`; the app tool returned its splash window, with no synced Theatre
project observed. The process-list read (`ps`) was denied by the sandbox and was not retried;
it did not prevent file-based version discovery or the capture attempt.

Two concrete packages are needed: **official Android CLI for macOS arm64** and
**Android Studio Quail 2 | 2026.1.2 or newer**. Google's
[render command](https://developer.android.com/tools/agents/android-cli/commands/studio_render-compose-preview)
requires Quail 2+. The
[IDE/AGP compatibility table](https://developer.android.com/studio/releases#android_gradle_plugin_and_android_studio_compatibility)
also excludes the installed Meerkat-era IDE from this project's pinned AGP `8.11.1`; direct
preview in that old IDE is not a supported fallback. Gradle `8.14.3`, SDK Platform 35,
Build Tools `35.0.0` and a full Microsoft JDK `21.0.7` are already present. No installation,
upgrade, Gradle sync, app/full/release/iOS build or toolchain download was performed. Minimal
compilation was unnecessary because the capture prerequisites already failed.

The preview blocker is **environment / UNSUPPORTED**, not a denied installation request.
Install/update needs separate user permission and may require the user to accept vendor terms.
After that, open the fixed Theatre Gradle root, sync with `-Pcantos.android=true`, perform only
the compilation needed for the preview, and rerun the exact capture. Open its PNG and assert
observed semantics. This remains host-preview evidence, never live-login/device acceptance.

### Second-session verification and preservation

Raw output and source hashes live in ignored `artifacts/agent-runs/issue191-ultra2/`.
The focused Python/plist/whitespace results and final preflight summary are recorded in the
updated PR body and local run record. Lightweight byte comparison confirmed the 307-line
CMP skill and all four Rust files named by the first session's rust-format failure are
identical to BASE. They remain inherited failures, without unrelated cleanup or a repeated
Rust verification campaign. Kotlin/native/configured-artifact/live checks remain `NOT_RUN`.
No `CORE_VERIFIED` or production-ready claim is made.

Stash `cb8f104ec2285a36fecab85388623cd09c112950`, all 11 auxiliary worktree registrations,
the Documents safety worktree and earlier evidence are preserved. That safety worktree was
not opened or queried. The same Draft PR #196 is updated without force push; no second PR,
provider/account/grant operation, remote CI inspection, merge or issue closure is authorized
or performed by this review.

## Next acceptance

The independent source review is complete; compiler/artifact and runtime acceptance are still
deferred. First supply the official Android CLI and Quail 2+ IDE, then rerun the exact preview
command and inspect its image/semantics. Do not install
tools, run full builds or activate providers merely to remove a gap in this record.

When separately authorized later, artifact acceptance must compare selected nonsecret inputs
with the actual merged/packaged Android metadata and built iOS Info.plist. Provider/device
acceptance requires an existing approved account/client, reachable API/provider, and a named
browser/device. Confirm guest Home → Login → provider → callback → API-confirmed session →
authorized Saved read → logout → refusal, then restore/reopen, expiry/revocation, cancellation,
delayed completion, cleanup retry and A→B isolation. Browser Back/BFCache/reload/focus and OS
Keychain/Keystore/system-browser return need their own observations. Existing mock/synthetic
tests remain separate from those live claims. Next live-acceptance operator is
**unassigned — assignment required before acceptance**.

Signup remains blocked on #174's actual provider capability and self-signup/invitation/defer
decision; neither source login activation nor registration-unavailable UI closes it. The
[identity handoff](../work-plan/theatre-identity-handoff.md) and
[#179/#180 client matrix](../work-plan/live-client-api-matrix.md#theatre-identity-activation)
link this record without changing remote issue status. Keep #191 open and the PR Draft while
these acceptance items remain unproved.
