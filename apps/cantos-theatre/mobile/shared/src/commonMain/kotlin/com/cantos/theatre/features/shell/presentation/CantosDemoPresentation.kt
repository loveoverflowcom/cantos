package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material.icons.outlined.AccountCircle
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import com.cantos.theatre.app.CopyKey
import com.cantos.theatre.app.DemoCopy
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoCatalogView
import com.cantos.theatre.demo.DemoEpisode
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoListeningFailure
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoWork
import com.cantos.theatre.demo.canGoBack
import com.cantos.theatre.demo.continuingProgress
import com.cantos.theatre.demo.filteredWorks
import com.cantos.ui.CantosShapes
import com.cantos.ui.CantosTheme
import com.cantos.ui.Density
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

/** The same isolated mock composition is mounted by Android, iOS and the JVM diagnostic host.
 * Native Material controls consume the canonical theme. Unqualified Cantos component wrappers
 * are deliberately not adopted, copied, re-exported or registered as qualified here.
 */
@Composable
fun CantosDemoPresentation(
    state: DemoState,
    catalog: DemoCatalogView,
    dispatch: (DemoEvent) -> Unit,
    modifier: Modifier = Modifier,
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
) {
    val copy = DemoCopy(state.locale)
    val back = { dispatch(DemoEvent.Back) }
    SideEffect { onThemeChanged(state.theme == DemoTheme.Dark) }
    backHandler(canGoBack(state), back)
    CantosTheme(
        UiPreferences(
            theme = if (state.theme == DemoTheme.Dark) Theme.Dark else Theme.Light,
            density = Density.Comfortable,
            // There is no travel, spring or continuously animated indicator in this mock.
            reducedMotion = true,
            textScale = textScale,
        ),
        uiFontFamily = theatreSans(),
        editorialFontFamily = theatreSerif(),
    ) {
        Surface(modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Column(
                Modifier.fillMaxSize().safeDrawingPadding().imePadding().onPreviewKeyEvent {
                    if (it.key == Key.Escape && it.type == KeyEventType.KeyUp && canGoBack(state)) {
                        back()
                        true
                    } else {
                        false
                    }
                },
            ) {
                DemoHeader(state, copy, dispatch)
                when (state.listeningFailure) {
                    null -> Unit
                    DemoListeningFailure.MemoryStoreUnavailable -> Column(
                        Modifier.fillMaxWidth().padding(tokenDp("space.md")).testTag("listening-failure")
                            .semantics { liveRegion = LiveRegionMode.Polite },
                    ) {
                        Text(copy.text(CopyKey.ListeningStoreFailure))
                        TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(DemoEvent.RetryListening) }, modifier = Modifier.testTag("listening-retry")) {
                            Text(copy.text(CopyKey.RetryListening))
                        }
                    }
                }
                val scroll = remember(state.route) { androidx.compose.foundation.ScrollState(0) }
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                    Column(
                        Modifier.widthIn(max = tokenDp("breakpoint.expanded"))
                            .fillMaxWidth().verticalScroll(scroll).padding(tokenDp("space.md"))
                            .semantics { paneTitle = routeTitle(state.route, copy) }
                            .testTag("screen"),
                        verticalArrangement = Arrangement.spacedBy(tokenDp("space.section")),
                    ) {
                        Text(copy.text(CopyKey.DemoNotice), style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.testTag("demo-notice"))
                        when (val route = state.route) {
                            DemoRoute.Discovery -> DiscoveryScreen(state, catalog, copy, dispatch)
                            DemoRoute.Search -> SearchScreen(state, catalog, copy, dispatch)
                            DemoRoute.Library -> LibraryScreen(state, catalog, copy, dispatch)
                            is DemoRoute.Work -> {
                                val work = catalog.work(route.id)
                                if (work == null) SelectionUnavailable(copy, dispatch) else WorkScreen(work, state, copy, dispatch)
                            }
                            DemoRoute.Player -> PlayerScreen(state, catalog, copy, dispatch)
                            DemoRoute.Queue -> QueueScreen(state, catalog, copy, dispatch)
                            DemoRoute.Account -> AccountScreen(copy, dispatch)
                            DemoRoute.History -> HistoryScreen(state, catalog, copy, dispatch)
                            DemoRoute.Settings -> SettingsScreen(state, copy, dispatch)
                            DemoRoute.Unavailable -> SelectionUnavailable(copy, dispatch)
                        }
                    }
                }
                if (state.route != DemoRoute.Player && state.route != DemoRoute.Queue) {
                    MiniPlayer(state, catalog, copy, dispatch)
                    DemoTabs(state, copy, dispatch)
                }
            }
        }
    }
}

private fun routeTitle(route: DemoRoute, copy: DemoCopy): String = copy.text(
    when (route) {
        DemoRoute.Discovery -> CopyKey.Discover
        DemoRoute.Search -> CopyKey.Search
        DemoRoute.Library -> CopyKey.Library
        is DemoRoute.Work -> CopyKey.Work
        DemoRoute.Player -> CopyKey.Player
        DemoRoute.Queue -> CopyKey.Queue
        DemoRoute.Account -> CopyKey.Account
        DemoRoute.History -> CopyKey.History
        DemoRoute.Settings -> CopyKey.Settings
        DemoRoute.Unavailable -> CopyKey.SelectionUnavailable
    },
)

private fun headerTitleKey(route: DemoRoute): CopyKey = when (route) {
    DemoRoute.Player -> CopyKey.Player
    DemoRoute.Account -> CopyKey.AccountSpace
    DemoRoute.Discovery, DemoRoute.Search, DemoRoute.Library, is DemoRoute.Work, DemoRoute.Queue,
    DemoRoute.History, DemoRoute.Settings, DemoRoute.Unavailable -> CopyKey.Brand
}

@Composable
private fun DemoHeader(state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = tokenDp("space.sm"), vertical = tokenDp("space.xs")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (canGoBack(state)) {
            IconButton(onClick = { dispatch(DemoEvent.Back) }, modifier = Modifier.size(tokenDp("size.control")).testTag("back")) {
                Icon(
                    if (state.route == DemoRoute.Player) Icons.Outlined.Close else Icons.AutoMirrored.Outlined.ArrowBack,
                    copy.text(if (state.route == DemoRoute.Player) CopyKey.ClosePlayer else CopyKey.Back),
                )
            }
        }
        Text(
            copy.text(headerTitleKey(state.route)),
            modifier = Modifier.weight(1f).padding(horizontal = tokenDp("space.sm"))
                .then(if (state.route == DemoRoute.Account) Modifier.semantics { heading() } else Modifier)
                .testTag("header-title"),
            style = if (state.route == DemoRoute.Player) MaterialTheme.typography.labelLarge
                else MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.primary,
        )
        if (state.route != DemoRoute.Account) {
            IconButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Account)) }, modifier = Modifier.size(tokenDp("size.control")).testTag("account")) {
                Icon(Icons.Outlined.AccountCircle, copy.text(CopyKey.Account))
            }
        }
    }
}

@Composable
private fun DiscoveryScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(CopyKey.ContinuingHeading), color = MaterialTheme.colorScheme.tertiary,
            style = MaterialTheme.typography.labelLarge)
        Text(copy.text(CopyKey.Welcome), style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        val point = continuingProgress(state).firstOrNull()
        if (point != null) ListeningPoint(point, catalog, copy, dispatch, fromHistory = false, tag = "continue")
        TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Search)) }, modifier = Modifier.testTag("search")) {
            Icon(Icons.Outlined.Search, null)
            Text(copy.text(CopyKey.Search))
        }
    }
    Text(copy.text(CopyKey.DiscoverHeading), style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
        modifier = Modifier.semantics { heading() })
    if (state.catalogMode == DemoCatalogMode.Ready && catalog.works.isNotEmpty()) {
        WorkSummary(catalog.works.first(), state, copy, dispatch, Modifier, featured = true)
        if (catalog.works.size > 1) {
            SectionHeading(CopyKey.ExploreHeading, copy)
            WorkShelf(catalog.works.drop(1), state, copy, dispatch)
        }
    } else {
        CatalogRecovery(if (state.catalogMode == DemoCatalogMode.Ready) DemoCatalogMode.Empty else state.catalogMode, copy, dispatch)
    }
}

@Composable
internal fun WorkShelf(works: List<DemoWork>, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val columns = if (maxWidth >= tokenDp("breakpoint.medium")) 2 else 1
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            works.chunked(columns).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                    row.forEach { work ->
                        WorkSummary(work, state, copy, dispatch, Modifier.weight(1f))
                    }
                    if (row.size < columns) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun WorkSummary(
    work: DemoWork,
    state: DemoState,
    copy: DemoCopy,
    dispatch: (DemoEvent) -> Unit,
    modifier: Modifier,
    featured: Boolean = false,
) {
    Card(
        onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Work(work.id))) },
        modifier = modifier.fillMaxWidth().testTag("work-${work.id}")
            .semantics { contentDescription = copy.text(CopyKey.Details, "title" to work.title) },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = MaterialTheme.shapes.large,
    ) {
        if (featured) {
            Box(Modifier.fillMaxWidth()) {
                WorkArtwork(work.id, Modifier.fillMaxWidth().aspectRatio(1.6f))
                Column(Modifier.align(Alignment.BottomStart).fillMaxWidth()
                    .background(tokenColor("color.playerSurface")).padding(tokenDp("space.inset"))) {
                    Text(work.category, color = tokenColor("color.playerAccent"), style = MaterialTheme.typography.bodySmall)
                    Text(work.title, style = MaterialTheme.typography.headlineSmall, color = tokenColor("color.onPlayerSurface"))
                }
            }
        }
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            if (!featured) {
                Row(verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
                    WorkArtwork(work.id, Modifier.size(tokenDp("size.control") + tokenDp("space.xl")), shape = CantosShapes.thumbnail)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        Text(work.category, color = MaterialTheme.colorScheme.tertiary, style = MaterialTheme.typography.bodySmall)
                        Text(work.title, style = MaterialTheme.typography.headlineSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        Text(copy.text(CopyKey.EpisodesCount, "count" to work.episodes.size.toString()),
                            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            Text(work.description)
            if (featured) Text(copy.text(CopyKey.EpisodesCount, "count" to work.episodes.size.toString()),
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            SaveWorkButton(work, state, copy, dispatch, "save-card-${work.id}")
        }
    }
}

@Composable
private fun SearchScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Text(copy.text(CopyKey.Search), style = MaterialTheme.typography.headlineSmall,
        modifier = Modifier.semantics { heading() })
    OutlinedTextField(
        shape = MaterialTheme.shapes.medium,
        value = state.query, onValueChange = { dispatch(DemoEvent.Search(it)) },
        modifier = Modifier.fillMaxWidth().testTag("search-input"),
        label = { Text(copy.text(CopyKey.SearchLabel)) },
        placeholder = { Text(copy.text(CopyKey.SearchHint)) },
        leadingIcon = { Icon(Icons.Outlined.Search, null) },
        trailingIcon = {
            if (state.query.isNotEmpty()) IconButton(onClick = { dispatch(DemoEvent.Search("")) }) {
                Icon(Icons.Outlined.Close, copy.text(CopyKey.SearchClear))
            }
        },
    )
    val works = filteredWorks(state, catalog)
    Text(copy.text(CopyKey.Results, "count" to works.size.toString()), style = MaterialTheme.typography.bodySmall)
    when {
        state.catalogMode != DemoCatalogMode.Ready -> CatalogRecovery(state.catalogMode, copy, dispatch)
        works.isEmpty() -> CatalogRecovery(DemoCatalogMode.Empty, copy, dispatch)
        else -> WorkShelf(works, state, copy, dispatch)
    }
}

@Composable
internal fun SelectionUnavailable(copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Column(Modifier.testTag("selection-unavailable"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(copy.text(CopyKey.SelectionUnavailable), style = MaterialTheme.typography.titleLarge)
        Text(copy.text(CopyKey.SelectionUnavailableNote))
        Button(shape = MaterialTheme.shapes.medium, onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) }, modifier = Modifier.testTag("selection-discover")) {
            Text(copy.text(CopyKey.Discover))
        }
    }
}

@Composable
private fun CatalogRecovery(mode: DemoCatalogMode, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val title = when (mode) {
        DemoCatalogMode.Ready -> CopyKey.Ready
        DemoCatalogMode.Loading -> CopyKey.Loading
        DemoCatalogMode.Empty -> CopyKey.Empty
        DemoCatalogMode.Error -> CopyKey.Error
    }
    val message = when (mode) {
        DemoCatalogMode.Ready -> CopyKey.DemoNotice
        DemoCatalogMode.Loading -> CopyKey.LoadingNote
        DemoCatalogMode.Empty -> CopyKey.EmptyNote
        DemoCatalogMode.Error -> CopyKey.ErrorNote
    }
    Column(Modifier.semantics { liveRegion = LiveRegionMode.Polite }, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(copy.text(title), style = MaterialTheme.typography.titleLarge)
        Text(copy.text(message))
        if (mode == DemoCatalogMode.Error || mode == DemoCatalogMode.Loading) {
            Button(shape = MaterialTheme.shapes.medium, onClick = { dispatch(DemoEvent.Retry) }, modifier = Modifier.testTag("retry")) { Text(copy.text(CopyKey.Retry)) }
        } else {
            TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(DemoEvent.Search("")); dispatch(DemoEvent.Retry); dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) }) {
                Text(copy.text(CopyKey.Discover))
            }
        }
    }
}

@Composable
private fun WorkScreen(work: DemoWork, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    WorkArtwork(work.id, Modifier.fillMaxWidth().aspectRatio(1.6f))
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(work.category, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.tertiary)
        Text(work.title, style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        Text(work.description)
        Text(copy.text(CopyKey.EpisodesCount, "count" to work.episodes.size.toString()), style = MaterialTheme.typography.bodySmall)
    }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        val firstEpisode = work.episodes.firstOrNull()
        Button(shape = MaterialTheme.shapes.medium, enabled = firstEpisode != null && firstEpisode.durationSeconds > 0,
            onClick = { firstEpisode?.let { dispatch(DemoEvent.OpenEpisode(it.id)) } }) {
            Icon(Icons.Outlined.PlayArrow, null)
            Text(copy.text(CopyKey.StartDemo))
        }
        SaveWorkButton(work, state, copy, dispatch, "save-work")
    }
    Text(copy.text(CopyKey.SavedOnlyHere), style = MaterialTheme.typography.bodySmall)
    Text(copy.text(CopyKey.Episodes), style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
    if (work.episodes.isEmpty()) Text(copy.text(CopyKey.SelectionUnavailableNote))
    work.episodes.forEach { EpisodeSelection(it, state, copy, dispatch) }
}

@Composable
internal fun EpisodeSelection(episode: DemoEpisode, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val selected = episode.id == state.selectedEpisode
    Surface(
        onClick = { dispatch(DemoEvent.OpenEpisode(episode.id)) },
        enabled = episode.durationSeconds > 0,
        modifier = Modifier.fillMaxWidth().testTag("episode-${episode.id}").semantics {
            role = Role.Button
            contentDescription = copy.text(CopyKey.PlayEpisode, "title" to episode.title)
            if (selected) stateDescription = copy.text(CopyKey.CurrentEpisode)
        },
        shape = MaterialTheme.shapes.medium,
        color = if (selected) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Row(Modifier.padding(tokenDp("space.inset")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(episode.index.toString(), style = MaterialTheme.typography.bodySmall)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(episode.title, style = MaterialTheme.typography.titleMedium,
                    maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(copy.text(CopyKey.Duration, "minutes" to (episode.durationSeconds / 60).toString()),
                    style = MaterialTheme.typography.bodySmall)
                if (selected) Text(copy.text(CopyKey.CurrentEpisode), style = MaterialTheme.typography.bodySmall)
            }
            Icon(Icons.Outlined.PlayArrow, null)
        }
    }
    if (episode.durationSeconds <= 0) Text(copy.text(CopyKey.SelectionUnavailableNote), style = MaterialTheme.typography.bodySmall)
}

@Composable
private fun SaveWorkButton(work: DemoWork, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit, tag: String) {
    val saved = work.id in state.savedWorks
    FilledTonalButton(shape = MaterialTheme.shapes.medium,
        onClick = { dispatch(DemoEvent.SetSaved(work.id, saved = !saved)) },
        modifier = Modifier.testTag(tag).semantics {
            contentDescription = copy.text(if (saved) CopyKey.UnsaveName else CopyKey.SaveName, "title" to work.title)
            stateDescription = copy.text(if (saved) CopyKey.SavedTab else CopyKey.Save)
        },
    ) {
        Icon(Icons.Outlined.Star, null)
        Text(copy.text(if (saved) CopyKey.Saved else CopyKey.Save))
    }
}


@Composable
internal fun SectionHeading(key: CopyKey, copy: DemoCopy) {
    Text(copy.text(key), style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
        modifier = Modifier.semantics { heading() })
}

@Composable
private fun DemoTabs(state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val root = state.stack.firstOrNull()
    NavigationBar(containerColor = MaterialTheme.colorScheme.surface, windowInsets = androidx.compose.foundation.layout.WindowInsets(0)) {
        NavigationBarItem(selected = root == DemoRoute.Discovery || root == DemoRoute.Search, onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) },
            icon = { Icon(Icons.Outlined.Home, null) }, label = { Text(copy.text(CopyKey.Discover)) }, modifier = Modifier.testTag("nav-discovery"))
        NavigationBarItem(selected = root == DemoRoute.Library || root == DemoRoute.History, onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Library)) },
            icon = { Icon(Icons.AutoMirrored.Outlined.MenuBook, null) }, label = { Text(copy.text(CopyKey.Library)) }, modifier = Modifier.testTag("nav-library"))
    }
}
