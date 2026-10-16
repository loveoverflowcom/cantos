package com.cantos.theatre.app.workbench

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import com.cantos.ui.ThemeMode
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewSettings
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals

/** Desktop semantics and pixels only; every exported image still needs an explicit review. */
@OptIn(ExperimentalTestApi::class)
class TheatreWorkbenchCaptureTest {
    @Test
    fun snapshot_export_reset_and_multiline_paste_restore_work_through_actual_controls() {
        lateinit var controller: TheatreWorkbenchController
        runDesktopComposeUiTest(width = 1200, height = 1000) {
            setContent {
                val scope = rememberCoroutineScope()
                controller = remember { TheatreWorkbenchController(scope, TheatreRecipe.fixture(TheatreFixture.Restored)) }
                androidx.compose.runtime.DisposableEffect(controller) { onDispose { controller.close() } }
                TheatreWorkbench(controller)
            }
            runOnIdle {
                controller.callback()(ShellEvent.Navigate(ShellRoute.Search))
                controller.callback()(ShellEvent.Search("Sơn Hà"))
            }
            waitForIdle()
            val before = controller.snapshot().state
            onNodeWithTag("workbench-export").performScrollTo().performClick()
            onNodeWithTag("workbench-transfer-input").performScrollTo()
            val payload = onNodeWithTag("workbench-transfer-input").fetchSemanticsNode().config[SemanticsProperties.EditableText].text
            kotlin.test.assertTrue(payload.contains('\n'), "Export must retain the multiline JSON value")
            onNodeWithTag("workbench-reset").performScrollTo().performClick()
            waitForIdle()
            assertEquals("", controller.snapshot().state.query)
            assertEquals(ShellRoute.Library, controller.snapshot().state.route)
            onNodeWithTag("workbench-transfer-input").performScrollTo().performTextReplacement(payload)
            onNodeWithTag("workbench-restore").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("workbench-import-result").assertTextContains("Restored theatre-restored-v1", substring = true)
            assertEquals(before, controller.snapshot().state)
        }
    }

    @Test
    fun changing_locale_and_theme_preserves_search_caret_focus_and_application_owners() {
        lateinit var controller: TheatreWorkbenchController
        runDesktopComposeUiTest(width = 1200, height = 1000) {
            setContent {
                val scope = rememberCoroutineScope()
                controller = remember { TheatreWorkbenchController(scope) }
                androidx.compose.runtime.DisposableEffect(controller) { onDispose { controller.close() } }
                TheatreWorkbench(controller)
            }
            runOnIdle {
                controller.callback()(ShellEvent.Navigate(ShellRoute.Search))
                controller.callback()(ShellEvent.Search("Sơn Hà"))
            }
            waitForIdle()
            onNodeWithTag("search-input").performClick().performTextInputSelection(TextRange(1, 3))
            onNodeWithTag("search-input").assertIsFocused()
            val original = controller.mount.value
            val app = original.session.container
            val playback = app.playback.state.value
            val listening = app.listening.state.value
            val navigation = app.navigation.value
            val account = app.accountState.value
            val membership = original.session.ui.value.membership
            runOnIdle { controller.updateSettings(controller.selectedRecipe.settings.copy(themeMode = ThemeMode.Dark, locale = PreviewLocale.En)) }
            waitForIdle()
            kotlin.test.assertSame(original, controller.mount.value)
            assertEquals(ShellLocale.English, controller.snapshot().state.locale)
            assertEquals(ShellTheme.Dark, controller.snapshot().state.theme)
            assertEquals(playback, app.playback.state.value)
            assertEquals(listening, app.listening.state.value)
            assertEquals(navigation, app.navigation.value)
            assertEquals(account, app.accountState.value)
            assertEquals(membership, original.session.ui.value.membership)
            onNodeWithTag("search-input").assertIsFocused()
            assertEquals(TextRange(1, 3), onNodeWithTag("search-input").fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
            assertEquals("Sơn Hà", controller.snapshot().state.query)
        }
    }

    @Test
    fun fresh_light_and_restored_dark_large_text_use_actual_theatre_composition() {
        capture("workbench-fresh-light-390", TheatreFixture.Fresh, PreviewSettings(reducedMotion = true))
        capture("workbench-restored-dark-text2-320", TheatreFixture.Restored,
            PreviewSettings(themeMode = ThemeMode.Dark, fontScale = 2f, constraints = ConstraintFrame(320, 800), reducedMotion = true))
    }

    private fun capture(name: String, fixture: TheatreFixture, settings: PreviewSettings) {
        var density = 0f
        runDesktopComposeUiTest(width = 1200, height = 1000) {
            setContent {
                val scope = rememberCoroutineScope()
                val controller = remember { TheatreWorkbenchController(scope, TheatreRecipe.fixture(fixture, settings)) }
                androidx.compose.runtime.DisposableEffect(controller) { onDispose { controller.close() } }
                density = LocalDensity.current.density
                Box(Modifier.requiredSize(1200.dp, 1000.dp).testTag("workbench-capture")) { TheatreWorkbench(controller) }
            }
            waitForIdle()
            onNodeWithTag("workbench-snapshot").assertTextContains("route=Library", substring = true)
            val image = onNodeWithTag("workbench-capture").captureToImage()
            assertEquals(1200, image.width)
            assertEquals(1000, image.height)
            val bytes = requireNotNull(Image.makeFromBitmap(image.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
            check(output.isDirectory || output.mkdirs())
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag("workbench-capture", useUnmergedTree = true).printToString(maxDepth = 100))
            val sha = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
            File(output, "$name.json").writeText("""
                {"renderer":"JVM Desktop Compose UI test","entrypoint":"TheatreWorkbench → CantosDemoApp",
                 "recipe":"${fixture.id}","seed":1,"windowWidthDp":1200,"windowHeightDp":1000,
                 "constraintWidthDp":${settings.constraints.widthDp},"constraintHeightDp":${settings.constraints.heightDp},
                 "rendererDensity":$density,"requestedAppTextScale":${settings.fontScale},
                 "themeMode":"${settings.themeMode}","locale":"${settings.locale}","reducedMotion":${settings.reducedMotion},
                 "png_sha256":"$sha","evidence":"screenshot-captured; inspection pending"}
            """.trimIndent())
        }
    }
}
