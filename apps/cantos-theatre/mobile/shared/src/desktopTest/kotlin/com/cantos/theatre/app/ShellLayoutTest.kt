package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.SampleEpisodeIds
import com.cantos.theatre.presentation.model.SampleWorkIds
import com.cantos.theatre.presentation.model.sampleState
import com.cantos.theatre.presentation.model.sampleWorks
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.features.auth.presentation.GuestEntry
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class ShellLayoutTest {
    // cantos.presentation.guest-navigation.run.session-root / run.preferences-player
    @Test fun guest_landing_has_no_navigation_but_keeps_public_player_and_preference_dialogs() {
        for ((width, height, scale, theme, locale) in listOf(
            GuestFrame(390, 844, 1f, ShellTheme.Light, ShellLocale.Vietnamese),
            GuestFrame(390, 844, 1f, ShellTheme.Dark, ShellLocale.English),
            GuestFrame(840, 900, 1f, ShellTheme.Light, ShellLocale.English),
            GuestFrame(320, 568, 2f, ShellTheme.Dark, ShellLocale.Vietnamese),
        )) runDesktopComposeUiTest(width = width, height = height) {
            val events = mutableListOf<ShellEvent>()
            var state by mutableStateOf(sampleState(stack = listOf(ShellRoute.Landing), theme = theme, locale = locale,
                savedWorks = emptySet(), progress = emptyList(), history = emptyList(), bookmarks = emptyList()))
            var signedIn by mutableStateOf(false)
            var rendererDensity = Float.NaN
            setContent {
                rendererDensity = LocalDensity.current.density
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("guest-capture")) {
                    CantosPresentation(state, ShellCatalogView(sampleWorks), { events += it }, textScale = scale,
                        accountSignedIn = signedIn,
                        guestEntry = { focus -> GuestEntry(locale, AccountView.SignedOut, {}, focus) })
                }
            }
            onNodeWithTag("landing").assertExists()
            onNodeWithTag("nav-discovery").assertDoesNotExist()
            onNodeWithTag("nav-library").assertDoesNotExist()
            onNodeWithTag("mini-player").assertIsDisplayed()
            onNodeWithTag("guest-theme").assertIsDisplayed().performClick()
            onNodeWithTag("theme-Light").assertExists()
            onNodeWithTag("theme-Dark").assertExists()
            onNodeWithTag("theme-System").performClick()
            onNodeWithTag("guest-theme").assertIsFocused()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.SetThemeMode(ShellThemeMode.System)), events)
            onNodeWithTag("guest-language").performClick()
            onNodeWithTag("locale-English").performClick()
            onNodeWithTag("guest-language").assertIsFocused()
            assertEquals(ShellEvent.SetLocale(ShellLocale.English), events.last())
            val output = File("build/reports/guest-navigation").apply { mkdirs() }
            val name = "landing-$width-$theme-$locale-text$scale"
            val bitmap = onNodeWithTag("guest-capture").captureToImage()
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag("guest-capture", useUnmergedTree = true).printToString())
            val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
            File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","entrypoint":"CantosPresentation","widthDp":$width,"heightDp":$height,"density":$rendererDensity,"textScale":$scale,"theme":"$theme","locale":"$locale","motion":"Reduced","png_sha256":"$hash"}""")
            onNodeWithTag("sign-in").performScrollTo().assertIsDisplayed()
            onNodeWithTag("create-account").performScrollTo().assertIsDisplayed()
            onNodeWithTag("search").performScrollTo().assertIsDisplayed()
            runOnIdle { signedIn = true; state = state.copy(stack = listOf(ShellRoute.Discovery)) }
            onNodeWithTag("nav-discovery").assertIsDisplayed()
            onNodeWithTag("nav-library").assertIsDisplayed()
            onNodeWithTag("guest-theme").assertDoesNotExist()
            onNodeWithTag("mini-player").assertIsDisplayed()
        }
    }

    // cantos.presentation.guest-navigation.run.unresolved-session
    @Test fun unresolved_private_route_mounts_recovery_without_stale_library_or_account_content() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            var route by mutableStateOf<ShellRoute>(ShellRoute.Library)
            setContent {
                CantosPresentation(sampleState(stack = listOf(route)), ShellCatalogView(sampleWorks), {},
                    accountSignedIn = false, guestAuthEnabled = false,
                    guestEntry = { focus -> GuestEntry(ShellLocale.Vietnamese, AccountView.CleanupRequired, {}, focus) })
            }
            for (private in listOf(ShellRoute.Library, ShellRoute.History, ShellRoute.Account)) {
                runOnIdle { route = private }
                onNodeWithTag("guest-entry").assertExists()
                onNodeWithTag("sign-in").assertIsNotEnabled()
                onNodeWithTag("auth-retry-cleanup").assertExists()
                onNodeWithTag("create-account").assertDoesNotExist()
                onNodeWithTag("library-continue").assertDoesNotExist()
                onNodeWithTag("history-list").assertDoesNotExist()
                onNodeWithTag("account-profile").assertDoesNotExist()
                onNodeWithTag("nav-library").assertDoesNotExist()
            }
        }
    }

    private data class GuestFrame(val width: Int, val height: Int, val scale: Float, val theme: ShellTheme, val locale: ShellLocale)

    @Test
    fun settings_options_are_scoped_to_a_dismissible_dialog_and_restore_opener_focus() {
        val events = mutableListOf<ShellEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Settings)),
                        ShellCatalogView(sampleWorks), { events += it }, accountSignedIn = true)
                }
            }
            onNodeWithTag("speed-125").assertDoesNotExist()
            onNodeWithTag("setting-Speed").performScrollTo()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button)).performClick()
            onNodeWithTag("preference-dialog").assertIsDisplayed()
            onNodeWithTag("speed-100").assertIsFocused()
            onNodeWithTag("speed-125").performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.SetSpeed(125)), events)
            onNodeWithTag("preference-dialog").assertDoesNotExist()
            onNodeWithTag("setting-Speed").assertIsFocused().performClick()
            onNodeWithTag("speed-100").performKeyInput { pressKey(Key.Escape) }
            onNodeWithTag("preference-dialog").assertDoesNotExist()
            onNodeWithTag("setting-Speed").assertIsFocused()
            onNodeWithTag("screen").assertExists()
            assertEquals(1, events.size)
        }
    }

    @Test
    fun transport_actions_keep_left_center_right_order_and_full_targets_at_double_text() {
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                Box(Modifier.requiredSize(320.dp, 568.dp)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Player)),
                        ShellCatalogView(sampleWorks), {}, textScale = 2f, accountSignedIn = true)
                }
            }
            onNodeWithTag("transport").performScrollTo().assertIsDisplayed()
            val controls = listOf("rewind", "transport", "forward").map {
                onNodeWithTag(it).assertIsDisplayed().fetchSemanticsNode().boundsInRoot
            }
            controls.forEach { assertTrue(it.width >= 48f && it.height >= 48f) }
            assertTrue(controls[0].right <= controls[1].left)
            assertTrue(controls[1].right <= controls[2].left)
            assertTrue(controls.maxOf { it.top } < controls.minOf { it.bottom })
        }
    }

    @Test
    fun featured_and_first_saved_work_titles_are_visible_before_scrolling() {
        var state by mutableStateOf(sampleState())
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosPresentation(state, ShellCatalogView(sampleWorks), {}, accountSignedIn = true)
                }
            }
            onNodeWithText("Sơn Hà Ký", useUnmergedTree = true).assertIsDisplayed()
            runOnIdle {
                state = state.copy(stack = listOf(ShellRoute.Library),
                    savedWorks = setOf(SampleWorkIds.SonHa),
                    progress = listOf(
                        ShellProgress(SampleEpisodeIds.SonHa03, 768),
                        ShellProgress(SampleEpisodeIds.DenKhua01, 91),
                    ),
                )
            }
            onNodeWithText("Sơn Hà Ký", useUnmergedTree = true).assertIsDisplayed()
        }
    }

    @Test
    fun large_text_episode_and_shell_controls_remain_reachable_on_a_short_screen() {
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                Box(Modifier.requiredSize(320.dp, 568.dp)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Discovery,
                        ShellRoute.Work(SampleWorkIds.SonHa))),
                        ShellCatalogView(sampleWorks), {}, textScale = 2f, accountSignedIn = true)
                }
            }
            onNodeWithTag("episode-SonHa03").performScrollTo().assertIsDisplayed()
            val row = onNodeWithTag("episode-SonHa03").fetchSemanticsNode().boundsInRoot
            val mini = onNodeWithTag("mini-open").fetchSemanticsNode().boundsInRoot
            assertTrue(row.bottom <= mini.top)
            onNodeWithTag("back").assertIsDisplayed()
            onNodeWithTag("account").assertIsDisplayed()
            onNodeWithTag("nav-discovery").assertIsDisplayed()
            onNodeWithTag("nav-library").assertIsDisplayed()
        }
    }
}
