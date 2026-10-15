package com.cantos.ui.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.cantos.ui.AccessibleName
import com.cantos.ui.CantosTheme
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.LogoVariant
import com.cantos.ui.Space
import com.cantos.ui.Stack
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.Theme

/** Real specimen for every registered state. Backgrounds belong to the preview, not the mark. */
@Composable
internal fun LogoSamples(state: String) {
    val variant = when {
        state.startsWith("studio-") || state == "aspect-fit" -> LogoVariant.CantosStudio
        state.startsWith("symbol-") || state in listOf("named", "decorative", "transparent-knockout") -> LogoVariant.Symbol
        else -> LogoVariant.Cantos
    }
    val tone = when {
        state.endsWith("-dark") -> LogoTone.Dark
        state.endsWith("-mono") || state == "transparent-knockout" -> LogoTone.Mono
        else -> LogoTone.Light
    }
    val theme = if (tone == LogoTone.Dark) Theme.Dark else Theme.Light
    CantosTheme(LocalUiPreferences.current.copy(theme = theme)) {
        Surface(tone = if (tone == LogoTone.Mono) SurfaceTone.PrimaryContainer else SurfaceTone.Surface) {
            Stack(gap = Space.Md) {
                Text("Logo chính thức · $state · không có font hoặc ảnh mạng")
                if (state == "aspect-fit") {
                    Box(Modifier.size(160.dp)) { Logo(variant, modifier = Modifier.size(160.dp), tone = tone) }
                    Text("Khung vuông giữ nguyên tỷ lệ của lockup, không kéo giãn chữ.")
                } else {
                    Logo(variant, tone = tone, modifier = Modifier.width(if (variant == LogoVariant.Symbol) 64.dp else 283.dp),
                        accessibleName = if (state == "named") AccessibleName.of("Biểu tượng Cantos") else null)
                }
                if (state == "decorative") Text("Cantos · tên liền kề, logo không đọc lặp lại")
                if (state == "transparent-knockout") Text("Khoảng âm để lộ surface thật, không tô nền giấy vào biểu tượng.")
            }
        }
    }
}
