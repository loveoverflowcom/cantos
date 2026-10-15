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
        listOf("commonMain", "androidMain", "iosMain").forEach { sourceSet ->
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
        val gradle = File(module, "build.gradle.kts").readText()
        val settingsImport = "kotlin.srcDir(rootProject.file(\"../../../packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/workbench/settings\"))"
        assertTrue(gradle.substringAfter("val desktopMain by getting").substringBefore("val desktopTest by getting").contains(settingsImport))
        assertTrue(gradle.contains("kotlin.exclude(\"com/cantos/ui/preview/**\")"))
    }
}
