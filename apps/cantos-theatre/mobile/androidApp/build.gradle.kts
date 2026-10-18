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
        // Public origins only. Explicit Gradle properties override the environment, including empty opt-out.
        // Manifest merger substitutes attribute values; no source or XML string is assembled from inputs.
        manifestPlaceholders["cantosApiOrigin"] = providers.gradleProperty("cantos.apiOrigin")
            .orElse(providers.environmentVariable("CANTOS_API_ORIGIN"))
            .orElse("").get()
        manifestPlaceholders["cantosNativeIssuer"] = providers.gradleProperty("cantos.nativeIssuer")
            .orElse(providers.environmentVariable("CANTOS_NATIVE_ISSUER"))
            .orElse("").get()
    }
    sourceSets {
        // The launcher icon, colors and launch theme are the application's; this mock host reuses them.
        getByName("main").res.srcDir("../androidProductionApp/src/main/res")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
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
