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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.Density as DisplayDensity
import androidx.compose.ui.unit.Dp
import com.cantos.ui.AccessibleName
import com.cantos.ui.Announcement
import com.cantos.ui.Avatar
import com.cantos.ui.AvatarInitials
import com.cantos.ui.AvatarSize
import com.cantos.ui.BackButton
import com.cantos.ui.CantosExpression
import com.cantos.ui.Badge
import com.cantos.ui.CantosShapes
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
import com.cantos.ui.SurfacePulse
import com.cantos.ui.EdgeProgress
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
import com.cantos.ui.preview.workbench.SyntheticPreviewText
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
import com.cantos.ui.RootContent
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
import com.cantos.ui.preview.workbench.ComponentSampleState
import com.cantos.ui.preview.workbench.ComponentSampleIntent
import com.cantos.ui.preview.workbench.ComponentSampleResult
import com.cantos.ui.preview.workbench.reduceComponentSample
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
    initialRecipe: com.cantos.ui.preview.workbench.PreviewRecipe? = null,
    /** Licensed families the composition root loaded; null keeps the reviewed platform fallback. */
    uiFontFamily: FontFamily? = null,
    editorialFontFamily: FontFamily? = null,
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
    var event by remember { mutableStateOf(PreviewToolCopy.NoEvents) }
    val components = filterComponents(query, category)
    val selected = resolvePreviewSelection(components, selectedId)
    // The inventory links back here: reset the filters so the chosen component is actually displayed.
    val openComponent = { id: String ->
        selectedId = id
        query = ""
        category = null
        section = 0
        event = PreviewToolCopy.openedComponent(id)
    }
    PreviewRoot(preferences, uiFontFamily, editorialFontFamily) {
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
                        Text(PreviewToolCopy.Title, style = MaterialTheme.typography.headlineMedium)
                        FieldHelp(PreviewToolCopy.rendererVersion(CantosTokens.version))
                    }
                    Badge(PreviewToolCopy.Unverified, tone = StatusTone.Warning)
                }
                ViewportProbe(viewportWidth, viewportHeight, host, preferences)
                PreviewControls(preferences, themeMode, systemIsDark, { themeMode = it }, { chosen = it }, onWidthPreset)
                NavTabs(listOf(PreviewToolCopy.Components, PreviewToolCopy.Tokens, PreviewToolCopy.Inventory, PreviewToolCopy.Workbench), section, { section = it },
                    groupName = AccessibleName.of(PreviewToolCopy.SectionsName))
                // Keep nested panes bounded when large text makes the outer page scroll.
                val region = if (scrollWholePage) Modifier.height(viewportHeight) else Modifier.weight(1f)
                if (section == 3) {
                    ComponentWorkbench(initialRecipe, systemIsDark, region)
                } else if (section == 1) {
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
                                TextField(PreviewToolCopy.FindComponent, query, { query = it })
                                if (components.isEmpty()) {
                                    EmptyState(PreviewToolCopy.NoResults, PreviewToolCopy.SearchHelp)
                                } else {
                                    Select(PreviewToolCopy.Component, components.map { it.name },
                                        components.indexOfFirst { it.id == selected?.id },
                                        { selectedId = components[it].id },
                                        expandedDescription = PreviewToolCopy.ComponentsOpen,
                                        collapsedDescription = PreviewToolCopy.ComponentsClosed)
                                }
                                ComponentDetail(selected, { event = it })
                            }
                        }
                    }
                }
                Surface(tone = SurfaceTone.Container) {
                    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        FieldLabel(PreviewToolCopy.EventLog)
                        FieldHelp(event)
                    }
                }
            }
        }
    }
}

/**
 * The gallery root: the same theme and background as [UiRoot], plus the families the composition root
 * injected. [UiRoot] stays unchanged so no registered API gains a parameter for a preview-only need;
 * nested specimen themes inherit these families.
 */
@Composable
private fun PreviewRoot(
    preferences: UiPreferences,
    uiFontFamily: FontFamily?,
    editorialFontFamily: FontFamily?,
    content: @Composable () -> Unit,
) {
    CantosTheme(preferences, uiFontFamily, editorialFontFamily) { RootContent(Modifier, content) }
}

/**
 * Live evidence line: the constraint, the host density and font scale, the preview multiplier and the
 * effective scale a capture was actually rendered with (the theme composes host × multiplier).
 */
@Composable
private fun ViewportProbe(width: Dp, height: Dp, host: DisplayDensity, preferences: UiPreferences) {
    val effective = LocalDensity.current.fontScale
    FieldHelp(
        PreviewToolCopy.viewport(width.value.roundToInt(), height.value.roundToInt(), host.density, host.fontScale, preferences.textScale, effective),
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
        FilterChip(PreviewToolCopy.Compact, preferences.density == Density.Compact, { onChange(preferences.copy(density = Density.Compact)) })
        FilterChip(PreviewToolCopy.Comfortable, preferences.density == Density.Comfortable, { onChange(preferences.copy(density = Density.Comfortable)) })
        FilterChip(PreviewToolCopy.ReducedMotion, preferences.reducedMotion, { onChange(preferences.copy(reducedMotion = it)) })
        FilterChip(PreviewToolCopy.StandardExpression, preferences.expression == CantosExpression.Standard,
            { onChange(preferences.copy(expression = CantosExpression.Standard)) })
        FilterChip(PreviewToolCopy.TheatreExpression, preferences.expression == CantosExpression.Theatre,
            { onChange(preferences.copy(expression = CantosExpression.Theatre)) })
        listOf(1f, 1.3f, 2f).forEach { scale ->
            FilterChip(PreviewToolCopy.textScale(scale), preferences.textScale == scale, { onChange(preferences.copy(textScale = scale)) })
        }
        // An inert control would mislead in a host that cannot resize its window (Android previews, test roots).
        onWidthPreset?.let { resize ->
            ViewportPreset.entries.forEach { preset ->
                Button(AccessibleName.of(PreviewToolCopy.setWidth(preset.label)), { resize(preset) }, variant = ButtonVariant.Text) {
                    Text(preset.label)
                }
            }
        }
    }
    FieldHelp(PreviewToolCopy.themeStatus(mode.name, preferences.theme.tokenName, systemIsDark))
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
        TextField(PreviewToolCopy.FindComponent, query, onQuery)
        FieldLabel(PreviewToolCopy.componentCount(components.size))
        FilterChip(PreviewToolCopy.AllFamilies, category == null, { onCategory(null) })
        PreviewCatalog.components.map { it.category }.distinct().forEach { family ->
            FilterChip(family, family == category, { onCategory(if (it) family else null) })
        }
        components.forEach { component ->
            NavItem(component.name, selected == component.id, { onSelect(component.id) })
        }
        if (components.isEmpty()) FieldHelp(PreviewToolCopy.NoResults)
    }
}

@Composable
private fun ComponentDetail(component: PreviewComponent?, onEvent: (String) -> Unit, modifier: Modifier = Modifier) {
    if (component == null) {
        EmptyState(PreviewToolCopy.ChooseComponent, PreviewToolCopy.ChooseComponentHelp, modifier)
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
            FieldHelp(PreviewToolCopy.FocusHelp)
            Surface(Modifier.fillMaxWidth(), tone = SurfaceTone.Surface) {
                val example = PreviewExample.fromId(component.id)
                if (example == null || component.cmpStatus == "unsupported") {
                    ErrorState(PreviewToolCopy.Unsupported, component.cmpReason ?: PreviewToolCopy.NoNativeExample, PreviewToolCopy.Retry, {})
                } else {
                    ComponentExample(example, state, onEvent, Modifier.focusRequester(focusRequester))
                    if (state == "focus") {
                        LaunchedEffect(component.id, state) { focusRequester.requestFocus() }
                    }
                }
            }
            if (component.id == "grid" || component.id == "container") {
                FieldHelp(PreviewToolCopy.WidthHelp)
            }
            if (component.id == "error-state" && state == "retry") {
                FieldHelp(PreviewToolCopy.RetryHelp)
            }
            ParityNotes(component.id)
        }
    }
}

@Composable
private fun ParityNotes(id: String) {
    val note = when (id) {
        "link" -> PreviewToolCopy.LinkParity
        "number-field" -> PreviewToolCopy.NumberFieldParity
        "field-label" -> PreviewToolCopy.FieldLabelParity
        "nav-tabs" -> PreviewToolCopy.NavTabsParity
        "select" -> PreviewToolCopy.SelectParity
        "dialog" -> PreviewToolCopy.DialogParity
        "loading-state" -> PreviewToolCopy.LoadingParity
        "ui-root" -> PreviewToolCopy.UiRootParity
        else -> PreviewToolCopy.CommonParity
    }
    FieldHelp(note)
}

@Composable
internal fun ComponentExample(
    example: PreviewExample,
    state: String,
    onEvent: (String) -> Unit,
    modifier: Modifier,
    sampleState: ComponentSampleState? = null,
    onSampleIntent: ((ComponentSampleIntent) -> Unit)? = null,
) {
    // Availability presets change the control contract; existing gallery edits and callback counts
    // outlive that switch. A workbench recipe reset is explicit and supplies its own complete value.
    var retainedSample by remember(example) { mutableStateOf(ComponentSampleState.initial(example, state)) }
    var localSample by remember(example, state) {
        mutableStateOf(ComponentSampleState.initial(example, state).copy(
            title = retainedSample.title, dialogue = retainedSample.dialogue, number = retainedSample.number,
            presses = retainedSample.presses, dialogNote = retainedSample.dialogNote,
        ))
    }
    val sample = sampleState ?: localSample
    val dispatch: (ComponentSampleIntent) -> Unit = { intent ->
        if (onSampleIntent != null) onSampleIntent(intent)
        else when (val next = reduceComponentSample(localSample, intent)) {
            is ComponentSampleResult.Accepted -> { localSample = next.state; retainedSample = next.state }
            is ComponentSampleResult.Rejected -> onEvent("Rejected sample intent: ${next.error}")
        }
    }
    if (state.startsWith(TheatreStatePrefix)) {
        TheatreSpecimen(example, state, onEvent, modifier)
        return
    }
    val richText = TextFieldValue(sample.richText, TextRange(sample.selectionStart, sample.selectionEnd),
        sample.compositionStart?.let { TextRange(it, sample.compositionEnd ?: it) })
    val setRichText: (TextFieldValue) -> Unit = { value ->
        dispatch(ComponentSampleIntent.SetRichText(value.text, value.selection.start, value.selection.end,
            value.composition?.start, value.composition?.end))
    }
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
    val onPress = { dispatch(ComponentSampleIntent.Press); onEvent("${example.id} · callback #${sample.presses + 1} · không có yêu cầu mạng") }
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
                        Text(vietnameseSpecimenText("Ánh đèn cuối sân khấu"), style = MaterialTheme.typography.headlineSmall)
                        Text(vietnameseSpecimenText("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không? · Ỗ, Ữ"), style = typeStyle("editor", editorial = true))
                        Button("Lưu bản nháp", onPress)
                        if (state == "nested-theme") {
                            UiRoot(
                                Modifier.height(PreviewGeometry.nestedSampleRoot).fillMaxWidth(),
                                preferences = nested.copy(theme = Theme.Dark),
                            ) {
                                Container {
                                    Text(PreviewToolCopy.NestedThemeHelp)
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
        PreviewExample.Button -> if (state.startsWith("listen-")) {
            CantosTheme(LocalUiPreferences.current.copy(expression = CantosExpression.Theatre)) {
                ListenHeroSample(state, onPress, modifier)
            }
        } else if (state in listOf("auth-primary", "auth-secondary", "auth-outlined", "pressed", "long-vietnamese", "large-text")) {
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
                            Text(vietnameseSpecimenText("Lưu · ${variant.name}"))
                        }
                    } else {
                        Button("Lưu · ${variant.name}", onPress, state = buttonState, variant = variant, icon = "save",
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
                    Text(vietnameseSpecimenText("Thêm cảnh · Ỗ, Ữ"))
                }
            }
            FieldHelp("Callback count: ${sample.presses}")
        }
        PreviewExample.IconButton -> IconButton(AccessibleName.of("Thêm cảnh cho Ánh đèn cuối sân khấu"), "add", onPress,
            state = buttonState, controlModifier = modifier)
        PreviewExample.FilterChip -> FilterChip("Kịch nói · tiếng Việt", sample.checked, { dispatch(ComponentSampleIntent.SetChecked(it)); onEvent("Selected: $it") },
            state = buttonState, controlModifier = modifier)
        PreviewExample.Link -> Link("Mở bản nháp Ánh đèn cuối sân khấu", "/draft/example", {
            dispatch(ComponentSampleIntent.Press); onEvent("Link callback: $it")
        }, state = buttonState, controlModifier = modifier)
        PreviewExample.NavItem -> NavItem("Tác phẩm của tôi", sample.checked, { dispatch(ComponentSampleIntent.SetChecked(true)); onEvent("NavItem selected") }, state = buttonState, controlModifier = modifier)
        PreviewExample.NavTabs -> NavTabs(
            if (state == "overflow") listOf("Kịch bản Ánh đèn cuối sân khấu", "Nguyễn Thị Minh Phương", "Lịch sử chỉnh sửa và ghi chú phát âm", "Bản nháp chưa xuất bản")
            else listOf("Kịch bản", "Nhân vật", "Lịch sử"),
            sample.tab, { dispatch(ComponentSampleIntent.SetTab(it)); onEvent("Tab: $it") }, modifier,
            groupName = AccessibleName.of("Các phần bản nháp"), state = buttonState)
        PreviewExample.TextField -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (state in listOf("auth-tonal", "long-vietnamese", "large-text")) {
                val preferences = LocalUiPreferences.current
                CantosTheme(preferences.copy(textScale = if (state == "large-text") 2f else preferences.textScale)) {
                    TextField("Tên hiển thị của Nguyễn Thị Minh Phương · Ỗ, Ữ", richText, { setRichText(it) },
                        help = "Nội dung chỉ là minh họa; không mở form mật khẩu hay gửi thông tin.",
                        variant = TextFieldVariant.AuthTonal, controlModifier = modifier)
                }
            } else if (state == "selection" || state == "ime-action") {
                TextField("Tên tác phẩm", richText, { setRichText(it) }, modifier,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { dispatch(ComponentSampleIntent.Press); onEvent("TextField · Done") }))
                FieldHelp("Selection: ${richText.selection.start}..${richText.selection.end}; composition: ${richText.composition}")
            } else {
                TextField("Tên tác phẩm", sample.title, { dispatch(ComponentSampleIntent.SetTitle(it)); onEvent("TextField: value updated") }, modifier, fieldState,
                    "Tên có dấu được giữ nguyên, không chuẩn hóa khi gõ.", error)
                TextField("Tên hiển thị minh họa", sample.title, { dispatch(ComponentSampleIntent.SetTitle(it)); onEvent("Auth TextField: value updated") },
                    state = fieldState, help = "Giữ nguyên nội dung đã nhập và dấu tiếng Việt.", error = error,
                    variant = TextFieldVariant.AuthTonal)
            }
        }
        PreviewExample.TextArea -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (state == "selection" || state == "ime-action") {
                TextArea("Lời thoại", richText, { setRichText(it) }, modifier,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { dispatch(ComponentSampleIntent.Press); onEvent("TextArea · Done") }),
                    minLines = 2, maxLines = 6)
                FieldHelp("Selection: ${richText.selection.start}..${richText.selection.end}; composition: ${richText.composition}")
            } else {
                TextArea("Lời thoại", sample.dialogue, { dispatch(ComponentSampleIntent.SetDialogue(it)); onEvent("TextArea: value updated") }, modifier, fieldState,
                    "Hãy thử chọn, sao chép và nhập tiếng Việt có dấu.", error)
            }
        }
        PreviewExample.NumberField -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (state == "selection" || state == "ime-action") {
                NumberField("Tốc độ tham khảo", richText, { setRichText(it) }, modifier,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { dispatch(ComponentSampleIntent.Press); onEvent("NumberField · Done") }))
                FieldHelp("Selection: ${richText.selection.start}..${richText.selection.end}; raw text: ${richText.text}")
            } else {
                NumberField("Tốc độ tham khảo", sample.number, { dispatch(ComponentSampleIntent.SetNumber(it)); onEvent("NumberField raw text: $it") }, modifier, fieldState,
                    "Dữ liệu thử: 1,25. Chủ ứng dụng quyết định quy tắc phân tích số.", error)
            }
        }
        PreviewExample.Select -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            val options = if (state == "empty-options") emptyList() else listOf(
                SelectOption("narrator", SyntheticPreviewText.narratorVi, icon = "mic"),
                SelectOption("minh-phuong", SyntheticPreviewText.characterNameVi, icon = "person"),
                SelectOption("unavailable", "Người giữ đèn cuối sân khấu", ButtonState.Disabled(DisabledReason.of("Chưa có quyền dùng giọng này"))),
            ).let { if (sample.reverseOptions) it.reversed() else it }
            val availability = if (sample.interrupted) ButtonState.Loading(pendingReason) else buttonState
            Select("Nhân vật", options, sample.stableChoice,
                { dispatch(ComponentSampleIntent.SetStableChoice(it)); onEvent("Select stable value: $it") }, state = availability,
                expandedDescription = "Danh sách nhân vật đang mở",
                collapsedDescription = "Danh sách nhân vật đang đóng",
                placeholder = "Chọn nhân vật", emptyDescription = "Chưa có nhân vật. Hãy thêm nhân vật để tiếp tục.",
                controlModifier = modifier, error = error)
            if (state == "reordered-options") Button("Đảo thứ tự nhân vật", { dispatch(ComponentSampleIntent.ReverseOptions) })
            if (state == "interrupted") Switch("Tạm ngừng lựa chọn", sample.interrupted, { dispatch(ComponentSampleIntent.SetInterrupted(it)) })
            FieldHelp("Stable value: ${sample.stableChoice}")
        }
        PreviewExample.Checkbox -> Checkbox("Giữ nguyên ghi chú phát âm tiếng Việt", sample.checked,
            { dispatch(ComponentSampleIntent.SetChecked(it)); onEvent("Checkbox: $it") }, state = buttonState, controlModifier = modifier,
            help = if (state == "label-activation") "Nhấn cả nhãn hoặc Space để đổi lựa chọn." else null, error = error)
        PreviewExample.Radio -> Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Radio("Bản nháp của Nguyễn Thị Minh Phương", sample.checked,
                { dispatch(ComponentSampleIntent.SetChecked(true)); onEvent("Radio: draft selected") }, state = buttonState,
                controlModifier = modifier, error = error)
            Radio("Bản nháp của người dẫn chuyện", !sample.checked,
                { dispatch(ComponentSampleIntent.SetChecked(false)); onEvent("Radio: narration selected") }, state = buttonState)
        }
        PreviewExample.Switch -> Switch("Hiện ghi chú người dẫn chuyện", sample.checked,
            { dispatch(ComponentSampleIntent.SetChecked(it)); onEvent("Switch: $it") }, state = buttonState, controlModifier = modifier, error = error)
        PreviewExample.RangeField -> RangeField("Mức âm lượng thử", sample.range,
            { dispatch(ComponentSampleIntent.SetRange(it)); onEvent("RangeField: $it") }, state = buttonState,
            steps = if (state == "stepped") 3 else 0,
            valueText = "${sample.range.toString().replace('.', ',')} trên 100", controlModifier = modifier, error = error,
            onValueChangeFinished = { onEvent("RangeField completed: ${sample.range}") },
            help = "Dùng phím mũi tên, Home/End hoặc kéo. Đây là dữ liệu thử.")
        PreviewExample.FieldLabel -> Column(modifier) {
            FieldLabel("Tên tác phẩm · nhãn được lồng trong control")
            TextField("Tên tác phẩm", sample.title, { dispatch(ComponentSampleIntent.SetTitle(it)) }, state = fieldState, error = error)
        }
        PreviewExample.FieldHelp -> Column(modifier) {
            FieldHelp(if (state == "empty-hidden") "" else "Không đổi tên nhân vật hoặc dấu câu khi nhập văn bản.")
            TextField("Tên tác phẩm", sample.title, { dispatch(ComponentSampleIntent.SetTitle(it)) }, state = fieldState, help = "Người dẫn chuyện · Ỗ, Ữ", error = error)
        }
        PreviewExample.FieldError -> Column(modifier) {
            FieldError(if (state == "empty-hidden") "" else "Chưa thể lưu: cần chọn phiên bản kịch bản.")
            TextField("Tên tác phẩm", sample.title, { dispatch(ComponentSampleIntent.SetTitle(it)) }, state = fieldState, error = error)
        }
        PreviewExample.Badge -> Badge(feedback, modifier, tone)
        PreviewExample.Status -> Status(feedback, modifier, tone, announce = state == "announcement")
        PreviewExample.LoadingState -> LoadingState("Đang nạp danh sách bản nháp. Lựa chọn hiện tại vẫn được giữ lại.", modifier,
            announcement = if (state == "announcement") Announcement.Polite else Announcement.Off)
        PreviewExample.EdgeProgress -> EdgeProgressSamples(state, modifier) { active, visible, foreground, name, edgeModifier ->
            EdgeProgress(active, visible, foreground, name, edgeModifier, Announcement.Polite)
        }
        PreviewExample.SurfacePulse -> {
            val stage = PulseStage.fromState(state)
            Column(modifier.pulseRegionSemantics(stage), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                PulseRow(stage, "Ánh đèn cuối sân khấu", "Người dẫn chuyện · 12 chương", placeholderTitleLines = 1)
                PulseRow(stage, "Những người kể chuyện đêm khuya ở phố cổ Hội An — phần Ỗ, Ữ kéo dài",
                    "Nguyễn Thị Minh Phương · Bản nháp", placeholderTitleLines = 2, compact = true)
                PulseSlot(stage, Modifier.fillMaxWidth(),
                    placeholder = { SurfacePulse(Modifier.fillMaxWidth().height(tokenDp("size.control")), stage == PulseStage.Running, CantosShapes.full) },
                    loaded = { PulseBarLoaded("Thư viện của bạn") })
                PulseRow(stage, "Midnight storytellers of the old quarter, a long unbroken title", "Narrator · 8 chapters",
                    placeholderTitleLines = 2, language = "en")
            }
        }
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
                Text(vietnameseSpecimenText(if (state == "long-text") List(4) { text }.joinToString("\n\n") else text),
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
                if (state == "long-text") Text(vietnameseSpecimenText("Ngày mai, mình có diễn tiếp không? Nguyễn Thị Minh Phương giữ nguyên ghi chú phát âm và lời thoại tiếng Việt dài, không thu nhỏ chữ để ép nội dung vào một hàng."))
            }
        }
        PreviewExample.Grid -> Grid(listOf(
            { Surface { Text(vietnameseSpecimenText("Ánh đèn cuối sân khấu · cảnh một")) } },
            { Surface(tone = SurfaceTone.Container) { Text(vietnameseSpecimenText("Ngày mai, mình có diễn tiếp không? · cảnh hai")) } },
            { Surface { Text(vietnameseSpecimenText("Nguyễn Thị Minh Phương · lời thoại dài có dấu tiếng Việt")) } },
        ), if (state == "narrow") modifier.widthIn(max = PreviewGeometry.narrowSampleMaxWidth) else modifier,
            requestedColumns = when (state) { "one-column", "narrow" -> 1; "three-columns", "wide" -> 3; else -> 2 },
            gap = if (state == "gap-lg") Space.Lg else Space.Md)
        PreviewExample.Container -> Container(if (state == "narrow") modifier.widthIn(max = PreviewGeometry.narrowSampleMaxWidth) else modifier, width = when (state) {
            "wide" -> ContainerWidth.Wide; "fluid" -> ContainerWidth.Fluid; else -> ContainerWidth.Reading
        }, inset = if (state == "inset-none") Space.None else Space.Md) {
            Text(vietnameseSpecimenText("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không? Ánh đèn cuối sân khấu còn sáng; bản nháp này dùng để thử xuống dòng và tăng cỡ chữ."))
        }
        PreviewExample.Dialog -> DialogExample(sample, dispatch, onEvent, modifier)
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
    if (state == "sizes") {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalAlignment = Alignment.CenterVertically) {
                AvatarSize.entries.forEach { Avatar("Lan", size = it) }
            }
            FieldHelp(SyntheticPreviewText.narratorVi)
        }
        return
    }
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        verticalAlignment = Alignment.CenterVertically) {
        when (state) {
            "generated-alias" -> Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                for ((label, tone) in listOf("A · Lavender Monogram" to com.cantos.ui.AvatarTone.Lavender,
                    "B · Golden Editorial" to com.cantos.ui.AvatarTone.Gold, "C · Personal Tonal" to null)) {
                    FieldLabel(label)
                    Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalAlignment = Alignment.CenterVertically) {
                        for (size in listOf(AvatarSize.Large, AvatarSize.Control)) Avatar("Mây Lam",
                            stableId = "6e45675a6ee0e0dd5a16f55c6fcaafd1f8737a27c0572ffc39a2a1bd22100ecb",
                            initials = AvatarInitials.Pair, size = size, tone = tone)
                    }
                }
            }
            "initials-pair" -> Avatar("Người dẫn chuyện", initials = AvatarInitials.Pair)
            "no-initial" -> Avatar("😀")
            "image" -> Avatar("Đỗ Văn An", image = image)
            // The host passes null after a failed decode; initials return immediately.
            "image-failed" -> Avatar("Đỗ Văn An", image = null)
            "named" -> Avatar("岑 昕", initials = AvatarInitials.Pair,
                accessibleName = AccessibleName.of("Ảnh đại diện của 岑 昕"))
            else -> Avatar("Nguyễn Thị Ánh", stableId = "user-42")
        }
        if (state != "named") FieldHelp("Nguyễn Thị Ánh · Người dẫn chuyện")
    }
}

@Composable
private fun DialogExample(sample: ComponentSampleState, dispatch: (ComponentSampleIntent) -> Unit,
    onEvent: (String) -> Unit, modifier: Modifier) {
    val triggerFocus = remember { FocusRequester() }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Button("Mở thông tin bản nháp", { dispatch(ComponentSampleIntent.SetDialogOpen(true)); onEvent("Dialog opened") },
            controlModifier = Modifier.focusRequester(triggerFocus))
        FieldHelp(PreviewToolCopy.DialogFocusHelp)
        Dialog(sample.dialogOpen, "Ánh đèn cuối sân khấu · bản nháp", "Đóng", {
            dispatch(ComponentSampleIntent.SetDialogOpen(false))
            onEvent("Dialog dismissed; requested trigger focus restoration")
        }, returnFocus = triggerFocus) {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(vietnameseSpecimenText("Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?"))
                TextField("Ghi chú bản nháp", sample.dialogNote, { dispatch(ComponentSampleIntent.SetDialogNote(it)) })
            }
        }
        FieldHelp("Ghi chú được giữ: ${sample.dialogNote}")
    }
}

@Composable
private fun TokenCatalog(preferences: UiPreferences, modifier: Modifier) {
    var query by remember { mutableStateOf("") }
    val entries = CantosTokens.entries.filter { it.name.contains(query.trim(), ignoreCase = true) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        TextField(PreviewToolCopy.FindToken, query, { query = it })
        FieldHelp(PreviewToolCopy.tokenCount(entries.size, preferences.theme.tokenName, preferences.density.tokenName))
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
                                entry.aliasOf?.let { FieldHelp(PreviewToolCopy.alias(it)) }
                            }
                        }
                    }
                }
                if (entries.isEmpty()) EmptyState(PreviewToolCopy.NoTokens, PreviewToolCopy.TokenSearchHelp)
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
                Text(vietnameseSpecimenText(reading), style = typeStyle(mappedRole, editorial = editorial))
                FieldHelp(PreviewToolCopy.fontFallback(editorial))
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
                if (amount > bound) FieldHelp(PreviewToolCopy.ClippedSample)
            }
        }
        entry.name.startsWith("state.") -> Box(
            Modifier.size(tokenDp("size.control"))
                .background(tokenColor("color.primary").copy(alpha = CantosTokens.number(entry.name).toFloat())),
        )
        entry.kind == "duration" || entry.kind == "cubicBezier" -> FieldHelp(PreviewToolCopy.MotionNotDemonstrated)
    }
}
