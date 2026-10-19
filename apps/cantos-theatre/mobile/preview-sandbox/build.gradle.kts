import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    kotlin("plugin.serialization")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

// Match the supervised foreground JVM. A detached single-use Gradle daemon must not open a
// preview window outside xtask's owned process group, even when Gradle received --no-daemon.
if (gradle.startParameter.taskNames.any { it.substringAfterLast(':') == "run" }) {
    val expectedLauncherPid = System.getProperty("cantos.theatre.preview.launcherPid")?.toLongOrNull()
    check(expectedLauncherPid != null && expectedLauncherPid == ProcessHandle.current().pid()) {
        "Cantos Desktop Preview run requires its owned foreground launcher PID. Use cargo xtask theatre-preview run."
    }
}

kotlin {
    jvm("desktop") {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
        }
    }

    sourceSets {
        commonMain {
            dependencies {
                api(project(":shared"))
                implementation(compose.runtime)
                implementation(compose.components.resources)
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-core:1.9.0")
                implementation(compose.foundation)
                implementation(compose.ui)
                implementation("org.jetbrains.compose.material3:material3:1.8.2")
                // Glyphs come from the shared offline registry (CantosIcons); the large
                // material-icons-extended artifact is deliberately not a dependency (#245).
            }
        }
        commonTest {
            dependencies {
                implementation(kotlin("test"))
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.8.1")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
            }
        }
        val desktopMain by getting {
            // Only the diagnostic host consumes preview settings; Android/iOS and commonMain
            // keep the existing exclusion of the entire shared preview package.
            kotlin.srcDir(rootProject.file("../../../packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/workbench/settings"))
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
    }
}

compose.desktop {
    application {
        mainClass = "com.cantos.theatre.app.MainKt"
    }
}

compose.resources {
    packageOfResClass = "com.cantos.theatre.preview.resources"
}
