pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }
    plugins {
        kotlin("multiplatform") version "2.2.20"
        kotlin("android") version "2.2.20"
        kotlin("plugin.serialization") version "2.2.20"
        id("org.jetbrains.kotlin.plugin.compose") version "2.2.20"
        id("org.jetbrains.compose") version "1.9.0"
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
        google()
    }
}

rootProject.name = "cantos-theatre-mobile"
include(":shared", ":preview-sandbox")

// Native hosts are explicit opt-ins. Desktop verification needs neither an SDK nor Kotlin/Native.
val androidEnabled = providers.gradleProperty("cantos.android").orNull == "true" ||
    providers.gradleProperty("cantos.native").orNull == "true"
if (androidEnabled) {
    // The one Android application consumes :shared only; :preview-sandbox is JVM desktop tooling.
    include(":androidApp")
}
