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
                implementation(compose.foundation)
                implementation(compose.ui)
                implementation("org.jetbrains.compose.material3:material3:1.8.2")
                // Official outlined icon vectors, published for common/JVM/Android/iOS targets.
                // This pinned artifact adds no Cantos component or shared registry declaration.
                implementation("org.jetbrains.compose.material:material-icons-extended:1.7.3")
            }
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
        val desktopMain by getting {
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
