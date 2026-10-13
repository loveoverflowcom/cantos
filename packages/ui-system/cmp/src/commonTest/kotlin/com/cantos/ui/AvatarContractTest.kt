package com.cantos.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class AvatarContractTest {
    @Test
    fun google_style_single_initial_preserves_vietnamese_and_has_explicit_empty_fallback() {
        assertEquals("N", avatarInitials("  Nguyễn Thị Ánh  "))
        assertEquals("Đ", avatarInitials("  __Đỗ Văn An"))
        assertEquals("岑", avatarInitials("岑 昕"))
        assertEquals("6", avatarInitials("6 Nguyễn"))
        assertEquals("?", avatarInitials("  😀 \n \t "))
        // Uppercase may expand one Unicode character; constrain a single avatar slot.
        assertEquals("S", avatarInitials("ß"))
    }

    @Test
    fun pair_mode_uses_first_and_last_meaningful_words() {
        assertEquals("NÁ", avatarInitials("Nguyễn Thị Ánh", AvatarInitialMode.Pair))
        assertEquals("ĐA", avatarInitials("__Đỗ Văn An", AvatarInitialMode.Pair))
        assertEquals("岑昕", avatarInitials("岑 昕", AvatarInitialMode.Pair))
        assertEquals("N", avatarInitials("Nguyễn", AvatarInitialMode.Pair))
        assertEquals("?", avatarInitials("", AvatarInitialMode.Pair))
    }

    @Test
    fun colors_are_stable_across_platforms_and_unaffected_by_randomized_hashing() {
        assertEquals(AvatarTone.Quiet, avatarTone("user-42"))
        assertEquals(AvatarTone.Soft, avatarTone("user-1"))
        assertEquals(AvatarTone.Elevated, avatarTone("alice@example.com"))
        assertEquals(AvatarTone.Strong, avatarTone(""))
        assertEquals(avatarTone("user-42"), avatarTone(" user-42 "))
        assertEquals(AvatarTone.Soft, avatarTone("Nguyễn Thị Ánh"))
    }
}
