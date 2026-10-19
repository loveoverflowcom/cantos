package com.cantos.theatre.app.workbench

import java.io.File
import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Bounded source-set guard for this host; #122 owns the full production release graph proof. */
class TheatreWorkbenchBoundaryTest {
    @Test
    fun workbench_stays_in_desktop_sources_and_only_imports_the_shared_settings_directory() {
        val module = File(System.getProperty("user.dir"))
        listOf("commonMain").forEach { sourceSet ->
            val sourceRoot = File(module, "src/$sourceSet")
            assertTrue(sourceRoot.isDirectory, "$sourceSet sources must exist before inspection")
            val files = sourceRoot.walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()
            assertTrue(files.isNotEmpty(), "$sourceSet import guard must inspect Kotlin files")
            files.forEach { file ->
                val source = file.readText()
                assertFalse(source.contains("com.cantos.theatre.app.workbench"), "Preview host leaked into $file")
                assertFalse(source.contains("com.cantos.ui.preview.workbench"), "Preview settings leaked into $file")
            }
        }
        // The sandbox is JVM desktop only: native source sets and targets must not exist (#287).
        listOf("androidMain", "iosMain").forEach { sourceSet ->
            assertFalse(File(module, "src/$sourceSet").exists(), "$sourceSet must not exist in the JVM-only sandbox")
        }
        val gradle = File(module, "build.gradle.kts").readText()
        listOf("androidTarget", "iosArm64", "iosSimulatorArm64", "com.android.library", "CantosTheatreDemo", "kotlin.native.home")
            .forEach { assertFalse(gradle.contains(it), "Sandbox build must not mention $it") }
        val settingsImport = "kotlin.srcDir(rootProject.file(\"../../../packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/workbench/settings\"))"
        assertTrue(gradle.substringAfter("val desktopMain by getting").substringBefore("val desktopTest by getting").contains(settingsImport))
        val shared = File(module.parentFile, "shared/build.gradle.kts")
        assertTrue(shared.isFile, "The production-safe shared build must exist before inspection")
        assertTrue(shared.readText().contains("kotlin.exclude(\"com/cantos/ui/preview/**\")"))
        val entireUiSources = "kotlin.srcDir(rootProject.file(\"../../../packages/ui-system/cmp/src/commonMain/kotlin\"))"
        assertFalse(gradle.contains(entireUiSources), "Sandbox must consume shared UI through its project dependency")
    }
}
