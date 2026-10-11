package com.cantos.ui

import com.cantos.ui.generated.CantosIcons
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.generated.PreviewCatalog
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class PreviewModelTest {
    @Test
    fun every_registered_component_has_exactly_one_native_example() {
        val registered = PreviewCatalog.components.map { it.id }.sorted()
        val implemented = PreviewExample.entries.map { it.id }.sorted()
        assertEquals(29, registered.size)
        assertEquals(registered, registered.distinct())
        assertEquals(registered, implemented)
        PreviewCatalog.components.forEach { component ->
            assertNotNull(PreviewExample.fromId(component.id))
            assertTrue(component.states.isNotEmpty(), component.id)
            assertEquals("implemented-unverified", component.cmpStatus)
            assertTrue(!component.cmpReason.isNullOrBlank(), component.id)
        }
        assertEquals(null, PreviewExample.fromId("not-a-registered-component"))
    }

    @Test
    fun blank_accessible_names_and_disabled_reasons_are_rejected() {
        assertEquals("Thêm cảnh", AccessibleName.of("Thêm cảnh").text)
        assertEquals("Chưa chọn bản nháp", DisabledReason.of("Chưa chọn bản nháp").text)
        assertFailsWith<IllegalArgumentException> { AccessibleName.of(" \n") }
        assertFailsWith<IllegalArgumentException> { DisabledReason.of(" \t") }
    }

    @Test
    fun disabled_and_loading_actions_never_dispatch_and_keep_exact_reason() {
        val reason = DisabledReason.of("Chưa chọn bản nháp")
        val pending = DisabledReason.of("Đang lưu bản nháp")
        assertTrue(ButtonState.Enabled.canPress())
        assertEquals(null, ButtonState.Enabled.reason())
        assertFalse(ButtonState.Disabled(reason).canPress())
        assertEquals(reason.text, ButtonState.Disabled(reason).reason())
        assertFalse(ButtonState.Loading(pending).canPress())
        assertEquals(pending.text, ButtonState.Loading(pending).reason())
    }

    @Test
    fun readonly_text_remains_enabled_but_refuses_edits() {
        assertTrue(FieldState.Editable.acceptsInput())
        assertTrue(FieldState.Editable.isEnabled())
        assertFalse(FieldState.ReadOnly.acceptsInput())
        assertTrue(FieldState.ReadOnly.isEnabled())
        val reason = DisabledReason.of("Đang tải")
        val blocked = listOf(FieldState.Disabled(reason), FieldState.Loading(reason))
        blocked.forEach {
            assertFalse(it.acceptsInput())
            assertFalse(it.isEnabled())
            assertEquals("Đang tải", it.reason())
        }
    }

    @Test
    fun native_choice_mode_explicitly_rejects_readonly() {
        assertEquals(ButtonState.Enabled, nativeChoiceState(FieldState.Editable))
        val native = nativeChoiceState(FieldState.ReadOnly)
        assertTrue(native is ButtonState.Disabled)
        assertEquals("Read-only is not a native choice mode", native.reason())
        val reason = DisabledReason.of("Không có lựa chọn")
        assertEquals(ButtonState.Disabled(reason), nativeChoiceState(FieldState.Disabled(reason)))
        assertEquals(ButtonState.Loading(reason), nativeChoiceState(FieldState.Loading(reason)))
    }

    @Test
    fun catalog_search_and_family_filter_compose_without_mutation() {
        assertEquals(29, filterComponents("", null).size)
        assertEquals(listOf("icon-button"), filterComponents(" ICON-BUTTON ", "actions").map { it.id })
        assertEquals(emptyList(), filterComponents("icon-button", "fields"))
        assertEquals(listOf("text-area", "text-field"), filterComponents("text", "fields").map { it.id }.sorted())
        assertEquals(emptyList(), filterComponents("không tồn tại", null))
        assertEquals(29, PreviewCatalog.components.size)
    }

    @Test
    fun generated_density_aliases_and_motion_are_resolved_by_role() {
        assertEquals("0.2.0", CantosTokens.version)
        assertEquals(CantosTokens.names.size, CantosTokens.names.distinct().size)
        assertEquals(44.0, CantosTokens.number("size.control", "compact"))
        assertEquals(48.0, CantosTokens.number("size.control", "comfortable"))
        assertEquals(12.0, CantosTokens.number("space.controlInline", "compact"))
        assertEquals(16.0, CantosTokens.number("space.controlInline", "comfortable"))
        assertEquals(CantosTokens.number("shape.reading"), CantosTokens.number("shape.field"))
        listOf("motion.feedback.duration", "motion.navigation.duration", "motion.emphasis.duration").forEach { role ->
            assertTrue(CantosTokens.number(role) > 0)
            assertEquals(0.0, CantosTokens.number(role, reducedMotion = true))
        }
        assertEquals(8.0, CantosTokens.number("space.sm", reducedMotion = true))
        assertTrue(CantosTokens.text("type.family.ui").contains("Noto Sans"))
        assertTrue(CantosTokens.text("type.family.editorial").contains("Noto Serif"))
    }

    @Test
    fun token_lookup_fails_on_unknown_roles_and_invalid_modes() {
        assertFailsWith<IllegalArgumentException> { CantosTokens.raw("missing") }
        assertFailsWith<IllegalArgumentException> { CantosTokens.raw("space.sm", theme = "system") }
        assertFailsWith<IllegalArgumentException> { CantosTokens.raw("space.sm", density = "tiny") }
        assertFailsWith<IllegalArgumentException> { CantosTokens.color("space.sm", "light") }
        assertFailsWith<IllegalArgumentException> { CantosTokens.number("color.primary") }
        CantosTokens.names.filter { it.startsWith("color.") }.forEach { role ->
            listOf("light", "dark").forEach { theme ->
                assertTrue(CantosTokens.color(role, theme).matches(Regex("#[0-9a-fA-F]{6}")))
            }
        }
    }

    @Test
    fun every_official_icon_has_visible_vector_paths() {
        val names = listOf("add", "check", "close", "dark_mode", "expand_more", "light_mode", "menu_book", "search", "warning_amber")
        assertEquals(names, CantosIcons.icons.map { it.name })
        CantosIcons.icons.forEach { icon ->
            assertEquals("0 0 24 24", icon.viewBox)
            assertTrue(icon.pathData.isNotEmpty())
            assertTrue(icon.pathData.all { it.isNotBlank() })
            assertEquals(icon.pathData, CantosIcons.paths(icon.name))
        }
        assertFailsWith<IllegalArgumentException> { CantosIcons.paths("not-an-icon") }
    }
}
