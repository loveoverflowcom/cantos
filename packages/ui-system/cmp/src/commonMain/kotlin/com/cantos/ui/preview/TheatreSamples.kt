package com.cantos.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import com.cantos.ui.AccessibleName
import com.cantos.ui.Avatar
import com.cantos.ui.AvatarSize
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosExpression
import com.cantos.ui.CantosShapes
import com.cantos.ui.CantosTheme
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldHelp
import com.cantos.ui.FieldState
import com.cantos.ui.FilterChip
import com.cantos.ui.IconButton
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.LogoVariant
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.PreviewExample
import com.cantos.ui.Select
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.Theme
import com.cantos.ui.parseTokenColor
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp
import com.cantos.ui.generated.CantosTokens

/** Registered Theatre-expression states all start with this prefix; the dispatch in PreviewApp.kt keys on it. */
internal const val TheatreStatePrefix = "theatre-"

/**
 * Theatre expression specimens (token v0.7.0): the production components under an explicit
 * expression scope, with local state only. Nothing here calls a service, stores a secret or
 * implies an app composition; the surrounding gallery keeps its own expression.
 */
@Composable
internal fun TheatreSpecimen(example: PreviewExample, state: String, onEvent: (String) -> Unit, modifier: Modifier) {
    val outer = LocalUiPreferences.current
    CantosTheme(outer.copy(expression = CantosExpression.Theatre)) {
        Column(
            modifier.fillMaxWidth().background(tokenColor("color.background"), CantosShapes.container).padding(tokenDp("space.md")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        ) {
            when (example) {
                PreviewExample.UiRoot -> ExpressionSpecimen()
                PreviewExample.Button -> GroupSpecimen(onEvent)
                PreviewExample.IconButton -> if (state == "theatre-docked") DockedSpecimen(onEvent) else TonalSpecimen(onEvent)
                PreviewExample.FilterChip -> SelectionSpecimen()
                PreviewExample.TextField -> when (state) {
                    "theatre-trailing-eye" -> EyeSpecimen()
                    "theatre-docked-search" -> SearchSpecimen(onEvent)
                    else -> FieldMatrixSpecimen()
                }
                PreviewExample.Select -> UtilitySelectSpecimen()
                PreviewExample.Avatar -> AvatarSpecimen()
                PreviewExample.Logo -> HeaderSpecimen(outer.theme, onEvent)
                else -> FieldHelp("No Theatre specimen is registered for ${example.id}/$state.")
            }
        }
    }
}

@Composable
private fun ExpressionSpecimen() {
    val hero = tokenColor("color.theatreHero")
    val ink = tokenColor("color.theatreOnHero")
    Column(
        Modifier.fillMaxWidth().background(hero, CantosShapes.container).padding(tokenDp("space.lg")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
    ) {
        Text(vietnameseSpecimenText("Mở một miền tưởng tượng."), color = ink, style = MaterialTheme.typography.headlineMedium)
        Text(vietnameseSpecimenText("Tiểu thuyết, truyện kể và podcast cho mỗi ngày."), color = ink, style = MaterialTheme.typography.bodyLarge)
    }
    FieldHelp(
        "Hero pair #1d0b28 on #c6a0f5 is two solid canonical roles; headings use weight " +
            "${CantosTokens.number("type.theatreHeading.weight").toInt()}.",
    )
}

@Composable
private fun GroupSpecimen(onEvent: (String) -> Unit) {
    val reason = DisabledReason.of("Đang gửi, vui lòng chờ.")
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Button("Đăng nhập", { onEvent("Button · Đăng nhập") }, icon = "login")
        Button("Tạo tài khoản", { onEvent("Button · Tạo tài khoản") }, variant = ButtonVariant.Tonal, icon = "person_add")
        Button("Xác nhận", {}, state = ButtonState.Loading(reason), icon = "check")
    }
    Button("Gửi liên kết xác minh cho Nguyễn Thị Ánh Nguyệt", { onEvent("Button · long label") }, icon = "mail", modifier = Modifier.fillMaxWidth())
}

@Composable
private fun TonalSpecimen(onEvent: (String) -> Unit) {
    var dark by remember { mutableStateOf(false) }
    IconButton(
        AccessibleName.of(if (dark) "Chuyển sang giao diện sáng" else "Chuyển sang giao diện tối"),
        if (dark) "light_mode" else "dark_mode",
        { dark = !dark; onEvent("Tonal icon button · ${if (dark) "dark" else "light"} requested") },
        variant = ButtonVariant.Tonal,
    )
    FieldHelp("48-unit tonal rounded square; the host owns the theme and the localized name.")
}

@Composable
private fun DockedSpecimen(onEvent: (String) -> Unit) {
    IconButton(
        AccessibleName.of("Tìm trong danh mục"), "search", { onEvent("Docked search action") },
        variant = ButtonVariant.Primary, circular = true, controlModifier = Modifier.size(tokenDp("size.searchAction")),
    )
    FieldHelp("52-unit visible circle, a 48-unit minimum touch target; one magnifier, no caption.")
}

@Composable
private fun SelectionSpecimen() {
    var all by remember { mutableStateOf(true) }
    var novel by remember { mutableStateOf(false) }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        FilterChip("Tất cả loại", all, { all = it })
        FilterChip("Tiểu thuyết", novel, { novel = it })
        FilterChip("Truyện huyền huyễn và lời kể dưới chân Thanh Vân Sơn", false, {},
            state = ButtonState.Disabled(DisabledReason.of("Chưa có nội dung cho mục này.")))
    }
}

@Composable
private fun FieldMatrixSpecimen() {
    var idle by remember { mutableStateOf("") }
    var filled by remember { mutableStateOf("ban@example.com") }
    var invalid by remember { mutableStateOf("ban@") }
    var long by remember { mutableStateOf("Giữ nguyên nội dung dài có dấu: Ỗ, Ữ") }
    val focus = remember { FocusRequester() }
    TextField("Email", idle, { idle = it }, variant = TextFieldVariant.Floating,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email))
    TextField("Email", filled, { filled = it }, variant = TextFieldVariant.Floating,
        controlModifier = Modifier.focusRequester(focus))
    TextField("Email", invalid, { invalid = it }, variant = TextFieldVariant.Floating, error = "Kiểm tra địa chỉ email.")
    TextField("Email", "ban@example.com", {}, variant = TextFieldVariant.Floating, state = FieldState.ReadOnly)
    TextField("Email", "ban@example.com", {}, variant = TextFieldVariant.Floating,
        state = FieldState.Disabled(DisabledReason.of("Chưa thể chỉnh sửa.")))
    TextField("Email", "ban@example.com", {}, variant = TextFieldVariant.Floating,
        state = FieldState.Loading(DisabledReason.of("Đang nạp thông tin.")))
    TextField("Tên hiển thị của Nguyễn Thị Ánh Nguyệt để tiếp tục câu chuyện dài", long, { long = it },
        variant = TextFieldVariant.Floating, help = "Nhãn dài không đè lên giá trị; ô tự cao hơn khi chữ lớn.")
    FieldHelp("Tab into the second field for the 2-unit inset focus perimeter; the third shows the 1-unit error stroke.")
}

@Composable
private fun PasswordRow(label: String, initial: String, error: String?) {
    var value by remember { mutableStateOf(initial) }
    var visible by remember { mutableStateOf(false) }
    TextField(label, value, { value = it }, variant = TextFieldVariant.Floating, error = error,
        help = if (error == null) "Giá trị chỉ nằm trong ô này; không ghi nhật ký, không lưu." else null,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions.Default,
        visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
        trailing = {
            IconButton(
                AccessibleName.of(if (visible) "Ẩn mật khẩu" else "Hiện mật khẩu"),
                if (visible) "visibility_off" else "visibility",
                { visible = !visible },
            )
        })
}

@Composable
private fun EyeSpecimen() {
    PasswordRow("Mật khẩu mới", "mật-khẩu-Ỗ", null)
    PasswordRow("Nhập lại mật khẩu", "mật-khẩu", "Hai mật khẩu chưa giống nhau.")
    FieldHelp("Each eye owns its own visibility; the masked default never reaches a snapshot.")
}

@Composable
private fun SearchSpecimen(onEvent: (String) -> Unit) {
    var query by remember { mutableStateOf("") }
    TextField("Tìm trong danh mục", query, { query = it }, variant = TextFieldVariant.FloatingSearch,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onEvent("Search IME action · \"$query\"") }),
        trailing = {
            IconButton(
                AccessibleName.of("Tìm trong danh mục"), "search", { onEvent("Docked search · \"$query\"") },
                variant = ButtonVariant.Primary, circular = true, controlModifier = Modifier.size(tokenDp("size.searchAction")),
            )
        })
    FieldHelp("The host decides whether this commits a query; CMP search keeps its own route and query-on-change semantics.")
}

@Composable
private fun UtilitySelectSpecimen() {
    var language by remember { mutableStateOf(0) }
    Select("Ngôn ngữ", listOf("Tiếng Việt", "English"), language, { language = it },
        expandedDescription = "Danh sách ngôn ngữ đang mở", collapsedDescription = "Danh sách ngôn ngữ đang đóng")
}

@Composable
private fun AvatarSpecimen() {
    Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")), verticalAlignment = Alignment.CenterVertically) {
        Avatar("Cantos", size = AvatarSize.Control, accent = true, accessibleName = AccessibleName.of("Tài khoản của bạn"))
        Avatar("Nguyễn Thị Ánh", size = AvatarSize.Control)
    }
    FieldHelp("Left: the pinned Theatre accent. Right: the identity hue, never inferred as the accent.")
}

@Composable
private fun HeaderSpecimen(theme: Theme, onEvent: (String) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Logo(LogoVariant.Symbol, Modifier.size(tokenDp("size.brandMark")),
            tone = if (theme == Theme.Dark) LogoTone.Dark else LogoTone.Light)
        Spacer(Modifier.width(tokenDp("space.inset")))
        Text("Cantos", style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.weight(1f))
        IconButton(AccessibleName.of("Chuyển giao diện"), if (theme == Theme.Dark) "light_mode" else "dark_mode",
            { onEvent("Theme action requested") }, variant = ButtonVariant.Tonal)
        Spacer(Modifier.width(tokenDp("space.inset")))
        Avatar("Cantos", size = AvatarSize.Control, accent = true)
    }
    FieldHelp("Official symbol 44, 12 before the bold word; brand, theme and avatar share one 48-unit centerline.")
}
