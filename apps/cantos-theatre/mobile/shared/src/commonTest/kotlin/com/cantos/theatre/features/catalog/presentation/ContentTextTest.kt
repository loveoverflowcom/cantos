package com.cantos.theatre.features.catalog.presentation

import androidx.compose.ui.text.intl.LocaleList
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ContentTextTest {
    @Test
    fun source_text_and_its_language_are_preserved_without_expanding_title_parameters() {
        val source = "Lời hẹn {time} · Ỗ, Ữ"
        val actual = contentText(source, "vi-VN")
        assertEquals(source, actual.text)
        assertEquals(1, actual.spanStyles.size)
        val annotation = actual.spanStyles.single()
        assertEquals(0, annotation.start)
        assertEquals(source.length, annotation.end)
        assertEquals(LocaleList("vi-VN"), annotation.item.localeList)
    }

    @Test
    fun absent_source_language_never_invents_the_interface_or_system_language() {
        assertTrue(contentText("Lời hẹn", "").spanStyles.isEmpty())
        assertTrue(contentText("", "vi-VN").spanStyles.isEmpty())
    }
}
