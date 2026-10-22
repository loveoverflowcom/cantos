package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.LocalePreferenceRead
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.adapters.preview.PreviewLocalePreferenceStore
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewSeed
import com.cantos.theatre.app.PreviewSession
import com.cantos.theatre.app.PreviewTestRoot
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import com.cantos.theatre.app.previewState
import com.cantos.theatre.app.seed
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The one Cantos Theatre application, mounted exactly as the Desktop Preview mounts it, driven
 * through its own controls. The sample data, decoder stand-in and identity provider are the only
 * mocks: the JVM renderer proves flows and semantics, never native audio, OS input or a device.
 */
@OptIn(ExperimentalTestApi::class)
class PreviewShellFlowTest {
    private val sonHa = PreviewWorkIds.SonHa.value
    private val denKhua = PreviewWorkIds.DenKhua.value
    private val canhBuom = PreviewWorkIds.CanhBuom.value

    private fun run(
        seed: PreviewSeed = PreviewSession.Guest.seed(),
        width: Int = 390,
        height: Int = 844,
        textScale: Float = 1f,
        localeStore: PreviewLocalePreferenceStore? = null,
        body: ComposeUiTest.(PreviewOwners) -> Unit,
    ) = runDesktopComposeUiTest(width = width, height = height) {
        lateinit var owners: PreviewOwners
        setContent {
            Box(Modifier.requiredSize(width.dp, height.dp)) {
                PreviewTestRoot(seed, textScale = textScale, localeStore = localeStore, onOwners = { owners = it })
            }
        }
        body(owners)
    }

    private fun ComposeUiTest.untilTag(tag: String) = waitUntil(timeoutMillis = 5_000) {
        onAllNodesWithTag(tag).fetchSemanticsNodes().isNotEmpty()
    }

    private fun ComposeUiTest.untilNoTag(tag: String) = waitUntil(timeoutMillis = 5_000) {
        onAllNodesWithTag(tag).fetchSemanticsNodes().isEmpty()
    }

    private fun ComposeUiTest.title(expected: String) =
        onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, expected))

    @Test
    fun a_guest_walks_discover_detail_player_library_account_settings_and_back_on_one_path() = run { owners ->
        untilTag("work-$sonHa")
        title("Khám phá")
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        untilTag("episode-${PreviewEpisodeIds.SonHa03.value}")
        title("Tác phẩm")
        onNodeWithTag("episode-${PreviewEpisodeIds.SonHa03.value}").performScrollTo().performClick()
        title("Phòng nghe")
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.status == PlaybackStatus.Playing }
        // The decoder stand-in acknowledged the commands; no audio was produced and no time elapsed.
        onNodeWithTag("elapsed").assertTextEquals("0:00")
        assertTrue(owners.engine.commands.any { it.startsWith("start@") })
        onNodeWithTag("back").performClick()
        title("Tác phẩm")
        onNodeWithTag("mini-open").assert(hasText("Người giữ tiếng chuông"))
        onNodeWithTag("nav-library").performClick()
        title("Của tôi")
        onNodeWithTag("account").performClick()
        onNodeWithTag("settings").performScrollTo().performClick()
        title("Cài đặt nghe")
        onNodeWithTag("back").performClick()
        title("Tài khoản")
        onNodeWithTag("back").performClick()
        title("Của tôi")
        assertEquals(ShellRoute.Library, owners.projection().route)
    }

    @Test
    fun source_search_filters_by_the_catalog_port_empty_results_recover_and_clear_restores_the_list() = run { owners ->
        untilTag("work-$sonHa")
        onNodeWithTag("search").performScrollTo().performClick()
        onNodeWithTag("nav-discovery").assertIsSelected()
        val input = onNodeWithTag("search-input")
        input.performTextReplacement("Đèn")
        untilNoTag("work-$sonHa")
        onNodeWithTag("work-$denKhua").assertExists()
        input.performTextReplacement("không có trong tủ sách")
        untilNoTag("work-$denKhua")
        onNodeWithContentDescription("Xóa tìm kiếm").performClick()
        input.assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
        untilTag("work-$canhBuom")
        onNodeWithTag("work-$sonHa").assertExists()
        assertEquals("", owners.shell.ui.value.query)
    }

    @Test
    fun a_guest_save_is_taken_to_sign_in_and_back_returns_to_the_work() = run { owners ->
        untilTag("work-$sonHa")
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        onNodeWithTag("save-work").performScrollTo().performClick()
        assertEquals(TheatreRoute.Login, owners.shell.navigation.value.route)
        onNodeWithTag("back").performClick()
        title("Tác phẩm")
        assertEquals(emptySet(), owners.projection().savedWorks)
    }

    @Test
    fun a_signed_in_listener_saves_and_unsaves_and_the_library_follows_the_remote_server() = run(PreviewSession.Restored.seed().let {
        it.copy(state = it.state.copy(stack = listOf(ShellRoute.Discovery), savedWorks = emptySet(), progress = emptyList(), history = emptyList()))
    }) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.account.state.value is NativeAccountState.SignedIn }
        untilTag("work-$sonHa")
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        onNodeWithTag("save-work").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { PreviewWorkIds.SonHa in owners.projection().savedWorks }
        onNodeWithTag("nav-library").performClick()
        onNodeWithTag("library-saved").performScrollTo()
        onNodeWithTag("work-$sonHa").assertExists()
        onNodeWithTag("work-$denKhua").assertDoesNotExist()
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        onNodeWithTag("save-work").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { PreviewWorkIds.SonHa !in owners.projection().savedWorks }
        assertTrue(owners.libraryApi.mutations >= 2, "each change reached the remote Library through the real outbox")
    }

    @Test
    fun a_fresh_guest_library_explains_sign_in_and_a_restored_account_shows_continue_saved_and_history() {
        run(PreviewSession.Fresh.seed()) { owners ->
            title("Của tôi")
            onNodeWithTag("library-sync").assertExists()
            onNodeWithTag("library-sign-in").assertExists()
            assertEquals(null, owners.shell.library.value.account)
        }
        run(PreviewSession.Restored.seed()) { owners ->
            waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.continueListening.isNotEmpty() }
            onNodeWithTag("library-continue").assertExists()
            onNodeWithTag("library-saved").assertExists()
            onNodeWithTag("library-history").assertExists()
            onNodeWithTag("resume-${PreviewEpisodeIds.SonHa03.value}").assertExists()
            onNodeWithTag("resume-${PreviewEpisodeIds.DenKhua01.value}").assertExists()
        }
    }

    @Test
    fun resuming_from_the_library_reopens_the_saved_position_paused() = run(PreviewSession.Restored.seed()) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.continueListening.isNotEmpty() }
        untilTag("resume-${PreviewEpisodeIds.SonHa03.value}")
        onNodeWithTag("resume-${PreviewEpisodeIds.SonHa03.value}").performScrollTo().performClick()
        title("Phòng nghe")
        onNodeWithTag("elapsed").assertTextEquals("12:48")
        assertTrue(owners.engine.commands.none { it.startsWith("start@") }, "restoring never plays")
    }

    @Test
    fun logging_out_hides_the_library_and_signing_in_as_the_other_listener_shows_none_of_their_data() = run(PreviewSession.Restored.seed()) { owners ->
        owners.identity.mode = PreviewLoginMode.Approve
        waitUntil(timeoutMillis = 5_000) { owners.account.state.value is NativeAccountState.SignedIn }
        onNodeWithTag("account").performClick()
        onNodeWithTag("logout").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.account.state.value == NativeAccountState.SignedOut }
        assertEquals(null, owners.shell.library.value.account)
        onNodeWithTag("sign-in").performScrollTo().assertIsEnabled().performClick()
        onNodeWithTag("login-email").performScrollTo().performTextInput("listener@example.test")
        onNodeWithTag("login-password").performScrollTo().performTextInput("preview-only")
        onNodeWithTag("login-primary").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.account.state.value is NativeAccountState.SignedIn }
        assertEquals("00000000-0000-4000-8000-0000000000a2", owners.shell.library.value.account)
        assertTrue(owners.projection().savedWorks.isEmpty())
    }

    @Test
    fun an_unreachable_catalog_shows_the_retryable_error_and_recovery_keeps_the_selection() = run(PreviewSeed(previewState(
        selectedEpisode = PreviewEpisodeIds.SonHa03, positionMs = 768_000L, catalogMode = ShellCatalogMode.Error,
    ))) { owners ->
        untilTag("error-dialog")
        // The modal owns focus; recovery uses its safe read action and keeps the player owner.
        assertEquals(768_000L, owners.runtime.listening.playback.state.value.session?.positionMs)
        owners.catalogPort.setScenario(com.cantos.theatre.adapters.preview.PreviewCatalogScenario.Ready)
        onNodeWithTag("error-dialog-primary").performClick()
        untilTag("work-$sonHa")
        onNodeWithTag("mini-open").assert(hasText("12:48"))
    }

    @Test
    fun the_loading_scenario_keeps_the_query_and_an_empty_catalog_offers_a_way_back_to_discover() {
        run(PreviewSeed(previewState(stack = listOf(ShellRoute.Search), query = "Sơn Hà", catalogMode = ShellCatalogMode.Loading))) { owners ->
            untilTag("retry")
            onNodeWithTag("search-input").assert(
                SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("Sơn Hà")),
            )
            assertEquals("Sơn Hà", owners.shell.ui.value.query)
        }
        run(PreviewSeed(previewState(catalogMode = ShellCatalogMode.Empty))) {
            waitForIdle()
            onNodeWithTag("nav-discovery").assertIsDisplayed()
            onAllNodesWithTag("work-$sonHa").assertCountEquals(0)
        }
    }

    @Test
    fun settings_edit_the_shared_preferences_and_a_refused_locale_write_is_reported_then_retried() {
        val store = PreviewLocalePreferenceStore().apply { writesAvailable = false }
        run(localeStore = store) { owners ->
            onNodeWithTag("account").performClick()
            onNodeWithTag("settings").performScrollTo().performClick()
            onNodeWithTag("setting-Language").performScrollTo().performClick()
            onNodeWithTag("locale-English").performClick()
            onNodeWithTag("locale-save-failure").performScrollTo().assertTextEquals(
                "Không lưu được ngôn ngữ. Lựa chọn trước đó vẫn được giữ. Hãy thử lại.",
            )
            onNodeWithTag("setting-Language").assert(hasText("Tiếng Việt"))
            runOnIdle { store.writesAvailable = true }
            onNodeWithTag("setting-Language").performScrollTo().performClick()
            onNodeWithTag("locale-English").performClick()
            onNodeWithTag("locale-save-failure").assertDoesNotExist()
            onNodeWithTag("setting-Language").assert(hasText("English"))
            assertEquals(LocalePreferenceRead.Saved(com.cantos.theatre.domain.preferences.UiLocale.English), store.readLocale())
            assertEquals(ShellLocale.English, owners.projection().locale)
        }
    }

    @Test
    fun history_confirmation_clears_only_history_and_the_continue_entry_survives() = run(PreviewSession.Restored.seed()) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.history.isNotEmpty() }
        onNodeWithTag("history-all").performScrollTo().performClick()
        onNodeWithTag("history-clear").performScrollTo().performClick()
        onNodeWithTag("history-cancel").performClick()
        onNodeWithTag("history-dialog").assertDoesNotExist()
        assertTrue(owners.shell.library.value.history.isNotEmpty())
        onNodeWithTag("history-clear").performClick()
        onNodeWithTag("history-confirm").performClick()
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.history.isEmpty() }
        assertTrue(owners.shell.library.value.continueListening.isNotEmpty(), "positions are kept")
        assertTrue(owners.shell.library.value.saved.any { it.saved }, "saved works are kept")
    }

    @Test
    fun an_offline_remote_keeps_a_signed_in_save_durable_and_visibly_pending() = run(PreviewSeed(
        previewState(), signedIn = true, remoteOffline = true,
    )) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.account.state.value is NativeAccountState.SignedIn }
        untilTag("work-$sonHa")
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        onNodeWithTag("save-work").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.pending > 0 }
        assertEquals(0, owners.libraryApi.mutations, "the unreachable remote accepted nothing")
        owners.libraryApi.unavailable = false
        onNodeWithTag("nav-library").performClick()
        onNodeWithTag("library-drain").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.pending == 0 }
        assertTrue(owners.libraryApi.mutations >= 1)
    }
}
