package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material.icons.automirrored.outlined.QueueMusic
import androidx.compose.material.icons.outlined.AccountCircle
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Pause
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import com.cantos.theatre.app.CopyKey
import com.cantos.theatre.app.DemoCopy
import com.cantos.theatre.app.clockTime
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoCatalogView
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEpisode
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoListeningFailure
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.demo.DemoWork
import com.cantos.theatre.demo.PlaybackUiStatus
import com.cantos.theatre.demo.SupportedUiSpeedPercents
import com.cantos.theatre.demo.continuingProgress
import com.cantos.theatre.demo.canGoBack
import com.cantos.theatre.demo.filteredWorks
import com.cantos.ui.CantosTheme
import com.cantos.ui.Density
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
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
                        TextButton(onClick = { dispatch(DemoEvent.RetryListening) }, modifier = Modifier.testTag("listening-retry")) {
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

@Composable
private fun DemoHeader(state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = tokenDp("space.sm"), vertical = tokenDp("space.xs")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (canGoBack(state)) {
            IconButton(onClick = { dispatch(DemoEvent.Back) }, modifier = Modifier.testTag("back")) {
                Icon(
                    if (state.route == DemoRoute.Player) Icons.Outlined.Close else Icons.AutoMirrored.Outlined.ArrowBack,
                    copy.text(if (state.route == DemoRoute.Player) CopyKey.ClosePlayer else CopyKey.Back),
                )
            }
        }
        Column(Modifier.weight(1f).padding(horizontal = tokenDp("space.sm"))) {
            Text(
                copy.text(CopyKey.Brand), style = MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif),
                color = MaterialTheme.colorScheme.primary,
            )
            Text(copy.text(CopyKey.DemoNotice), style = MaterialTheme.typography.bodySmall)
        }
        if (state.route != DemoRoute.Account) {
            IconButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Account)) }, modifier = Modifier.testTag("account")) {
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
        Text(copy.text(CopyKey.Welcome), style = MaterialTheme.typography.headlineLarge.copy(fontFamily = FontFamily.Serif),
            modifier = Modifier.semantics { heading() })
        val point = continuingProgress(state).firstOrNull()
        if (point != null) ListeningPoint(point, catalog, copy, dispatch, fromHistory = false, tag = "continue")
        TextButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Search)) }, modifier = Modifier.testTag("search")) {
            Icon(Icons.Outlined.Search, null)
            Text(copy.text(CopyKey.Search))
        }
    }
    Text(copy.text(CopyKey.DiscoverHeading), style = MaterialTheme.typography.titleLarge.copy(fontFamily = FontFamily.Serif),
        modifier = Modifier.semantics { heading() })
    if (state.catalogMode == DemoCatalogMode.Ready && catalog.works.isNotEmpty()) {
        WorkShelf(catalog.works, state, copy, dispatch)
    } else {
        CatalogRecovery(if (state.catalogMode == DemoCatalogMode.Ready) DemoCatalogMode.Empty else state.catalogMode, copy, dispatch)
    }
}

@Composable
private fun WorkShelf(works: List<DemoWork>, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
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
private fun WorkSummary(work: DemoWork, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit, modifier: Modifier) {
    Card(
        onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Work(work.id))) },
        modifier = modifier.fillMaxWidth().testTag("work-${work.id}")
            .semantics { contentDescription = copy.text(CopyKey.Details, "title" to work.title) },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = MaterialTheme.shapes.large,
    ) {
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(work.category, color = MaterialTheme.colorScheme.tertiary, style = MaterialTheme.typography.labelLarge)
            Text(work.title, style = MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif))
            Text(work.description)
            Text(copy.text(CopyKey.EpisodesCount, "count" to work.episodes.size.toString()),
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
private fun SelectionUnavailable(copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Column(Modifier.testTag("selection-unavailable"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(copy.text(CopyKey.SelectionUnavailable), style = MaterialTheme.typography.titleLarge)
        Text(copy.text(CopyKey.SelectionUnavailableNote))
        Button(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) }, modifier = Modifier.testTag("selection-discover")) {
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
            Button(onClick = { dispatch(DemoEvent.Retry) }, modifier = Modifier.testTag("retry")) { Text(copy.text(CopyKey.Retry)) }
        } else {
            TextButton(onClick = { dispatch(DemoEvent.Search("")); dispatch(DemoEvent.Retry); dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) }) {
                Text(copy.text(CopyKey.Discover))
            }
        }
    }
}

@Composable
private fun WorkScreen(work: DemoWork, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(work.category, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.tertiary)
        Text(work.title, style = MaterialTheme.typography.headlineLarge.copy(fontFamily = FontFamily.Serif),
            modifier = Modifier.semantics { heading() })
        Text(work.description)
        Text(copy.text(CopyKey.EpisodesCount, "count" to work.episodes.size.toString()), style = MaterialTheme.typography.bodySmall)
    }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        val firstEpisode = work.episodes.firstOrNull()
        Button(enabled = firstEpisode != null && firstEpisode.durationSeconds > 0,
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
private fun EpisodeSelection(episode: DemoEpisode, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    FilledTonalButton(
        onClick = { dispatch(DemoEvent.OpenEpisode(episode.id)) },
        enabled = episode.durationSeconds > 0,
        modifier = Modifier.fillMaxWidth().testTag("episode-${episode.id}")
            .semantics { contentDescription = copy.text(CopyKey.PlayEpisode, "title" to episode.title) },
        shape = MaterialTheme.shapes.large,
        colors = androidx.compose.material3.ButtonDefaults.filledTonalButtonColors(
            containerColor = if (episode.id == state.selectedEpisode) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainer,
        ),
    ) {
        Text(episode.index.toString(), style = MaterialTheme.typography.titleLarge)
        Column(Modifier.weight(1f).padding(tokenDp("space.sm"))) {
            Text(episode.title, style = MaterialTheme.typography.titleMedium)
            Text(copy.text(CopyKey.Duration, "minutes" to (episode.durationSeconds / 60).toString()), style = MaterialTheme.typography.bodySmall)
            if (episode.id == state.selectedEpisode) Text(copy.text(CopyKey.CurrentEpisode), style = MaterialTheme.typography.bodySmall)
        }
        Icon(Icons.Outlined.PlayArrow, null)
    }
    if (episode.durationSeconds <= 0) Text(copy.text(CopyKey.SelectionUnavailableNote), style = MaterialTheme.typography.bodySmall)
}

@Composable
private fun SaveWorkButton(work: DemoWork, state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit, tag: String) {
    val saved = work.id in state.savedWorks
    FilledTonalButton(
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
private fun LibraryScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Text(copy.text(CopyKey.Library), style = MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif),
        modifier = Modifier.semantics { heading() })
    Text(copy.text(CopyKey.LibrarySession), style = MaterialTheme.typography.bodySmall)
    Column(Modifier.testTag("library-continue"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        SectionHeading(CopyKey.Continuing, copy)
        val points = continuingProgress(state)
        if (points.isEmpty()) SectionEmpty(CopyKey.NoContinue, CopyKey.NoContinueHelp, copy, dispatch)
        points.forEach { ListeningPoint(it, catalog, copy, dispatch, fromHistory = false, tag = "resume-${it.episodeId}") }
    }
    Column(Modifier.testTag("library-saved"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        SectionHeading(CopyKey.SavedTab, copy)
        val works = catalog.works.filter { it.id in state.savedWorks }
        if (works.isEmpty()) SectionEmpty(CopyKey.NoSaved, CopyKey.NoSavedHelp, copy, dispatch)
        else WorkShelf(works, state, copy, dispatch)
        if (works.size < state.savedWorks.size) Text(copy.text(CopyKey.SelectionUnavailableNote))
    }
    Column(Modifier.testTag("library-history"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        SectionHeading(CopyKey.History, copy)
        TextButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.History)) }, modifier = Modifier.testTag("history-all")) {
            Text(copy.text(CopyKey.ViewAllHistory))
        }
        if (state.history.isEmpty()) SectionEmpty(CopyKey.NoHistory, CopyKey.NoHistoryHelp, copy, dispatch)
        state.history.take(3).forEach {
            ListeningPoint(it, catalog, copy, dispatch, fromHistory = true, tag = "recent-${it.episodeId}")
        }
    }
}

@Composable
private fun SectionHeading(key: CopyKey, copy: DemoCopy) {
    Text(copy.text(key), style = MaterialTheme.typography.titleLarge.copy(fontFamily = FontFamily.Serif),
        modifier = Modifier.semantics { heading() })
}

@Composable
private fun SectionEmpty(title: CopyKey, help: CopyKey, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Text(copy.text(title), style = MaterialTheme.typography.titleMedium)
    Text(copy.text(help), style = MaterialTheme.typography.bodySmall)
    TextButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) }) { Text(copy.text(CopyKey.Discover)) }
}

@Composable
private fun ListeningPoint(
    point: DemoProgress,
    catalog: DemoCatalogView,
    copy: DemoCopy,
    dispatch: (DemoEvent) -> Unit,
    fromHistory: Boolean,
    tag: String,
) {
    val episode = catalog.episode(point.episodeId)
    if (episode == null || episode.durationSeconds <= 0) {
        Text(copy.text(CopyKey.SelectionUnavailableNote))
        return
    }
    FilledTonalButton(
        onClick = {
            dispatch(if (fromHistory) DemoEvent.ResumeHistoryEpisode(point.episodeId) else DemoEvent.ResumeEpisode(point.episodeId))
        },
        modifier = Modifier.fillMaxWidth().testTag(tag).semantics {
            contentDescription = copy.text(CopyKey.ResumeSampleName, "title" to episode.title, "time" to copy.spokenTime(point.positionSeconds))
        },
        shape = MaterialTheme.shapes.large,
    ) {
        Icon(Icons.Outlined.PlayArrow, null)
        Column(Modifier.weight(1f).padding(tokenDp("space.sm"))) {
            Text(catalog.work(episode.workId)?.title.orEmpty(), style = MaterialTheme.typography.labelLarge)
            Text(copy.text(CopyKey.EpisodeLabel, "index" to episode.index.toString(), "title" to episode.title))
            Text(copy.text(CopyKey.PositionCaption, "elapsed" to clockTime(point.positionSeconds), "total" to clockTime(episode.durationSeconds)))
            Text(copy.text(CopyKey.ResumeSample), style = MaterialTheme.typography.labelLarge)
        }
    }
    Text(copy.text(CopyKey.ResumeSampleHelp), style = MaterialTheme.typography.bodySmall)
}

@Composable
private fun HistoryScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    var confirming by remember { mutableStateOf(false) }
    SectionHeading(CopyKey.History, copy)
    Text(copy.text(CopyKey.HistoryHelp))
    if (state.history.isEmpty()) SectionEmpty(CopyKey.NoHistory, CopyKey.NoHistoryHelp, copy, dispatch)
    else {
        TextButton(onClick = { confirming = true }, modifier = Modifier.testTag("history-clear")) { Text(copy.text(CopyKey.ClearHistory)) }
        state.history.forEach { ListeningPoint(it, catalog, copy, dispatch, fromHistory = true, tag = "history-${it.episodeId}") }
    }
    if (confirming) AlertDialog(
        modifier = Modifier.testTag("history-dialog"),
        onDismissRequest = { confirming = false },
        title = { Text(copy.text(CopyKey.ClearHistoryConfirmTitle)) },
        text = { Text(copy.text(CopyKey.ClearHistoryHelp)) },
        confirmButton = {
            TextButton(onClick = { confirming = false; dispatch(DemoEvent.ClearHistory) }, modifier = Modifier.testTag("history-confirm")) {
                Text(copy.text(CopyKey.ConfirmClearHistory))
            }
        },
        dismissButton = {
            TextButton(onClick = { confirming = false }, modifier = Modifier.testTag("history-cancel")) { Text(copy.text(CopyKey.Cancel)) }
        },
    )
}

@Composable
private fun AccountScreen(copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    SectionHeading(CopyKey.Account, copy)
    Text(copy.text(CopyKey.AccountHelp))
    Text(copy.text(CopyKey.LibrarySession))
    Button(onClick = {}, enabled = false, modifier = Modifier.testTag("sign-in").semantics {
        stateDescription = copy.text(CopyKey.SignInUnavailable)
    }) { Text(copy.text(CopyKey.SignIn)) }
    Text(copy.text(CopyKey.SignInUnavailable), style = MaterialTheme.typography.bodySmall)
    FilledTonalButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Settings)) }, modifier = Modifier.testTag("settings")) {
        Icon(Icons.Outlined.Settings, null)
        Text(copy.text(CopyKey.Settings))
    }
}

@Composable
private fun PlayerScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val episode = catalog.episode(state.selectedEpisode)
    val work = episode?.let { catalog.work(it.workId) }
    if (episode == null || work == null || episode.durationSeconds <= 0 || state.positionSeconds > episode.durationSeconds) {
        SelectionUnavailable(copy, dispatch)
        return
    }
    Surface(color = MaterialTheme.colorScheme.primaryContainer, shape = MaterialTheme.shapes.extraLarge, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(tokenDp("space.lg")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(work.title, style = MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif))
            Text(copy.text(CopyKey.ArtworkPending), style = MaterialTheme.typography.bodySmall)
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(CopyKey.EpisodeLabel, "index" to episode.index.toString(), "title" to work.title), style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.tertiary)
        Text(episode.title, style = MaterialTheme.typography.headlineLarge.copy(fontFamily = FontFamily.Serif),
            modifier = Modifier.semantics { heading() })
        Text(copy.text(playbackStatusKey(state.playbackStatus)),
            modifier = Modifier.testTag("simulation-status").semantics { liveRegion = LiveRegionMode.Polite })
        Slider(value = state.positionSeconds.toFloat(), onValueChange = { dispatch(DemoEvent.SeekTo(it.toInt())) },
            valueRange = 0f..episode.durationSeconds.toFloat(), modifier = Modifier.fillMaxWidth().testTag("seek").semantics {
                contentDescription = copy.text(CopyKey.Position)
                stateDescription = copy.text(CopyKey.PositionSpoken, "elapsed" to copy.spokenTime(state.positionSeconds), "total" to copy.spokenTime(episode.durationSeconds))
            })
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(clockTime(state.positionSeconds), modifier = Modifier.testTag("elapsed"))
            Text(clockTime(episode.durationSeconds))
        }
    }
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        FilledTonalButton(onClick = { dispatch(DemoEvent.Seek(-15)) }, modifier = Modifier.testTag("rewind")) { Text(copy.text(CopyKey.Rewind)) }
        Button(onClick = { dispatch(DemoEvent.ToggleSimulation) }, modifier = Modifier.testTag("transport").semantics {
            contentDescription = copy.text(if (state.transportRequested) CopyKey.PauseEpisode else CopyKey.PlayEpisode, "title" to episode.title)
        }) {
            Icon(if (state.transportRequested) Icons.Outlined.Pause else Icons.Outlined.PlayArrow, null)
            Text(copy.text(if (state.transportRequested) CopyKey.Pause else CopyKey.Play))
        }
        FilledTonalButton(onClick = { dispatch(DemoEvent.Seek(15)) }, modifier = Modifier.testTag("forward")) { Text(copy.text(CopyKey.Forward)) }
    }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        val bookmarked = DemoBookmark(state.selectedEpisode, state.positionSeconds, state.positionMs) in state.bookmarks
        FilledTonalButton(onClick = { dispatch(DemoEvent.Bookmark) }, modifier = Modifier.testTag("bookmark")) {
            Icon(Icons.Outlined.Star, null)
            Text(copy.text(if (bookmarked) CopyKey.Bookmarked else CopyKey.Bookmark))
        }
        FilledTonalButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Queue)) }, modifier = Modifier.testTag("queue")) {
            Icon(Icons.AutoMirrored.Outlined.QueueMusic, null)
            Text(copy.text(CopyKey.Queue))
        }
        TextButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Settings)) }) {
            Text(copy.text(CopyKey.SpeedCaption, "speed" to copy.speed(state.speedPercent)))
        }
    }
    Text(copy.text(CopyKey.PlayerExplanation), style = MaterialTheme.typography.bodySmall)
    if (state.bookmarks.isNotEmpty()) {
        SectionHeading(CopyKey.Bookmarks, copy)
        state.bookmarks.forEach { bookmark ->
            val markedEpisode = catalog.episode(bookmark.episodeId)
            if (markedEpisode != null) TextButton(onClick = { dispatch(DemoEvent.OpenBookmark(bookmark)) }, modifier = Modifier.semantics {
                contentDescription = copy.text(CopyKey.OpenBookmark, "title" to markedEpisode.title, "time" to copy.spokenTime(bookmark.positionSeconds))
            }) {
                Text(copy.text(CopyKey.BookmarkCaption, "title" to markedEpisode.title, "time" to clockTime(bookmark.positionSeconds)))
            }
        }
    }

}

@Composable
private fun QueueScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Text(copy.text(CopyKey.Queue), style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
    Text(copy.text(CopyKey.NextExplanation))
    val episode = catalog.episode(state.selectedEpisode)
    val work = episode?.let { catalog.work(it.workId) }
    if (work == null || work.episodes.isEmpty()) {
        SelectionUnavailable(copy, dispatch)
        return
    }
    work.episodes.forEach {
        EpisodeSelection(it, state, copy, dispatch)
    }
}

@Composable
private fun SettingsScreen(state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Text(copy.text(CopyKey.PersonalRhythm), style = MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif),
        modifier = Modifier.semantics { heading() })
    Text(copy.text(CopyKey.PreferencesNote))
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Text(copy.text(CopyKey.Theme), style = MaterialTheme.typography.titleLarge)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            DemoTheme.entries.forEach { theme ->
                FilterChip(selected = state.theme == theme, onClick = { dispatch(DemoEvent.SetTheme(theme)) },
                    label = { Text(copy.text(if (theme == DemoTheme.Light) CopyKey.Light else CopyKey.Dark)) }, modifier = Modifier.testTag("theme-$theme"))
            }
        }
        Text(copy.text(CopyKey.Language), style = MaterialTheme.typography.titleLarge)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            DemoLocale.entries.forEach { locale ->
                FilterChip(selected = state.locale == locale, onClick = { dispatch(DemoEvent.SetLocale(locale)) },
                    label = { Text(copy.text(if (locale == DemoLocale.Vietnamese) CopyKey.Vietnamese else CopyKey.English)) })
            }
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Text(copy.text(CopyKey.Speed), style = MaterialTheme.typography.titleLarge)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            SupportedUiSpeedPercents.forEach { speed ->
                FilterChip(selected = state.speedPercent == speed, onClick = { dispatch(DemoEvent.SetSpeed(speed)) }, label = { Text(copy.speed(speed)) },
                    modifier = Modifier.testTag("speed-$speed"))
            }
        }
        Text(copy.text(CopyKey.Timer), style = MaterialTheme.typography.titleLarge)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            DemoTimer.entries.forEach { timer ->
                val key = when (timer) {
                    DemoTimer.Off -> CopyKey.TimerOff
                    DemoTimer.Fifteen -> CopyKey.Timer15
                    DemoTimer.Thirty -> CopyKey.Timer30
                    DemoTimer.EndOfEpisode -> CopyKey.TimerEnd
                }
                FilterChip(selected = state.timer == timer, onClick = { dispatch(DemoEvent.SetTimer(timer)) }, label = { Text(copy.text(key)) })
            }
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Text(copy.text(CopyKey.CatalogState), style = MaterialTheme.typography.titleLarge)
        Text(copy.text(CopyKey.CatalogStateNote), style = MaterialTheme.typography.bodySmall)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            DemoCatalogMode.entries.forEach { mode ->
                val key = when (mode) {
                    DemoCatalogMode.Ready -> CopyKey.Ready
                    DemoCatalogMode.Loading -> CopyKey.LoadingMode
                    DemoCatalogMode.Empty -> CopyKey.EmptyMode
                    DemoCatalogMode.Error -> CopyKey.ErrorMode
                }
                FilterChip(selected = state.catalogMode == mode, onClick = { dispatch(DemoEvent.CatalogMode(mode)) },
                    label = { Text(copy.text(key)) }, modifier = Modifier.testTag("catalog-$mode"))
            }
        }
    }
    Text(copy.text(CopyKey.Unsupported), style = MaterialTheme.typography.titleLarge)
    Text(copy.text(CopyKey.IntegrationsPending), style = MaterialTheme.typography.bodySmall)
    Text(copy.text(CopyKey.ResetNote), style = MaterialTheme.typography.bodySmall)
    Button(onClick = { dispatch(DemoEvent.Reset) }, modifier = Modifier.testTag("reset")) { Text(copy.text(CopyKey.Reset)) }
}

@Composable
private fun MiniPlayer(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val episode = catalog.episode(state.selectedEpisode) ?: return
    Surface(color = MaterialTheme.colorScheme.inverseSurface, contentColor = MaterialTheme.colorScheme.inverseOnSurface,
        shape = MaterialTheme.shapes.large, modifier = Modifier.fillMaxWidth().padding(horizontal = tokenDp("space.md"), vertical = tokenDp("space.sm"))) {
        Row(Modifier.padding(horizontal = tokenDp("space.sm")), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Player)) }, modifier = Modifier.weight(1f).testTag("mini-open").semantics {
                contentDescription = copy.text(CopyKey.OpenPlayer, "title" to episode.title)
                stateDescription = copy.text(playbackStatusKey(state.playbackStatus))
            }) {
                Column(Modifier.fillMaxWidth()) {
                    Text(episode.title, maxLines = 1, overflow = TextOverflow.Ellipsis, color = MaterialTheme.colorScheme.inverseOnSurface)
                    Text(clockTime(state.positionSeconds), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.inverseOnSurface)
                }
            }
            IconButton(onClick = { dispatch(DemoEvent.ToggleSimulation) }, modifier = Modifier.testTag("mini-transport")) {
                Icon(if (state.transportRequested) Icons.Outlined.Pause else Icons.Outlined.PlayArrow,
                    copy.text(if (state.transportRequested) CopyKey.PauseEpisode else CopyKey.PlayEpisode, "title" to episode.title), tint = MaterialTheme.colorScheme.inverseOnSurface)
            }
        }
    }
}

private fun playbackStatusKey(status: PlaybackUiStatus): CopyKey = when (status) {
    PlaybackUiStatus.Idle -> CopyKey.MockIdle
    PlaybackUiStatus.Loading -> CopyKey.MockLoading
    PlaybackUiStatus.Paused -> CopyKey.MockPaused
    PlaybackUiStatus.Simulating -> CopyKey.MockPlaying
    PlaybackUiStatus.Buffering -> CopyKey.MockBuffering
    PlaybackUiStatus.Ended -> CopyKey.MockEnded
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
