package com.cantos.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle

/** Scoped design-v2 type roles; regular listening and editor typography stays independent. */
object CantosTypography {
    val authHeading: TextStyle @Composable get() = typeStyle("authHeading", editorial = true, fontFamily = LocalEditorialFontFamily.current, expression = LocalUiPreferences.current.expression)
    val membershipTitle: TextStyle @Composable get() = typeStyle("membershipTitle", editorial = true, fontFamily = LocalEditorialFontFamily.current, expression = LocalUiPreferences.current.expression)
}
