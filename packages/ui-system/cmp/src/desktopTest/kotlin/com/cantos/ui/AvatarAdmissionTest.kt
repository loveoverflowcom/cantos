package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.ComponentExample
import org.junit.Test

/** Actual production components on a JVM root, not Android/iOS or OS assistive technology. */
@OptIn(ExperimentalTestApi::class)
class AvatarAdmissionTest {
    @Test
    fun decoded_image_loss_preserves_the_named_identity_and_control_geometry() {
        runDesktopComposeUiTest {
            var image by mutableStateOf<ImageBitmap?>(ImageBitmap(16, 16))
            setContent {
                UiRoot {
                    Avatar("Đỗ Văn An", image = image, size = AvatarSize.Control,
                        accessibleName = AccessibleName.of("Ảnh đại diện của Đỗ Văn An"),
                        modifier = Modifier.testTag("identity"))
                }
            }
            onNodeWithTag("identity").assertContentDescriptionEquals("Ảnh đại diện của Đỗ Văn An")
                .assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            runOnIdle { image = null }
            onNodeWithTag("identity").assertContentDescriptionEquals("Ảnh đại diện của Đỗ Văn An")
                .assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
        }
    }

    @Test
    fun avatar_states_and_the_library_icon_are_captured_with_the_current_fonts_and_tokens() {
        for (theme in Theme.entries) for (width in listOf(320, 840)) {
            val preferences = UiPreferences(theme = theme, textScale = if (width == 320) 2f else 1f)
            runDesktopComposeUiTest(width = width, height = 1800) {
                var density = 1f
                var scale = 1f
                setContent {
                    density = LocalDensity.current.density
                    scale = LocalDensity.current.fontScale
                    BundledPreviewCaptureRoot(preferences) {
                        Column(Modifier.testTag("avatar-admission").padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(16.dp)) {
                            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                                Icon("home")
                                Icon("library_spines")
                                Icon("library_spines", tint = tokenColor("color.primary"))
                            }
                            Avatar("", size = AvatarSize.Control, accent = true)
                            for (state in listOf("initials-single", "initials-pair", "no-initial", "image", "image-failed", "sizes", "named")) {
                                FieldLabel(state)
                                ComponentExample(PreviewExample.Avatar, state, {}, Modifier)
                            }
                        }
                    }
                }
                waitForIdle()
                writeCapture(CaptureScenario("avatar-library-646-${theme.tokenName}-$width",
                    "icons", "avatar/icon", "fallback-and-registry",
                    themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                    systemIsDark = false, preferences = preferences,
                    viewportWidthDp = width, viewportHeightDp = 1800,
                    hostDensity = density, hostFontScale = scale, fontSource = BundledPreviewFontSource),
                    onNodeWithTag("avatar-admission").captureToImage())
            }
        }
    }
}
