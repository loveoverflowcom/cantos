package com.cantos.theatre.screens

import kotlin.test.Test
import kotlin.test.assertEquals

class SearchTest {
    @Test
    fun local_demo_search_preserves_vietnamese_titles_and_distinguishes_no_results() {
        assertEquals(listOf("Đèn khuya bên sông"), matchingTitles("  ĐÈN  "))
        assertEquals(emptyList(), matchingTitles("không tồn tại"))
        assertEquals(3, matchingTitles("").size)
    }
}
