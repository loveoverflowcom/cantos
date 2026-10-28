package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyDescendant
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.unit.Density as DisplayDensity
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.DesignInventoryView
import com.cantos.ui.preview.InventoryFilter
import com.cantos.ui.preview.PreviewApp
import com.cantos.ui.preview.ViewportPreset
import com.cantos.ui.preview.filterInventory
import com.cantos.ui.preview.generated.DesignInventory
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals

/** Real JVM semantics for the gallery's design inventory tab and root-injected theme mode. */
class DesignInventoryViewTest {
    @get:Rule
    val compose = createComposeRule()

    private fun setPreview(widthDp: Int, heightDp: Int = 840, content: @Composable () -> Unit) {
        compose.setContent { Box(Modifier.requiredSize(widthDp.dp, heightDp.dp)) { content() } }
    }

    private fun shown(filter: InventoryFilter) = "${filterInventory(DesignInventory.specimens, filter).size} of 102 specimens"

    private fun findSpecimen(text: String) =
        compose.onNode(hasSetTextAction() and hasText("Find specimen")).performTextReplacement(text)

    @Test
    fun expanded_inventory_states_counts_and_keeps_a_proposal_free_of_native_samples() {
        setPreview(1000) { PreviewApp(initialSection = 2) }
        compose.onNodeWithText("Design v3 inventory").assertExists()
        compose.onNodeWithText("102 specimens = 32 in the shared registry + 70 proposals", substring = true).assertExists()
        compose.onNodeWithText("Existing 32").assertExists()
        compose.onNodeWithText("Proposed 70").assertExists()
        compose.onNodeWithText("CMP only 73").assertExists()
        compose.onNodeWithText("app-owned 57").assertExists()

        findSpecimen("mini-player")
        compose.onNodeWithText("1 of 102 specimens").assertExists()
        compose.onNodeWithText("Mini-player · mini-player").performScrollTo().performClick()
        compose.onNodeWithText("No native sample: a proposal is not a public API", substring = true).assertExists()
        compose.onNodeWithText("design: proposed").assertExists()
        compose.onNodeWithText("scope: app-owned").assertExists()
        compose.onNodeWithText("owner: cantos-theatre").assertExists()
        compose.onNodeWithText("platforms: web · cmp").assertExists()
        compose.onNodeWithText("Open Mini-player in Components").assertDoesNotExist()
        // Reuse links name registered components only; the proposal itself offers no sample to open.
        compose.onNodeWithContentDescription("Open surface in Components").assertExists()
        compose.onNodeWithContentDescription("Open range-field in Components").assertExists()
    }

    @Test
    fun a_web_only_specimen_says_so_and_never_offers_cmp() {
        setPreview(1000) { PreviewApp(initialSection = 2) }
        findSpecimen("studio-workspace")
        compose.onNodeWithText("Web only: Studio and Admin compositions never target CMP", substring = true).assertExists()
        compose.onNodeWithText("platforms: web").assertExists()
    }

    @Test
    fun an_existing_specimen_opens_its_component_and_clears_the_component_filters_that_were_set() {
        setPreview(1000) { PreviewApp() }
        // Narrow the Components tab first: a family and a query that exclude the specimen opened below.
        compose.onNodeWithText("fields").performScrollTo().performClick()
        compose.onNode(hasSetTextAction() and hasText("Find component")).performTextReplacement("text-field")
        compose.onNodeWithText("1 components").assertExists()
        compose.onNodeWithText("Design v3").performClick()
        findSpecimen("badge")
        compose.onNodeWithText("Badges · badge").performScrollTo().performClick()
        compose.onNodeWithText("Registered in the shared preview registry. Registration is not qualification", substring = true).assertExists()
        compose.onNodeWithText("Open Badges in Components").performScrollTo().performClick()
        compose.onNodeWithText("Opened badge from the design inventory. Registration is not qualification.").assertExists()
        // Back on Components with the filters cleared: the chosen component is displayed, not filtered away.
        compose.onNodeWithText("feedback · badge").assertIsDisplayed()
        compose.onNodeWithText("34 components").assertExists()
        compose.onNodeWithText("Design v3 inventory").assertDoesNotExist()
    }

    @Test
    fun a_reuse_link_opens_the_registered_component_it_names() {
        setPreview(1000) { PreviewApp(initialSection = 2) }
        findSpecimen("episode-list")
        compose.onNodeWithText("Danh sách tập · episode-list").performScrollTo().performClick()
        compose.onNodeWithContentDescription("Open status in Components").performScrollTo().performClick()
        compose.onNodeWithText("feedback · status").assertIsDisplayed()
    }

    @Test
    fun narrow_inventory_uses_one_specimen_picker_and_keeps_filter_and_detail_in_agreement() {
        setPreview(420) { PreviewApp(initialSection = 2) }
        findSpecimen("tooltip")
        compose.onNodeWithText("1 of 102 specimens").assertExists()
        compose.onNodeWithText("Tooltip & supporting text").assertExists()
        compose.onNodeWithText("scope: shared-primitive").assertExists()
        findSpecimen("không tồn tại")
        compose.onNodeWithText("No specimens").assertExists()
        findSpecimen("")
        compose.onNodeWithText("102 of 102 specimens").assertExists()
    }

    @Test
    fun at_exactly_the_expanded_breakpoint_the_window_not_its_padded_content_selects_the_two_pane_layout() {
        // The two-pane list item exists only in the master/detail layout; a narrow window shows a picker.
        setPreview(840) { PreviewApp(initialSection = 2) }
        compose.onNodeWithText("Buttons · button").assertExists()
    }

    @Test
    fun one_dp_below_the_expanded_breakpoint_the_layout_is_the_narrow_picker() {
        setPreview(839) { PreviewApp(initialSection = 2) }
        compose.onNodeWithText("Buttons · button").assertDoesNotExist()
        compose.onNodeWithText("Theme & density · ui-root").assertExists()
    }

    @Test
    fun a_fixed_height_page_scroll_region_keeps_the_specimen_detail_reachable_when_header_and_filters_fill_it() {
        // 2x text in a short narrow window: the header and filters alone are taller than the region.
        compose.setContent {
            Box(Modifier.requiredSize(390.dp, 500.dp)) {
                UiRoot(preferences = UiPreferences(textScale = 2f)) {
                    DesignInventoryView({}, Modifier.fillMaxSize(), expanded = false, scrollPage = true, initialSelectedId = "mini-player")
                }
            }
        }
        compose.onNodeWithText("No native sample: a proposal is not a public API", substring = true).performScrollTo().assertIsDisplayed()
    }

    @Test
    fun status_cmp_and_scope_chips_compose_and_clear() {
        setPreview(1000) { PreviewApp(initialSection = 2) }
        compose.onNodeWithText("Proposed 70").performClick()
        compose.onNodeWithText(shown(InventoryFilter(designStatus = "proposed"))).assertExists()
        compose.onNodeWithText("CMP only 73").performClick()
        compose.onNodeWithText(shown(InventoryFilter(designStatus = "proposed", cmpOnly = true))).assertExists()
        compose.onNodeWithText("app-owned 57").performClick()
        compose.onNodeWithText(shown(InventoryFilter(designStatus = "proposed", cmpOnly = true, scope = "app-owned"))).assertExists()
        compose.onNodeWithText("app-owned 57").performClick()
        compose.onNodeWithText("CMP only 73").performClick()
        compose.onNodeWithText("Proposed 70").performClick()
        compose.onNodeWithText("102 of 102 specimens").assertExists()
    }

    @Test
    fun system_theme_follows_the_injected_host_fact_and_explicit_modes_do_not() {
        var host by mutableStateOf(true)
        setPreview(1000) { PreviewApp(systemIsDark = host, initialThemeMode = ThemeMode.System) }
        compose.onNodeWithText("Theme mode System → dark; host reports dark. Only System follows the host.").assertExists()
        compose.runOnIdle { host = false }
        compose.onNodeWithText("Theme mode System → light; host reports light. Only System follows the host.").assertExists()
        compose.onNodeWithText("Dark").performClick()
        compose.runOnIdle { host = true }
        compose.onNodeWithText("Theme mode Dark → dark; host reports dark. Only System follows the host.").assertExists()
        compose.onNodeWithText("Light").performClick()
        compose.onNodeWithText("Theme mode Light → light; host reports dark. Only System follows the host.").assertExists()
        compose.onNodeWithText("System").performClick()
        compose.onNodeWithText("Theme mode System → dark; host reports dark. Only System follows the host.").assertExists()
    }

    @Test
    fun the_gallery_reports_the_constraint_host_scale_preview_multiplier_and_effective_scale() {
        setPreview(390) { PreviewApp(initialPreferences = UiPreferences(textScale = 2f)) }
        compose.onNodeWithText("Viewport 390×840 dp", substring = true).assertExists()
        // The JVM host runs at 1 font scale; the preview multiplier is applied on top by the theme.
        compose.onNodeWithText("font scale host 1.0 × preview 2.0 = 2.0", substring = true).assertExists()
        compose.onNodeWithText("fixtures vi-VN", substring = true).assertExists()
    }

    private fun pageScrollContainer() = compose.onNode(hasScrollAction() and hasAnyDescendant(hasText("Cantos UI System")))

    @Test
    fun enlarged_host_text_in_an_expanded_window_makes_the_whole_page_scroll() {
        // No preview multiplier: the host alone reports 2x text, and the header grows with it.
        compose.setContent {
            CompositionLocalProvider(LocalDensity provides DisplayDensity(1f, fontScale = 2f)) {
                Box(Modifier.requiredSize(1000.dp, 700.dp)) { PreviewApp(initialSection = 2) }
            }
        }
        pageScrollContainer().assertExists()
    }

    @Test
    fun normal_text_in_an_expanded_window_keeps_a_fixed_page_with_independently_scrolling_panes() {
        setPreview(1000, 700) { PreviewApp(initialSection = 2) }
        pageScrollContainer().assertDoesNotExist()
    }

    @Test
    fun a_host_that_cannot_resize_its_window_shows_no_width_buttons() {
        setPreview(1000) { PreviewApp() }
        ViewportPreset.entries.forEach { preset ->
            compose.onNodeWithContentDescription("Set window width to ${preset.label}").assertDoesNotExist()
        }
    }

    @Test
    fun a_host_that_owns_its_window_receives_the_chosen_width_preset() {
        val chosen = mutableListOf<ViewportPreset>()
        setPreview(1000) { PreviewApp(onWidthPreset = { chosen += it }) }
        compose.onNodeWithContentDescription("Set window width to 390 dp").performClick()
        compose.onNodeWithContentDescription("Set window width to 1140 dp").performClick()
        assertEquals(listOf(ViewportPreset.Typical, ViewportPreset.Wide), chosen)
    }
}
