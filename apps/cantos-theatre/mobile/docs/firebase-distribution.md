# Firebase distribution of Theatre APKs — retired lanes

`cargo xtask distribute firebase cantos-theatre --channel demo|demo-qa|production` is **refused
for every channel** since [#286](https://github.com/loveoverflowcom/cantos/issues/286). The refusal
is a typed `UNSUPPORTED` result produced before any tool probe, plan (`--dry-run` included), build,
login, upload or file write.

| Retired channel | Why it cannot be remapped |
| --- | --- |
| `demo` | built `com.cantos.theatre.demo` from the Android mock host, which no longer exists |
| `demo-qa` | built the mock host's `qa` build type (R8, operator signing), which no longer exists |
| `production` | expected a debuggable APK of `com.cantos.theatre`; debug is now the side-by-side `com.cantos.theatre.dev`, a different identity from the registered production Firebase app |

The single Android application `:androidApp` has only `debug` (`com.cantos.theatre.dev`, label
**Cantos Dev**) and `release` (`com.cantos.theatre`, label **Cantos**). A new distribution policy,
its Firebase app registrations, certificate pins and R8 audit belong to the distribution step of
the #286 series; until then build locally with `cargo xtask theatre-host android-debug` /
`android-release` and install yourself. The previous lane contract (packages, certificate checks,
mapping files, keep rules) is available in Git history before this change; it is not current
policy.

An older `com.cantos.theatre.demo` install can remain on a device and may still register the
production callback scheme. Remove it yourself if callbacks misroute; no tool here uninstalls apps
or deletes user data.
