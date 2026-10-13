# Portable Studio pilot codec: CMP compatibility evidence

Issue [#97](https://github.com/loveoverflowcom/cantos/issues/97), C4. This is a portable transport
fixture harness under Theatre Mobile, not a Studio Mobile app, Theatre endpoint or production
HTTP client. Nothing mounts it in the UI, player, authentication or persistence. The canonical
Script IR remains an exact string. Received identifiers, revisions, digests and acceptance fields
are untrusted transport values, never authentication, authorization or validation witnesses.

## Authority and implementation

The authority is [studio_v1.proto](../../../../packages/contracts/protobuf/studio_v1.proto) and
its [frozen admission policy](../../../../packages/contracts/protobuf/README.md), established at
`9f0368e18de867c58da31619b6be171478779e9e`. No schema or application contract is changed by C4.
`commonMain/com/cantos/theatre/adapters/protobuf` contains:

- Four exhaustive pilot transport values, plus nested `FieldIssue`, with required public scalars
- Private nullable proto2 wire representations, explicit `@ProtoNumber` on every field and no
  custom defaults; repeated absence and empty intentionally map to one empty list
- `ULong` application revisions and `Long?` wire raw bits, converted with `toLong`/`toULong`;
  no signed revision arithmetic or narrowing
- An allocation-free envelope pre-scan before kotlinx creates strings/lists; bounds and exact
  error categories match the frozen policy. Encode size/UTF-16 checks precede serialization.
- A closed `ProtobufResult` success/failure boundary, without networking, retries, FFI or Ktor

Current DTOs have no enum, oneof, packed scalar collection, float or map. They are not invented
for this test. Schema-valid depth is at most one nested issue; deeper messages fail the declared
field shape before the general depth ceiling can be reached. `TrailingBytes` is retained in the
shared category vocabulary but no outer terminator exists: malformed trailing bytes use the
actual scan category; concatenated repeated-only validation responses are one ordered list.

## Dependency evaluation and pins

Kotlin compiler and serialization compiler plugin are **2.2.20**; serialization core, JSON and
Protobuf are consistently **1.9.0**. The existing JSON module moves from 1.7.3 to align its core
runtime with the protobuf pin. Compose, coroutines, Android SDK/AGP and Kotlin/Native versions
are unchanged. Gradle stays **8.14.3**, with the existing distribution SHA-256
`bd71102213493060956ec229d946beee57158dbd89d0e62b91bca0fa2c5f3531`.

Primary references: [1.9.0 release](https://github.com/Kotlin/kotlinx.serialization/releases/tag/v1.9.0),
[ProtoBuf mapping](https://kotlinlang.org/api/kotlinx.serialization/kotlinx-serialization-protobuf/kotlinx.serialization.protobuf/-proto-buf/),
[pinned core metadata](https://repo.maven.apache.org/maven2/org/jetbrains/kotlinx/kotlinx-serialization-core/1.9.0/kotlinx-serialization-core-1.9.0.module),
[pinned protobuf metadata](https://repo.maven.apache.org/maven2/org/jetbrains/kotlinx/kotlinx-serialization-protobuf/1.9.0/kotlinx-serialization-protobuf-1.9.0.module).
The downloaded official metadata declares common/JVM, iOS Arm64 and iOS Simulator Arm64 artifacts;
Android JVM consumers use the JVM variant. Published artifacts establish availability only,
not compilation or runtime acceptance. The library is Apache-2.0 and the format API remains
experimental; opt-in is scoped to codec/fixture files.

## Independent oracle and executed scope

`generateProtobufVectors` invokes the mobile Python embedding primitive and wires its output into
`commonTest` for all configured targets. It reads the one shared corpus without recalculating
expected bytes. There are no checked-in Kotlin copies of goldens. Expected values and bytes come
from the independent Python wire-spec oracle, already cross-checked by generated prost 0.13.5.

- Golden corpus: 13 cases, all four roots, frozen SHA-256
  `f9370eed5574a6a4066dfccbe50a37be6033bf05d76a4973371fa4d48f02e829`
- C1 hostile corpus: 26 exact-category cases at `cb3721d23d248726e23a43d429eb08c6e756578f`, SHA-256
  `bb843b118aba3acdd03029fb3ce6058ca6e01dc49b08290f89866c690ce6d821`
- Focused commonTest compiled on JVM with the official Kotlin compiler and matching plugin,
  core/protobuf jars and JUnit 4.13.2/Hamcrest 1.3: **PASS, 14 tests**
- Each accepted golden decodes to its independent expected value and encodes to its exact bytes.
  Absent/null goldens fail `Shape`, while explicit empty strings and zero retain presence.
- Exact tests cover full uint64, reordered/duplicate/unknown/wrong-type fields, nested presence,
  Unicode and malformed UTF-8, canonical/overflow varints, truncation, repeated merge behavior,
  invalid UTF-16 encoding, inclusive 8 MiB and 65,536-issue limits and their next value.
- 65,536 two-byte envelopes × four roots = 262,144 deterministic closed-outcome checks; 10,000
  reproducible longer envelopes (length 0..256, LCG seed 0x12345678) × four roots = 40,000 checks.
  This is bounded exhaustive/example evidence, not fuzzing, formal proof or Native evidence.

## Platform and layer ledger

| Scope | Result | Evidence / limitation |
| --- | --- | --- |
| L1 boundary | documented | Codec isolated in a transport adapter; no app or domain consumer |
| L2/L3 focused commonTest on JVM | PASS | Compiled, example-tested and differentially-tested; 14 JUnit tests |
| Repository hygiene | PASS | `cargo xtask verify repository`; not an app build |
| Whole Theatre desktop Gradle suite | PASS | 176 tests, zero failures/skips; all 14 protobuf methods executed |
| C4 changed-only preflight against C0 | PASS | Repository, Rust format and theatre-cmp-demo gates |
| Android compile | UNSUPPORTED | xtask reports no installed Android SDK; SDK download disabled |
| Android host/device tests | NOT_RUN | No Android target/test execution or emulator/device |
| iOS compile | UNSUPPORTED | No installed Kotlin/Native; Linux also lacks required macOS/Xcode |
| iOS simulator/device tests | NOT_RUN | Must execute on a supported macOS simulator/device host |
| L4/L5 HTTP/DB/native media | Inapplicable to C4 | No HTTP, storage or media implementation was added |
| Kotlin formatter/linter | NOT_RUN | No configured Kotlin formatter/linter; official style manually reviewed |

The final Gradle XML records the protobuf suite at `2026-10-13T14:48:09.206Z`: 14 tests,
zero errors/failures/skips. Initial environment attempts failed on a stale proxy, a Java runtime
without javac and Skiko extraction into an unwritable default cache (45 rendering failures out
of 176 tests). These were resolved with the execution environment's existing HTTPS proxy,
an isolated official Temurin JDK 21.0.12.1 and Skiko's supported writable data-cache property.
No tests were skipped and no UI or test assertions were changed to make that gate pass.
A later documentation-only preflight rerun encountered a 5-second timeout in the unchanged
repository signal-cleanup test `test_signals_stop_cli_children_and_record_failure`. Its focused
retry passed (one test, including SIGINT/SIGTERM/SIGHUP); no source or timeout was changed.
The earlier full CMP and preflight passes remain attributable to the same codec/test sources.
The JDK vendor checksum was
`ce79869e1307ed8ee1e2baa86a412b1eb5b75d10a01006d788a6f968bcfaee94`.

Android and Apple acceptance belong to the integration/release owner. The commit label requested
by issue #97 does not assert that any Native compiler or runtime passed. JVM execution alone
cannot settle Apple compilation, device integer behavior or Native runtime allocation.

## Reproduction through the existing verification route

From the repository root with installed Rust 1.87.0, Python 3, a full JDK and Gradle 8.14.3:

```sh
cargo xtask doctor
cargo xtask verify repository
cargo xtask theatre-demo verify
cargo xtask preflight --changed-only --base 9f0368e18de867c58da31619b6be171478779e9e
cargo xtask theatre-demo android-build
# macOS + full Xcode + installed Kotlin/Native 2.2.20 only:
cargo xtask theatre-demo ios-build --kotlin-native-home "$KOTLIN_NATIVE_HOME"
```

On a restricted Linux host, configure writable caches without changing system toolchains:

```sh
# JAVA_HOME is an installed full JDK 21; Gradle remains the existing pinned 8.14.3.
export GRADLE_USER_HOME="$BUILD_TOOLS/gradle-home"
mkdir -p "$BUILD_TOOLS/skiko-cache"
export JAVA_TOOL_OPTIONS="-Dskiko.data.path=$BUILD_TOOLS/skiko-cache"
# Use the environment's authorized proxy and existing system certificate trust store where needed.
# No certificate checks are disabled and no new trust roots are installed.
```

C4's isolated worktree uses the committed C1 hostile corpus through the optional Gradle property
`cantos.protobuf.hostileFixture` (`-P` or `-Dorg.gradle.project.cantos.protobuf.hostileFixture`);
the integrated branch uses the standard shared fixture path.
Repository gates remain xtask-owned; the commands below are focused implementation primitives
for reproducing the exact codec-only compiler/test result, not a parallel repository gate:

```sh
# TOOLS: official Kotlin 2.2.20 distribution and core/protobuf JVM 1.9.0 jars.
# TEST_TOOLS: official Maven JUnit 4.13.2 / Hamcrest 1.3 jars; OUT: ignored build directory.
mkdir -p "$OUT"
MOBILE=apps/cantos-theatre/mobile
SRC=$MOBILE/shared/src/commonMain/kotlin/com/cantos/theatre/adapters/protobuf
TEST=$MOBILE/shared/src/commonTest/kotlin/com/cantos/theatre/adapters/protobuf
FIXTURES=packages/contracts/fixtures/protobuf/v1
python3 "$MOBILE/tooling/embed_protobuf_fixtures.py" "$FIXTURES/goldens.json" \
  "$FIXTURES/hostile.json" "$OUT/CanonicalProtobufVectors.kt"
CP=$TOOLS/kotlinx-serialization-core-jvm-1.9.0.jar:$TOOLS/kotlinx-serialization-protobuf-jvm-1.9.0.jar
TCP=$CP:$TOOLS/kotlinc/lib/kotlin-test.jar:$TOOLS/kotlinc/lib/kotlin-test-junit.jar
TCP=$TCP:$TEST_TOOLS/junit-4.13.2.jar:$TEST_TOOLS/hamcrest-core-1.3.jar
"$TOOLS/kotlinc/bin/kotlinc" "$SRC"/*.kt "$TEST"/*.kt "$OUT/CanonicalProtobufVectors.kt" \
  -jvm-target 17 -cp "$TCP" -Xplugin="$TOOLS/kotlinc/lib/kotlinx-serialization-compiler-plugin.jar" -d "$OUT/tests.jar"
"$TOOLS/kotlinc/bin/kotlin" -J-Xmx512m -cp "$OUT/tests.jar:$TCP" \
  org.junit.runner.JUnitCore com.cantos.theatre.adapters.protobuf.StudioPilotCodecTest
```

## Payload measurements: JVM only

One local run on Linux x86_64, Intel Xeon Platinum 8573C, OpenJDK 21.0.12.1; 128..512 MiB heap.
Input is a valid ASCII JSON string `{"text":"..."}` in `ValidateScriptRequest`, with exact UTF-8
sizes 1/8/64/512 KiB. 100 warmups per size; timed iterations 2000/2000/400/100. The measured codec
includes strict scanning, UTF-16 prechecks, adapters and serializer allocation. Decode timing
also compares the decoded value to the expected value. Thread allocation uses the enabled JDK
`ThreadMXBean` current-thread counter; it is not peak heap, retained heap or Native allocation.

| Script bytes | Protobuf bytes | Encode ns/op | Decode ns/op | Encode allocated bytes/op | Decode allocated bytes/op |
| --- | --- | --- | --- | --- | --- |
| 1024 | 1027 | 15636 | 12782 | 4520 | 1488 |
| 8192 | 8195 | 41131 | 16255 | 33192 | 8640 |
| 65536 | 65540 | 267910 | 73108 | 262520 | 65952 |
| 524288 | 524292 | 1491637 | 743480 | 2097528 | 524705 |

This small noisy single-process measurement has no statistical confidence claim. No Kotlin
JSON/MessagePack comparison, compression, transport latency, WASM or Android/iOS performance was
measured here. It supports no CMP speedup inference from Rust measurements or format choice.
The optional measurement primitive is reproducible against the same compiled codec:

```sh
"$TOOLS/kotlinc/bin/kotlinc" "$MOBILE/tooling/ProtobufJvmBenchmark.kt" -cp "$OUT/tests.jar:$CP" \
  -d "$OUT/benchmark.jar"
"$TOOLS/kotlinc/bin/kotlin" -J-Xms128m -J-Xmx512m \
  -cp "$OUT/benchmark.jar:$OUT/tests.jar:$CP" ProtobufJvmBenchmarkKt
```

Residual acceptance: Android compilation/tests, Apple
compilation/tests on macOS, Native/device runtime and platform-specific measurements remain
separate required evidence. No production codec activation is performed by this harness.
