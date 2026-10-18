# Opt-in CircleCI artifact builds

The configuration implements build-only jobs independently of manual production deployment.
Account/project integration and actual CircleCI execution are not established. Ordinary pushes
and PRs run no CircleCI jobs; the user stopped integration before the first run.

After separately qualifying account access and available credits, select the reviewed source
branch, set `build=true`, `build-component=all` (or `api`, `theatre-web`, `studio-web`, `android`)
and leave `target` empty. Each selected workflow runs the repository contract before building;
API and Web also run their existing Rust/Web CI contracts. Failed checks block packaging.
Providing both build and deployment inputs selects neither workflow. Build jobs attach no
production context and need no VPS/Firebase credentials. Existing deploy workflows stay manual.

The command is `cargo xtask release-build <component> [--dry-run]`, with deterministic regression
checks through `cargo xtask release-build --self-test`. It refuses dirty source or mismatched
CircleCI SHA and writes a fresh `artifacts/release-build/<component>/<SHA>/manifest.json` with
actual artifact SHA-256/size, full source SHA, version and tool metadata. Existing output paths
are refused, including interrupted attempts. Outputs without a manifest are incomplete.

| Component | Output and acceptance boundary |
| --- | --- |
| API | Linux amd64 image archive from the existing digest-pinned Rust/trixie Containerfile, commit label and image ID. `git archive` excludes private/local inputs. No SSH, migration or activation. Actual VPS architecture/schema compatibility still needs inventory. |
| Theatre/Studio Web | Separate release bundle archives with `https://api.cantos.cc`; fresh dist required. No Hosting write or browser acceptance. |
| Android | Production `com.cantos.theatre` unsigned release APK/AAB and existing release-isolation reports; real API origin, no demo APK. Current versionCode/versionName recorded. Signing, issuer/login, installed-version compatibility and device acceptance remain unqualified. |

Rust follows `rust-toolchain.toml` (1.98.1); Trunk is 0.21.14, Gradle is checksum-pinned 8.14.3,
and Android uses repository SDK 35/build-tools 35.0.0 with installed JDK 17. Missing SDK/JDK
fails without accepting licenses. CI installs Podman inside its Linux VM only. Executor images
are rolling vendor images; this is not a reproducible environment snapshot or a verified run.

Caches hold Cargo registry/git downloads, pinned Trunk/Gradle binaries and Gradle dependency
modules, separated by architecture/toolchain and branch/lock inputs. Credentials, signing keys,
environment files, release outputs and Cargo targets are excluded. Image layers are not persisted.
Selecting `all` runs four independently gated workflows, including a repository check in each;
select one component for bounded acceptance/cost.

Firebase destinations remain project `cantos-a348b`, sites `cantoss-theatre`/`cantoss-studio`,
domains `cantos.cc`/`studio.cantos.cc`. Build source grants no destination permissions. A future
operator must qualify exact project/repository mapping, account terms, effective permissions,
credits and artifact visibility before enabling execution. No account, OAuth/app access,
token/context, payment or production permission is created by this source change.

Official references: [configuration](https://circleci.com/docs/reference/configuration-reference/),
[Android images](https://circleci.com/docs/guides/execution-managed/android-machine-image/).
