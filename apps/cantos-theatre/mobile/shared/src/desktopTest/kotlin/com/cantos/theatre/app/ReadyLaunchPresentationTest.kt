package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.ui.UiPreferences
import java.io.File
import kotlin.test.assertEquals
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** Real ready-handoff, app and shell on Desktop Compose; HTTP/player ports are explicit doubles.
 * This is not Android SplashScreen, Activity recreation or device acceptance (#754).
 */
@OptIn(ExperimentalTestApi::class, ExperimentalCoroutinesApi::class)
class ReadyLaunchPresentationTest {
    private class Fixture(scope: CoroutineScope) {
        val response = CompletableDeferred<PublicCatalogRead<PublicWorkPage>>()
        var playerCreations = 0
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) = response.await()
            override suspend fun work(id: PublicWorkId): PublicCatalogRead<PublicWork> = error("No detail read during startup")
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) = error("No unit read during startup")
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val runtime = NativeSessionRuntime(
            scope,
            nativeFactory = { playerCreations++; SilentPlayer },
            access = object : PlaybackAccessPort {
                override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> =
                    error("Startup must not request playback")
                override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> =
                    error("Startup must not authorize playback")
            },
            account = null,
            library = null,
            newUuid = { "00000000-0000-4000-8000-000000000001" },
            nowMs = { 1_000 },
        )
        val preferences = PreferencesController()
        val shell = TheatreShell(scope, catalog, runtime, preferences)
    }

    private object SilentPlayer : StreamingMediaEngine {
        override val events = emptyFlow<PlaybackObservation>()
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = error("No audio during startup")
        override suspend fun prepare(command: PlaybackEngineCommand) = error("No audio during startup")
        override suspend fun start(command: PlaybackEngineCommand) = error("No audio during startup")
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = error("No audio during startup")
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    @Test
    fun ready_host_renders_guest_content_without_waiting_for_catalog_and_then_shows_recovery() = runTest {
        val fixture = Fixture(backgroundScope)
        runCurrent()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.fillMaxSize().testTag("startup-content")) {
                    CantosLocalLaunch(localReady = true, preferences = UiPreferences()) {
                        CantosTheatreApp(fixture.shell)
                    }
                }
            }
            onNodeWithTag("local-launch").assertDoesNotExist()
            onNodeWithTag("landing").assertIsDisplayed()
            onNodeWithTag("catalog-recovery").assertExists()
            onNodeWithTag("retry").assertDoesNotExist()
            assertEquals(false, fixture.response.isCompleted, "The guest reached content with HTTP still pending")
            saveCapture("ready-pending-light", onNodeWithTag("startup-content").captureToImage())
            fixture.response.complete(PublicCatalogRead.Unavailable(retryable = true))
            runCurrent()
            onNodeWithTag("retry").assertExists()
            onNodeWithTag("error-dialog-primary").assertIsDisplayed().assertIsEnabled()
            onNodeWithTag("local-launch").assertDoesNotExist()
            assertEquals(listOf(TheatreRoute.Landing), fixture.shell.navigation.value.stack)
            saveCapture("ready-offline-light", onNodeWithTag("startup-content").captureToImage())
        }
    }

    @Test
    fun ready_handoff_remount_and_theme_changes_preserve_the_destination_and_one_player() = runTest {
        val fixture = Fixture(backgroundScope)
        fixture.response.complete(PublicCatalogRead.Unavailable(retryable = true))
        runCurrent()
        fixture.shell.dispatch(ShellEvent.Navigate(ShellRoute.Search))
        runCurrent()
        runDesktopComposeUiTest(width = 390, height = 844) {
            var mounted by mutableStateOf(true)
            setContent {
                Box(Modifier.fillMaxSize().testTag("startup-content")) {
                    if (mounted) CantosLocalLaunch(localReady = true, preferences = UiPreferences()) {
                        CantosTheatreApp(fixture.shell)
                    }
                }
            }
            for (theme in listOf(PreferenceTheme.Dark, PreferenceTheme.Light)) {
                runOnIdle { mounted = false }
                runOnIdle { fixture.preferences.setTheme(theme); mounted = true }
                runCurrent()
                onNodeWithTag("local-launch").assertDoesNotExist()
                onNodeWithTag("retry").assertIsDisplayed()
                onNodeWithTag("error-dialog-primary").assertIsDisplayed().assertIsEnabled()
                assertEquals(listOf(TheatreRoute.Landing, TheatreRoute.Search), fixture.shell.navigation.value.stack)
                assertEquals(1, fixture.playerCreations)
                saveCapture("ready-remount-${theme.name.lowercase()}", onNodeWithTag("startup-content").captureToImage())
            }
        }
    }

    private fun saveCapture(name: String, bitmap: androidx.compose.ui.graphics.ImageBitmap) {
        val output = File("build/reports/startup-754").apply { mkdirs() }
        File(output, "$name.png").writeBytes(
            requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes,
        )
    }
}
