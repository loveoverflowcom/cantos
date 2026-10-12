buildscript {
    val androidEnabled = providers.gradleProperty("cantos.android").orNull == "true" ||
        providers.gradleProperty("cantos.native").orNull == "true"
    if (androidEnabled) {
        repositories {
            google()
            mavenCentral()
        }
        dependencies {
            // The existing Android UI-system host uses this exact AGP version.
            classpath("com.android.tools.build:gradle:8.11.1")
        }
    }
}

plugins {
    kotlin("multiplatform") apply false
    kotlin("android") apply false
    id("org.jetbrains.kotlin.plugin.compose") apply false
    id("org.jetbrains.compose") apply false
}
