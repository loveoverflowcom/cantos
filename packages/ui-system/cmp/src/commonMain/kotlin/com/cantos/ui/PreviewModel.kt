package com.cantos.ui

import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.generated.PreviewComponent

/** Explicit inventory: a registry addition needs a real example before it can pass parity. */
enum class PreviewExample(val id: String) {
    UiRoot("ui-root"),
    Button("button"),
    IconButton("icon-button"),
    FilterChip("filter-chip"),
    Link("link"),
    NavItem("nav-item"),
    NavTabs("nav-tabs"),
    TextField("text-field"),
    TextArea("text-area"),
    NumberField("number-field"),
    Select("select"),
    Checkbox("checkbox"),
    Radio("radio"),
    Switch("switch"),
    RangeField("range-field"),
    FieldLabel("field-label"),
    FieldHelp("field-help"),
    FieldError("field-error"),
    Badge("badge"),
    Status("status"),
    LoadingState("loading-state"),
    SurfacePulse("surface-pulse"),
    EmptyState("empty-state"),
    ErrorState("error-state"),
    Surface("surface"),
    Stack("stack"),
    Grid("grid"),
    Container("container"),
    Dialog("dialog"),
    Icon("icon"),
    Avatar("avatar"),
    BackButton("back-button"),
    Logo("logo");

    companion object {
        fun fromId(id: String): PreviewExample? = entries.firstOrNull { it.id == id }
    }
}

fun filterComponents(query: String, category: String?): List<PreviewComponent> {
    val normalized = query.trim().lowercase()
    return PreviewCatalog.components.filter { component ->
        (category == null || component.category == category) &&
            (normalized.isEmpty() || component.name.lowercase().contains(normalized) ||
                component.id.contains(normalized) || component.category.contains(normalized))
    }
}

/** One displayed selection for the picker and detail; filtering does not overwrite the preference. */
fun resolvePreviewSelection(components: List<PreviewComponent>, preferredId: String): PreviewComponent? =
    components.firstOrNull { it.id == preferredId } ?: components.firstOrNull()

/** Read-only HTML is not a supported native choice-control mode. */
fun nativeChoiceState(state: FieldState): ButtonState = when (state) {
    FieldState.Editable -> ButtonState.Enabled
    FieldState.ReadOnly -> ButtonState.Disabled(DisabledReason.of("Read-only is not a native choice mode"))
    is FieldState.Disabled -> ButtonState.Disabled(state.reason)
    is FieldState.Loading -> ButtonState.Loading(state.reason)
}
