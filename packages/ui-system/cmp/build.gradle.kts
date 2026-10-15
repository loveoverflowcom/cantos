import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform") version "2.2.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.20"
    id("org.jetbrains.compose") version "1.9.0"
}

// Gradle 8.14.3 may fork a detached single-use daemon even with --no-daemon. Fail before any
// run-task build/window work when it does. The repository's fixed POSIX exec chain supplies
// the owned launcher's PID and disables configuration caching so this guard runs every time.
// A forwarded property in a detached JVM still carries the original PID and fails this check.
// JVM matching reduces the chance of a fork, but is not itself a lifecycle guarantee.
if (gradle.startParameter.taskNames.any { it.substringAfterLast(':') == "run" }) {
    val expectedLauncherPid = System.getProperty("cantos.ui.preview.launcherPid")?.toLongOrNull()
    check(expectedLauncherPid != null && expectedLauncherPid == ProcessHandle.current().pid()) {
        "CMP preview run requires its owned foreground launcher PID; refusing a missing, invalid or detached JVM. Use cargo xtask ui-preview cmp run."
    }
}

// The shared, byte-identical Noto bundle (provenance and OFL licence included) is copied, never
// duplicated in Git, under one namespaced classpath root for the desktop host and its tests.
val syncPreviewFonts by tasks.registering(Sync::class) {
    from(layout.projectDirectory.dir("../shared/assets/noto-fonts")) {
        include("*.ttf", "OFL.txt", "provenance.json")
    }
    into(layout.buildDirectory.dir("generated/preview-fonts/cantos-preview-fonts"))
}

kotlin {
    jvm("desktop") {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
        }
    }
    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.ui)
            // Compose 1.9 defaults to this stable Material3 version. No Expressive alpha APIs.
            implementation("org.jetbrains.compose.material3:material3:1.8.2")
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
        val desktopMain by getting {
            dependencies {
                implementation(compose.desktop.currentOs)
            }
            resources.srcDir(files(layout.buildDirectory.dir("generated/preview-fonts")).builtBy(syncPreviewFonts))
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test-junit"))
                implementation(compose.desktop.uiTestJUnit4)
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "com.cantos.ui.preview.MainKt"
    }
}

// Do not select an auto-downloading JDK toolchain. The repository doctor checks local Java.
