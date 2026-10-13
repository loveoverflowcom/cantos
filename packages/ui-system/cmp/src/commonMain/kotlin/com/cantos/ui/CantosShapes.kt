package com.cantos.ui

import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable

/** Semantic shapes from the one shared token source; Material slots are mapped in CantosTheme.
 * Full is reserved for circular icon controls and actual pills (chips, badges, indicators).
 */
object CantosShapes {
    val detail: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.detail"))
    val control: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.control"))
    val reading: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.reading"))
    val container: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.container"))
    val overlay: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.overlay"))
    val thumbnail: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.thumbnail"))
    // CMP maps the shared shape.pill/Full intent to proportional corners rather than a dp sentinel.
    val full: RoundedCornerShape get() = CircleShape
}
