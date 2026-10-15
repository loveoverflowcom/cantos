package com.cantos.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.Density as DisplayDensity
import androidx.compose.ui.unit.Dp
import com.cantos.ui.AccessibleName
import com.cantos.ui.Announcement
import com.cantos.ui.Avatar
import com.cantos.ui.AvatarInitials
import com.cantos.ui.AvatarSize
import com.cantos.ui.BackButton
import com.cantos.ui.Badge
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTheme
import com.cantos.ui.Checkbox
import com.cantos.ui.Container
import com.cantos.ui.ContainerWidth
import com.cantos.ui.Density
import com.cantos.ui.Dialog
import com.cantos.ui.DisabledReason
import com.cantos.ui.EmptyState
import com.cantos.ui.ErrorState
import com.cantos.ui.FieldError
import com.cantos.ui.FieldHelp
import com.cantos.ui.FieldLabel
import com.cantos.ui.FieldState
import com.cantos.ui.FilterChip
import com.cantos.ui.Grid
import com.cantos.ui.Icon
import com.cantos.ui.IconButton
import com.cantos.ui.Link
import com.cantos.ui.LoadingState
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.LogoVariant
import com.cantos.ui.NavItem
import com.cantos.ui.NavTabs
import com.cantos.ui.NumberField
import com.cantos.ui.PreviewExample
import com.cantos.ui.Radio
import com.cantos.ui.RangeField
import com.cantos.ui.Select
import com.cantos.ui.SelectOption
import com.cantos.ui.Space
import com.cantos.ui.Stack
import com.cantos.ui.StackDirection
import com.cantos.ui.Status
import com.cantos.ui.StatusTone
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.Switch
import com.cantos.ui.TextArea
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.Theme
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.UiRoot
import com.cantos.ui.filterComponents
import com.cantos.ui.generated.CantosIcons
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.generated.PreviewComponent
import com.cantos.ui.generated.TokenValue
import com.cantos.ui.parseTokenColor
import com.cantos.ui.resolve
import com.cantos.ui.resolvePreviewSelection
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp
import com.cantos.ui.typeStyle
import kotlin.math.roundToInt

/**
 * Development-only sample data: never touches Studio, Theatre, a network or playback engine.
 *
 * Appearance is injected: [systemIsDark] is the host's dark-mode fact, supplied by the composition root
 * that owns the operating-system query. The gallery resolves [ThemeMode.System] against it once and
 * hands components only the resolved theme.
 */
@Composable
fun PreviewApp(
    initialPreferences: UiPreferences = UiPreferences(),
    initialSection: Int = 0,
    /** Supplied only by a host that owns its window and can resize it; elsewhere the width buttons are not shown. */
    onWidthPreset: ((ViewportPreset) -> Unit)? = null,
    systemIsDark: Boolean = false,
    initialThemeMode: ThemeMode = if (initialPreferences.theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
    initialSpecimenId: String? = null,
) {
    // Read outside UiRoot: the host's own density and font scale, before the preview text multiplier.
    val host = LocalDensity.current
    var themeMode by remember { mutableStateOf(initialThemeMode) }
    var chosen by remember { mutableStateOf(initialPreferences) }
    val preferences = chosen.copy(theme = themeMode.resolve(systemIsDark))
    var query by remember { mutableStateOf("") }
    var category by remember { mutableStateOf<String?>(null) }
    var selectedId by remember { mutableStateOf("button") }
    var section by remember { mutableStateOf(initialSection) }
    var event by remember { mutableStateOf("Chưa có thao tác. Các ví dụ chỉ thay đổi dữ liệu tại chỗ.") }
    val components = filterComponents(query, category)
    val selected = resolvePreviewSelection(components, selectedId)
    // The inventory links back here: reset the filters so the chosen component is actually displayed.
    val openComponent = { id: String ->
        selectedId = id
        query = ""
        category = null
        section = 0
        event = "Opened $id from the design inventory. Registration is not qualification."
    }
    UiRoot(preferences = preferences) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val viewportWidth = maxWidth
            val viewportHeight = maxHeight
            // One window-size decision for every tab: the canonical expanded breakpoint, measured on the window.
            val expanded = viewportWidth >= tokenDp("breakpoint.expanded")
            // The theme above composed host x preview: this density's font scale is the effective one.
            val scrollWholePage = scrollsWholePage(expanded, LocalDensity.current.fontScale)
            val page = if (scrollWholePage) Modifier.verticalScroll(rememberScrollState()) else Modifier.fillMaxSize()
            Column(page.padding(tokenDp("space.lg")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.lg")),
                    verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    Column {
                        Text("Cantos UI System", style = MaterialTheme.typography.headlineMedium)
                        FieldHelp("CMP · Desktop preview · tokens v${CantosTokens.version}")
                    }
                    Badge("Implemented · runtime unverified", tone = StatusTone.Warning)
                }
                ViewportProbe(viewportWidth, viewportHeight, host, preferences)
                PreviewControls(preferences, themeMode, systemIsDark, { themeMode = it }, { chosen = it }, onWidthPreset)
                NavTabs(listOf("Components", "Named tokens", "Design v3"), section, { section = it },
                    groupName = AccessibleName.of("Khu vực phòng mẫu"))
                // Keep nested panes bounded when large text makes the outer page scroll.
                val region = if (scrollWholePage) Modifier.height(viewportHeight) else Modifier.weight(1f)
                if (section == 1) {
                    TokenCatalog(preferences, region)
                } else if (section == 2) {
                    DesignInventoryView(openComponent, region, expanded, scrollWholePage, initialSpecimenId)
                } else {
                    Box(region.fillMaxWidth()) {
                        if (expanded) {
                            Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
                                ComponentBrowser(query, { query = it }, category, { category = it }, components,
                                    selected?.id ?: "", { selectedId = it }, Modifier.width(PreviewGeometry.masterPane))
                                ComponentDetail(selected, { event = it }, Modifier.weight(1f).verticalScroll(rememberScrollState()))
                            }
                        } else {
                            Column(Modifier.verticalScroll(rememberScrollState()),
                                verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                                TextField("Find component", query, { query = it })
                                if (components.isEmpty()) {
                                    EmptyState("Không có kết quả", "Thử tìm theo tên component hoặc nhóm.")
                                } else {
                                    Select("Component", components.map { it.name },
                                        components.indexOfFirst { it.id == selected?.id },
                                        { selectedId = components[it].id },
                                        expandedDescription = "Danh sách component đang mở",
                                        collapsedDescription = "Danh sách component đang đóng")
                                }
                                ComponentDetail(selected, { event = it })
                            }
                        }
                    }
                }
                Surface(tone = SurfaceTone.Container) {
                    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        FieldLabel("Event log")
                        FieldHelp(event)
                    }
                }
            }
        }
    }
}

/**
 * Live evidence line: the constraint, the host density and font scale, the preview multiplier and the
 * effective scale a capture was actually rendered with (the theme composes host × multiplier).
 */
@Composable
private fun ViewportProbe(width: Dp, height: Dp, host: DisplayDensity, preferences: UiPreferences) {
    val effective = LocalDensity.current.fontScale
    FieldHelp(
        "Viewport ${width.value.roundToInt()}×${height.value.roundToInt()} dp · ${host.density} px/dp · " +
            "font scale host ${host.fontScale} × preview ${preferences.textScale} = $effective · fixtures vi-VN",
    )
}

@Composable
private fun PreviewControls(
    preferences: UiPreferences,
    mode: ThemeMode,
    systemIsDark: Boolean,
    onMode: (ThemeMode) -> Unit,
    onChange: (UiPreferences) -> Unit,
    onWidthPreset: ((ViewportPreset) -> Unit)?,
) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        ThemeMode.entries.forEach { choice ->
            FilterChip(choice.name, mode == choice, { onMode(choice) })
        }
        FilterChip("Compact", preferences.density == Density.Compact, { onChange(preferences.copy(density = Density.Compact)) })
        FilterChip("Comfortable", preferences.density == Density.Comfortable, { onChange(preferences.copy(density = Density.Comfortable)) })
        FilterChip("Reduced motion", preferences.reducedMotion, { onChange(preferences.copy(reducedMotion = it)) })
        listOf(1f, 1.3f, 2f).forEach { scale ->
            FilterChip("Text ${scale}×", preferences.textScale == scale, { onChange(preferences.copy(textScale = scale)) })
        }
        // An inert control would mislead in a host that cannot resize its window (Android previews, test roots).
        onWidthPreset?.let { resize ->
            ViewportPreset.entries.forEach { preset ->
                Button(AccessibleName.of("Set window width to ${preset.label}"), { resize(preset) }, variant = ButtonVariant.Text) {
                    Text(preset.label)
                }
            }
        }
    }
    FieldHelp("Theme mode ${mode.name} → ${preferences.theme.tokenName}; host reports ${if (systemIsDark) "dark" else "light"}. Only System follows the host.")
}

@Composable
private fun ComponentBrowser(
    query: String,
    onQuery: (String) -> Unit,
    category: String?,
    onCategory: (String?) -> Unit,
    components: List<PreviewComponent>,
    selected: String,
    onSelect: (String) -> Unit,
    modifier: Modifier,
) {
    Column(modifier.verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        TextField("Find component", query, onQuery)
        FieldLabel("${components.size} components")
        FilterChip("All families", category == null, { onCategory(null) })
        PreviewCatalog.components.map { it.category }.distinct().forEach { family ->
            FilterChip(family, family == category, { onCategory(if (it) family else null) })
        }
        components.forEach { component ->
            NavItem(component.name, selected == component.id, { onSelect(component.id) })
        }
        if (components.isEmpty()) FieldHelp("Không có kết quả")
    }
}

@Composable
private fun ComponentDetail(component: PreviewComponent?, onEvent: (String) -> Unit, modifier: Modifier = Modifier) {
    if (component == null) {
        EmptyState("Chọn component", "Chọn một ví dụ để thử các trạng thái.", modifier)
        return
    }
    key(component.id) {
        var state by remember { mutableStateOf(component.states.first()) }
        val focusRequester = remember { FocusRequester() }
        Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Text(component.name, style = MaterialTheme.typography.titleLarge)
            FieldHelp("${component.category} · ${component.id}")
            Badge(component.cmpStatus, tone = StatusTone.Warning)
            component.cmpReason?.let { FieldHelp(it) }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                component.states.forEach { choice ->
                    FilterChip(choice, state == choice, { state = choice })
                }
            }
            FieldHelp("Focus is real keyboard focus. Use Tab/Shift+Tab, Enter, Space and Escape; no state is a verified badge.")
            Surface(Modifier.fillMaxWidth(), tone = SurfaceTone.Surface) {
                val example = PreviewExample.fromId(component.id)
                if (example == null || component.cmpStatus == "unsupported") {
                    ErrorState("Unsupported", component.cmpReason ?: "No native example has been declared.", "Retry", {})
                } else {
                    ComponentExample(example, state, onEvent, Modifier.focusRequester(focusRequester))
                    if (state == "focus") {
                        LaunchedEffect(component.id, state) { focusRequester.requestFocus() }
                    }
                }
            }
            if (component.id == "grid" || component.id == "container") {
                FieldHelp("The available host width wins. Wide/column presets reflow at named breakpoints; use the width controls to inspect the actual viewport.")
            }
            if (component.id == "error-state" && state == "retry") {
                FieldHelp("Activate Thử lại to inspect the real retry callback and event count; choosing this preset does not dispatch an action.")
            }
            ParityNotes(component.id)
        }
    }
}

@Composable
private fun ParityNotes(id: String) {
    val note = when (id) {
        "link" -> "Native Link dispatches a URI callback. Browser href, target, download and history are web-only; the preview does not launch a site."
        "number-field" -> "Native decimal keyboard hint preserves raw text. HTML min/max/step browser validation is platform-specific; the caller owns numeric validation."
        "field-label" -> "Native labels are composed into each control's semantics. HTML for/id associations do not exist in Compose."
        "nav-tabs" -> "Native in-place tabs expose selection and a named group; Arrow/Home/End move focus and request selection. Leptos uses navigation links, not ARIA tabs."
        "select" -> "Stable values survive reordered labels. Empty/missing selection never emits a callback. Unsupported options retain their reason. External input changes close the menu."
        "dialog" -> "Native modal and return-focus sample; DOM inertness/showModal and Android/iOS Back require their own renderer evidence."
        "loading-state" -> "Static labelled pending state. No indeterminate animation is presented as reduced-motion verification."
        "ui-root" -> "Fonts map shared Noto family intent to platform sans/serif fallback. Embedded Noto assets and mobile safe-area/reduced-motion adapters remain unverified."
        else -> "Common Compose wrapper and callbacks implemented. Desktop build, native pixels, screen-reader operation and Android/iOS parity are not verified here."
    }
    FieldHelp(note)
}

@Composable
internal fun ComponentExample(example: PreviewExample, state: String, onEvent: (String) -> Unit, modifier: Modifier) {
    var title by remember(example) { mutableStateOf("Ánh đèn cuối sân khấu") }
    var dialogue by remember(example) { mutableStateOf("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?\nỖ, Ữ, Nguyễn Thị Minh Phương") }
    var number by remember(example) { mutableStateOf("1,25") }
    var richText by remember(example, state) {
        val text = if (example == PreviewExample.TextArea) {
            "Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?\nỖ, Ữ"
        } else if (example == PreviewExample.NumberField) {
            "1,25"
        } else {
            "Ánh đèn cuối sân khấu · Ỗ, Ữ"
        }
        mutableStateOf(TextFieldValue(text, selection = TextRange(0, minOf(7, text.length))))
    }
    var checked by remember(example, state) { mutableStateOf(state == "selected" || state == "current") }
    var choice by remember(example, state) { mutableStateOf(if (state == "current") 1 else 0) }
    var stableChoice by remember(example, state) { mutableStateOf<String?>(if (state == "missing-value") "removed" else "narrator") }
    var reverseOptions by remember(example, state) { mutableStateOf(false) }
    var interrupted by remember(example, state) { mutableStateOf(false) }
    var range by remember(example, state) { mutableStateOf(when (state) {
        "zero", "minimum" -> 0f; "maximum" -> 100f; "stepped" -> 50f; else -> 45f
    }) }
    var presses by remember(example) { mutableStateOf(0) }
    val reason = DisabledReason.of("Chưa chọn phiên bản kịch bản. Hãy chọn phiên bản để tiếp tục.")
    val pendingReason = DisabledReason.of("Đang lưu bản nháp. Nội dung đã nhập vẫn được giữ lại.")
    val buttonState = when (state) {
        "disabled" -> ButtonState.Disabled(reason)
        "loading" -> ButtonState.Loading(pendingReason)
        else -> ButtonState.Enabled
    }
    val fieldState = when (state) {
        "disabled" -> FieldState.Disabled(reason)
        "loading" -> FieldState.Loading(pendingReason)
        "readonly" -> FieldState.ReadOnly
        else -> FieldState.Editable
    }
    val error = when (state) {
        "error" -> if (example in listOf(PreviewExample.Checkbox, PreviewExample.Radio, PreviewExample.Switch, PreviewExample.Select)) {
            "Cần duyệt lựa chọn trước khi tiếp tục. Giá trị vẫn được giữ lại."
        } else if (example == PreviewExample.RangeField) {
            "Giá trị tham khảo cần được kiểm tra trước khi tiếp tục."
        } else "Tên tác phẩm cần có ít nhất 3 ký tự. Nội dung chưa bị xóa."
        "blank-error" -> " \n\t"
        else -> null
    }
    val onPress = { presses += 1; onEvent("${example.id} · callback #$presses · không có yêu cầu mạng") }
    val tone = when (state) {
        "error" -> StatusTone.Error
        "info" -> StatusTone.Info
        "success" -> StatusTone.Success
        "loading", "warning" -> StatusTone.Warning
        else -> StatusTone.Neutral
    }
    val feedback = when (state) {
        "error" -> "Không thể lưu bản nháp"
        "success" -> "Đã lưu bản nháp"
        "loading" -> "Đang xử lý bản nháp"
        "warning" -> "Cần kiểm tra ghi chú phát âm"
        "info" -> "Thông tin bản nháp"
        "announcement" -> "Đã đổi cảnh đang chọn"
        "empty" -> "Chưa có bản nháp"
        else -> "Bản nháp · chưa xuất bản"
    }
    when (example) {
        PreviewExample.UiRoot -> {
            val current = LocalUiPreferences.current
            val nested = when (state) {
                "light" -> current.copy(theme = Theme.Light)
                "dark" -> current.copy(theme = Theme.Dark)
                "compact" -> current.copy(density = Density.Compact)
                "comfortable" -> current.copy(density = Density.Comfortable)
                else -> current
            }
            UiRoot(modifier.height(PreviewGeometry.sampleRoot).fillMaxWidth(), preferences = nested) {
                Container {
                    Stack {
                        Text("Ánh đèn cuối sân khấu", style = MaterialTheme.typography.headlineSmall)
                        Text("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không? · Ỗ, Ữ", style = typeStyle("editor", editorial = true))
                        Button("Lưu bản nháp", onPress)
                        if (state == "nested-theme") {
                            UiRoot(
                                Modifier.height(PreviewGeometry.nestedSampleRoot).fillMaxWidth(),
                                preferences = nested.copy(theme = Theme.Dark),
                            ) {
                                Container {
                                    Text("Theme lồng nhau · cùng cỡ chữ: Ỗ, Ữ")
                                }
                            }
                        }
                    }
                }
            }
        }
        PreviewExample.BackButton -> {
            val name = if (state == "long-vietnamese" || state == "large-text") {
                "Quay lại Tài khoản của Nguyễn Thị Minh Phương · giữ nguyên nội dung đã nhập · Ỗ, Ữ"
            } else "Quay lại Tài khoản"
            val preferences = LocalUiPreferences.current
            CantosTheme(preferences.copy(
                textScale = if (state == "large-text") 2f else preferences.textScale,
                theme = when (state) { "light" -> Theme.Light; "dark" -> Theme.Dark; else -> preferences.theme },
            )) {
                BackButton(
                    AccessibleName.of(name), onPress, state = buttonState,
                    iconOnly = state !in listOf("named", "long-vietnamese", "large-text"),
                    controlModifier = modifier,
                    interactionSource = rememberSamplePress(state == "pressed"),
                )
            }
        }
        PreviewExample.Logo -> {
            Logo(variant = LogoVariant.Cantos,
                tone = if (LocalUiPreferences.current.theme == Theme.Dark) LogoTone.Dark else LogoTone.Light)
            LogoSamples(state)
        }
        PreviewExample.Button -> if (state in listOf("auth-primary", "auth-secondary", "auth-outlined", "pressed", "long-vietnamese", "large-text")) {
            val preferences = LocalUiPreferences.current
            CantosTheme(preferences.copy(textScale = if (state == "large-text") 2f else preferences.textScale)) {
                AuthActionSample(when (state) {
                    "pressed" -> "auth-pressed"
                    "long-vietnamese", "large-text" -> "auth-long-label"
                    else -> state
                }, onPress, modifier)
            }
        } else Stack {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                ButtonVariant.entries.forEach { variant ->
                    if (state == "composed-content") {
                        Button(AccessibleName.of("Lưu · ${variant.name} · bản nháp Ánh đèn cuối sân khấu"), onPress,
                            state = buttonState, variant = variant,
                            controlModifier = if (variant == ButtonVariant.Primary) modifier else Modifier) {
                            Icon("add")
                            androidx.compose.foundation.layout.Spacer(Modifier.size(tokenDp("space.sm")))
                            Text("Lưu · ${variant.name}")
                        }
                    } else {
                        Button("Lưu · ${variant.name}", onPress, state = buttonState, variant = variant,
                            controlModifier = if (variant == ButtonVariant.Primary) modifier else Modifier)
                    }
                }
            }
            if (state == "composed-content") {
                Button(
                    name = AccessibleName.of("Thêm cảnh · Ỗ, Ữ · cho Ánh đèn cuối sân khấu"),
                    onPress = onPress,
                    variant = ButtonVariant.Tonal,
                ) {
                    Icon("add")
                    androidx.compose.foundation.layout.Spacer(Modifier.size(tokenDp("space.sm")))
                    Text("Thêm cảnh · Ỗ, Ữ")
                }
            }
            FieldHelp("Callback count: $presses")
        }
        PreviewExample.IconButton -> IconButton(AccessibleName.of("Thêm cảnh cho Ánh đèn cuối sân khấu"), "add", onPress,
            state = buttonState, controlModifier = modifier)
        PreviewExample.FilterChip -> FilterChip("Kịch nói · tiếng Việt", checked, { checked = it; onEvent("Selected: $it") },
            state = buttonState, controlModifier = modifier)
        PreviewExample.Link -> Link("Mở bản nháp Ánh đèn cuối sân khấu", "/draft/example", { onEvent("Link callback: $it") }, state = buttonState, controlModifier = modifier)
        PreviewExample.NavItem -> NavItem("Tác phẩm của tôi", checked, { checked = true; onEvent("NavItem selected") }, state = buttonState, controlModifier = modifier)
        PreviewExample.NavTabs -> NavTabs(
            if (state == "overflow") listOf("Kịch bản Ánh đèn cuối sân khấu", "Nguyễn Thị Minh Phương", "Lịch sử chỉnh sửa và ghi chú phát âm", "Bản nháp chưa xuất bản")
            else listOf("Kịch bản", "Nhân vật", "Lịch sử"),
            choice, { choice = it; onEvent("Tab: $it") }, modifier,
            groupName = AccessibleName.of("Các phần bản nháp"), state = buttonState)
        PreviewExample.TextField -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (state in listOf("auth-tonal", "long-vietnamese", "large-text")) {
                val preferences = LocalUiPreferences.current
                CantosTheme(preferences.copy(textScale = if (state == "large-text") 2f else preferences.textScale)) {
                    TextField("Tên hiển thị của Nguyễn Thị Minh Phương · Ỗ, Ữ", richText, { richText = it },
                        help = "Nội dung chỉ là minh họa; không mở form mật khẩu hay gửi thông tin.",
                        variant = TextFieldVariant.AuthTonal, controlModifier = modifier)
                }
            } else if (state == "selection" || state == "ime-action") {
                TextField("Tên tác phẩm", richText, { richText = it }, modifier,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onEvent("TextField · Done · ${richText.text}") }))
                FieldHelp("Selection: ${richText.selection.start}..${richText.selection.end}; composition: ${richText.composition}")
            } else {
                TextField("Tên tác phẩm", title, { title = it; onEvent("TextField: value updated") }, modifier, fieldState,
                    "Tên có dấu được giữ nguyên, không chuẩn hóa khi gõ.", error)
                TextField("Tên hiển thị minh họa", title, { title = it; onEvent("Auth TextField: value updated") },
                    state = fieldState, help = "Giữ nguyên nội dung đã nhập và dấu tiếng Việt.", error = error,
                    variant = TextFieldVariant.AuthTonal)
            }
        }
        PreviewExample.TextArea -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (state == "selection" || state == "ime-action") {
                TextArea("Lời thoại", richText, { richText = it }, modifier,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onEvent("TextArea · Done · ${richText.text}") }),
                    minLines = 2, maxLines = 6)
                FieldHelp("Selection: ${richText.selection.start}..${richText.selection.end}; composition: ${richText.composition}")
            } else {
                TextArea("Lời thoại", dialogue, { dialogue = it; onEvent("TextArea: value updated") }, modifier, fieldState,
                    "Hãy thử chọn, sao chép và nhập tiếng Việt có dấu.", error)
            }
        }
        PreviewExample.NumberField -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (state == "selection" || state == "ime-action") {
                NumberField("Tốc độ tham khảo", richText, { richText = it }, modifier,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onEvent("NumberField · Done · ${richText.text}") }))
                FieldHelp("Selection: ${richText.selection.start}..${richText.selection.end}; raw text: ${richText.text}")
            } else {
                NumberField("Tốc độ tham khảo", number, { number = it; onEvent("NumberField raw text: $it") }, modifier, fieldState,
                    "Dữ liệu thử: 1,25. Chủ ứng dụng quyết định quy tắc phân tích số.", error)
            }
        }
        PreviewExample.Select -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            val options = if (state == "empty-options") emptyList() else listOf(
                SelectOption("narrator", "Người dẫn chuyện"),
                SelectOption("minh-phuong", "Nguyễn Thị Minh Phương"),
                SelectOption("unavailable", "Người giữ đèn cuối sân khấu", ButtonState.Disabled(DisabledReason.of("Chưa có quyền dùng giọng này"))),
            ).let { if (reverseOptions) it.reversed() else it }
            val availability = if (interrupted) ButtonState.Loading(pendingReason) else buttonState
            Select("Nhân vật", options, stableChoice,
                { stableChoice = it; onEvent("Select stable value: $it") }, state = availability,
                expandedDescription = "Danh sách nhân vật đang mở",
                collapsedDescription = "Danh sách nhân vật đang đóng",
                placeholder = "Chọn nhân vật", emptyDescription = "Chưa có nhân vật. Hãy thêm nhân vật để tiếp tục.",
                controlModifier = modifier, error = error)
            if (state == "reordered-options") Button("Đảo thứ tự nhân vật", { reverseOptions = !reverseOptions })
            if (state == "interrupted") Switch("Tạm ngừng lựa chọn", interrupted, { interrupted = it })
            FieldHelp("Stable value: $stableChoice")
        }
        PreviewExample.Checkbox -> Checkbox("Giữ nguyên ghi chú phát âm tiếng Việt", checked,
            { checked = it; onEvent("Checkbox: $it") }, state = buttonState, controlModifier = modifier,
            help = if (state == "label-activation") "Nhấn cả nhãn hoặc Space để đổi lựa chọn." else null, error = error)
        PreviewExample.Radio -> Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Radio("Bản nháp của Nguyễn Thị Minh Phương", checked,
                { checked = true; onEvent("Radio: draft selected") }, state = buttonState,
                controlModifier = modifier, error = error)
            Radio("Bản nháp của người dẫn chuyện", !checked,
                { checked = false; onEvent("Radio: narration selected") }, state = buttonState)
        }
        PreviewExample.Switch -> Switch("Hiện ghi chú người dẫn chuyện", checked,
            { checked = it; onEvent("Switch: $it") }, state = buttonState, controlModifier = modifier, error = error)
        PreviewExample.RangeField -> RangeField("Mức âm lượng thử", range,
            { range = it; onEvent("RangeField: $it") }, state = buttonState,
            steps = if (state == "stepped") 3 else 0,
            valueText = "${range.toString().replace('.', ',')} trên 100", controlModifier = modifier, error = error,
            onValueChangeFinished = { onEvent("RangeField completed: $range") },
            help = "Dùng phím mũi tên, Home/End hoặc kéo. Đây là dữ liệu thử.")
        PreviewExample.FieldLabel -> Column(modifier) {
            FieldLabel("Tên tác phẩm · nhãn được lồng trong control")
            TextField("Tên tác phẩm", title, { title = it }, state = fieldState, error = error)
        }
        PreviewExample.FieldHelp -> Column(modifier) {
            FieldHelp(if (state == "empty-hidden") "" else "Không đổi tên nhân vật hoặc dấu câu khi nhập văn bản.")
            TextField("Tên tác phẩm", title, { title = it }, state = fieldState, help = "Người dẫn chuyện · Ỗ, Ữ", error = error)
        }
        PreviewExample.FieldError -> Column(modifier) {
            FieldError(if (state == "empty-hidden") "" else "Chưa thể lưu: cần chọn phiên bản kịch bản.")
            TextField("Tên tác phẩm", title, { title = it }, state = fieldState, error = error)
        }
        PreviewExample.Badge -> Badge(feedback, modifier, tone)
        PreviewExample.Status -> Status(feedback, modifier, tone, announce = state == "announcement")
        PreviewExample.LoadingState -> LoadingState("Đang nạp danh sách bản nháp. Lựa chọn hiện tại vẫn được giữ lại.", modifier,
            announcement = if (state == "announcement") Announcement.Polite else Announcement.Off)
        PreviewExample.EmptyState -> {
            val recovery: (@Composable () -> Unit)? = if (state == "recovery") ({ Button("Thêm cảnh", onPress) }) else null
            EmptyState("Chưa có cảnh nào", "Thêm cảnh đầu tiên cho Ánh đèn cuối sân khấu.", modifier, action = recovery)
        }
        PreviewExample.ErrorState -> ErrorState("Không thể nạp bản nháp", "Bản nháp đã nhập vẫn còn. Hãy thử lại khi kết nối ổn định.", modifier,
            announcement = if (state == "announcement") Announcement.Assertive else Announcement.Off) {
            Stack(direction = StackDirection.Horizontal, gap = Space.Sm) {
                Button("Thử lại", onPress, variant = ButtonVariant.Secondary)
                if (state == "recovery-slot") Button("Xem ghi chú bản nháp", onPress, variant = ButtonVariant.Text)
            }
        }
        PreviewExample.Surface -> Surface(modifier, tone = when (state) {
            "container" -> SurfaceTone.Container
            "high" -> SurfaceTone.Raised
            "reading" -> SurfaceTone.Reading
            "profile", "primary-container" -> SurfaceTone.PrimaryContainer
            "membership" -> SurfaceTone.Container
            else -> SurfaceTone.Surface
        }, inset = if (state == "inset-none") Space.None else Space.Md,
            variant = when (state) {
                "auth" -> SurfaceVariant.Auth
                "profile", "primary-container" -> SurfaceVariant.Profile
                "membership" -> SurfaceVariant.Membership
                "account-group", "large-text" -> SurfaceVariant.AccountGroup
                else -> SurfaceVariant.Regular
            }) {
            val text = "Ánh đèn cuối sân khấu · Ngày mai, mình có diễn tiếp không? Nguyễn Thị Minh Phương giữ nguyên dấu tiếng Việt trong lời thoại dài."
            val preferences = LocalUiPreferences.current
            CantosTheme(preferences.copy(textScale = if (state == "large-text") 2f else preferences.textScale)) {
                Text(if (state == "long-text") List(4) { text }.joinToString("\n\n") else text,
                    style = if (state == "reading") typeStyle("editor", editorial = true) else MaterialTheme.typography.bodyMedium)
            }
        }
        PreviewExample.Stack -> Stack(modifier, when (state) {
            "horizontal" -> StackDirection.Horizontal
            "wrap" -> StackDirection.Wrap
            else -> StackDirection.Vertical
        }, gap = if (state == "gap-lg") Space.Lg else Space.Sm) {
            if (state == "horizontal") {
                Badge("Bản nháp")
                Badge("Cảnh 1")
            } else {
                Badge("Người dẫn chuyện")
                Badge("Nguyễn Thị Minh Phương")
                Badge("Ánh đèn cuối sân khấu · bản nháp chưa xuất bản")
                if (state == "long-text") Text("Ngày mai, mình có diễn tiếp không? Nguyễn Thị Minh Phương giữ nguyên ghi chú phát âm và lời thoại tiếng Việt dài, không thu nhỏ chữ để ép nội dung vào một hàng.")
            }
        }
        PreviewExample.Grid -> Grid(listOf(
            { Surface { Text("Ánh đèn cuối sân khấu · cảnh một") } },
            { Surface(tone = SurfaceTone.Container) { Text("Ngày mai, mình có diễn tiếp không? · cảnh hai") } },
            { Surface { Text("Nguyễn Thị Minh Phương · lời thoại dài có dấu tiếng Việt") } },
        ), if (state == "narrow") modifier.widthIn(max = PreviewGeometry.narrowSampleMaxWidth) else modifier,
            requestedColumns = when (state) { "one-column", "narrow" -> 1; "three-columns", "wide" -> 3; else -> 2 },
            gap = if (state == "gap-lg") Space.Lg else Space.Md)
        PreviewExample.Container -> Container(if (state == "narrow") modifier.widthIn(max = PreviewGeometry.narrowSampleMaxWidth) else modifier, width = when (state) {
            "wide" -> ContainerWidth.Wide; "fluid" -> ContainerWidth.Fluid; else -> ContainerWidth.Reading
        }, inset = if (state == "inset-none") Space.None else Space.Md) {
            Text("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không? Ánh đèn cuối sân khấu còn sáng; bản nháp này dùng để thử xuống dòng và tăng cỡ chữ.")
        }
        PreviewExample.Dialog -> DialogExample(state, onEvent, modifier)
        PreviewExample.Icon -> FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            CantosIcons.icons.forEach { icon -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Icon(icon.name)
                FieldHelp(icon.name)
            } }
        }
        PreviewExample.Avatar -> AvatarExample(state, modifier)
    }
}

/** Offline sample picture; real hosts decode an authorized image themselves. */
private fun previewAvatarImage(): ImageBitmap {
    val bitmap = ImageBitmap(64, 64)
    Canvas(bitmap).apply {
        drawRect(0f, 0f, 64f, 64f, Paint().apply { color = parseTokenColor("#886638") })
        drawCircle(Offset(32f, 26f), 12f, Paint().apply { color = parseTokenColor("#f6e7ca") })
    }
    return bitmap
}

@Composable
private fun AvatarExample(state: String, modifier: Modifier) {
    val image = remember { previewAvatarImage() }
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        verticalAlignment = Alignment.CenterVertically) {
        when (state) {
            "initials-pair" -> Avatar("Người dẫn chuyện", initials = AvatarInitials.Pair)
            "no-initial" -> Avatar("😀")
            "image" -> Avatar("Đỗ Văn An", image = image)
            // The host passes null after a failed decode; initials return immediately.
            "image-failed" -> Avatar("Đỗ Văn An", image = null)
            "sizes" -> AvatarSize.entries.forEach { Avatar("Lan", size = it) }
            "named" -> Avatar("岑 昕", initials = AvatarInitials.Pair,
                accessibleName = AccessibleName.of("Ảnh đại diện của 岑 昕"))
            else -> Avatar("Nguyễn Thị Ánh", stableId = "user-42")
        }
        if (state != "named") FieldHelp("Nguyễn Thị Ánh · Người dẫn chuyện")
    }
}

@Composable
private fun DialogExample(state: String, onEvent: (String) -> Unit, modifier: Modifier) {
    var open by remember(state) { mutableStateOf(state == "open") }
    var note by remember { mutableStateOf("Nguyễn Thị Minh Phương") }
    val triggerFocus = remember { FocusRequester() }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Button("Mở thông tin bản nháp", { open = true; onEvent("Dialog opened") },
            controlModifier = Modifier.focusRequester(triggerFocus))
        FieldHelp("Mở, nhấn Tab/Shift+Tab, rồi Escape hoặc Đóng. Kiểm tra focus quay về nút mở; preset không chứng minh kết quả.")
        Dialog(open, "Ánh đèn cuối sân khấu · bản nháp", "Đóng", {
            open = false
            onEvent("Dialog dismissed; requested trigger focus restoration")
        }, returnFocus = triggerFocus) {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?")
                TextField("Ghi chú bản nháp", note, { note = it })
            }
        }
        FieldHelp("Ghi chú được giữ: $note")
    }
}

@Composable
private fun TokenCatalog(preferences: UiPreferences, modifier: Modifier) {
    var query by remember { mutableStateOf("") }
    val entries = CantosTokens.entries.filter { it.name.contains(query.trim(), ignoreCase = true) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        TextField("Find named token", query, { query = it })
        FieldHelp("${entries.size} roles · ${preferences.theme.tokenName} · ${preferences.density.tokenName} · one generated source")
        SelectionContainer {
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                entries.forEach { entry ->
                    Surface(Modifier.fillMaxWidth()) {
                        Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                            TokenSample(entry)
                            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                                FieldLabel(entry.name)
                                val value = if (entry.kind == "duration" && preferences.reducedMotion)
                                    CantosTokens.number(entry.name, reducedMotion = true).toInt().toString()
                                else CantosTokens.raw(entry.name, preferences.theme.tokenName, preferences.density.tokenName)
                                Text("${entry.kind} · $value", style = MaterialTheme.typography.bodySmall)
                                entry.aliasOf?.let { FieldHelp("Alias of $it") }
                            }
                        }
                    }
                }
                if (entries.isEmpty()) EmptyState("Không có token", "Thử color.primary, space.md hoặc type.body.size.")
            }
        }
    }
}


@Composable
private fun TokenSample(entry: TokenValue) {
    val reading = "Người dẫn chuyện · Ỗ, Ữ"
    when {
        entry.kind == "color" -> Box(Modifier.size(tokenDp("size.control")).background(tokenColor(entry.name)))
        entry.name.startsWith("type.") -> {
            val role = entry.name.split('.').getOrNull(1)
            val editorial = role == "editor" || entry.name == "type.family.editorial"
            val mappedRole = if (role in listOf("body", "title", "headline", "label", "metadata", "editor")) role ?: "body" else "body"
            Column(Modifier.width(PreviewGeometry.tokenSampleColumn)) {
                Text(reading, style = typeStyle(mappedRole, editorial = editorial))
                FieldHelp("Platform font fallback · ${if (editorial) "serif" else "sans"}")
            }
        }
        entry.name.startsWith("shape.") -> Box(
            Modifier.size(width = tokenDp("size.control") * 2, height = tokenDp("size.control"))
                .background(tokenColor("color.primaryContainer"), RoundedCornerShape(tokenDp(entry.name))),
        )
        entry.name.startsWith("space.") || entry.name.startsWith("size.") -> {
            val amount = tokenDp(entry.name)
            val bound = PreviewGeometry.tokenBarBound
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Box(Modifier.width(minOf(amount, bound)).height(tokenDp("space.sm")).background(tokenColor("color.primary")))
                if (amount > bound) FieldHelp("Sample clipped; value stays exact")
            }
        }
        entry.name.startsWith("state.") -> Box(
            Modifier.size(tokenDp("size.control"))
                .background(tokenColor("color.primary").copy(alpha = CantosTokens.number(entry.name).toFloat())),
        )
        entry.kind == "duration" || entry.kind == "cubicBezier" -> FieldHelp("Intent value · motion not demonstrated")
    }
}
