plugins {
    kotlin("multiplatform") version "2.2.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.20"
    id("org.jetbrains.compose") version "1.9.0"

}
kotlin {
    jvm("desktop")
    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(compose.components.resources)
            implementation("cafe.adriel.lyricist:lyricist:1.8.0")
        }
        val desktopMain by getting { dependencies { implementation(compose.desktop.currentOs) } }
        val desktopTest by getting { dependencies { implementation(kotlin("test-junit")); implementation(compose.desktop.uiTestJUnit4) } }
    }
}
