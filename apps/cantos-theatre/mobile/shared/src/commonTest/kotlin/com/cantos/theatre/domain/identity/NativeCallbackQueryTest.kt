package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class NativeCallbackQueryTest {
    @Test
    fun canonical_names_preserve_platform_encoded_values() {
        for (query in listOf(
            "state=pending&code=code",
            "code=code&state=pending",
            "state=pending%2Bstate&code=code%26part%3Dvalue%2Ftail",
            "state=pending&error=access_denied&error_description=user+cancelled%20login",
            "state=pending&error=access_denied&error_description=",
        )) assertTrue(hasCanonicalNativeCallbackQuery(query), query)
    }

    @Test
    fun percent_encoded_name_aliases_are_refused_even_with_a_canonical_value() {
        for (query in listOf(
            "state=pending&%73tate=other&code=code",
            "%73tate=other&state=pending&code=code",
            "state=pending&%73tate=pending&code=code",
            "%73tate=pending&code=code",
            "state=pending&code=code&%63ode=other",
            "state=pending&%63ode=other&code=code",
            "state=pending&code=code&error=denied&%65rror=other",
            "state=pending&%65rror=other&error=denied",
            "state=pending&error=denied&error%5Fdescription=other",
        )) assertFalse(hasCanonicalNativeCallbackQuery(query), query)
    }

    @Test
    fun duplicate_unknown_and_malformed_fields_are_refused() {
        for (query in listOf(
            "state=pending&state=pending&code=code",
            "state=pending&code=code&code=other",
            "state=pending&error=denied&error=denied",
            "state=pending&error_description=one&error_description=two",
            "state=pending&code=code&scope=openid",
            "state=pending&code=code&State=other",
            "state=pending&code=code&=other",
            "state=pending&code=code&error",
            "state&code=code",
            "&state=pending&code=code",
            "state=pending&&code=code",
            "state=pending&code=code&",
        )) assertFalse(hasCanonicalNativeCallbackQuery(query), query)
    }

    @Test
    fun absent_state_empty_query_and_oversized_input_are_refused() {
        assertFalse(hasCanonicalNativeCallbackQuery(null))
        for (query in listOf("", "code=code", "error=access_denied", "error_description=cancelled")) {
            assertFalse(hasCanonicalNativeCallbackQuery(query), query)
        }
        assertFalse(hasCanonicalNativeCallbackQuery("state=pending&code=" + "a".repeat(16_384)))
    }
}
