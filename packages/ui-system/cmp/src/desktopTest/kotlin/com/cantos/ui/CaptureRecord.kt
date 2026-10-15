package com.cantos.ui

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import com.cantos.ui.generated.CantosTokens
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image

/** Renderer label of every record written here; it never claims Android, iOS, an OS window or a screen reader. */
internal const val JvmTestRootRenderer = "jvm-desktop-compose-test-root"

/** Everything needed to reproduce and honestly label one JVM test-root capture. */
internal data class CaptureScenario(
    val name: String,
    val section: String,
    val component: String?,
    val state: String?,
    val specimen: String? = null,
    val themeMode: ThemeMode,
    val systemIsDark: Boolean,
    val preferences: UiPreferences,
    val viewportWidthDp: Int,
    val viewportHeightDp: Int,
    val hostDensity: Float,
    val hostFontScale: Float,
)

internal fun captureDirectory(): File =
    File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")

/**
 * Writes `<name>.png` and a `<name>.json` sidecar beside it. The sidecar states the scenario, the
 * pixel size, the token version, the token file digest and the PNG digest, so a reviewer can open the
 * right image for the right scenario. `CANTOS_SOURCE_REVISION` may name the checked-out revision; an
 * unset value is recorded as `unrecorded` rather than guessed.
 */
internal fun writeCapture(scenario: CaptureScenario, bitmap: ImageBitmap, output: File = captureDirectory()): File {
    check(output.isDirectory || output.mkdirs()) { "Could not create screenshot output directory" }
    val png = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
    val image = File(output, "${scenario.name}.png")
    image.writeBytes(png)
    File(output, "${scenario.name}.json").writeText(sidecar(scenario, bitmap, png) + "\n", Charsets.UTF_8)
    return image
}

private fun sha256(bytes: ByteArray): String =
    MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

private fun tokenFileDigest(): String {
    // The Gradle project directory is packages/ui-system/cmp; the canonical source is its sibling.
    val file = File("../shared/tokens.json")
    return if (file.isFile) sha256(file.readBytes()) else "unrecorded"
}

private fun quoted(value: String): String =
    "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n") + "\""

private fun nullable(value: String?): String = value?.let(::quoted) ?: "null"

internal fun sidecar(scenario: CaptureScenario, bitmap: ImageBitmap, png: ByteArray): String {
    val preferences = scenario.preferences
    val fields = listOf(
        "schema" to quoted("cantos-ui-capture-v1"),
        "name" to quoted(scenario.name),
        "renderer" to quoted(JvmTestRootRenderer),
        "not_evidence_of" to "[\"android-device\", \"ios-device\", \"os-window\", \"screen-reader\", \"design-pixel-match\"]",
        "section" to quoted(scenario.section),
        "component" to nullable(scenario.component),
        "state" to nullable(scenario.state),
        "design_specimen" to nullable(scenario.specimen),
        "theme_mode" to quoted(scenario.themeMode.name),
        "system_is_dark" to scenario.systemIsDark.toString(),
        "theme" to quoted(preferences.theme.tokenName),
        "ui_density" to quoted(preferences.density.tokenName),
        "preview_text_scale" to preferences.textScale.toString(),
        "reduced_motion" to preferences.reducedMotion.toString(),
        "locale_fixtures" to quoted("vi-VN"),
        "viewport_dp" to "{\"width\": ${scenario.viewportWidthDp}, \"height\": ${scenario.viewportHeightDp}}",
        "bitmap_px" to "{\"width\": ${bitmap.width}, \"height\": ${bitmap.height}}",
        "host_density_px_per_dp" to scenario.hostDensity.toString(),
        "host_font_scale" to scenario.hostFontScale.toString(),
        "tokens_version" to quoted(CantosTokens.version),
        "tokens_file_sha256" to quoted(tokenFileDigest()),
        "source_revision" to quoted(System.getenv("CANTOS_SOURCE_REVISION") ?: "unrecorded"),
        "png_sha256" to quoted(sha256(png)),
        "png_bytes" to png.size.toString(),
    )
    return fields.joinToString(",\n", "{\n", "\n}") { (key, value) -> "  ${quoted(key)}: $value" }
}
