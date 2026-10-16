package spike
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import java.io.File
import java.util.Locale
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
@OptIn(ExperimentalTestApi::class)
class CandidateTest {
    @Test fun renders_both_languages_with_literal_content_and_exact_plural() {
        val previous = Locale.getDefault()
        try {
            for (language in listOf("vi", "en")) {
                Locale.setDefault(Locale.forLanguageTag(language))
                lyricist.languageTag = language
                val expected = if (language == "vi") listOf(
                    "Hủy", "Thêm dấu trang cho Ánh đèn {time} %1\$s cuối sân khấu", "0 tập", "1 tập", "2 tập", "Không thể phát nội dung này.",
                ) else listOf(
                    "Cancel", "Add bookmark for Ánh đèn {time} %1\$s cuối sân khấu", "0 episodes", "1 episode", "2 episodes", "This content cannot be played.",
                )
                runDesktopComposeUiTest(width = 800, height = 360) {
                    setContent { Fixture() }
                    expected.forEachIndexed { index, text -> onNodeWithTag("row-$index").assertTextEquals(text) }
                    val output = File("build/reports/candidate").apply { mkdirs() }
                    val bitmap = onNodeWithTag("fixture").captureToImage()
                    val data = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
                    File(output, "$language.png").writeBytes(data.bytes)
                    File(output, "$language.semantics.txt").writeText(onNodeWithTag("fixture").printToString())
                }
            }
        } finally { Locale.setDefault(previous) }
    }
}
