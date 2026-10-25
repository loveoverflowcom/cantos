package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip as MaterialFilterChip
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.window.DialogProperties

/** Native URI dispatch only; routing/history and platform opening stay caller-owned. */
@Composable
fun Link(
    label: String,
    href: String,
    onOpen: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
) {
    require(label.isNotBlank())
    require(supportedLink(href)) { "Only a local route, fragment or HTTPS URI without whitespace is supported" }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        TextButton(onClick = { if (state.canPress()) onOpen(href) }, enabled = state.canPress(),
            modifier = controlModifier.cantosFocus().actionState(state)) { Text(label) }
        state.reason()?.let { FieldHelp(it) }
    }
}

internal fun supportedLink(href: String): Boolean {
    if (href.isBlank() || href.any { it.isWhitespace() || it.code < 32 || it == '\\' }) return false
    if (href.startsWith("#")) return href.length > 1
    if (href.startsWith("/")) return !href.startsWith("//")
    if (!href.startsWith("https://")) return false
    return href.removePrefix("https://").takeWhile { it !in "/?#" }.isNotBlank()
}

@Composable
fun NavItem(
    label: String,
    selected: Boolean,
    onSelect: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
) {
    require(label.isNotBlank())
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        MaterialFilterChip(selected = selected, onClick = { if (state.canPress()) onSelect() },
            enabled = state.canPress(), label = { Text(label) },
            modifier = controlModifier.heightIn(min = tokenDp("size.control")).cantosFocus().actionState(state))
        state.reason()?.let { FieldHelp(it) }
    }
}

/** In-place native tabs, unlike Leptos navigation links. Selection is a callback request. */
@Composable
fun NavTabs(
    labels: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    groupName: AccessibleName? = null,
    state: ButtonState = ButtonState.Enabled,
) {
    require(labels.isNotEmpty() && labels.all { it.isNotBlank() } && selected in labels.indices)
    val focus = remember(labels.toList()) { labels.map { FocusRequester() } }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val tabWidth = (maxWidth - tokenDp("space.xs") * 2).coerceAtLeast(tokenDp("size.control"))
            FlowRow(Modifier.selectableGroup().semantics { groupName?.let { contentDescription = it.text } },
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                labels.forEachIndexed { index, label ->
                    Tab(selected = index == selected, enabled = state.canPress(),
                        onClick = { if (state.canPress()) onSelect(index) },
                        modifier = Modifier.widthIn(max = tabWidth).width(IntrinsicSize.Max).heightIn(min = tokenDp("size.control"))
                            .focusRequester(focus[index]).cantosFocus().actionState(state)
                            .onKeyEvent { event ->
                                val next = tabKeyTarget(index, labels.size, event.key)
                                if (state.canPress() && event.type == KeyEventType.KeyDown && next != null) {
                                    focus[next].requestFocus()
                                    onSelect(next)
                                    true
                                } else false
                            }) {
                        Text(label, Modifier.padding(horizontal = tokenDp("space.sm"), vertical = tokenDp("space.inset")),
                            style = MaterialTheme.typography.labelLarge)
                        Box(Modifier.fillMaxWidth().height(tokenDp("size.border"))
                            .background(tokenColor(if (index == selected) "color.primary" else "color.surface")))
                    }
                }
            }
        }
        state.reason()?.let { FieldHelp(it) }
    }
}

internal fun tabKeyTarget(index: Int, count: Int, key: Key): Int? = when (key) {
    Key.DirectionLeft -> (index - 1).coerceAtLeast(0)
    Key.DirectionRight -> (index + 1).coerceAtMost(count - 1)
    Key.MoveHome -> 0
    Key.MoveEnd -> count - 1
    else -> null
}

/** Keep mounted across close so caller-provided return focus can run. No input is owned here.
 * Initial focus must refer to a mounted focusable descendant; the default is the close action.
 * Native dialog properties own Escape/Back and outside dismissal; default outside clicks stay open.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Dialog(
    open: Boolean,
    title: String,
    closeLabel: String,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    initialFocus: FocusRequester? = null,
    returnFocus: FocusRequester? = null,
    properties: DialogProperties = DialogProperties(dismissOnClickOutside = false),
    content: @Composable () -> Unit,
) {
    require(title.isNotBlank() && closeLabel.isNotBlank())
    val closeFocus = remember { FocusRequester() }
    val inheritedBringIntoView = LocalBringIntoViewSpec.current
    val focusClearance = tokenDp("size.focusOffset") + tokenDp("size.focusWidth") + tokenDp("space.xs")
    val focusClearancePx = with(LocalDensity.current) { focusClearance.toPx() }
    val modalBringIntoView = remember(inheritedBringIntoView, focusClearancePx) {
        object : BringIntoViewSpec {
            override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float =
                inheritedBringIntoView.calculateScrollDistance(
                    offset - focusClearancePx, size + 2 * focusClearancePx, containerSize,
                )
        }
    }
    var wasOpen by remember { mutableStateOf(false) }
    LaunchedEffect(open) {
        if (!open && wasOpen) returnFocus?.requestFocus()
        wasOpen = open
    }
    if (open) {
        AlertDialog(onDismissRequest = { if (open) onDismiss() },
            modifier = modifier.semantics { paneTitle = title },
            // Keep the close action outside the bounded scrolling region. A fixed title can
            // consume the entire modal at large text before Material measures its text/actions.
            title = null,
            text = {
                CompositionLocalProvider(LocalBringIntoViewSpec provides modalBringIntoView) {
                    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = focusClearance),
                        verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                        Text(title, Modifier.semantics { heading() },
                            color = tokenColor("color.onSurface"), style = MaterialTheme.typography.headlineSmall)
                        // Only this modal's vertical viewport expands a focus request. Preserve
                        // platform behavior for nested editors/scroll containers supplied by hosts.
                        CompositionLocalProvider(LocalBringIntoViewSpec provides inheritedBringIntoView) {
                            Box { content() }
                        }
                    }
                }
            },
            confirmButton = {
                Button(closeLabel, { if (open) onDismiss() }, variant = ButtonVariant.Text,
                    controlModifier = Modifier.focusRequester(closeFocus))
                LaunchedEffect(Unit) { (initialFocus ?: closeFocus).requestFocus() }
            }, properties = properties,
            containerColor = tokenColor("color.surface"),
            shape = CantosShapes.overlay)
    }
}
