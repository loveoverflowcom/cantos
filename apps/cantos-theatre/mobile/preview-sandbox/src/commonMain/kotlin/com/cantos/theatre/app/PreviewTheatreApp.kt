package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewWorkIds
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import com.cantos.theatre.app.CantosTheatreApp
import com.cantos.theatre.features.membership.presentation.LocalMembershipDiagnostics
import com.cantos.theatre.features.membership.presentation.LocalMembershipIntroArtwork
import com.cantos.theatre.features.shell.presentation.CoverImageSource
import com.cantos.theatre.features.shell.presentation.LocalWorkArtwork
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.Density
import com.cantos.ui.MotionPreference
import com.cantos.ui.tokenDp

/**
 * The Desktop Preview of the one Cantos Theatre application. It mounts the shared
 * [CantosTheatreApp] over [owners]' [com.cantos.theatre.app.TheatreShell]: the same dispatch,
 * projection, navigation and presentation as the Android and iOS hosts. Only the sample artwork
 * and the Membership fixture controls are supplied here, as optional slots the shared screens
 * already define; scenario controls (Fresh, Restored, reset, failure injection) live in the
 * workbench, never in product navigation.
 */
@Composable
fun PreviewTheatreApp(
    owners: PreviewOwners,
    modifier: Modifier = Modifier,
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    density: Density = Density.Comfortable,
    motionPreference: MotionPreference = MotionPreference.Reduced,
    /** Deterministic cover pipeline for fixtures; none keeps every cover on its fallback. */
    coverImages: CoverImageSource? = null,
) {
    WithPreviewArtwork {
        CompositionLocalProvider(
            LocalMembershipIntroArtwork provides { artworkModifier ->
                LocalWorkArtwork.current?.invoke(
                    PreviewWorkIds.SonHa, artworkModifier, RoundedCornerShape(tokenDp("shape.membershipContainer")),
                )
            },
            LocalMembershipDiagnostics provides { state, locale, dispatch, plansLabel ->
                MembershipFixtureControls(state, locale, dispatch, plansLabel)
            },
        ) {
            CantosTheatreApp(
                shell = owners.shell,
                modifier = modifier,
                textScale = textScale,
                onThemeChanged = onThemeChanged,
                backHandler = backHandler,
                density = density,
                motionPreference = motionPreference,
                coverImages = coverImages,
            )
        }
    }
}

/** Manual outcomes for the Membership fixture; wording belongs to the sandbox, labels to the shared screen. */
internal enum class MembershipFixtureText(val vi: String, val en: String) {
    FixtureTools("Công cụ fixture · Chọn kết quả thủ công", "Fixture tools · Choose an outcome manually"),
    ConfirmFixture("Fixture: xác nhận đã nhận đăng ký", "Fixture: confirm receipt"),
    FailFixture("Fixture: lỗi chưa nhận đăng ký", "Fixture: request failed"),
    TimeoutFixture("Fixture: timeout, kết quả chưa biết", "Fixture: timeout, outcome unknown"),
    AccountFixtureAction("Fixture: đăng ký tài khoản hoàn tất", "Fixture: account registration complete"),
    ClearSession("Fixture: đăng xuất / đổi tài khoản / hết phiên", "Fixture: logout / switch account / session expired");

    fun localized(locale: ShellLocale): String = if (locale == ShellLocale.English) en else vi
}
