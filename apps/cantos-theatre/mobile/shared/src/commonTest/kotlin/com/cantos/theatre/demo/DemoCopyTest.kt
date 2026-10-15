package com.cantos.theatre.demo

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class DemoCopyTest {
    @Test
    fun every_copy_key_has_nonblank_vietnamese_and_english_resources() {
        CopyKey.entries.forEach { key ->
            assertTrue(key.vi.isNotBlank(), "${key.name} is blank in Vietnamese")
            assertTrue(key.en.isNotBlank(), "${key.name} is blank in English")
            DemoLocale.entries.forEach { locale ->
                assertTrue(DemoCopy(locale).text(key).isNotBlank(), "${key.name} is blank for $locale")
            }
        }
    }

    @Test
    fun both_locales_have_the_same_placeholder_names_and_multiplicity_for_every_key() {
        CopyKey.entries.forEach { key ->
            assertEquals(
                placeholderCounts(key.vi),
                placeholderCounts(key.en),
                "${key.name} has different Vietnamese and English placeholders",
            )
        }
    }

    @Test
    fun named_parameters_replace_every_placeholder_occurrence_in_both_locales() {
        DemoLocale.entries.forEach { locale ->
            val copy = DemoCopy(locale)
            CopyKey.entries.forEach { key ->
                val template = if (locale == DemoLocale.Vietnamese) key.vi else key.en
                val occurrences = placeholderCounts(template)
                val parameters = occurrences.keys.map { name -> name to "<$name-value>" }
                val rendered = copy.text(key, *parameters.toTypedArray())
                assertFalse(rendered.contains('{'), "${key.name} has an unresolved opening brace for $locale")
                assertFalse(rendered.contains('}'), "${key.name} has an unresolved closing brace for $locale")
                parameters.forEach { (name, replacement) ->
                    assertEquals(
                        occurrences.getValue(name),
                        Regex(Regex.escape(replacement)).findAll(rendered).count(),
                        "${key.name} did not substitute every $name occurrence for $locale",
                    )
                }
            }
        }
    }

    @Test
    fun supplied_titles_containing_placeholder_syntax_are_preserved_literally() {
        val title = "Lời hẹn {time}"
        assertEquals(
            "Mở dấu trang Lời hẹn {time}, 12:48",
            DemoCopy(DemoLocale.Vietnamese).text(CopyKey.OpenBookmark, "title" to title, "time" to "12:48"),
        )
        assertEquals(
            "Open bookmark for Lời hẹn {time}, 12:48",
            DemoCopy(DemoLocale.English).text(CopyKey.OpenBookmark, "title" to title, "time" to "12:48"),
        )
    }

    @Test
    fun locale_changes_labels_without_translating_the_vietnamese_episode_title() {
        val title = "Người giữ tiếng chuông"
        val vietnamese = DemoCopy(DemoLocale.Vietnamese)
        val english = DemoCopy(DemoLocale.English)
        assertEquals("Khám phá", vietnamese.text(CopyKey.Discover))
        assertEquals("Discover", english.text(CopyKey.Discover))
        assertEquals("Của tôi", vietnamese.text(CopyKey.Library))
        assertEquals("My library", english.text(CopyKey.Library))
        assertEquals("Nghe tiếp", vietnamese.text(CopyKey.Continuing))
        assertEquals("Lịch sử", vietnamese.text(CopyKey.History))
        assertEquals("Tài khoản", vietnamese.text(CopyKey.Account))
        assertEquals("Không gian của bạn", vietnamese.text(CopyKey.AccountSpace))
        assertEquals("Your space", english.text(CopyKey.AccountSpace))
        assertEquals("Đăng nhập", vietnamese.text(CopyKey.SignIn))
        assertEquals("Tạo tài khoản", vietnamese.text(CopyKey.CreateAccount))
        assertEquals(
            "Tập 3 · Người giữ tiếng chuông",
            vietnamese.text(CopyKey.EpisodeLabel, "index" to "3", "title" to title),
        )
        assertEquals(
            "Episode 3 · Người giữ tiếng chuông",
            english.text(CopyKey.EpisodeLabel, "index" to "3", "title" to title),
        )
        assertEquals(
            "Mở dấu trang Người giữ tiếng chuông, 12:48",
            vietnamese.text(CopyKey.OpenBookmark, "title" to title, "time" to "12:48"),
        )
        assertEquals(
            "Open bookmark for Người giữ tiếng chuông, 12:48",
            english.text(CopyKey.OpenBookmark, "title" to title, "time" to "12:48"),
        )
    }

    @Test
    fun listener_brand_is_cantos_and_no_copy_exposes_a_technical_or_creator_product_name() {
        val forbiddenNames = listOf("Cantos Theatre", "cantos-theatre", "Cantos Studio")
        DemoLocale.entries.forEach { locale ->
            val copy = DemoCopy(locale)
            assertEquals("Cantos", copy.text(CopyKey.Brand))
            CopyKey.entries.forEach { key ->
                forbiddenNames.forEach { forbidden ->
                    assertFalse(
                        copy.text(key).contains(forbidden, ignoreCase = true),
                        "${key.name} exposes $forbidden to listeners for $locale",
                    )
                }
            }
        }
    }

    @Test
    fun supported_speed_labels_use_vietnamese_commas_english_dots_and_exact_trailing_digits() {
        val expected = listOf(
            Triple(75, "0,75×", "0.75×"),
            Triple(100, "1×", "1×"),
            Triple(125, "1,25×", "1.25×"),
            Triple(150, "1,5×", "1.5×"),
            Triple(200, "2×", "2×"),
        )
        val vietnamese = DemoCopy(DemoLocale.Vietnamese)
        val english = DemoCopy(DemoLocale.English)
        expected.forEach { (percent, vi, en) ->
            assertEquals(vi, vietnamese.speed(percent), "Vietnamese speed for $percent percent")
            assertEquals(en, english.speed(percent), "English speed for $percent percent")
        }
    }

    @Test
    fun clock_positions_cover_zero_subminute_minute_boundary_and_reference_positions() {
        val expected = listOf(
            0 to "0:00",
            59 to "0:59",
            60 to "1:00",
            768 to "12:48",
            1722 to "28:42",
        )
        expected.forEach { (seconds, formatted) ->
            assertEquals(formatted, clockTime(seconds), "Clock position for $seconds seconds")
        }
    }

    @Test
    fun spoken_positions_use_localized_duration_words_for_the_reference_time_boundaries() {
        val expected = listOf(
            Triple(0, "0 phút 0 giây", "0 minutes 0 seconds"),
            Triple(1, "0 phút 1 giây", "0 minutes 1 second"),
            Triple(59, "0 phút 59 giây", "0 minutes 59 seconds"),
            Triple(60, "1 phút 0 giây", "1 minute 0 seconds"),
            Triple(61, "1 phút 1 giây", "1 minute 1 second"),
            Triple(768, "12 phút 48 giây", "12 minutes 48 seconds"),
            Triple(1722, "28 phút 42 giây", "28 minutes 42 seconds"),
        )
        val vietnamese = DemoCopy(DemoLocale.Vietnamese)
        val english = DemoCopy(DemoLocale.English)
        expected.forEach { (seconds, vi, en) ->
            assertEquals(vi, vietnamese.spokenTime(seconds), "Vietnamese duration for $seconds seconds")
            assertEquals(en, english.spokenTime(seconds), "English duration for $seconds seconds")
        }
    }

    private fun placeholderCounts(template: String): Map<String, Int> =
        Regex("\\{([^{}]+)\\}")
            .findAll(template)
            .map { match -> match.groupValues[1] }
            .groupingBy { name -> name }
            .eachCount()
}
