import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform") version "2.2.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.20"
    id("org.jetbrains.compose") version "1.9.0"
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
