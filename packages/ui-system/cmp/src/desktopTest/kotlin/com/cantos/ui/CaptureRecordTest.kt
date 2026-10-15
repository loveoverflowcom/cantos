package com.cantos.ui

import androidx.compose.ui.graphics.ImageBitmap
import com.cantos.ui.generated.CantosTokens
import java.io.File
import java.nio.file.Files
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** The sidecar must label a capture honestly and bind itself to the exact PNG bytes it describes. */
class CaptureRecordTest {
    private val scenario = CaptureScenario(
        name = "cmp-jvm-fixture",
        section = "design-v3",
        component = null,
        state = null,
        themeMode = ThemeMode.System,
        systemIsDark = true,
        preferences = UiPreferences(theme = Theme.Dark, density = Density.Comfortable, textScale = 2f),
        viewportWidthDp = 390,
        viewportHeightDp = 840,
        hostDensity = 1f,
        hostFontScale = 1f,
    )

    @Test
    fun sidecar_records_scenario_pixel_size_and_the_digest_of_the_written_png() {
        val directory = Files.createTempDirectory("cantos-capture-record").toFile()
        try {
            val image = writeCapture(scenario, ImageBitmap(3, 2), directory)
            val json = File(directory, "cmp-jvm-fixture.json").readText()
            val digest = MessageDigest.getInstance("SHA-256").digest(image.readBytes()).joinToString("") { "%02x".format(it) }
            assertTrue(json.contains("\"png_sha256\": \"$digest\""))
            assertTrue(json.contains("\"png_bytes\": ${image.length()}"))
            assertTrue(json.contains("\"renderer\": \"$JvmTestRootRenderer\""))
            assertTrue(json.contains("\"theme_mode\": \"System\""))
            assertTrue(json.contains("\"system_is_dark\": true"))
            assertTrue(json.contains("\"theme\": \"dark\""))
            assertTrue(json.contains("\"ui_density\": \"comfortable\""))
            assertTrue(json.contains("\"preview_text_scale\": 2.0"))
            assertTrue(json.contains("\"viewport_dp\": {\"width\": 390, \"height\": 840}"))
            assertTrue(json.contains("\"bitmap_px\": {\"width\": 3, \"height\": 2}"))
            assertTrue(json.contains("\"tokens_version\": \"${CantosTokens.version}\""))
            // The canonical token file is the sibling of the Gradle project directory these tests run in.
            val tokens = File("../shared/tokens.json")
            assertTrue(tokens.isFile, "the canonical token source must be reachable from the test working directory")
            val tokensDigest = MessageDigest.getInstance("SHA-256").digest(tokens.readBytes()).joinToString("") { "%02x".format(it) }
            assertTrue(json.contains("\"tokens_file_sha256\": \"$tokensDigest\""))
            assertTrue(json.contains("\"source_revision\":"))
            assertEquals(setOf("cmp-jvm-fixture.json", "cmp-jvm-fixture.png"), directory.list()!!.toSet())
        } finally {
            directory.deleteRecursively()
        }
    }

    @Test
    fun the_png_digest_matches_the_published_sha256_vector_for_abc() {
        // NIST/FIPS 180-4 example: SHA-256("abc"), independent of this file's own hex formatting.
        val json = sidecar(scenario, ImageBitmap(1, 1), "abc".encodeToByteArray())
        assertTrue(json.contains("\"png_sha256\": \"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\""))
        assertTrue(json.contains("\"png_bytes\": 3"))
    }

    @Test
    fun sidecar_never_claims_device_window_reader_or_design_pixel_evidence() {
        val json = sidecar(scenario, ImageBitmap(1, 1), ByteArray(1))
        listOf("android-device", "ios-device", "os-window", "screen-reader", "design-pixel-match").forEach {
            assertTrue(json.contains("\"$it\""), it)
        }
        assertTrue(json.contains("\"not_evidence_of\""))
        assertFalse(json.contains("\"device\":"))
        assertTrue(json.contains("\"renderer\": \"jvm-desktop-compose-test-root\""))
    }

    @Test
    fun quotes_and_line_breaks_in_a_scenario_name_cannot_break_the_record() {
        val json = sidecar(scenario.copy(name = "a\"b\\c\nd"), ImageBitmap(1, 1), ByteArray(1))
        assertTrue(json.contains("\"name\": \"a\\\"b\\\\c\\nd\""))
    }

    @Test
    fun a_motion_frame_states_its_recipe_mark_time_and_timebase_and_disclaims_recordings_and_the_os_signal() {
        val frame = CaptureFrame(recipe = "press-hold", mark = "mid", timeMillis = 112, timebase = ManualClockTimebase)
        val json = sidecar(scenario.copy(frame = frame), ImageBitmap(1, 1), ByteArray(1))
        assertTrue(json.contains("\"frame\": {\"recipe\": \"press-hold\", \"mark\": \"mid\", \"time_ms\": 112, \"timebase\": \"$ManualClockTimebase\"}"))
        listOf("android-device", "ios-device", "os-window", "screen-reader", "design-pixel-match", "runtime-recording", "os-reduce-motion").forEach {
            assertTrue(json.contains("\"$it\""), it)
        }
        assertTrue(json.contains("\"reduced_motion\": false"))
    }

    @Test
    fun a_capture_without_a_frame_keeps_the_sidecar_it_always_had() {
        val json = sidecar(scenario, ImageBitmap(1, 1), ByteArray(1))
        assertFalse(json.contains("\"frame\""))
        assertFalse(json.contains("runtime-recording"))
        assertFalse(json.contains("os-reduce-motion"))
    }
}
