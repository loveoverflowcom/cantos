import groovy.json.JsonSlurper
import java.security.MessageDigest
import java.util.zip.ZipFile
import org.gradle.kotlin.dsl.withGroovyBuilder
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

val androidEnabled = providers.gradleProperty("cantos.android").orNull == "true" ||
    providers.gradleProperty("cantos.native").orNull == "true"
val iosEnabled = providers.gradleProperty("cantos.ios").orNull == "true" ||
    providers.gradleProperty("cantos.native").orNull == "true"

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


val generateClientBoundaryVectors = tasks.register("generateClientBoundaryVectors") {
    val fixture = rootProject.file("../../../packages/contracts/fixtures/client-boundary/v1/probe-cases.json")
    val outputDirectory = layout.buildDirectory.dir("generated/client-boundary/commonTest")
    inputs.file(fixture)
    outputs.dir(outputDirectory)
    doLast {
        val raw = fixture.readText(Charsets.UTF_8)
        val escaped = raw.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$").replace("\n", "\\n").replace("\r", "\\r")
        val output = outputDirectory.get().asFile.resolve("com/cantos/theatre/adapters/http/CanonicalClientBoundary.kt")
        output.parentFile.mkdirs()
        output.writeText("package com.cantos.theatre.adapters.http\ninternal object CanonicalClientBoundary { val json: String = \"$escaped\" }\n", Charsets.UTF_8)
    }
}

if (androidEnabled) {
    // AGP enters the root classpath only for the Android opt-in, keeping desktop configuration
    // independent of AGP and the SDK. This is configuration of its actual extension, not a stub.
    apply(plugin = "com.android.library")
    extensions.getByName("android").withGroovyBuilder {
        setProperty("namespace", "com.cantos.theatre.demo.shared")
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

// Match the supervised foreground JVM. A detached single-use Gradle daemon must not open a
// demo window outside xtask's owned process group, even when Gradle received --no-daemon.
if (gradle.startParameter.taskNames.any { it.substringAfterLast(':') == "run" }) {
    val expectedLauncherPid = System.getProperty("cantos.theatre.demo.launcherPid")?.toLongOrNull()
    check(expectedLauncherPid != null && expectedLauncherPid == ProcessHandle.current().pid()) {
        "Cantos demo run requires its owned foreground launcher PID. Use cargo xtask theatre-demo run."
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
            target.binaries.framework {
                baseName = "CantosTheatreDemo"
                isStatic = true
                binaryOption("bundleId", "com.cantos.theatre.demo.shared")
            }
        }
    }

    sourceSets {
        commonMain {
            // Consume the canonical source exactly as the isolated Android preview does.
            // App compositions use its theme/tokens; unqualified registered wrappers stay unused.
            kotlin.srcDir(rootProject.file("../../../packages/ui-system/cmp/src/commonMain/kotlin"))
            kotlin.exclude("com/cantos/ui/preview/**")
            dependencies {
                implementation(compose.runtime)
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
                implementation(compose.foundation)
                implementation(compose.ui)
                implementation("org.jetbrains.compose.material3:material3:1.8.2")
                // Official outlined icon vectors, published for common/JVM/Android/iOS targets.
                // This pinned artifact adds no Cantos component or shared registry declaration.
                implementation("org.jetbrains.compose.material:material-icons-extended:1.7.3")
            }
        }
        commonTest {
            kotlin.srcDir(generatePlaybackVectors)
            kotlin.srcDir(generateClientBoundaryVectors)
            dependencies {
                implementation(kotlin("test"))
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.8.1")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
            }
        }
        val desktopMain by getting {
            kotlin.srcDir("src/jvmHttpMain/kotlin")
            dependencies {
                implementation(compose.desktop.currentOs)
            }
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test-junit"))
                implementation(compose.desktop.uiTestJUnit4)
            }
        }
        if (androidEnabled) {
            val androidMain by getting {
                kotlin.srcDir("src/jvmHttpMain/kotlin")
                dependencies {
                    implementation("androidx.compose.ui:ui-tooling-preview:1.9.0")
                }
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "com.cantos.theatre.app.MainKt"
    }
}
