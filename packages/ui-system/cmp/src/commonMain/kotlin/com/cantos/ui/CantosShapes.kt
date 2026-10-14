package com.cantos.ui

import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable

/** Semantic shapes from the one shared token source; Material slots are mapped in CantosTheme.
 * Full is reserved for circular controls, actual pills and scoped auth action capsules.
 * Regular listening/editor controls keep their existing geometry.
 */
object CantosShapes {
    val detail: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.detail"))
    val control: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.control"))
    val reading: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.reading"))
    val container: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.container"))
    val overlay: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.overlay"))
    val thumbnail: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.thumbnail"))
    val authField: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.authField"))
    val auth: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.authContainer"))
    val profile: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.profileContainer"))
    val membership: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.membershipContainer"))
    val accountGroup: RoundedCornerShape @Composable get() = RoundedCornerShape(tokenDp("shape.accountGroup"))
    // CMP maps the shared shape.pill/Full intent to proportional corners rather than a dp sentinel.
    val full: RoundedCornerShape get() = CircleShape
}
