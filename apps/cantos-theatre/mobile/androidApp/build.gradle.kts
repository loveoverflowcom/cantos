import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    // Root buildscript supplies pinned AGP 8.11.1 only when Android is selected.
    id("com.android.application")
    kotlin("android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.cantos.theatre.demo"
    compileSdk = 35
    buildToolsVersion = "35.0.0"
    defaultConfig {
        applicationId = "com.cantos.theatre.demo"
        // Mock host floor, not the production platform support decision.
        minSdk = 23
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0-demo"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // Optional stable demo key for tester distribution. Supplied only through Gradle properties
    // (for example ORG_GRADLE_PROJECT_cantosDemoStoreFile in the environment) from outside Git;
    // without them the AGP debug keystore signs as before. Production signing is separate.
    val demoStoreFile = providers.gradleProperty("cantosDemoStoreFile").orNull
    if (demoStoreFile != null) {
        signingConfigs.getByName("debug") {
            storeFile = file(demoStoreFile)
            storePassword = providers.gradleProperty("cantosDemoStorePassword").get()
            keyAlias = providers.gradleProperty("cantosDemoKeyAlias").get()
            keyPassword = providers.gradleProperty("cantosDemoKeyPassword").get()
        }
    }
    buildFeatures {
        compose = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(project(":shared"))
    implementation(project(":preview-sandbox"))
    // Android-only Activity host and native system Back integration, no media dependency.
    implementation("androidx.activity:activity-compose:1.10.1")
    // Stable compat release; the existing diagnostic host floor is API 23.
    implementation("androidx.core:core-splashscreen:1.0.1")
    debugImplementation("androidx.compose.ui:ui-tooling:1.9.0")
}
