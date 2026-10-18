import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    // Root buildscript supplies pinned AGP 8.11.1 only when Android is selected.
    id("com.android.application")
    kotlin("android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.cantos.theatre"
    compileSdk = 35
    buildToolsVersion = "35.0.0"
    defaultConfig {
        applicationId = "com.cantos.theatre"
        // The shared library floor, not a production platform support decision (#14 ADR).
        minSdk = 23
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        // Public API origin for the live catalog in both debug and release. Explicit Gradle property
        // overrides the environment (an empty property opts out). Manifest merger substitutes
        // the attribute; no source is generated. Runtime admits https:// for any build and
        // loopback http:// only for a debuggable build with the debug network config.
        manifestPlaceholders["cantosApiOrigin"] = providers.gradleProperty("cantos.apiOrigin")
            .orElse(providers.environmentVariable("CANTOS_API_ORIGIN"))
            .orElse("").get()
    }
    buildTypes {
        release {
            // Inspect unshrunk release bytes: sandbox absence must come from the dependency
            // graph, never from R8. Shrinking and signing are separate production decisions.
            isMinifyEnabled = false
            isShrinkResources = false
        }
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
    // Production-safe shared code only. The demo sandbox and its hosts are never reachable.
    implementation(project(":shared"))
    // Android-only Activity host and system Back; no media or diagnostic dependency. Catalog
    // HTTP uses the platform HttpURLConnection transport already in :shared.
    implementation("androidx.activity:activity-compose:1.10.1")
}

// Inspect the resolved release graphs before any build of this host. Local projects are an
// allowlist; external test/mock libraries are rejected. Packaged bytes are inspected separately
// by `cargo xtask theatre-host android-release`.
val assertReleaseDependencyBoundary = tasks.register("assertReleaseDependencyBoundary") {
    group = "verification"
    description = "Reject every local project except :shared and all test/mock libraries from release classpaths."
    val report = layout.buildDirectory.file("reports/release-boundary/resolved-dependencies.txt")
    outputs.file(report)
    // Resolve on every invocation: dependency metadata can change without a source edit.
    outputs.upToDateWhen { false }
    val hostPath = project.path
    doLast {
        val allowedProjects = setOf(hostPath, ":shared")
        val lines = mutableListOf("Production Android release dependency boundary: $hostPath")
        val violations = mutableListOf<String>()
        listOf("releaseCompileClasspath", "releaseRuntimeClasspath").forEach { name ->
            val configuration = configurations.findByName(name) ?: error("Missing required release configuration: $name")
            check(configuration.isCanBeResolved) { "Release configuration is not resolvable: $name" }
            lines += "configuration $name"
            val unresolved = configuration.incoming.resolutionResult.allDependencies
                .filterIsInstance<org.gradle.api.artifacts.result.UnresolvedDependencyResult>()
            check(unresolved.isEmpty()) {
                "Incomplete release graph for $name: " +
                    unresolved.joinToString { "${it.attempted.displayName}: ${it.failure.message}" }
            }
            configuration.incoming.resolutionResult.allComponents.forEach { component ->
                val id = component.id
                lines += "  ${id.displayName}"
                when (id) {
                    is org.gradle.api.artifacts.component.ProjectComponentIdentifier ->
                        if (id.projectPath !in allowedProjects) violations += "$name: ${id.displayName}"
                    is org.gradle.api.artifacts.component.ModuleComponentIdentifier ->
                        if (id.module.startsWith("okio-fakefilesystem") || id.module.startsWith("ktor-client-mock") ||
                            id.module.startsWith("kotlin-test") || id.module.startsWith("ui-test") ||
                            id.module.startsWith("ui-tooling") ||
                            id.module.startsWith("kotlinx-coroutines-test") || id.module.startsWith("mockito-") ||
                            id.group == "io.mockk" || id.group == "io.kotest" ||
                            id.group == "junit" || id.group.startsWith("org.junit")) {
                            violations += "$name: ${id.displayName}"
                        }
                }
            }
        }
        report.get().asFile.apply { parentFile.mkdirs(); writeText(lines.joinToString("\n", postfix = "\n")) }
        lines.filter { it.startsWith("configuration ") }.forEach(::println)
        println("Production release dependency report: ${report.get().asFile}")
        check(violations.isEmpty()) { "Forbidden production release dependencies: ${violations.joinToString()}" }
    }
}

tasks.named("preBuild") { dependsOn(assertReleaseDependencyBoundary) }
