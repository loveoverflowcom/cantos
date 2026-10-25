package com.cantos.ui

import com.cantos.ui.generated.CantosTokens
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Vectors mirror packages/ui-system/shared/README.md#avatar and the Leptos avatar tests. */
class AvatarContractTest {
    @Test
    fun single_initial_follows_google_and_preserves_vietnamese() {
        assertEquals("N", avatarInitials("  Nguyễn Thị Ánh  "))
        assertEquals("Đ", avatarInitials("  __Đỗ Văn An"))
        assertEquals("N", avatarInitials("Người dẫn chuyện"))
        assertEquals("岑", avatarInitials("岑 昕"))
        assertEquals("6", avatarInitials("6 Nguyễn"))
        assertEquals("C", avatarInitials("@cantos.studio"))
        // Uppercasing may expand one character; one avatar slot keeps the first.
        assertEquals("S", avatarInitials("ß"))
    }

    @Test
    fun decomposed_vietnamese_keeps_its_combining_marks() {
        assertEquals("Á", avatarInitials("ánh  Dương"))
        assertEquals("Á", avatarInitials("ánh Dương"))
        assertEquals("ÁD", avatarInitials("Ánh Dương", AvatarInitials.Pair))
    }

    @Test
    fun no_letter_gives_a_plain_avatar_instead_of_an_error_glyph() {
        assertEquals("", avatarInitials("  😀 \n \t "))
        assertEquals("", avatarInitials(""))
        assertEquals("", avatarInitials("", AvatarInitials.Pair))
        // Supplementary-plane characters are skipped identically on both renderers.
        assertEquals("L", avatarInitials("𠀀 Lan"))
    }

    @Test
    fun pair_mode_uses_first_and_last_named_words() {
        assertEquals("NÁ", avatarInitials("Nguyễn Thị Ánh", AvatarInitials.Pair))
        assertEquals("ĐA", avatarInitials("__Đỗ Văn An", AvatarInitials.Pair))
        assertEquals("NC", avatarInitials("Người dẫn chuyện", AvatarInitials.Pair))
        assertEquals("岑昕", avatarInitials("岑 昕", AvatarInitials.Pair))
        assertEquals("6N", avatarInitials("6 Nguyễn", AvatarInitials.Pair))
        assertEquals("N", avatarInitials("Nguyễn", AvatarInitials.Pair))
    }

    @Test
    fun tones_are_fnv1a_vectors_shared_with_leptos() {
        assertEquals(AvatarTone.Rose, avatarTone("user-42"))
        assertEquals(AvatarTone.Sky, avatarTone("user-1"))
        assertEquals(AvatarTone.Sage, avatarTone("alice@example.com"))
        assertEquals(AvatarTone.Gold, avatarTone(""))
        assertEquals(AvatarTone.Gold, avatarTone("creator-7"))
        assertEquals(AvatarTone.Lavender, avatarTone("Nguyễn Thị Ánh"))
        assertEquals(AvatarTone.Lavender, avatarTone("Người dẫn chuyện"))
    }

    @Test
    fun stable_id_wins_and_name_seed_ignores_whitespace_noise() {
        assertEquals("user-42", avatarSeed("Nguyễn Thị Ánh", " user-42 "))
        assertEquals("Nguyễn Thị Ánh", avatarSeed("  Nguyễn   Thị Ánh ", "  "))
        assertEquals("Nguyễn Thị Ánh", avatarSeed("Nguyễn\tThị\nÁnh"))
    }

    @Test
    fun every_tone_and_size_resolves_a_shared_token_in_both_themes() {
        for (tone in AvatarTone.entries) for (theme in listOf("light", "dark")) {
            assertTrue(CantosTokens.color("color.avatar${tone.name}", theme).startsWith("#"))
            assertTrue(CantosTokens.color("color.onAvatar${tone.name}", theme).startsWith("#"))
        }
        assertEquals(listOf(32.0, 40.0, 48.0, 64.0), AvatarSize.entries.map { CantosTokens.number(it.token).toDouble() })
    }
}
