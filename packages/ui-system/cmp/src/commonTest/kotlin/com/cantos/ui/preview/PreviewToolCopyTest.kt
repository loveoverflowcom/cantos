package com.cantos.ui.preview

import com.cantos.ui.preview.workbench.ComponentSampleState
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewMode
import com.cantos.ui.preview.workbench.PreviewRecipe
import com.cantos.ui.preview.workbench.PreviewRecipeResult
import com.cantos.ui.preview.workbench.PreviewSettings
import androidx.compose.ui.text.intl.LocaleList
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

class PreviewToolCopyTest {
    @Test
    fun tool_language_is_explicit_and_fixture_locale_only_selects_specimen_content() {
        assertEquals("en", PreviewToolCopy.LanguageTag)
        assertEquals("Preview sections", PreviewToolCopy.SectionsName)
        assertEquals("No results", PreviewToolCopy.NoResults)
        val vi = (PreviewRecipe.of("text-field", "editable", PreviewMode.Isolated,
            settings = PreviewSettings(locale = PreviewLocale.ViVn)) as PreviewRecipeResult.Accepted).recipe
        val en = (PreviewRecipe.of("text-field", "editable", PreviewMode.Isolated,
            settings = PreviewSettings(locale = PreviewLocale.En)) as PreviewRecipeResult.Accepted).recipe
        assertNotEquals(ComponentSampleState.initial(vi).title, ComponentSampleState.initial(en).title)
        assertEquals("Find component", PreviewToolCopy.FindComponent)
    }

    @Test
    fun vietnamese_typography_specimen_retains_its_literal_text_and_language_annotation() {
        val source = "Người dẫn chuyện: Ngày mai, mình có diễn tiếp không? · Ỗ, Ữ"
        val actual = vietnameseSpecimenText(source)
        assertEquals(source, actual.text)
        assertEquals(LocaleList("vi-VN"), actual.spanStyles.single().item.localeList)
    }

    @Test
    fun developer_tool_parameters_remain_literal_when_they_contain_braces() {
        assertEquals("Open Lời hẹn {time} in Components", PreviewToolCopy.openInComponents("Lời hẹn {time}"))
    }
}
