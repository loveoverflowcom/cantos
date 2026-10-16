package spike
import androidx.compose.runtime.Composable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.staticCompositionLocalOf
import cafe.adriel.lyricist.Lyricist
import cafe.adriel.lyricist.ProvideStrings
internal data class Copy(
    val cancel: String,
    val bookmark: (title: String) -> String,
    val episodes: (count: Int) -> String,
    val error: (error: DisplayError) -> String,
)
private val Vietnamese = Copy(
    "Hủy", { title -> "Thêm dấu trang cho $title" }, { count -> "$count tập" },
    { error -> when (error) { DisplayError.Unavailable -> "Không thể phát nội dung này." } },
)
private val English = Copy(
    "Cancel", { title -> "Add bookmark for $title" },
    { count -> if (count == 1) "$count episode" else "$count episodes" },
    { error -> when (error) { DisplayError.Unavailable -> "This content cannot be played." } },
)
internal val lyricist = Lyricist("vi", mapOf("vi" to Vietnamese, "en" to English))
private val LocalCopy = staticCompositionLocalOf { Vietnamese }
@Composable internal fun Rows(): List<String> {
    var result = emptyList<String>()
    ProvideStrings(lyricist, LocalCopy) {
        val copy = LocalCopy.current
        result = listOf(copy.cancel, copy.bookmark(StoryTitle), copy.episodes(0), copy.episodes(1), copy.episodes(2), copy.error(DisplayError.Unavailable))
    }
    return result
}

internal enum class DisplayError { Unavailable }
internal const val StoryTitle = "Ánh đèn {time} %1\$s cuối sân khấu"
@Composable internal fun Fixture() {
    MaterialTheme {
        Surface(Modifier.testTag("fixture")) {
            Column(Modifier.padding(16.dp)) {
                Rows().forEachIndexed { index, text -> Text(text, Modifier.testTag("row-$index")) }
            }
        }
    }
}
