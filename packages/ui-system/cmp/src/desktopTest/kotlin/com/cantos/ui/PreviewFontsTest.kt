package com.cantos.ui

import androidx.compose.material3.Text

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.PreviewApp
import com.cantos.ui.preview.PreviewFonts
import org.junit.Rule
import org.junit.Test
import java.security.MessageDigest
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The desktop host's font delivery, on real bytes and a real JVM Compose root. A family that loads is
 * not evidence of glyph placement; the opened captures carry that observation.
 */
class PreviewFontsTest {
    @get:Rule
    val compose = createComposeRule()

    private val provenance: String = String(PreviewFonts.bytes("provenance.json"), Charsets.UTF_8)

    private fun recorded(file: String, key: String): String =
        Regex("\"${Regex.escape(file)}\"\\s*:\\s*\\{[^}]*?\"$key\"\\s*:\\s*(?:\"([^\"]*)\"|(\\d+))")
            .find(provenance)?.let { it.groupValues[1].ifEmpty { it.groupValues[2] } }
            ?: error("provenance.json has no $key for $file")

    private fun sha256(bytes: ByteArray) =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    @Test
    fun each_bundled_face_has_the_bytes_family_and_weight_the_provenance_record_names() {
        assertEquals(4, PreviewFonts.faces.size)
        PreviewFonts.faces.forEach { face ->
            assertEquals(recorded(face.file, "sha256"), sha256(PreviewFonts.bytes(face.file)), face.file)
            assertEquals(face.weight.weight, recorded(face.file, "weight").toInt(), face.file)
            val expectedFamily = if (face.family == PreviewFonts.Family.Sans) "Noto Sans" else "Noto Serif"
            assertEquals(expectedFamily, recorded(face.file, "family"), face.file)
        }
        assertTrue(provenance.contains("SIL-OFL-1.1"))
    }

    @Test
    fun a_missing_face_fails_loudly_instead_of_falling_back_silently() {
        val failure = runCatching { PreviewFonts.bytes("not_a_font.ttf") }.exceptionOrNull()
        assertTrue(failure is IllegalStateException && "not on the classpath" in failure.message.orEmpty())
    }

    @Test
    fun the_gallery_renders_every_text_in_the_injected_families_including_nested_specimen_themes() {
        compose.setContent {
            Box(Modifier.requiredSize(1140.dp, 840.dp)) {
                PreviewApp(uiFontFamily = FontFamily.Cursive, editorialFontFamily = FontFamily.Monospace)
            }
        }
        val families = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.GetTextLayoutResult))
            .fetchSemanticsNodes()
            .mapNotNull { node ->
                val results = mutableListOf<TextLayoutResult>()
                node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(results)
                results.firstOrNull()?.layoutInput?.style?.fontFamily
            }
        assertTrue(families.size > 20, "expected the whole gallery to be inspected, saw ${families.size} text runs")
        val unexpected = families.filter { it != FontFamily.Cursive && it != FontFamily.Monospace }
        assertEquals(emptyList(), unexpected, "text runs outside the injected families, including nested specimen themes")
        assertTrue(families.any { it == FontFamily.Monospace }, "the editorial headline must use the editorial family")
        assertTrue(families.any { it == FontFamily.Cursive }, "UI text must use the injected UI family")
    }

    @Test
    fun qualification_capture_roots_inherit_the_actual_bundled_families_in_nested_components() {
        compose.setContent {
            BundledPreviewCaptureRoot {
                Stack {
                    Button("Nguyễn Thị Ánh Nguyệt", {})
                    Text("Sơn Hà Ký · Ỗ, Ữ", style = androidx.compose.material3.MaterialTheme.typography.headlineSmall)
                    UiRoot(preferences = UiPreferences(theme = Theme.Dark)) {
                        TextField("Ghi chú", "Lời hẹn", {})
                    }
                }
            }
        }
        val layouts = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.GetTextLayoutResult),
            useUnmergedTree = true).fetchSemanticsNodes().flatMap { node ->
                val results = mutableListOf<TextLayoutResult>()
                node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(results)
                results
            }
        val examined = layouts.map { it.layoutInput.text.text }.toSet()
        assertTrue(examined.containsAll(setOf("Nguyễn Thị Ánh Nguyệt", "Sơn Hà Ký · Ỗ, Ữ", "Lời hẹn")),
            "Actual button, headline and nested editable text must be examined: $examined")
        val families = layouts.map { it.layoutInput.style.fontFamily }
        assertTrue(families.all { it == PreviewFonts.ui || it == PreviewFonts.editorial })
        assertTrue(families.any { it == PreviewFonts.editorial })
        assertTrue(families.any { it == PreviewFonts.ui })
    }

}
