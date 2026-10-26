package com.cantos.theatre.presentation.model

import com.cantos.theatre.presentation.model.generated.ShellMessage
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ShellCopyTest {
    @Test
    fun generated_plain_resources_are_complete_for_every_release_locale() {
        ShellLocale.entries.forEach { locale ->
            CopyKey.entries.forEach { key -> assertTrue(ShellCopy(locale).text(key).isNotBlank(), "$key/$locale") }
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
            assertEquals(episodes, english.text(LegacyCopyKey.EpisodesCount, "count" to quantity.toString()))
            assertEquals("$quantity tập · Tiếng Việt", vietnamese.text(LegacyCopyKey.EpisodesCount, "count" to quantity.toString()))
        }
    }

    @Test
    fun work_quantities_use_english_one_other_and_vietnamese_other() {
        val english = ShellCopy(ShellLocale.English)
        val vietnamese = ShellCopy(ShellLocale.Vietnamese)
        listOf(0 to "0 works", 1 to "1 work", 2 to "2 works").forEach { (quantity, works) ->
            assertEquals(works, english.text(LegacyCopyKey.Results, "count" to quantity.toString()))
            assertEquals("$quantity tác phẩm", vietnamese.text(LegacyCopyKey.Results, "count" to quantity.toString()))
        }
    }

    @Test
    fun duration_quantities_use_english_one_other_and_vietnamese_other() {
        val english = ShellCopy(ShellLocale.English)
        val vietnamese = ShellCopy(ShellLocale.Vietnamese)
        listOf(0 to "0 minutes", 1 to "1 minute", 2 to "2 minutes").forEach { (quantity, duration) ->
            assertEquals(duration, english.text(LegacyCopyKey.Duration, "minutes" to quantity.toString()))
            assertEquals("$quantity phút", vietnamese.text(LegacyCopyKey.Duration, "minutes" to quantity.toString()))
        }
    }

    @Test
    fun visible_decimal_fraction_does_not_select_the_english_one_category() {
        assertEquals("1.0 episodes · Vietnamese", ShellCopy(ShellLocale.English).text(LegacyCopyKey.EpisodesCount, "count" to "1.0"))
        assertEquals("1.0 minutes", ShellCopy(ShellLocale.English).text(LegacyCopyKey.Duration, "minutes" to "1.0"))
    }

    @Test
    fun typed_messages_keep_title_tokens_opaque_and_named_parameter_order_independent() {
        val title = "Người giữ {time} và {title} — $1 \\"
        assertEquals("Mở dấu trang $title, 1:01", ShellCopy(ShellLocale.Vietnamese).text(
            ShellMessage.OpenBookmark(time = "1:01", title = title),
        ))
        assertEquals("Open bookmark for $title, 1:01", ShellCopy(ShellLocale.English).text(
            ShellMessage.OpenBookmark(title = title, time = "1:01"),
        ))
    }

    @Test
    fun generated_live_copy_uses_explicit_variants_without_bilingual_override_maps() {
        assertEquals("Các câu chuyện đã nghe, gần nhất ở trên.", ShellCopy(ShellLocale.Vietnamese, live = true).text(CopyKey.HistoryHelp))
        assertEquals("Stories you have listened to, most recent first.", ShellCopy(ShellLocale.English, live = true).text(CopyKey.HistoryHelp))
        listOf(0, 1, 2).forEach { count ->
            assertEquals("$count tập", ShellCopy(ShellLocale.Vietnamese, live = true).text(LegacyCopyKey.EpisodesCount, "count" to count.toString()))
        }
    }
}
