package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ProvideTextStyle
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

@Composable
internal fun Space.value(): Dp = when (this) {
    Space.None -> 0.dp
    Space.Xs -> tokenDp("space.xs")
    Space.Sm -> tokenDp("space.sm")
    Space.Md -> tokenDp("space.md")
    Space.Lg -> tokenDp("space.lg")
}

@Composable
fun Surface(
    modifier: Modifier = Modifier,
    tone: SurfaceTone = SurfaceTone.Surface,
    inset: Space = Space.Md,
    content: @Composable () -> Unit,
) {
    val role = when (tone) {
        SurfaceTone.Surface, SurfaceTone.Reading -> "color.surface"
        SurfaceTone.Container -> "color.surfaceContainer"
        SurfaceTone.Raised -> "color.surfaceContainerHigh"
    }
    androidx.compose.material3.Surface(modifier, color = tokenColor(role), contentColor = tokenColor("color.onSurface"),
        shape = if (tone == SurfaceTone.Reading) CantosShapes.reading else CantosShapes.container) {
        Box(Modifier.padding(inset.value())) {
            if (tone == SurfaceTone.Reading) ProvideTextStyle(typeStyle("editor", editorial = true), content)
            else content()
        }
    }
}

/** Horizontal groups reflow at intrinsic child size; Wrap remains a compatible alias. */
@Composable
fun Stack(
    modifier: Modifier = Modifier,
    direction: StackDirection = StackDirection.Vertical,
    gap: Space = if (direction == StackDirection.Vertical) Space.Md else Space.Sm,
    content: @Composable () -> Unit,
) {
    when (direction) {
        StackDirection.Vertical -> Column(modifier, verticalArrangement = Arrangement.spacedBy(gap.value())) { content() }
        StackDirection.Horizontal, StackDirection.Wrap -> FlowRow(modifier,
            horizontalArrangement = Arrangement.spacedBy(gap.value()),
            verticalArrangement = Arrangement.spacedBy(gap.value())) { content() }
    }
}

/** Reflow child groups rather than shrinking text or native controls. */
@Composable
fun Grid(
    items: List<@Composable () -> Unit>,
    modifier: Modifier = Modifier,
    requestedColumns: Int = 2,
    gap: Space = Space.Md,
) {
    require(requestedColumns in 1..3)
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val capacity = when {
            maxWidth >= tokenDp("breakpoint.expanded") -> 3
            maxWidth >= tokenDp("breakpoint.medium") -> 2
            else -> 1
        }
        val columns = minOf(capacity, requestedColumns)
        Column(verticalArrangement = Arrangement.spacedBy(gap.value())) {
            items.chunked(columns).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(gap.value())) {
                    row.forEach { child -> Box(Modifier.weight(1f)) { child() } }
                    repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
fun Container(
    modifier: Modifier = Modifier,
    width: ContainerWidth = ContainerWidth.Reading,
    inset: Space = Space.Md,
    content: @Composable () -> Unit,
) {
    val limit = when (width) {
        ContainerWidth.Reading -> Modifier.widthIn(max = tokenDp("breakpoint.expanded"))
        ContainerWidth.Wide -> Modifier.widthIn(max = tokenDp("breakpoint.expanded") + tokenDp("breakpoint.medium") / 2)
        ContainerWidth.Fluid -> Modifier
    }
    Box(modifier.then(limit).fillMaxWidth().padding(inset.value())) { content() }
}
