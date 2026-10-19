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
        manifestPlaceholders["cantosNativeIssuer"] = providers.gradleProperty("cantos.nativeIssuer")
            .orElse(providers.environmentVariable("CANTOS_NATIVE_ISSUER"))
            .orElse("").get()
    }
    buildTypes {
        // One real-only application, two build types and no flavors. Debug is a side-by-side
        // developer identity with its own app data and callback scheme (NativeCallbackProfile).
        debug {
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-dev"
        }
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
    // Stable compat release; the launch window is the application's, not a second Activity.
    implementation("androidx.core:core-splashscreen:1.0.1")
}

// Inspect the resolved graphs of BOTH build types before they are built. Local projects are an
// allowlist (:shared only, so the preview sandbox is unreachable from debug and release); external
// test/mock libraries are rejected from compile and runtime classpaths, transitively. Debug keeps
// no ui-tooling runtime either. Test classpaths are separate and are not inspected here. Packaged
// bytes are inspected separately by `cargo xtask theatre-host android-release`.
fun registerBoundary(variant: String) = tasks.register("assert${variant.replaceFirstChar { it.uppercase() }}DependencyBoundary") {
    group = "verification"
    description = "Reject every local project except :shared and all test/mock libraries from $variant classpaths."
    val report = layout.buildDirectory.file("reports/$variant-boundary/resolved-dependencies.txt")
    outputs.file(report)
    // Resolve on every invocation: dependency metadata can change without a source edit.
    outputs.upToDateWhen { false }
    val hostPath = project.path
    doLast {
        val allowedProjects = setOf(hostPath, ":shared")
        val lines = mutableListOf("Production Android $variant dependency boundary: $hostPath")
        val violations = mutableListOf<String>()
        listOf("${variant}CompileClasspath", "${variant}RuntimeClasspath").forEach { name ->
            val configuration = configurations.findByName(name) ?: error("Missing required $variant configuration: $name")
            check(configuration.isCanBeResolved) { "$variant configuration is not resolvable: $name" }
            lines += "configuration $name"
            val unresolved = configuration.incoming.resolutionResult.allDependencies
                .filterIsInstance<org.gradle.api.artifacts.result.UnresolvedDependencyResult>()
            check(unresolved.isEmpty()) {
                "Incomplete $variant graph for $name: " +
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
        println("Production $variant dependency report: ${report.get().asFile}")
        check(violations.isEmpty()) { "Forbidden production $variant dependencies: ${violations.joinToString()}" }
    }
}

val assertDebugDependencyBoundary = registerBoundary("debug")
val assertReleaseDependencyBoundary = registerBoundary("release")
// Each variant audits only itself, so a debug build never assembles or audits release.
tasks.matching { it.name == "preDebugBuild" }.configureEach { dependsOn(assertDebugDependencyBoundary) }
tasks.matching { it.name == "preReleaseBuild" }.configureEach { dependsOn(assertReleaseDependencyBoundary) }
tasks.register("assertDependencyBoundaries") {
    group = "verification"
    description = "Compatibility aggregate: audit the debug and release dependency boundaries."
    dependsOn(assertDebugDependencyBoundary, assertReleaseDependencyBoundary)
}
