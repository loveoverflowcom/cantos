package com.cantos.theatre.localization

import com.cantos.theatre.domain.preferences.UiLocale
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.app.toPreferenceLocale
import com.cantos.theatre.app.toShellLocale
import com.cantos.theatre.domain.preferences.savedUiLocale
import com.cantos.theatre.localization.fixture.ProbeKey
import com.cantos.theatre.localization.fixture.ProbeMessage
import com.cantos.theatre.localization.fixture.UiLocale as FixtureLocale
import com.cantos.theatre.presentation.model.generated.AuthKey
import com.cantos.theatre.presentation.model.generated.LaunchKey
import com.cantos.theatre.presentation.model.generated.MembershipKey
import com.cantos.theatre.presentation.model.generated.ShellKey
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class GeneratedResourcesTest {
    @Test
    fun release_registry_is_the_only_locale_and_autonym_source() {
        assertEquals(listOf("vi-VN", "en"), UiLocale.entries.map { it.tag })
        assertEquals(listOf("Tiếng Việt", "English"), UiLocale.entries.map { it.autonym })
        assertEquals(UiLocale.Vietnamese, UiLocale.Default)
        UiLocale.entries.forEach { assertEquals(it, it.toShellLocale().toPreferenceLocale()) }
        assertEquals(null, savedUiLocale("ja"))
        assertEquals(null, savedUiLocale("zh-Hant"))
        assertEquals(null, savedUiLocale("en-GB"))
    }

    @Test
    fun third_locale_text_is_generated_from_the_same_fixture_without_becoming_a_release_choice() {
        assertEquals(listOf("vi-VN", "en", "ja"), FixtureLocale.entries.map { it.tag })
        assertEquals("言語", ProbeKey.Language.text(FixtureLocale.Japanese))
        val title = "Ánh đèn {time} %1\$s <b> cuối sân khấu"
        val message = ProbeMessage.Bookmark(time = "1:01", title = title)
        assertEquals("Nghe $title tại 1:01", message.text(FixtureLocale.Vietnamese))
        assertEquals("Listen to $title at 1:01", message.text(FixtureLocale.English))
        assertEquals("1:01に${title}を聴く", message.text(FixtureLocale.Japanese))
        assertEquals("このコンテンツは再生できません。", ProbeKey.Unavailable.text(FixtureLocale.Japanese))
    }

    @Test
    fun every_generated_plain_namespace_has_nonblank_release_resources() {
        ShellLocale.entries.forEach { locale ->
            val values = ShellKey.entries.map { it.text(locale) } + AuthKey.entries.map { it.text(locale) } +
                MembershipKey.entries.map { it.text(locale) } + LaunchKey.entries.map { it.text(locale) }
            assertTrue(values.all { it.isNotBlank() })
        }
    }
}
