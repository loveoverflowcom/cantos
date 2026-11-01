package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.CantosTheme
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/**
 * #686 (AC-06): the warning glyph and its message share one vertical axis in `ErrorText`, for one
 * line and for wrapped lines, in both locales and at a doubled text scale. JVM layout bounds say
 * nothing about an Android device, TalkBack or a captured render.
 */
@OptIn(ExperimentalTestApi::class)
class ErrorTextAlignmentTest {
    private val oneLine = mapOf(ShellLocale.Vietnamese to "Lỗi mạng.", ShellLocale.English to "Network error.")

    private fun unknownOutcome(locale: ShellLocale) =
        AuthCopy(locale).failure(NativeAccountFailure.Api, AccountLoginFailure.UnknownOutcome)

    private fun layout(text: String, widthDp: Int, fontScale: Float, body: ComposeUiTest.() -> Unit) =
        runDesktopComposeUiTest(width = widthDp, height = 600) {
            setContent {
                CantosTheme(UiPreferences(theme = Theme.Light)) {
                    CompositionLocalProvider(
                        LocalDensity provides Density(density.density, fontScale),
                    ) {
                        Box(Modifier.requiredWidth(widthDp.dp)) { ErrorText(text) }
                    }
                }
            }
            body()
        }

    private fun ComposeUiTest.centers(): Triple<Float, Float, Float> {
        val icon = onNodeWithTag("error-icon", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        val text = onNodeWithTag("error-text", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        return Triple((icon.top + icon.bottom) / 2, (text.top + text.bottom) / 2, text.height)
    }

    @Test
    fun icon_and_message_share_a_vertical_centre_across_the_representative_matrix() {
        for (locale in ShellLocale.entries) for (width in listOf(320, 390)) for (scale in listOf(1f, 2f)) {
            for (text in listOf(oneLine.getValue(locale), unknownOutcome(locale))) {
                layout(text, width, scale) {
                    val (icon, message, _) = centers()
                    assertTrue(abs(icon - message) <= 1f, "$locale ${width}dp x$scale '$text': icon=$icon text=$message")
                }
            }
        }
    }

    @Test
    fun a_long_message_wraps_inside_the_frame_and_the_error_semantics_stay_merged() {
        val text = unknownOutcome(ShellLocale.Vietnamese)
        layout(text, 320, 2f) {
            val (_, _, textHeight) = centers()
            val frame = onNodeWithTag("error-icon", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
            assertTrue(textHeight > frame.height, "message must wrap beyond one icon height: $textHeight")
            val text320 = onNodeWithTag("error-text", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
            assertTrue(text320.right <= 320f * density.density, "message must stay inside the 320dp frame: ${text320.right}")
            val row = onNodeWithTag("error-row").fetchSemanticsNode()
            assertEquals(text, row.config.getOrNull(SemanticsProperties.Error))
        }
    }
}
