# Disposable CMP localization experiment inputs

These three tiny projects are retained inputs for the [issue #169 spike](../../cmp-localization-spike.md).
They are **not** application modules, adopted dependencies, a supported build interface, or a
replacement for `cargo xtask`. No application/settings file includes them. There is no wrapper,
launcher, CI registration, production dependency, or Android/iOS host here.

Copy them into ignored `artifacts/` before reproducing the one-off experiment. The Gradle calls
below are experiment primitives against the copy, not repository verification commands. Use an
already installed Gradle 8.14.3 and JDK 17; do not provision a toolchain through this experiment.

```sh
mkdir -p artifacts/agent-runs/issue169-replay
cp -R docs/evidence/fixtures/cmp-localization-spike/compose artifacts/agent-runs/issue169-replay/
cp -R docs/evidence/fixtures/cmp-localization-spike/moko artifacts/agent-runs/issue169-replay/
cp -R docs/evidence/fixtures/cmp-localization-spike/lyricist artifacts/agent-runs/issue169-replay/
# Set spike_gradle to the absolute path of an installed Gradle 8.14.3 executable.
# Set JAVA_HOME to an installed JDK 17. Run one candidate at a time.
"$spike_gradle" -p artifacts/agent-runs/issue169-replay/compose desktopTest --console=plain
"$spike_gradle" -p artifacts/agent-runs/issue169-replay/moko desktopTest --console=plain
"$spike_gradle" -p artifacts/agent-runs/issue169-replay/lyricist desktopTest --console=plain
```

Each project uses Kotlin 2.2.20, Compose 1.9.0 and a `desktop` JVM target. Its single JUnit test
renders both languages through the actual candidate composable API, asserts six exact strings
per language, and writes `build/reports/candidate/{vi,en}.{png,semantics.txt}`. Inputs and
expected outputs are hand-authored, not generated from the candidate under test. A typed
`DisplayError.Unavailable` maps exhaustively to presentation copy. The title includes literal
`{time}` and `%1$s`, which must survive insertion unchanged. Numbers 0/1/2 cover this fixture's
English/Vietnamese integer plural distinction; they do not establish general CLDR support.

The common experiment skeleton includes Compose Resources in all three projects. Its empty
resource scaffolding for moko/Lyricist is counted separately in the evidence record. Lyricist
uses its documented manual catalog mode, with **no KSP processor**; zero generated catalog bytes
therefore says nothing about its KSP footprint. This small catalog is not the #167 source format
or an application catalog migration. Its default Material theme is only a readable render host,
not Cantos design-system acceptance.

A Desktop semantics assertion or captured image is never Android/iOS acceptance. The test sets
the process JVM locale before each fresh composition and restores it afterwards; it does not
prove app-owned runtime language switching, preference persistence, focus preservation or
process death. Moko additionally sets `StringDesc.localeType` and Lyricist selects its state
before mounting. Native packaging, native locale behavior and KSP need their own experiment
before any adoption decision.
