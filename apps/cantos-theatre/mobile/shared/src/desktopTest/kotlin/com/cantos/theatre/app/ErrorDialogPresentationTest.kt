package com.cantos.theatre.app

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.errors.ErrorAction
import com.cantos.theatre.application.errors.ErrorKind
import com.cantos.theatre.application.errors.ErrorNotice
import com.cantos.theatre.application.errors.ErrorOperation
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.features.shell.presentation.ErrorDialog
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.CantosTheme
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class ErrorDialogPresentationTest {
    @Test
    fun modal_anatomy_localization_touch_targets_escape_and_focus_restoration_hold_across_themes_and_scales() {
        for (theme in listOf(Theme.Light, Theme.Dark)) {
            for ((locale, expectedTitle, expectedBody, expectedPrimary, expectedClose) in listOf(
                listOf(
                    ShellLocale.Vietnamese,
                    "Chưa hoàn tất thao tác",
                    "Chưa kết nối được với máy chủ. Hãy kiểm tra mạng và thử lại.",
                    "Thử lại",
                    "Đóng",
                ),
                listOf(
                    ShellLocale.English,
                    "Couldn’t complete that",
                    "Couldn’t reach the server. Check your connection and try again.",
                    "Retry",
                    "Close",
                ),
            )) {
                val shellLocale = locale as ShellLocale
                val titleText = expectedTitle as String
                val bodyText = expectedBody as String
                val primaryText = expectedPrimary as String
                val closeText = expectedClose as String
                runDesktopComposeUiTest(width = 320, height = 568) {
                    val triggerFocus = FocusRequester()
                    var active by mutableStateOf<ErrorNotice?>(
                        ErrorNotice(ErrorKind.Network, ErrorOperation.StartPlayback),
                    )
                    val chosen = mutableListOf<ErrorAction>()
                    setContent {
                        CantosTheme(UiPreferences(theme = theme, textScale = 2f)) {
                            Column {
                                Button(
                                    onClick = {},
                                    modifier = Modifier.focusRequester(triggerFocus).testTag("caller-trigger"),
                                ) {
                                    Text(ShellCopy(shellLocale).text(com.cantos.theatre.presentation.model.CopyKey.Play))
                                }
                                active?.let { notice ->
                                    ErrorDialog(
                                        notice = notice,
                                        copy = ShellCopy(shellLocale),
                                        onAction = { action ->
                                            chosen += action
                                            active = null
                                        },
                                        onDismiss = { active = null },
                                        returnFocus = triggerFocus,
                                    )
                                }
                            }
                        }
                    }
                    waitForIdle()
                    onNodeWithTag("error-dialog").assertIsDisplayed()
                    onNodeWithTag("error-dialog-title")
                        .assertTextEquals(titleText)
                        .assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading))
                    onNodeWithTag("error-dialog-body").assertTextEquals(bodyText)
                    onNodeWithTag("error-dialog-primary").assertTextEquals(primaryText).assertIsFocused()
                    onNodeWithTag("error-dialog-close").assertTextEquals(closeText)

                    for (tag in listOf("error-dialog-primary", "error-dialog-close")) {
                        val bounds = onNodeWithTag(tag).fetchSemanticsNode().boundsInRoot
                        assertTrue(bounds.height >= 48f, "undersized $tag at 2x scale: $bounds")
                    }

                    // Escape dismisses the modal and restores focus to the caller control.
                    onNodeWithTag("error-dialog").performKeyInput { pressKey(Key.Escape) }
                    waitForIdle()
                    onNodeWithTag("error-dialog").assertDoesNotExist()
                    onNodeWithTag("caller-trigger").assertIsFocused()
                    assertTrue(chosen.isEmpty())
                }
            }
        }
    }

    @Test
    fun playback_manifest_network_failure_opens_the_error_dialog_and_retry_resolves_it() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val work = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
            val unit = PublicCatalogSamples.unit(1, title = "Chương 1: Người dẫn chuyện trở về", durationMs = 125_000)
            val audio = requireNotNull(
                AudioIdentity.of(
                    work.id.value,
                    unit.id.value,
                    unit.publicationId.value,
                    "60000000-0000-4000-8000-000000000006",
                    "b".repeat(64),
                    unit.durationMs,
                ),
            )
            var failManifest = true
            var manifestCalls = 0
            var starts = 0
            setContent {
                val scope = rememberCoroutineScope()
                val shell = remember {
                    val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
                        override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) =
                            PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
                        override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
                        override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                            PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(unit)))
                    }).also { it.dispatch(PublicCatalogIntent.Start) }
                    val engine = object : StreamingMediaEngine {
                        private val obs = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 16)
                        override val events = obs.asSharedFlow()
                        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
                        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
                        override suspend fun start(command: PlaybackEngineCommand) { starts += 1 }
                        override suspend fun pause(command: PlaybackEngineCommand) = Unit
                        override suspend fun seek(command: PlaybackEngineCommand) = Unit
                        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
                        override fun close() = Unit
                    }
                    val runtime = NativeSessionRuntime(
                        scope = scope,
                        nativeFactory = { engine },
                        access = object : PlaybackAccessPort {
                            override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> {
                                manifestCalls += 1
                                return if (failManifest) PlaybackAccessResult.Refused(PlaybackAccessFailure.Network)
                                else PlaybackAccessResult.Accepted(audio)
                            }
                            override suspend fun authorize(identity: AudioIdentity) =
                                PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 1_000_000))
                        },
                        account = null,
                        library = null,
                        newUuid = { "00000000-0000-4000-8000-000000000001" },
                        nowMs = { 1_000 },
                    )
                    TheatreShell(scope, catalog, runtime, PreferencesController()) to Triple(catalog, runtime, Unit)
                }
                DisposableEffect(shell) {
                    onDispose {
                        shell.first.close()
                        shell.second.second.close()
                        shell.second.first.close()
                    }
                }
                CantosTheatreApp(shell.first)
            }
            waitForIdle()
            onNodeWithTag("work-${work.id.value}").performScrollTo().performClick()
            waitUntil { onAllNodes(androidx.compose.ui.test.hasTestTag("episode-${unit.id.value}")).fetchSemanticsNodes().isNotEmpty() }
            onNodeWithTag("episode-${unit.id.value}").performScrollTo().performClick()
            waitForIdle()

            // The network refusal opens the standard modal error window with Retry + Close.
            onNodeWithTag("error-dialog").assertIsDisplayed()
            onNodeWithTag("error-dialog-body")
                .assertTextEquals("Chưa kết nối được với máy chủ. Hãy kiểm tra mạng và thử lại.")
            assertEquals(1, manifestCalls)

            // Retry repeats the failed playback start and closes the dialog once resolved.
            failManifest = false
            onNodeWithTag("error-dialog-primary").performClick()
            waitUntil { starts == 1 }
            waitForIdle()
            onNodeWithTag("error-dialog").assertDoesNotExist()
            assertEquals(2, manifestCalls)
        }
    }
}
