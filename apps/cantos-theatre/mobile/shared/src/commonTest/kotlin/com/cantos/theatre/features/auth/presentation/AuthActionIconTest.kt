package com.cantos.theatre.features.auth.presentation

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import com.cantos.ui.generated.CantosIcons

class AuthActionIconTest {
    @Test
    fun action_icons_describe_intents_without_claiming_verified_or_disclosure_state() {
        val expected = mapOf(
            AuthIntent.BeginLogin to "login", AuthIntent.OpenLogin to "login",
            AuthIntent.OpenRegister to "person_add", AuthIntent.CancelLogin to "close",
            AuthIntent.RestoreSession to "refresh", AuthIntent.RetryCleanup to "refresh",
            AuthIntent.OpenHome to "home", AuthIntent.Logout to "logout", AuthIntent.Back to "arrow_back",
            AuthIntent.OpenLibrary to "arrow_forward", AuthIntent.OpenMembership to "arrow_forward",
            AuthIntent.OpenSettings to "settings", AuthIntent.ToggleTheme to null,
        )
        expected.forEach { (intent, glyph) ->
            assertEquals(glyph, authActionIcon(intent), "$intent")
            if (glyph != null) assertTrue(CantosIcons.paths(glyph).isNotEmpty(), "$intent uses a registered glyph")
        }
    }
}
