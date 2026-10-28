package com.cantos.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.cantos.ui.AccessibleName
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.Select
import com.cantos.ui.SelectOption
import com.cantos.ui.TextField
import com.cantos.ui.tokenDp

internal val EdgeStates = listOf("idle", "pending", "success", "error", "cancel", "hidden", "background", "disposed", "list-pending")

/** Host-owned synthetic request. Both placements keep a slot, but only one indicator is active. */
@Composable
internal fun EdgeProgressSamples(
    initialState: String,
    modifier: Modifier = Modifier,
    edge: @Composable (Boolean, Boolean, Boolean, AccessibleName, Modifier) -> Unit,
) {
    var state by remember(initialState) { mutableStateOf(initialState) }
    var list by remember { mutableStateOf(initialState == "list-pending") }
    var query by remember { mutableStateOf(PreviewToolCopy.EdgeFixture.Title) }
    val pending = state in listOf("pending", "list-pending", "hidden", "background")
    val name = AccessibleName.of(PreviewToolCopy.EdgeFixture.Pending)
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Select(PreviewToolCopy.EdgeState, EdgeStates.map { SelectOption(it, it) }, state, { state = it },
            expandedDescription = PreviewToolCopy.RecipesOpen, collapsedDescription = PreviewToolCopy.RecipesClosed,
            placeholder = PreviewToolCopy.EdgeState, emptyDescription = PreviewToolCopy.NoMatchingRecipe)
        Select(PreviewToolCopy.EdgePlacement,
            listOf(SelectOption("search", PreviewToolCopy.EdgeSearchPlacement), SelectOption("list", PreviewToolCopy.EdgeListPlacement)),
            if (list) "list" else "search", { list = it == "list" },
            expandedDescription = PreviewToolCopy.RecipesOpen, collapsedDescription = PreviewToolCopy.RecipesClosed,
            placeholder = PreviewToolCopy.EdgePlacement, emptyDescription = PreviewToolCopy.NoMatchingRecipe)
        TextField(PreviewToolCopy.EdgeFixture.Search, query, { query = it }, modifier = Modifier.testTag("edge-query"))
        Box(Modifier.fillMaxWidth().height(tokenDp("size.loadingEdge"))) {
            if (state != "disposed") edge(pending && !list, state != "hidden", state != "background", name, Modifier.testTag("edge-search"))
        }
        // Reserve the largest three-line outcome at either text scale; request outcome never shifts the list.
        Box(Modifier.heightIn(min = with(androidx.compose.ui.platform.LocalDensity.current) {
            (MaterialTheme.typography.bodyMedium.lineHeight * 3).toDp()
        })) { Text(vietnameseSpecimenText(PreviewToolCopy.EdgeFixture.outcome(state))) }
        Box(Modifier.fillMaxWidth().height(tokenDp("size.loadingEdge"))) {
            if (state != "disposed") edge(pending && list, state != "hidden", state != "background", name, Modifier.testTag("edge-list"))
        }
        Column(Modifier.fillMaxWidth().testTag("edge-retained").padding(tokenDp("space.md"))) {
            Button(PreviewToolCopy.EdgeFixture.Title, {}, variant = ButtonVariant.Text)
            Text(vietnameseSpecimenText(PreviewToolCopy.EdgeFixture.Retained))
        }
    }
}
