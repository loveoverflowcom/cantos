package com.cantos.ui.preview

import android.content.res.Configuration
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import com.cantos.ui.Density
import com.cantos.ui.PreviewExample
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import com.cantos.ui.UiRoot
import com.cantos.ui.tokenDp

/** Android Studio layout-renderer specimens of the actual common UI and preview fixtures.
 * These functions define static display inputs, never a product app or a device interaction.
 * The annotation supplies the platform font scale; UiPreferences keeps its multiplier at 1x.
 */
@Composable
private fun AndroidSamplePage(
    theme: Theme,
    density: Density,
    content: @Composable () -> Unit,
) {
    UiRoot(preferences = UiPreferences(theme = theme, density = density)) {
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState())
                .padding(tokenDp("space.lg")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        ) {
            content()
        }
    }
}

@Preview(
    name = "Trường nhập · sáng · 372 dp · tiếng Việt",
    widthDp = 372, heightDp = 840, locale = "vi-rVN", fontScale = 1f,
    uiMode = Configuration.UI_MODE_NIGHT_NO,
)
@Composable
fun AndroidFieldsLightPreview() {
    AndroidSamplePage(Theme.Light, Density.Comfortable) {
        Text("Trường nhập · Người dẫn chuyện · Ỗ, Ữ")
        ComponentExample(PreviewExample.TextField, "selection", {}, Modifier)
        ComponentExample(PreviewExample.NumberField, "ime-action", {}, Modifier)
        ComponentExample(PreviewExample.TextArea, "error", {}, Modifier)
    }
}

@Preview(
    name = "Lựa chọn · tối · 372 dp · chữ 2×",
    widthDp = 372, heightDp = 1000, locale = "vi-rVN", fontScale = 2f,
    uiMode = Configuration.UI_MODE_NIGHT_YES,
)
@Composable
fun AndroidChoicesDarkLargePreview() {
    AndroidSamplePage(Theme.Dark, Density.Comfortable) {
        Text("Lựa chọn · Nguyễn Thị Minh Phương")
        ComponentExample(PreviewExample.Checkbox, "error", {}, Modifier)
        ComponentExample(PreviewExample.Select, "enabled", {}, Modifier)
        ComponentExample(PreviewExample.RangeField, "stepped", {}, Modifier)
    }
}

@Preview(
    name = "Điều hướng · sáng · 840 dp · tiếng Việt",
    widthDp = 840, heightDp = 840, locale = "vi-rVN", fontScale = 1f,
    uiMode = Configuration.UI_MODE_NIGHT_NO,
)
@Composable
fun AndroidNavigationLightWidePreview() {
    AndroidSamplePage(Theme.Light, Density.Compact) {
        Text("Điều hướng · Ánh đèn cuối sân khấu")
        ComponentExample(PreviewExample.NavTabs, "overflow", {}, Modifier)
        ComponentExample(PreviewExample.Button, "composed-content", {}, Modifier)
        ComponentExample(PreviewExample.ErrorState, "recovery-slot", {}, Modifier)
    }
}

@Preview(
    name = "Theme lồng nhau · tối · 840 dp · chữ 2×",
    widthDp = 840, heightDp = 840, locale = "vi-rVN", fontScale = 2f,
    uiMode = Configuration.UI_MODE_NIGHT_YES,
)
@Composable
fun AndroidNestedThemeDarkLargePreview() {
    AndroidSamplePage(Theme.Dark, Density.Comfortable) {
        Text("Theme lồng nhau · tiếng Việt giữ nguyên dấu")
        ComponentExample(PreviewExample.UiRoot, "nested-theme", {}, Modifier)
        ComponentExample(PreviewExample.Surface, "reading", {}, Modifier)
    }
}
