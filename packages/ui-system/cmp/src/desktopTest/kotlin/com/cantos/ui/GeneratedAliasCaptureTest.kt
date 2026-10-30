package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import org.junit.Test

/** Offline samples only: same alias/seed and production Avatar, three design directions. */
@OptIn(ExperimentalTestApi::class)
class GeneratedAliasCaptureTest {
    @Test
    fun alias_directions_render_profile_and_navigation_in_both_themes() {
        for (theme in Theme.entries) for (width in listOf(320, 840)) {
            val preferences = UiPreferences(theme = theme, textScale = if (width == 320) 2f else 1f,
                expression = CantosExpression.Theatre)
            runDesktopComposeUiTest(width = width, height = 840) {
                var density = 1f
                var scale = 1f
                setContent {
                    density = LocalDensity.current.density
                    scale = LocalDensity.current.fontScale
                    BundledPreviewCaptureRoot(preferences) {
                        Column(Modifier.testTag("alias-comparison").padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(16.dp)) {
                            for ((label, tone) in listOf("A · Lavender" to AvatarTone.Lavender,
                                "B · Gold" to AvatarTone.Gold, "C · Personal" to null)) {
                                Text(label)
                                for (size in listOf(AvatarSize.Large, AvatarSize.Control)) {
                                    Surface(tone = SurfaceTone.PrimaryContainer, variant = SurfaceVariant.Profile, inset = Space.Sm) {
                                        Row(verticalAlignment = Alignment.CenterVertically,
                                            horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                            Avatar("Mây Lam", stableId = "6e45675a6ee0e0dd5a16f55c6fcaafd1f8737a27c0572ffc39a2a1bd22100ecb",
                                                size = size, initials = AvatarInitials.Pair, tone = tone)
                                            Text("Mây Lam")
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                waitForIdle()
                writeCapture(CaptureScenario("generated-alias-${theme.tokenName}-$width", "icons", "avatar",
                    "generated-alias", themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                    systemIsDark = false, preferences = preferences, viewportWidthDp = width, viewportHeightDp = 840,
                    hostDensity = density, hostFontScale = scale, fontSource = BundledPreviewFontSource),
                    onNodeWithTag("alias-comparison").captureToImage())
            }
        }
    }
}
