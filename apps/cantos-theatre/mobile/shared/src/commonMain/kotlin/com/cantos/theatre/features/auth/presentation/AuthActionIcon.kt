package com.cantos.theatre.features.auth.presentation

/** Admitted semantic action mapping. Labels and navigation remain owned by the caller. */
internal fun authActionIcon(intent: AuthIntent): String? = when (intent) {
    is AuthIntent.Login, AuthIntent.OpenLogin -> "login"
    AuthIntent.OpenRegister -> "person_add"
    AuthIntent.CancelLogin -> "close"
    AuthIntent.RestoreSession, AuthIntent.RetryCleanup -> "refresh"
    AuthIntent.OpenHome -> "home"
    AuthIntent.Logout -> "logout"
    AuthIntent.Back -> "arrow_back"
    AuthIntent.OpenLibrary, AuthIntent.OpenMembership -> "arrow_forward"
    AuthIntent.OpenSettings -> "settings"
    AuthIntent.ToggleTheme -> null // Existing theme glyphs retain their own Material owner.
}
