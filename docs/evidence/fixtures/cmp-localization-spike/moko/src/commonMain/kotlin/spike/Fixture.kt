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
import dev.icerock.moko.resources.compose.stringResource
import spike.moko.resources.MR
import spike.moko.resources.common_cancel
import spike.moko.resources.theatre_player_bookmark_add
import spike.moko.resources.theatre_episodes
import spike.moko.resources.theatre_error_unavailable
@Composable internal fun Rows(): List<String> = listOf(
    stringResource(MR.strings.common_cancel),
    stringResource(MR.strings.theatre_player_bookmark_add, StoryTitle),
    stringResource(MR.plurals.theatre_episodes, 0, 0),
    stringResource(MR.plurals.theatre_episodes, 1, 1),
    stringResource(MR.plurals.theatre_episodes, 2, 2),
    ErrorText(DisplayError.Unavailable),
)
@Composable private fun ErrorText(error: DisplayError): String = when (error) {
    DisplayError.Unavailable -> stringResource(MR.strings.theatre_error_unavailable)
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
