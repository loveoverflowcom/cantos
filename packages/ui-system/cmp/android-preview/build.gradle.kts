import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform") version "2.2.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.20"
    id("org.jetbrains.compose") version "1.9.0"
    id("com.android.library") version "8.11.1"
}

// A separate diagnostic project keeps Android SDK prerequisites out of the desktop host.
// Compile the actual shared wrappers and fixtures; never maintain an Android copy of them.
kotlin {
    androidTarget {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
        }
    }
    sourceSets {
        val commonMain by getting {
            kotlin.srcDir("../src/commonMain/kotlin")
            dependencies {
                implementation(compose.runtime)
                implementation(compose.foundation)
                implementation(compose.ui)
                implementation("org.jetbrains.compose.material3:material3:1.8.2")
            }
        }
        androidMain.dependencies {
            implementation("androidx.compose.ui:ui-tooling-preview:1.9.0")
        }
    }
}

android {
    namespace = "com.cantos.ui.androidpreview"
    compileSdk = 35
    defaultConfig {
        // A diagnostic-host floor, not a production mobile support decision.
        minSdk = 23
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
}

dependencies {
    add("debugImplementation", "androidx.compose.ui:ui-tooling:1.9.0")
}

// The host uses the installed JDK/SDK; no wrapper or toolchain provisioning is configured.
