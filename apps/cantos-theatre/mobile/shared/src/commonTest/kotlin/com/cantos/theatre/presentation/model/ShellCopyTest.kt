package com.cantos.theatre.presentation.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ShellCopyTest {
    @Test
    fun every_shell_resource_has_nonblank_locales_and_identical_named_parameter_multiplicity() {
        val placeholder = Regex("\\{([^{}]+)\\}")
        fun parameters(value: String) = placeholder.findAll(value).map { it.groupValues[1] }.groupingBy { it }.eachCount()
        CopyKey.entries.forEach { key ->
            assertTrue(key.vi.isNotBlank(), "${key.name}: vi-VN")
            assertTrue(key.en.isNotBlank(), "${key.name}: en")
            assertEquals(parameters(key.vi), parameters(key.en), key.name)
            key.enOne?.let { one ->
                assertTrue(one.isNotBlank(), "${key.name}: en one")
                assertEquals(parameters(key.en), parameters(one), key.name)
            }
        }
    }

    @Test
    fun episode_quantities_use_english_one_other_and_vietnamese_other() {
        val english = ShellCopy(ShellLocale.English)
        val vietnamese = ShellCopy(ShellLocale.Vietnamese)
        listOf(
            0 to "0 episodes · Vietnamese",
            1 to "1 episode · Vietnamese",
            2 to "2 episodes · Vietnamese",
        ).forEach { (quantity, episodes) ->
            assertEquals(episodes, english.text(CopyKey.EpisodesCount, "count" to quantity.toString()))
            assertEquals("$quantity tập · Tiếng Việt", vietnamese.text(CopyKey.EpisodesCount, "count" to quantity.toString()))
        }
    }

    @Test
    fun work_quantities_use_english_one_other_and_vietnamese_other() {
        val english = ShellCopy(ShellLocale.English)
        val vietnamese = ShellCopy(ShellLocale.Vietnamese)
        listOf(0 to "0 works", 1 to "1 work", 2 to "2 works").forEach { (quantity, works) ->
            assertEquals(works, english.text(CopyKey.Results, "count" to quantity.toString()))
            assertEquals("$quantity tác phẩm", vietnamese.text(CopyKey.Results, "count" to quantity.toString()))
        }
    }

    @Test
    fun duration_quantities_use_english_one_other_and_vietnamese_other() {
        val english = ShellCopy(ShellLocale.English)
        val vietnamese = ShellCopy(ShellLocale.Vietnamese)
        listOf(0 to "0 minutes", 1 to "1 minute", 2 to "2 minutes").forEach { (quantity, duration) ->
            assertEquals(duration, english.text(CopyKey.Duration, "minutes" to quantity.toString()))
            assertEquals("$quantity phút", vietnamese.text(CopyKey.Duration, "minutes" to quantity.toString()))
        }
    }

    @Test
    fun visible_decimal_fraction_does_not_select_the_english_one_category() {
        assertEquals("1.0 episodes · Vietnamese", ShellCopy(ShellLocale.English).text(CopyKey.EpisodesCount, "count" to "1.0"))
        assertEquals("1.0 minutes", ShellCopy(ShellLocale.English).text(CopyKey.Duration, "minutes" to "1.0"))
    }

    @Test
    fun interpolation_reads_only_the_original_template_even_when_parameters_repeat_or_reorder() {
        val overrides = mapOf(CopyKey.OpenBookmark to ShellText("{title} · {time} · {title}", "{title} · {time} · {title}"))
        val title = "Người giữ {time} và {title} — $1 \\"
        ShellLocale.entries.forEach { locale ->
            val copy = ShellCopy(locale, overrides)
            val expected = "$title · 1:01 · $title"
            assertEquals(expected, copy.text(CopyKey.OpenBookmark, "title" to title, "time" to "1:01"))
            assertEquals(expected, copy.text(CopyKey.OpenBookmark, "time" to "1:01", "title" to title))
        }
    }

    @Test
    fun overrides_are_snapshotted_and_another_locale_keeps_source_content_literal() {
        val overrides = mutableMapOf(CopyKey.Details to ShellText("Mở {title}", "Open {title}"))
        val vietnamese = ShellCopy(ShellLocale.Vietnamese, overrides)
        val english = ShellCopy(ShellLocale.English, overrides)
        overrides[CopyKey.Details] = ShellText("Đã đổi", "Changed")
        val title = "Ánh đèn cuối sân khấu {time}"
        assertEquals("Mở $title", vietnamese.text(CopyKey.Details, "title" to title))
        assertEquals("Open $title", english.text(CopyKey.Details, "title" to title))
    }

    @Test
    fun quantity_override_without_one_form_retains_its_own_wording() {
        val overrides = mapOf(CopyKey.Results to ShellText("Tìm thấy: {count}", "Found: {count}"))
        val copy = ShellCopy(ShellLocale.English, overrides)
        assertEquals("Found: 1", copy.text(CopyKey.Results, "count" to "1"))
        assertEquals("Found: 2", copy.text(CopyKey.Results, "count" to "2"))
    }
}
