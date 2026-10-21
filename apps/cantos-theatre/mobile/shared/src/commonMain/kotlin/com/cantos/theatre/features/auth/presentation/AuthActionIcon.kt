package com.cantos.theatre.features.auth.presentation

/** Semantic candidates from the canonical registry; only the explicit sandbox opts in.
 * Existing product rendering remains unchanged until both-renderer admission is complete.
 */
internal fun authActionIcon(intent: AuthIntent): String? = when (intent) {
    is AuthIntent.Login -> "login"
    AuthIntent.BeginLogin, AuthIntent.OpenLogin -> "login"
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
