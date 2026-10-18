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
    // The optimized demo/QA lane (#245 part C). Its signing key is operator-provided through the
    // environment only: this build never creates, derives or falls back to any key. A missing or
    // partial set leaves the APK unsigned, and `packageQa` refuses to run, so nothing installable
    // is produced without the operator's key. Passwords are never Gradle properties (process lists).
    val qaSigningVariables = listOf(
        "CANTOS_QA_KEYSTORE", "CANTOS_QA_KEYSTORE_PASSWORD", "CANTOS_QA_KEY_ALIAS", "CANTOS_QA_KEY_PASSWORD",
    )
    val qaSigning = qaSigningVariables.associateWith { name ->
        providers.environmentVariable(name).orNull?.takeIf { it.isNotEmpty() }
    }
    val qaSigningComplete = qaSigning.values.all { it != null }
    if (qaSigningComplete) {
        signingConfigs.create("qa") {
            storeFile = file(qaSigning.getValue("CANTOS_QA_KEYSTORE")!!)
            storePassword = qaSigning.getValue("CANTOS_QA_KEYSTORE_PASSWORD")
            keyAlias = qaSigning.getValue("CANTOS_QA_KEY_ALIAS")
            keyPassword = qaSigning.getValue("CANTOS_QA_KEY_PASSWORD")
        }
    }
    buildTypes {
        // Release-like twin of the Debug demo: the same host, composition and mock scenarios, but
        // non-debuggable with R8 and resource shrinking, and no `ui-tooling` runtime
        // (`debugImplementation` only). Distributed only by `distribute firebase --channel demo-qa`.
        create("qa") {
            initWith(getByName("release"))
            matchingFallbacks += listOf("release")
            isDebuggable = false
            isMinifyEnabled = true
            isShrinkResources = true
            versionNameSuffix = "-qa"
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("qa")
        }
        // Unshrunk, unsigned audit twin of `qa`: R8 must never be the thing that makes a dependency
        // boundary hold (#122). Not distributable: it is unsigned and its version label differs.
        create("qaAudit") {
            initWith(getByName("qa"))
            matchingFallbacks += listOf("release")
            isMinifyEnabled = false
            isShrinkResources = false
            versionNameSuffix = "-qa-audit"
            signingConfig = null
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
    implementation(project(":shared"))
    implementation(project(":preview-sandbox"))
    // Android-only Activity host and native system Back integration, no media dependency.
    implementation("androidx.activity:activity-compose:1.10.1")
    // Stable compat release; the existing diagnostic host floor is API 23.
    implementation("androidx.core:core-splashscreen:1.0.1")
    debugImplementation("androidx.compose.ui:ui-tooling:1.9.0")
}

tasks.matching { it.name == "packageQa" }.configureEach {
    doFirst {
        val missing = listOf(
            "CANTOS_QA_KEYSTORE", "CANTOS_QA_KEYSTORE_PASSWORD", "CANTOS_QA_KEY_ALIAS", "CANTOS_QA_KEY_PASSWORD",
        ).filter { providers.environmentVariable(it).orNull.isNullOrEmpty() }
        check(missing.isEmpty()) {
            "The QA APK needs the operator's signing key; set ${missing.joinToString()}. " +
                "No key is created or substituted."
        }
    }
}

// Inspect the resolved QA graph before it is built. The demo host intentionally composes the mock
// sandbox, so the allowlist is :shared and :preview-sandbox; test libraries and the Compose tooling
// runtime are rejected. The annotation-only `ui-tooling-preview` is a different artifact and allowed.
val assertQaDependencyBoundary = tasks.register("assertQaDependencyBoundary") {
    group = "verification"
    description = "Reject test libraries and the ui-tooling runtime from the QA classpaths."
    val report = layout.buildDirectory.file("reports/qa-boundary/resolved-dependencies.txt")
    outputs.file(report)
    outputs.upToDateWhen { false }
    val hostPath = project.path
    doLast {
        val allowedProjects = setOf(hostPath, ":shared", ":preview-sandbox")
        val toolingRuntime = Regex("ui-tooling(-android|-data(-android)?)?")
        val lines = mutableListOf("Demo QA dependency boundary: $hostPath")
        val violations = mutableListOf<String>()
        listOf("qaCompileClasspath", "qaRuntimeClasspath").forEach { name ->
            val configuration = configurations.findByName(name) ?: error("Missing required QA configuration: $name")
            check(configuration.isCanBeResolved) { "QA configuration is not resolvable: $name" }
            lines += "configuration $name"
            val unresolved = configuration.incoming.resolutionResult.allDependencies
                .filterIsInstance<org.gradle.api.artifacts.result.UnresolvedDependencyResult>()
            check(unresolved.isEmpty()) {
                "Incomplete QA graph for $name: " +
                    unresolved.joinToString { "${it.attempted.displayName}: ${it.failure.message}" }
            }
            configuration.incoming.resolutionResult.allComponents.forEach { component ->
                val id = component.id
                lines += "  ${id.displayName}"
                when (id) {
                    is org.gradle.api.artifacts.component.ProjectComponentIdentifier ->
                        if (id.projectPath !in allowedProjects) violations += "$name: ${id.displayName}"
                    is org.gradle.api.artifacts.component.ModuleComponentIdentifier ->
                        if (toolingRuntime.matches(id.module) ||
                            id.module.startsWith("okio-fakefilesystem") || id.module.startsWith("ktor-client-mock") ||
                            id.module.startsWith("kotlin-test") || id.module.startsWith("ui-test") ||
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
        check(violations.isEmpty()) { "Forbidden QA dependencies: ${violations.joinToString()}" }
    }
}

tasks.matching { it.name == "preQaBuild" || it.name == "preQaAuditBuild" }.configureEach {
    dependsOn(assertQaDependencyBoundary)
}
