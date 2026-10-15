package com.cantos.theatre.app

import com.cantos.theatre.demo.demoState
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import java.io.File
import java.util.Locale
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertTrue

/** Real common presentation on the JVM renderer; no OS launch, callback or device claim. */
@OptIn(ExperimentalTestApi::class)
class LocalLaunchPresentationTest {
    @Test
    fun ready_guest_enters_discovery_even_while_catalog_is_offline() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            mainClock.autoAdvance = false
            setContent {
                CantosLocalLaunch(localReady = true, preferences = UiPreferences()) {
                    CantosDemoApp(initialState = demoState(catalogMode = ShellCatalogMode.Error))
                }
            }
            onNodeWithTag("local-launch").assertDoesNotExist()
            onNodeWithTag("header-title").assertTextEquals("Cantos")
            onNodeWithTag("nav-discovery").assertExists()
            onNodeWithTag("retry").assertExists()
        }
    }

    @Test
    fun readiness_interrupts_entrance_on_the_next_frame() {
        val ready = mutableStateOf(false)
        runDesktopComposeUiTest(width = 390, height = 844) {
            mainClock.autoAdvance = false
            setContent {
                CantosLocalLaunch(ready.value, UiPreferences()) {
                    Text("restored destination", Modifier.testTag("destination"))
                }
            }
            onNodeWithTag("local-launch").assertExists()
            mainClock.advanceTimeBy(32)
            runOnIdle { ready.value = true }
            // Deliver the readiness recomposition, without finishing the 260 ms entrance.
            mainClock.advanceTimeByFrame()
            onNodeWithTag("local-launch").assertDoesNotExist()
            onNodeWithTag("destination").assertTextEquals("restored destination")
            runOnIdle { ready.value = true }
            onNodeWithTag("local-launch").assertDoesNotExist()
        }
    }

    @Test
    fun ready_remount_preserves_supplied_navigation_instead_of_forcing_home() {
        val mounted = mutableStateOf(true)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                val scope = rememberCoroutineScope()
                val container = remember { mockAppContainer(scope, demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(DemoWorkIds.SonHa)))) }
                DisposableEffect(container) { onDispose { container.close() } }
                if (mounted.value) CantosLocalLaunch(true, UiPreferences()) {
                    CantosDemoApp(appContainer = container)
                }
            }
            onNodeWithText("Sơn Hà Ký").assertExists()
            runOnIdle { mounted.value = false }
            runOnIdle { mounted.value = true }
            onNodeWithTag("local-launch").assertDoesNotExist()
            onNodeWithText("Sơn Hà Ký").assertExists()
        }
    }

    @Test
    fun reduced_motion_layout_matrix_fits_and_is_captured_on_the_jvm() {
        val originalLocale = Locale.getDefault()
        try {
            for (language in listOf("vi", "en")) {
                Locale.setDefault(Locale.forLanguageTag(language))
                for (theme in listOf(Theme.Light, Theme.Dark)) {
                    for ((width, height, textScale) in listOf(Triple(390, 844, 1f), Triple(320, 568, 1f), Triple(844, 390, 1f), Triple(320, 568, 2f))) {
                        val name = "splash-${theme.name.lowercase()}-$language-${width}x$height-text$textScale"
                        runDesktopComposeUiTest(width = width, height = height) {
                            setContent {
                                Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) {
                                    CantosLocalLaunch(false, UiPreferences(theme = theme, reducedMotion = true, textScale = textScale)) {}
                                }
                            }
                            val rootBounds = onNodeWithTag("capture-root").fetchSemanticsNode().boundsInRoot
                            val copyBounds = onNodeWithTag("splash-copy", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
                            assertTrue(rootBounds.contains(copyBounds.topLeft) && rootBounds.contains(copyBounds.bottomRight), "$name clips essential copy: $copyBounds outside $rootBounds")
                            val bitmap = onNodeWithTag("capture-root").captureToImage()
                            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
                            check(output.isDirectory || output.mkdirs())
                            val data = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
                            File(output, "$name.png").writeBytes(data.bytes)
                            File(output, "$name.semantics.txt").writeText(onNodeWithTag("capture-root").printToString())
                        }
                    }
                }
            }
        } finally {
            Locale.setDefault(originalLocale)
        }
    }
}
