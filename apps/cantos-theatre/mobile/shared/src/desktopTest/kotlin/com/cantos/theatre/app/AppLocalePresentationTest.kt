package com.cantos.theatre.app

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.theatre.features.launch.presentation.launchResources
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.UiPreferences
import java.util.Locale
import kotlin.test.assertEquals
import kotlinx.coroutines.test.runTest
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class AppLocalePresentationTest {
    @Test
    fun app_owned_launch_and_unavailable_resources_ignore_the_opposing_system_language() {
        val original = Locale.getDefault()
        try {
            for ((locale, system) in listOf(ShellLocale.Vietnamese to "en", ShellLocale.English to "vi")) {
                Locale.setDefault(Locale.forLanguageTag(system))
                runTest {
                    val resources = launchResources(locale)
                    val actual = listOf(resources.status, resources.tagline, resources.category,
                        resources.unavailableTitle, resources.unavailableBody)
                    val expected = when (locale) {
                        ShellLocale.Vietnamese -> listOf("Đang mở Cantos", "Nhắm mắt lại.\nMở một thế giới.",
                            "Truyện · Podcast · Audio drama", "Cantos chưa sẵn sàng", "Phiên bản này chưa phát được truyện hay podcast.")
                        ShellLocale.English -> listOf("Opening Cantos", "Close your eyes.\nOpen a world.",
                            "Stories · Podcasts · Audio drama", "Cantos isn’t ready yet", "This version can’t play stories or podcasts yet.")
                    }
                    assertEquals(expected, actual)
                }
            }
        } finally {
            Locale.setDefault(original)
        }
    }

    @Test
    fun changing_app_locale_updates_the_existing_splash_accessible_status() {
        val locale = mutableStateOf(ShellLocale.Vietnamese)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosLocalLaunch(false, UiPreferences(reducedMotion = true), locale = locale.value) {} }
            onNodeWithTag("local-launch").assertContentDescriptionEquals("Đang mở Cantos")
            runOnIdle { locale.value = ShellLocale.English }
            onNodeWithTag("local-launch").assertContentDescriptionEquals("Opening Cantos")
            runOnIdle { locale.value = ShellLocale.Vietnamese }
            onNodeWithTag("local-launch").assertContentDescriptionEquals("Đang mở Cantos")
        }
    }

    @Test
    fun changing_app_locale_updates_both_unavailable_labels() {
        val locale = mutableStateOf(ShellLocale.Vietnamese)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosListeningUnavailable(UiPreferences(), locale = locale.value) }
            onNodeWithTag("listening-unavailable-title").assertTextEquals("Cantos chưa sẵn sàng")
            runOnIdle { locale.value = ShellLocale.English }
            onNodeWithTag("listening-unavailable-title").assertTextEquals("Cantos isn’t ready yet")
            onNodeWithTag("listening-unavailable-body").assertTextEquals("This version can’t play stories or podcasts yet.")
        }
    }
}
