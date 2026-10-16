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
import org.jetbrains.compose.resources.stringResource
import org.jetbrains.compose.resources.pluralStringResource
import spike.compose.resources.Res
import spike.compose.resources.common_cancel
import spike.compose.resources.theatre_player_bookmark_add
import spike.compose.resources.theatre_episodes
import spike.compose.resources.theatre_error_unavailable
@Composable internal fun Rows(): List<String> = listOf(
    stringResource(Res.string.common_cancel),
    stringResource(Res.string.theatre_player_bookmark_add, StoryTitle),
    pluralStringResource(Res.plurals.theatre_episodes, 0, 0),
    pluralStringResource(Res.plurals.theatre_episodes, 1, 1),
    pluralStringResource(Res.plurals.theatre_episodes, 2, 2),
    ErrorText(DisplayError.Unavailable),
)
@Composable private fun ErrorText(error: DisplayError): String = when (error) {
    DisplayError.Unavailable -> stringResource(Res.string.theatre_error_unavailable)
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
