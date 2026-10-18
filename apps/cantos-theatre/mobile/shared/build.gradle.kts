import groovy.json.JsonSlurper
import java.security.MessageDigest
import java.util.zip.ZipFile
import org.gradle.kotlin.dsl.withGroovyBuilder
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    kotlin("plugin.serialization")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
    id("app.cash.sqldelight") version "2.1.0"
}

val androidEnabled = providers.gradleProperty("cantos.android").orNull == "true" ||
    providers.gradleProperty("cantos.native").orNull == "true"
val iosEnabled = providers.gradleProperty("cantos.ios").orNull == "true" ||
    providers.gradleProperty("cantos.native").orNull == "true"

// SQLDelight 2.1.0 is pinned for this Kotlin 2.2.20 consumer; see ADR 0026 adoption.
sqldelight {
    databases {
        create("LibraryDatabase") { packageName.set("com.cantos.theatre.db") }
    }
}
val generatedLibraryMain = layout.buildDirectory.dir("generated/libraryWire/commonMain/kotlin")
val generatedLibraryTests = layout.buildDirectory.dir("generated/libraryWire/commonTest/kotlin")
val generateLibraryWire = tasks.register<Exec>("generateLibraryWire") {
    val schema = rootProject.file("../../../packages/contracts/schema/theatre/library/v1.schema.json")
    val fixtures = rootProject.file("../../../packages/contracts/fixtures/theatre/library/v1")
    val generator = rootProject.file("tooling/generate_library_wire.py")
    inputs.files(schema, generator, fileTree(fixtures) { include("*.json") })
    outputs.dirs(generatedLibraryMain, generatedLibraryTests)
    commandLine("python3", generator.absolutePath, schema.absolutePath, fixtures.absolutePath,
        generatedLibraryMain.get().asFile.resolve("com/cantos/theatre/adapters/library/wire/LibraryWire.kt").absolutePath,
        generatedLibraryTests.get().asFile.resolve("com/cantos/theatre/adapters/library/LibraryFixtures.kt").absolutePath)
}

// Tests consume the one reviewed corpus, including Native test sources when that target is
// available. These generated strings preserve input bytes; they never generate expectations.
val playbackCorpus = rootProject.file("../../../packages/contracts/fixtures/theatre/playback/v1")
val generatedPlaybackDirectory = layout.buildDirectory.dir("generated/playbackVectors/commonTest/kotlin")
val generatePlaybackVectors = tasks.register("generatePlaybackVectors") {
    inputs.files(fileTree(playbackCorpus) { include("*.json") })
    outputs.dir(generatedPlaybackDirectory)
    doLast {
        val indexFile = playbackCorpus.resolve("index.json")
        val indexText = indexFile.readText(Charsets.UTF_8)
        val index = JsonSlurper().parseText(indexText) as? Map<*, *>
            ?: error("Playback index must be an object")
        check(index.keys == setOf("vector_format", "contract", "vectors")) { "Unknown playback index field" }
        check(index["vector_format"] == 1 && index["contract"] == "theatre-mock-playback@1") {
            "Unsupported playback corpus version"
        }
        val entries = index["vectors"] as? List<*> ?: error("Playback vectors must be an array")
        check(entries.isNotEmpty()) { "Playback corpus cannot be empty" }
        val jsonByFile = linkedMapOf<String, String>()
        entries.forEach { value ->
            val entry = value as? Map<*, *> ?: error("Playback index entry must be an object")
            check(entry.keys == setOf("file", "sha256")) { "Unknown playback index entry field" }
            val name = entry["file"] as? String ?: error("Playback file name must be a string")
            check(name.matches(Regex("[a-z][a-z-]+\\.json")) && name != "index.json") {
                "Invalid playback corpus file name"
            }
            check(name !in jsonByFile) { "Duplicate playback corpus file" }
            val hash = entry["sha256"] as? String ?: error("Playback hash must be a string")
            check(hash.matches(Regex("[0-9a-f]{64}"))) { "Invalid playback hash" }
            val bytes = playbackCorpus.resolve(name).readBytes()
            val actualHash = MessageDigest.getInstance("SHA-256").digest(bytes)
                .joinToString("") { "%02x".format(it.toInt() and 0xff) }
            check(actualHash == hash) { "Playback corpus hash mismatch: $name" }
            val json = bytes.toString(Charsets.UTF_8)
            check(json.toByteArray(Charsets.UTF_8).contentEquals(bytes)) { "Invalid UTF-8 playback vector" }
            jsonByFile[name] = json
        }
        val actualFiles = playbackCorpus.listFiles()?.filter { it.extension == "json" }
            ?.map { it.name }?.toSet() ?: error("Playback corpus is missing")
        check(actualFiles == jsonByFile.keys + "index.json") { "Unknown/unlisted playback vector" }
        fun quoted(value: String): String = buildString {
            append('"')
            value.forEach { char ->
                append(when (char) {
                    '\\' -> "\\\\"
                    '"' -> "\\\""
                    '$' -> "\\$"
                    '\n' -> "\\n"
                    '\r' -> "\\r"
                    '\t' -> "\\t"
                    else -> char.toString()
                })
            }
            append('"')
        }
        val output = generatedPlaybackDirectory.get().asFile.resolve(
            "com/cantos/theatre/domain/playback/CanonicalPlaybackVectors.kt",
        )
        output.parentFile.mkdirs()
        output.writeText(buildString {
            appendLine("// Generated from the canonical, SHA-256 checked corpus. Do not edit.")
            appendLine("package com.cantos.theatre.domain.playback")
            appendLine("internal object CanonicalPlaybackVectors {")
            appendLine("    val indexJson: String = ${quoted(indexText)}")
            appendLine("    val jsonByFile: Map<String, String> = mapOf(")
            jsonByFile.forEach { (name, json) -> appendLine("        ${quoted(name)} to ${quoted(json)},") }
            appendLine("    )")
            appendLine("}")
        }, Charsets.UTF_8)
    }
}


// Theatre Discovery v1: embed the shared strict schema and every original fixture byte-for-byte
// for the hand-written DTO drift check. Test-only input; it generates no wire type or expectation.
val discoveryContract = rootProject.file("../../../packages/contracts")
val generatedDiscoveryDirectory = layout.buildDirectory.dir("generated/discoveryCorpus/commonTest/kotlin")
val generateDiscoveryCorpus = tasks.register("generateDiscoveryCorpus") {
    val fixtures = discoveryContract.resolve("fixtures/theatre/discovery/v1")
    val schema = discoveryContract.resolve("schema/theatre/discovery/v1.schema.json")
    inputs.files(fileTree(fixtures) { include("*.json") })
    inputs.file(schema)
    outputs.dir(generatedDiscoveryDirectory)
    doLast {
        fun utf8(file: File): String {
            val bytes = file.readBytes()
            val text = bytes.toString(Charsets.UTF_8)
            check(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) { "Invalid UTF-8 discovery input: ${file.name}" }
            return text
        }
        fun quoted(value: String): String = buildString {
            append('"')
            value.forEach { char ->
                append(when (char) {
                    '\\' -> "\\\\"
                    '"' -> "\\\""
                    '$' -> "\\$"
                    '\n' -> "\\n"
                    '\r' -> "\\r"
                    '\t' -> "\\t"
                    else -> char.toString()
                })
            }
            append('"')
        }
        val files = fixtures.listFiles()?.filter { it.isFile && it.extension == "json" }?.sortedBy { it.name }
            ?: error("Discovery fixtures are missing")
        check(files.isNotEmpty()) { "Discovery fixture corpus cannot be empty" }
        val output = generatedDiscoveryDirectory.get().asFile.resolve(
            "com/cantos/theatre/adapters/http/CanonicalDiscoveryCorpus.kt",
        )
        output.parentFile.mkdirs()
        output.writeText(buildString {
            appendLine("// Generated from packages/contracts (Theatre Discovery v1). Do not edit.")
            appendLine("package com.cantos.theatre.adapters.http")
            appendLine("internal object CanonicalDiscoveryCorpus {")
            appendLine("    val schemaJson: String = ${quoted(utf8(schema))}")
            appendLine("    val fixtureJsonByFile: Map<String, String> = mapOf(")
            files.forEach { appendLine("        ${quoted(it.name)} to ${quoted(utf8(it))},") }
            appendLine("    )")
            appendLine("}")
        }, Charsets.UTF_8)
    }
}

// Hand-authored semantic expectations shared with Web; copied into tests, never shipping code.
val generatedMembershipDirectory = layout.buildDirectory.dir("generated/membershipVectors/commonTest/kotlin")
val generateMembershipVectors = tasks.register("generateMembershipVectors") {
    val fixture = rootProject.file("../web/fixtures/membership-interest-v1.json")
    inputs.file(fixture)
    outputs.dir(generatedMembershipDirectory)
    doLast {
        val json = fixture.readText(Charsets.UTF_8)
        val quoted = json.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$")
            .replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")
        val output = generatedMembershipDirectory.get().asFile.resolve(
            "com/cantos/theatre/domain/membership/MembershipVectors.kt",
        )
        output.parentFile.mkdirs()
        output.writeText("// Generated from the shared hand-authored preview corpus. Do not edit.\n" +
            "package com.cantos.theatre.domain.membership\n" +
            "internal const val membershipVectorJson: String = \"$quoted\"\n", Charsets.UTF_8)
    }
}

val generateClientBoundaryVectors = tasks.register("generateClientBoundaryVectors") {
    val fixture = rootProject.file("../../../packages/contracts/fixtures/client-boundary/v1/probe-cases.json")
    val nativeSessionFixture = rootProject.file("../../../packages/contracts/fixtures/theatre-identity/v1/native-session.json")
    // Reviewed canonical bytes; update only with an intentional shared contract-fixture change.
    val nativeSessionSha256 = "bb457e56dcf34c603c73614bdb48238fb438a0e59cbfa9d0cb00fc8a376dd086"
    val bearerFixtureHashes = mapOf(
        "token-pair.json" to "e1807a6bb241721618be6551776e1caaf76c8120c8cc6dd7c08a7d970dabf83f",
        "session.json" to "d6cf5b3fa079d0d75cbe9353a90f036b47bd88edebd5b52964c2296e874cd101",
    )
    val bearerFixtures = rootProject.file("../../../packages/contracts/fixtures/bearer/v1")
    val outputDirectory = layout.buildDirectory.dir("generated/client-boundary/commonTest")
    inputs.file(fixture)
    inputs.file(nativeSessionFixture)
    inputs.property("nativeSessionSha256", nativeSessionSha256)
    inputs.files(bearerFixtureHashes.keys.map(bearerFixtures::resolve))
    inputs.property("bearerFixtureHashes", bearerFixtureHashes)
    outputs.dir(outputDirectory)
    doLast {
        val raw = fixture.readText(Charsets.UTF_8)
        val escaped = raw.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$").replace("\n", "\\n").replace("\r", "\\r")
        val output = outputDirectory.get().asFile.resolve("com/cantos/theatre/adapters/http/CanonicalClientBoundary.kt")
        output.parentFile.mkdirs()
        output.writeText("package com.cantos.theatre.adapters.http\ninternal object CanonicalClientBoundary { val json: String = \"$escaped\" }\n", Charsets.UTF_8)
        val nativeBytes = nativeSessionFixture.readBytes()
        val actualNativeHash = MessageDigest.getInstance("SHA-256").digest(nativeBytes)
            .joinToString("") { "%02x".format(it.toInt() and 0xff) }
        check(actualNativeHash == nativeSessionSha256) { "Canonical native identity fixture hash mismatch" }
        val nativeJson = nativeBytes.toString(Charsets.UTF_8)
        check(nativeJson.toByteArray(Charsets.UTF_8).contentEquals(nativeBytes)) { "Invalid UTF-8 native identity fixture" }
        val nativeEscaped = nativeJson.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$").replace("\n", "\\n").replace("\r", "\\r")
        val nativeOutput = outputDirectory.get().asFile.resolve("com/cantos/theatre/adapters/http/CanonicalNativeIdentity.kt")
        nativeOutput.writeText("package com.cantos.theatre.adapters.http\ninternal object CanonicalNativeIdentity { val json: String = \"$nativeEscaped\" }\n", Charsets.UTF_8)
        val bearerJson = bearerFixtureHashes.mapValues { (name, hash) ->
            val bytes = bearerFixtures.resolve(name).readBytes()
            val actualHash = MessageDigest.getInstance("SHA-256").digest(bytes)
                .joinToString("") { "%02x".format(it.toInt() and 0xff) }
            check(actualHash == hash) { "Canonical Bearer identity fixture hash mismatch: $name" }
            val text = bytes.toString(Charsets.UTF_8)
            check(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) { "Invalid UTF-8 Bearer identity fixture: $name" }
            text.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$").replace("\n", "\\n").replace("\r", "\\r")
        }
        val bearerOutput = outputDirectory.get().asFile.resolve("com/cantos/theatre/adapters/http/CanonicalBearerIdentity.kt")
        bearerOutput.writeText(
            "// Generated original Bearer v1 fixture bytes; no expectations or purpose mapping generated.\n" +
                "package com.cantos.theatre.adapters.http\ninternal object CanonicalBearerIdentity {\n" +
                "val tokenPairJson: String = \"${bearerJson.getValue("token-pair.json")}\"\n" +
                "val sessionJson: String = \"${bearerJson.getValue("session.json")}\"\n}\n",
            Charsets.UTF_8,
        )
    }
}

// Embed the shared independent corpus for every configured test target. Python is a primitive
// of the existing Gradle/xtask verification route, not another repository command surface.
// The Studio pilot codec it exercises is fixture-only too: commonTest, never shipping code.
val protobufCorpus = rootProject.file("../../../packages/contracts/fixtures/protobuf/v1")
val generatedProtobufDirectory = layout.buildDirectory.dir("generated/protobufVectors/commonTest/kotlin")
val generateProtobufVectors = tasks.register<Exec>("generateProtobufVectors") {
    val generator = rootProject.file("tooling/embed_protobuf_fixtures.py")
    val goldens = protobufCorpus.resolve("goldens.json")
    val hostile = providers.gradleProperty("cantos.protobuf.hostileFixture").orNull
        ?.let { file(it) } ?: protobufCorpus.resolve("hostile.json")
    inputs.files(generator, goldens, hostile)
    outputs.dir(generatedProtobufDirectory)
    // Exec never prunes its output: drop vectors left under an earlier package directory.
    doFirst { delete(generatedProtobufDirectory) }
    commandLine(
        "python3", generator.absolutePath, goldens.absolutePath, hostile.absolutePath,
        generatedProtobufDirectory.get().asFile.resolve(
            "com/cantos/theatre/studiopilotfixtures/CanonicalProtobufVectors.kt",
        ).absolutePath,
    )
}

// Artwork stays fixture-only in commonTest: no mobile route or shipping serializer.
val artworkCorpus = rootProject.file("../../../packages/contracts/fixtures/artwork/v1")
val generatedArtworkDirectory = layout.buildDirectory.dir("generated/artworkVectors/commonTest/kotlin")
val generateArtworkVectors = tasks.register<Exec>("generateArtworkVectors") {
    val generator = rootProject.file("tooling/embed_artwork_fixtures.py")
    val goldens = artworkCorpus.resolve("goldens.json")
    val hostile = artworkCorpus.resolve("hostile.json")
    inputs.files(
        generator, goldens, hostile,
        rootProject.file("../../../scripts/protobuf-oracle/Oracle.kt"),
        rootProject.file("../../../scripts/protobuf-oracle/generate_kotlin_vectors.py"),
    )
    outputs.dir(generatedArtworkDirectory)
    commandLine(
        "python3", generator.absolutePath, goldens.absolutePath, hostile.absolutePath,
        generatedArtworkDirectory.get().asFile.resolve(
            "com/cantos/theatre/artworkfixtures/CanonicalArtworkVectors.kt",
        ).absolutePath,
    )
}

if (androidEnabled) {
    // AGP enters the root classpath only for the Android opt-in, keeping desktop configuration
    // independent of AGP and the SDK. This is configuration of its actual extension, not a stub.
    apply(plugin = "com.android.library")
    extensions.getByName("android").withGroovyBuilder {
        setProperty("namespace", "com.cantos.theatre.shared")
        setProperty("compileSdk", 35)
        setProperty("buildToolsVersion", "35.0.0")
        "defaultConfig" {
            // Demo host floor only; production support remains an explicit future decision.
            setProperty("minSdk", 23)
        }
        "compileOptions" {
            setProperty("sourceCompatibility", JavaVersion.VERSION_17)
            setProperty("targetCompatibility", JavaVersion.VERSION_17)
        }
        "buildFeatures" {
            setProperty("compose", true)
        }
    }
}

if (iosEnabled) {
    val nativeHome = providers.gradleProperty("kotlin.native.home").orNull
        ?: error("BLOCKED: iOS requires -Pkotlin.native.home=<installed Kotlin/Native 2.2.20>; no compiler is downloaded.")
    val nativeDirectory = file(nativeHome)
    val compiler = nativeDirectory.resolve("bin/konanc")
    val manifest = nativeDirectory.resolve("klib/common/stdlib/default/manifest")
    val compilerJar = nativeDirectory.resolve("konan/lib/kotlin-native-compiler-embeddable.jar")
    check(compiler.isFile && compiler.canExecute() && manifest.isFile && compilerJar.isFile) {
        "BLOCKED: kotlin.native.home must contain the installed executable, compiler JAR and stdlib; no compiler is downloaded."
    }
    // The stdlib manifest describes its bootstrap compiler, not this installed distribution.
    val compilerVersion = ZipFile(compilerJar).use { archive ->
        archive.getEntry("META-INF/compiler.version")?.let { entry ->
            archive.getInputStream(entry).bufferedReader().use { it.readText().trim() }
        }
    }
    check(compilerVersion == "2.2.20") {
        "BLOCKED: installed Kotlin/Native must be 2.2.20; observed $compilerVersion."
    }
}


kotlin {
    jvm("desktop") {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
        }
    }
    if (androidEnabled) {
        androidTarget {
            compilerOptions {
                jvmTarget.set(JvmTarget.JVM_17)
            }
        }
    }
    if (iosEnabled) {
        listOf(iosArm64(), iosSimulatorArm64()).forEach { target ->
            target.compilerOptions {
                // Missing local LLVM/libffi dependencies fail; they are never downloaded.
                freeCompilerArgs.add("-Xoverride-konan-properties=airplaneMode=true")
            }
            // Shipping framework has no dependency on the separately named sandbox framework.
            target.binaries.framework {
                baseName = "CantosTheatre"
                isStatic = true
                binaryOption("bundleId", "com.cantos.theatre.shared")
            }
        }
    }

    sourceSets {
        commonMain {
            kotlin.srcDir(layout.buildDirectory.dir("generated/libraryWire/commonMain/kotlin"))
            // Consume the canonical source exactly as the isolated Android preview does.
            // App compositions use its theme/tokens; unqualified registered wrappers stay unused.
            kotlin.srcDir(rootProject.file("../../../packages/ui-system/cmp/src/commonMain/kotlin"))
            kotlin.exclude("com/cantos/ui/preview/**")
            dependencies {
                implementation("app.cash.sqldelight:runtime:2.1.0")
                implementation(compose.runtime)
                implementation(compose.components.resources)
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-core:1.9.0")
                implementation(compose.foundation)
                implementation(compose.ui)
                implementation("org.jetbrains.compose.material3:material3:1.8.2")
                // Official outlined icon vectors, published for common/JVM/Android/iOS targets.
                // This pinned artifact adds no Cantos component or shared registry declaration.
                implementation("org.jetbrains.compose.material:material-icons-extended:1.7.3")
            }
        }
        commonTest {
            kotlin.srcDir(generateLibraryWire.map { generatedLibraryTests.get() })
            kotlin.srcDir(generatePlaybackVectors)
            kotlin.srcDir(generateMembershipVectors)
            kotlin.srcDir(generateClientBoundaryVectors)
            kotlin.srcDir(generateProtobufVectors)
            kotlin.srcDir(generateArtworkVectors)
            kotlin.srcDir(generateDiscoveryCorpus)
            dependencies {
                implementation(kotlin("test"))
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.8.1")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
                // Protobuf serves only the Studio pilot and artwork fixtures; shipping code never links it.
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-protobuf:1.9.0")
            }
        }
        val desktopMain by getting {
            kotlin.srcDir("src/jvmHttpMain/kotlin")
            dependencies {
                implementation("app.cash.sqldelight:sqlite-driver:2.1.0")
                implementation(compose.desktop.currentOs)
            }
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test-junit"))
                implementation(compose.desktop.uiTestJUnit4)
            }
        }
        if (iosEnabled) {
            val iosMain by getting { dependencies { implementation("app.cash.sqldelight:native-driver:2.1.0") } }
        }
        if (androidEnabled) {
            val androidMain by getting {
                kotlin.srcDir("src/jvmHttpMain/kotlin")
                dependencies { implementation("app.cash.sqldelight:android-driver:2.1.0") }
            }
        }
    }
}


compose.resources {
    packageOfResClass = "com.cantos.theatre.resources"
}

// Inspect the actual resolved library graph before compiling shared or exercising its tests.
// This proves only :shared's configured boundaries; a future production host still needs its
// own release graph plus APK/AAB class/resource inspection.
val assertProductionSafeDependencies = tasks.register("assertProductionSafeDependencies") {
    group = "verification"
    description = "Reject sandbox and test-only dependencies from shared main classpaths."
    val report = layout.buildDirectory.file("reports/production-boundary/resolved-dependencies.txt")
    outputs.file(report)
    // Resolve on every invocation: dependency metadata can change without a source edit.
    outputs.upToDateWhen { false }
    doLast {
        val required = mutableListOf(
            listOf("desktopCompileClasspath"),
            listOf("desktopRuntimeClasspath"),
        )
        if (androidEnabled) {
            required += listOf("releaseCompileClasspath", "androidReleaseCompileClasspath")
            required += listOf("releaseRuntimeClasspath", "androidReleaseRuntimeClasspath")
        }
        val lines = mutableListOf("Production-safe library dependency boundary: :shared")
        val violations = mutableListOf<String>()
        required.forEach { names ->
            val configuration = names.firstNotNullOfOrNull { configurations.findByName(it) }
                ?: error("Missing required main configuration: ${names.joinToString(" or ")}")
            check(configuration.isCanBeResolved) { "Main configuration is not resolvable: ${configuration.name}" }
            lines += "configuration ${configuration.name}"
            val unresolved = configuration.incoming.resolutionResult.allDependencies
                .filterIsInstance<org.gradle.api.artifacts.result.UnresolvedDependencyResult>()
            check(unresolved.isEmpty()) {
                "Incomplete shared main graph for ${configuration.name}: " +
                    unresolved.joinToString { "${it.attempted.displayName}: ${it.failure.message}" }
            }
            configuration.incoming.resolutionResult.allComponents.forEach { component ->
                val id = component.id
                lines += "  ${id.displayName}"
                when (id) {
                    is org.gradle.api.artifacts.component.ProjectComponentIdentifier -> {
                        if (id.projectPath.split(':').any { it.contains("sandbox", ignoreCase = true) || it.contains("mock", ignoreCase = true) }) {
                            violations += "${configuration.name}: ${id.displayName}"
                        }
                    }
                    is org.gradle.api.artifacts.component.ModuleComponentIdentifier -> {
                        if (id.module.startsWith("okio-fakefilesystem") || id.module.startsWith("ktor-client-mock") ||
                            id.module.startsWith("kotlin-test") || id.module.startsWith("ui-test") ||
                            id.module.startsWith("kotlinx-coroutines-test") || id.module.startsWith("mockito-") ||
                            id.group == "io.mockk" || id.group == "io.kotest" ||
                            id.group == "junit" || id.group.startsWith("org.junit")) {
                            violations += "${configuration.name}: ${id.displayName}"
                        }
                    }
                }
            }
        }
        report.get().asFile.apply { parentFile.mkdirs(); writeText(lines.joinToString("\n", postfix = "\n")) }
        lines.filter { it.startsWith("configuration ") }.forEach(::println)
        println("Shared dependency boundary report: ${report.get().asFile}")
        check(violations.isEmpty()) { "Forbidden shared main dependencies: ${violations.joinToString()}" }
    }
}

tasks.configureEach {
    if (name == "desktopTest" || name.startsWith("assemble") ||
        (name.startsWith("compile") && !name.contains("Test", ignoreCase = true))) {
        dependsOn(assertProductionSafeDependencies)
        dependsOn(generateLibraryWire)
    }
}

// Opt-in live catalog walk (PublicCatalogLiveOriginTest). Unset by default: the test is skipped
// and verification stays hermetic. When set, never reuse an up-to-date result and show output.
val liveCatalogOrigin = providers.environmentVariable("CANTOS_THEATRE_LIVE_ORIGIN").orElse("")
tasks.matching { it.name == "desktopTest" }.configureEach {
    val test = this as Test
    test.inputs.property("cantosTheatreLiveOrigin", liveCatalogOrigin)
    if (liveCatalogOrigin.get().isNotEmpty()) {
        test.outputs.upToDateWhen { false }
        test.testLogging.showStandardStreams = true
    }
}
