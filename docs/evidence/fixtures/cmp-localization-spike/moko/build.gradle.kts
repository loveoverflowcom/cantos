plugins {
    kotlin("multiplatform") version "2.2.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.20"
    id("org.jetbrains.compose") version "1.9.0"
    id("dev.icerock.mobile.multiplatform-resources") version "0.27.1"
}
kotlin {
    jvm("desktop")
    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(compose.components.resources)
            implementation("dev.icerock.moko:resources:0.27.1")
            implementation("dev.icerock.moko:resources-compose:0.27.1")
        }
        val desktopMain by getting { dependencies { implementation(compose.desktop.currentOs) } }
        val desktopTest by getting { dependencies { implementation(kotlin("test-junit")); implementation(compose.desktop.uiTestJUnit4) } }
    }
}
multiplatformResources { resourcesPackage.set("spike.moko.resources") }
