package com.cantos.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.cantos.ui.AccessibleName
import com.cantos.ui.Badge
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.EmptyState
import com.cantos.ui.FieldHelp
import com.cantos.ui.FieldLabel
import com.cantos.ui.FilterChip
import com.cantos.ui.NavItem
import com.cantos.ui.Select
import com.cantos.ui.SelectOption
import com.cantos.ui.StatusTone
import com.cantos.ui.TextField
import com.cantos.ui.preview.generated.DesignInventory
import com.cantos.ui.preview.generated.DesignSpecimen
import com.cantos.ui.tokenDp

/**
 * Read-only map of the design export onto the shared registry. It samples nothing: a proposal has no
 * native example, is not a public API and is never qualified here, and an existing specimen defers to
 * the Components tab for its actual states. Design status and ownership scope are shown apart from
 * registration and qualification on purpose.
 */
@Composable
internal fun DesignInventoryView(
    onOpenComponent: (String) -> Unit,
    modifier: Modifier = Modifier,
    expanded: Boolean = false,
    /** True when the page already scrolls and this region has a fixed height (narrow window or enlarged text). */
    scrollPage: Boolean = !expanded,
    initialSelectedId: String? = null,
) {
    var filter by remember { mutableStateOf(InventoryFilter()) }
    var selectedId by remember { mutableStateOf(initialSelectedId) }
    val counts = remember { inventoryCounts() }
    val specimens = filterInventory(DesignInventory.specimens, filter)
    val selected = resolveInventorySelection(specimens, selectedId)
    val select = { id: String -> selectedId = id }
    if (scrollPage) {
        // A fixed-height region: header and filters grow with text scale, so everything shares one scroll
        // column. A weighted body would collapse to nothing here and could not be reached by any scroll.
        Column(modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            InventoryHeader(counts)
            InventoryFilters(filter, counts, { filter = it })
            FieldLabel(PreviewToolCopy.specimenCount(specimens.size, counts.total))
            if (expanded) {
                InventoryPanes(specimens, selected?.id, select, onOpenComponent, Modifier.fillMaxWidth().height(PreviewGeometry.twoPaneBody))
            } else {
                InventoryStack(specimens, selected, select, onOpenComponent)
            }
        }
    } else {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            InventoryHeader(counts)
            InventoryFilters(filter, counts, { filter = it })
            FieldLabel(PreviewToolCopy.specimenCount(specimens.size, counts.total))
            InventoryPanes(specimens, selected?.id, select, onOpenComponent, Modifier.fillMaxWidth().weight(1f))
        }
    }
}

/** Two panes, each scrolling on its own: the specimen list beside the selected specimen's detail. */
@Composable
private fun InventoryPanes(
    specimens: List<DesignSpecimen>,
    selectedId: String?,
    onSelect: (String) -> Unit,
    onOpenComponent: (String) -> Unit,
    modifier: Modifier,
) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        Column(Modifier.width(PreviewGeometry.masterPane).verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            specimens.forEach { specimen ->
                NavItem("${specimen.name} · ${specimen.id}", selectedId == specimen.id, { onSelect(specimen.id) })
            }
        }
        SpecimenDetail(specimens.firstOrNull { it.id == selectedId }, onOpenComponent,
            Modifier.weight(1f).verticalScroll(rememberScrollState()))
    }
}

/** One column for narrow windows: a specimen picker above the detail. */
@Composable
private fun InventoryStack(
    specimens: List<DesignSpecimen>,
    selected: DesignSpecimen?,
    onSelect: (String) -> Unit,
    onOpenComponent: (String) -> Unit,
) {
    Select(PreviewToolCopy.Specimen, specimens.map { SelectOption(it.id, "${it.name} · ${it.id}") }, selected?.id, onSelect,
        expandedDescription = PreviewToolCopy.SpecimensOpen,
        collapsedDescription = PreviewToolCopy.SpecimensClosed,
        placeholder = PreviewToolCopy.ChooseSpecimen,
        emptyDescription = PreviewToolCopy.NoMatchingSpecimen)
    SpecimenDetail(selected, onOpenComponent)
}

@Composable
private fun InventoryHeader(counts: InventoryCounts) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Text(PreviewToolCopy.InventoryTitle, style = MaterialTheme.typography.titleLarge)
        FieldHelp(
            PreviewToolCopy.inventorySource(DesignInventory.version, DesignInventory.sourceSha256),
        )
        FieldHelp(
            PreviewToolCopy.inventoryCounts(counts.total, counts.existing, counts.proposed),
        )
    }
}

@Composable
private fun InventoryFilters(filter: InventoryFilter, counts: InventoryCounts, onChange: (InventoryFilter) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        TextField(PreviewToolCopy.FindSpecimen, filter.query, { onChange(filter.copy(query = it)) })
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            FilterChip(PreviewToolCopy.existingCount(counts.existing), filter.designStatus == "existing",
                { onChange(filter.copy(designStatus = if (it) "existing" else null)) })
            FilterChip(PreviewToolCopy.proposedCount(counts.proposed), filter.designStatus == "proposed",
                { onChange(filter.copy(designStatus = if (it) "proposed" else null)) })
            FilterChip(PreviewToolCopy.cmpCount(counts.cmpApplicable), filter.cmpOnly, { onChange(filter.copy(cmpOnly = it)) })
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            InventoryScopeOrder.forEach { scope ->
                FilterChip("$scope ${counts.byScope[scope] ?: 0}", filter.scope == scope,
                    { onChange(filter.copy(scope = if (it) scope else null)) })
            }
        }
    }
}

@Composable
private fun SpecimenDetail(specimen: DesignSpecimen?, onOpenComponent: (String) -> Unit, modifier: Modifier = Modifier) {
    if (specimen == null) {
        EmptyState(PreviewToolCopy.NoSpecimens, PreviewToolCopy.ClearInventoryFilters, modifier)
        return
    }
    key(specimen.id) {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Text(specimen.name, style = MaterialTheme.typography.titleLarge)
            FieldHelp("${specimen.family} · ${specimen.id}")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Badge(PreviewToolCopy.designStatus(specimen.designStatus))
                Badge(PreviewToolCopy.scope(specimen.scope), tone = if (specimen.scope == "app-owned") StatusTone.Info else StatusTone.Neutral)
                Badge(PreviewToolCopy.owner(specimen.owner))
                Badge(PreviewToolCopy.platforms(specimen.platforms),
                    tone = if ("cmp" in specimen.platforms) StatusTone.Neutral else StatusTone.Warning)
            }
            FieldHelp(specimen.reason)
            FieldHelp(PreviewToolCopy.consumer(specimen.consumer))
            if ("cmp" !in specimen.platforms) {
                FieldHelp(PreviewToolCopy.WebOnly)
            }
            if (specimen.designStatus == "existing") {
                FieldHelp(PreviewToolCopy.RegisteredHelp)
                Button(PreviewToolCopy.openInComponents(specimen.name), { onOpenComponent(specimen.id) }, variant = ButtonVariant.Secondary)
            } else {
                FieldHelp(PreviewToolCopy.ProposalHelp)
                FieldLabel(PreviewToolCopy.ReusedComponents)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                    verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                    specimen.componentIds.forEach { id ->
                        Button(AccessibleName.of(PreviewToolCopy.openInComponents(id)), { onOpenComponent(id) }, variant = ButtonVariant.Text) { Text(id) }
                    }
                }
            }
            FieldLabel(PreviewToolCopy.DesignDescription)
            FieldHelp(specimen.description)
            DetailList(PreviewToolCopy.DesignStates, specimen.states, separator = " · ")
            DetailList(PreviewToolCopy.Anatomy, specimen.anatomy, separator = " · ")
            DetailList(PreviewToolCopy.TokenRoles, specimen.tokenRoles, separator = " · ")
        }
    }
}

@Composable
private fun DetailList(title: String, items: List<String>, separator: String = "\n") {
    if (items.isEmpty()) return
    FieldLabel(title)
    FieldHelp(items.joinToString(separator))
}
