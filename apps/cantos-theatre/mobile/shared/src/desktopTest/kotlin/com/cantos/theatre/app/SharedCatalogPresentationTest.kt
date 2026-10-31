package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.catalog.SharedCatalogPhase
import com.cantos.theatre.application.catalog.SharedCatalogState
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.valid
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.features.shell.presentation.LocalShellSharedLibrary
import com.cantos.theatre.features.shell.presentation.LocalShellSharedLibraryHasItems
import com.cantos.theatre.features.shared.presentation.SharedCatalogContent
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.ports.SharedSessionKey
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.UiRoot
import com.cantos.ui.resolve
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals

/** Real production composition, JVM Skia only. No native decoder/device/auth claim. */
@OptIn(ExperimentalTestApi::class)
class SharedCatalogPresentationTest {
    private val work = PublicCatalogSamples.work(1, title = "Sơn Hà Ký — Ánh đèn cuối sân khấu")
    private val session = SharedSessionKey("actor-a", "session-a", 1)
    private val metadata = PublicUnit.withoutAudio(PublicCatalogSamples.unitId(1), PublicUnitKind.Chapter,
        0, "Một lời hẹn", "Giữa non xanh và những lời thề cũ.").valid()
    private val audio = PublicCatalogSamples.unit(2, title = "Đêm trở về")

    @Test fun shared_rows_mount_in_the_real_library_without_a_false_empty_hero_or_public_selection() =
        runDesktopComposeUiTest(width = 390, height = 1400) {
            var opened: com.cantos.theatre.domain.catalog.PublicWorkId? = null
            val shared = SharedCatalogState(session, phase = SharedCatalogPhase.Ready, works = listOf(work))
            setContent {
                CompositionLocalProvider(
                    LocalShellSharedLibraryHasItems provides true,
                    LocalShellSharedLibrary provides {
                        SharedCatalogContent(shared, ShellCopy(ShellLocale.Vietnamese), { opened = it }, {}, {}, {})
                    },
                ) {
                    CantosPresentation(ShellState(selectedEpisode = com.cantos.theatre.presentation.model.ShellEpisodeId(
                        "no-selection", com.cantos.theatre.presentation.model.ShellWorkId("no-selection")),
                        stack = listOf(ShellRoute.Library)), ShellCatalogView(), {})
                }
            }
            onNodeWithTag("library-continue").assertExists()
            onNodeWithTag("library-empty-hero").assertDoesNotExist()
            onNodeWithTag("shared-work-${work.id.value}").performScrollTo().performClick()
            assertEquals(work.id, opened)
        }

    @Test fun metadata_and_audio_association_both_refuse_play_until_native_transport_is_qualified() =
        runDesktopComposeUiTest(width = 390, height = 1400) {
            val copy = ShellCopy(ShellLocale.Vietnamese)
            setContent {
                UiRoot {
                    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        SharedCatalogContent(SharedCatalogState(session, work.id, SharedCatalogPhase.Ready,
                            work = work, units = listOf(metadata, audio)), copy, {}, {}, {}, {}, detail = true)
                    }
                }
            }
            onNodeWithTag("shared-play-${metadata.id.value}").performScrollTo().assertIsNotEnabled()
            onNodeWithText(copy.text(CopyKey.EpisodeNoAudio)).assertExists()
            onNodeWithTag("shared-play-${audio.id.value}").performScrollTo().assertIsNotEnabled()
            onNodeWithText(copy.text(CopyKey.SharedNativeUnavailable)).assertExists()
            onNodeWithTag("save-work").assertDoesNotExist()
            onNodeWithTag("bookmark-add").assertDoesNotExist()
        }

    @Test fun sign_in_and_reload_dispatch_only_their_named_intents_and_denial_has_no_private_title() =
        runDesktopComposeUiTest(width = 390, height = 844) {
            var phase by mutableStateOf(SharedCatalogPhase.SignInRequired)
            var logins = 0
            var reads = 0
            val copy = ShellCopy(ShellLocale.Vietnamese)
            setContent {
                UiRoot { SharedCatalogContent(SharedCatalogState(phase = phase), copy, {}, { logins += 1 }, { reads += 1 }, {}) }
            }
            onNodeWithTag("shared-sign-in").performClick()
            assertEquals(1, logins)
            assertEquals(0, reads)
            runOnIdle { phase = SharedCatalogPhase.Denied }
            onNodeWithText(copy.text(CopyKey.SharedDenied)).assertExists()
            onNodeWithText(work.title).assertDoesNotExist()
            onNodeWithTag("shared-refresh").performClick()
            assertEquals(1, reads)
        }

    @Test fun inspectable_matrix_uses_native_components_with_vi_en_dark_and_large_text() {
        for ((width, theme, locale, scale) in listOf(
            Frame(320, ThemeMode.Light, ShellLocale.Vietnamese, 1f),
            Frame(390, ThemeMode.Dark, ShellLocale.Vietnamese, 1f),
            Frame(390, ThemeMode.Light, ShellLocale.English, 2f),
            Frame(840, ThemeMode.Dark, ShellLocale.Vietnamese, 2f),
        )) {
            runDesktopComposeUiTest(width = width, height = 1400) {
                setContent {
                    UiRoot(preferences = UiPreferences(theme = theme.resolve(false), reducedMotion = true)) {
                        CompositionLocalProvider(LocalDensity provides Density(LocalDensity.current.density, scale)) {
                            Box(Modifier.requiredSize(width.dp, 1400.dp).testTag("shared-capture")) {
                                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                                    SharedCatalogContent(SharedCatalogState(session, work.id, SharedCatalogPhase.Ready,
                                        work = work, units = listOf(metadata, audio)), ShellCopy(locale), {}, {}, {}, {}, detail = true)
                                }
                            }
                        }
                    }
                }
                onNodeWithTag("shared-play-${metadata.id.value}").assertIsNotEnabled()
                onNodeWithTag("shared-refresh").performScrollTo().assertExists()
                // Restore the same top viewport for the attributed matrix capture.
                onNodeWithText(ShellCopy(locale).text(CopyKey.SharedTitle)).performScrollTo()
                System.getenv("CANTOS_SHARED_REVIEW_CAPTURE_DIR")?.let { destination ->
                    val name = "shared-$width-${theme.name}-${locale.name}-$scale"
                    val bytes = requireNotNull(Image.makeFromBitmap(onNodeWithTag("shared-capture").captureToImage().asSkiaBitmap())
                        .encodeToData(EncodedImageFormat.PNG)).bytes
                    val directory = File(destination).apply { mkdirs() }
                    File(directory, "$name.png").writeBytes(bytes)
                    val sha = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it.toInt() and 255) }
                    File(directory, "$name.json").writeText("""{"renderer":"JVM desktop Skia","width_dp":$width,"height_dp":1400,"theme":"${theme.name}","locale":"${locale.name}","fontScale":$scale,"reducedMotion":true,"png_sha256":"$sha","fixture":"shared-metadata-and-unqualified-audio","native_playback":"NOT_RUN"}""")
                }
            }
        }
    }

    private data class Frame(val width: Int, val theme: ThemeMode, val locale: ShellLocale, val scale: Float)
}
