package com.cantos.ui

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.ui.generated.CantosTokens
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The hit target is a control role; the visible glyph is a separate, smaller icon role. After the v0.6.0
 * reconciliation `size.control` is 48 at both densities, so these checks prove the target follows the
 * role (and not the glyph) but cannot tell density handling from a hard-coded 48.
 */
class HitTargetTest {
    @OptIn(ExperimentalTestApi::class)
    @Test
    fun icon_button_hit_target_follows_the_control_token_at_both_densities_and_not_the_icon_size() {
        val icon = CantosTokens.number("size.icon").toFloat()
        Density.entries.forEach { density ->
            runDesktopComposeUiTest(width = 200, height = 200) {
                setContent {
                    UiRoot(preferences = UiPreferences(density = density)) {
                        IconButton(AccessibleName.of("Thêm cảnh cho Ánh đèn cuối sân khấu"), "add", {})
                    }
                }
                val bounds = onNodeWithContentDescription("Thêm cảnh cho Ánh đèn cuối sân khấu").fetchSemanticsNode().boundsInRoot
                val control = CantosTokens.number("size.control", density.tokenName).toFloat()
                // The host runs at 1 px per dp, so bounds are logical units.
                assertEquals(control, bounds.width, "${density.name} hit-target width")
                assertEquals(control, bounds.height, "${density.name} hit-target height")
                assertTrue(icon < control, "the visible glyph ($icon) stays smaller than its hit target ($control)")
            }
        }
        // Both densities keep the regular minimum target at 48 logical units after the v3 reconciliation.
        assertEquals(48.0, CantosTokens.number("size.control", Density.Compact.tokenName))
        assertEquals(24.0, CantosTokens.number("size.icon"))
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun back_button_keeps_its_own_48_unit_target_in_both_densities() {
        Density.entries.forEach { density ->
            runDesktopComposeUiTest(width = 200, height = 200) {
                setContent {
                    UiRoot(preferences = UiPreferences(density = density)) {
                        BackButton(AccessibleName.of("Quay lại Tài khoản"), {})
                    }
                }
                val bounds = onNodeWithContentDescription("Quay lại Tài khoản").fetchSemanticsNode().boundsInRoot
                val target = CantosTokens.number("size.backControl").toFloat()
                assertTrue(bounds.width >= target && bounds.height >= target, "${density.name} Back target ${bounds.width}×${bounds.height}")
            }
        }
    }
}
