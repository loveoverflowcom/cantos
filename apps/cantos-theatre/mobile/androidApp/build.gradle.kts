import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.gradle.api.file.Directory
import org.gradle.api.file.RegularFile
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import com.android.build.api.artifact.ScopedArtifact
import com.android.build.api.variant.ScopedArtifacts

// Explicit audit of the SAME release variant, never a third build type. Only the app output
// root, optimization flags and manifest audit marker differ; shared compilation is unchanged.
val releaseAudit = providers.gradleProperty("cantos.releaseAudit").orNull.let {
    check(it == null || it == "true" || it == "false") { "Invalid cantos.releaseAudit" }
    it == "true"
}
val releaseSigning = providers.gradleProperty("cantos.releaseSigning").orNull.let {
    check(it == null || it == "true" || it == "false") { "Invalid cantos.releaseSigning" }
    it == "true"
}
if (releaseAudit) layout.buildDirectory.set(layout.projectDirectory.dir("build-audit"))

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
        // Optional API origin override for the live catalog in both debug and release. Explicit
        // Gradle property overrides the environment. An empty value means "no override": the app
        // resolves the production default in Kotlin (`selectCatalogOrigin`, #298), the one owner.
        // Manifest merger substitutes the attribute; no source is generated. Runtime admits https:// for any build and
        // loopback http:// only for a debuggable build with the debug network config.
        manifestPlaceholders["cantosReleaseAudit"] = releaseAudit.toString()
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
            isMinifyEnabled = !releaseAudit
            isShrinkResources = !releaseAudit
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Audit stays unsigned even when the operator explicitly supplies a release key.
            if (!releaseAudit && releaseSigning) {
                signingConfig = signingConfigs.create("operatorRelease") {
                    fun supplied(name: String): String = providers.environmentVariable(name).orNull
                        ?.takeIf { it.isNotEmpty() } ?: error("Missing operator release signing input: $name")
                    storeFile = file(supplied("CANTOS_RELEASE_KEYSTORE"))
                    storePassword = supplied("CANTOS_RELEASE_KEYSTORE_PASSWORD")
                    keyAlias = supplied("CANTOS_RELEASE_KEY_ALIAS")
                    keyPassword = supplied("CANTOS_RELEASE_KEY_PASSWORD")
                }
            }
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
            // Reject an observed forbidden edge even when its own transitive graph cannot
            // resolve offline. Ordinary incomplete graphs still fail; no fallback is admitted.
            check(violations.isEmpty()) { "Forbidden production $variant dependencies: ${violations.joinToString()}" }
            check(unresolved.isEmpty()) {
                "Incomplete $variant graph for $name: " +
                    unresolved.joinToString { "${it.attempted.displayName}: ${it.failure.message}" }
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

// AGP's ALL scoped classes are the actual inputs before dexing/R8, including transitive jars.
// A read-only observer consumes them; it never rewrites classes or changes their dependency graph.
abstract class InspectProductionInputs : DefaultTask() {
    @get:InputFiles @get:PathSensitive(PathSensitivity.NONE)
    abstract val jars: ListProperty<RegularFile>
    @get:InputFiles @get:PathSensitive(PathSensitivity.NONE)
    abstract val directories: ListProperty<Directory>
    @get:Input abstract val buildVariant: Property<String>
    @get:OutputFile abstract val report: RegularFileProperty

    @TaskAction fun inspect() {
        val mobile = project.rootProject.projectDir
        val arguments = mutableListOf("python3", File(mobile, "../../../scripts/inspect_android_inputs.py").absolutePath,
            "--mobile", mobile.absolutePath, "--variant", buildVariant.get(),
            "--report", report.get().asFile.absolutePath)
        jars.get().forEach { arguments += listOf("--classes", it.asFile.absolutePath) }
        directories.get().forEach { arguments += listOf("--classes", it.asFile.absolutePath) }
        project.exec { commandLine(arguments) }.assertNormalExitValue()
    }
}
androidComponents.onVariants { variant ->
    val capital = variant.name.replaceFirstChar { it.uppercase() }
    val inspection = tasks.register<InspectProductionInputs>("inspect${capital}PreShrinkInputs") {
        buildVariant.set(variant.name)
        report.set(layout.buildDirectory.file("reports/${variant.name}-boundary/pre-shrink.json"))
        outputs.upToDateWhen { false }
    }
    variant.artifacts.forScope(ScopedArtifacts.Scope.ALL).use(inspection).toGet(
        ScopedArtifact.CLASSES, InspectProductionInputs::jars, InspectProductionInputs::directories)
    // No preBuild dependency: scoped classes are built AFTER preBuild. Consumers wait on observer.
    tasks.matching { it.name in setOf("minify${capital}WithR8", "merge${capital}Dex",
        "mergeExtDex${capital}", "mergeLibDex${capital}", "package$capital", "bundle$capital") }
        .configureEach { dependsOn(inspection) }
}
