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
            FieldLabel("${specimens.size} of ${counts.total} specimens")
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
            FieldLabel("${specimens.size} of ${counts.total} specimens")
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
    Select("Specimen", specimens.map { SelectOption(it.id, "${it.name} · ${it.id}") }, selected?.id, onSelect,
        expandedDescription = "Danh sách specimen đang mở",
        collapsedDescription = "Danh sách specimen đang đóng",
        placeholder = "Chọn specimen",
        emptyDescription = "Không có specimen khớp bộ lọc")
    SpecimenDetail(selected, onOpenComponent)
}

@Composable
private fun InventoryHeader(counts: InventoryCounts) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Text("Design v3 inventory", style = MaterialTheme.typography.titleLarge)
        FieldHelp(
            "Design export ${DesignInventory.version} · manifest ${DesignInventory.sourceSha256.take(12)} · offline proposal, " +
                "not a pixel oracle.",
        )
        FieldHelp(
            "${counts.total} specimens = ${counts.existing} in the shared registry + ${counts.proposed} proposals. " +
                "A proposal is not a public API, has no sample and is never qualified here.",
        )
    }
}

@Composable
private fun InventoryFilters(filter: InventoryFilter, counts: InventoryCounts, onChange: (InventoryFilter) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        TextField("Find specimen", filter.query, { onChange(filter.copy(query = it)) })
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            FilterChip("Existing ${counts.existing}", filter.designStatus == "existing",
                { onChange(filter.copy(designStatus = if (it) "existing" else null)) })
            FilterChip("Proposed ${counts.proposed}", filter.designStatus == "proposed",
                { onChange(filter.copy(designStatus = if (it) "proposed" else null)) })
            FilterChip("CMP only ${counts.cmpApplicable}", filter.cmpOnly, { onChange(filter.copy(cmpOnly = it)) })
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
        EmptyState("Không có specimen", "Bỏ bộ lọc để xem lại toàn bộ inventory.", modifier)
        return
    }
    key(specimen.id) {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Text(specimen.name, style = MaterialTheme.typography.titleLarge)
            FieldHelp("${specimen.family} · ${specimen.id}")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Badge("design: ${specimen.designStatus}")
                Badge("scope: ${specimen.scope}", tone = if (specimen.scope == "app-owned") StatusTone.Info else StatusTone.Neutral)
                Badge("owner: ${specimen.owner}")
                Badge("platforms: ${specimen.platforms.joinToString(" · ")}",
                    tone = if ("cmp" in specimen.platforms) StatusTone.Neutral else StatusTone.Warning)
            }
            FieldHelp(specimen.reason)
            FieldHelp("Consumer: ${specimen.consumer}")
            if ("cmp" !in specimen.platforms) {
                FieldHelp("Web only: Studio and Admin compositions never target CMP and do not authorize a Studio mobile client.")
            }
            if (specimen.designStatus == "existing") {
                FieldHelp("Registered in the shared preview registry. Registration is not qualification; the Components tab shows its actual states and examples.")
                Button("Open ${specimen.name} in Components", { onOpenComponent(specimen.id) }, variant = ButtonVariant.Secondary)
            } else {
                FieldHelp("No native sample: a proposal is not a public API and is never qualified here. Declare it in both previews and pass admission before product use.")
                FieldLabel("Reuses registered components")
                FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
                    verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                    specimen.componentIds.forEach { id ->
                        Button(AccessibleName.of("Open $id in Components"), { onOpenComponent(id) }, variant = ButtonVariant.Text) { Text(id) }
                    }
                }
            }
            FieldLabel("Design description (verbatim)")
            FieldHelp(specimen.description)
            DetailList("Design states (verbatim)", specimen.states, separator = " · ")
            DetailList("Anatomy", specimen.anatomy, separator = " · ")
            DetailList("Canonical token roles", specimen.tokenRoles, separator = " · ")
        }
    }
}

@Composable
private fun DetailList(title: String, items: List<String>, separator: String = "\n") {
    if (items.isEmpty()) return
    FieldLabel(title)
    FieldHelp(items.joinToString(separator))
}
