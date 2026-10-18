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
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
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
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCapabilities
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellDetailPhase
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellText
import com.cantos.theatre.presentation.model.ShellUnitKind
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.canGoBack
import com.cantos.theatre.presentation.model.continuingProgress
import com.cantos.theatre.presentation.model.filteredWorks
import com.cantos.theatre.presentation.model.isAuth
import com.cantos.ui.CantosShapes
import com.cantos.ui.CantosTheme
import com.cantos.ui.Density
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.MotionPreference
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

/** Optional host-owned presentation additions; absent in an ordinary application mount. */
typealias ShellNoticeContent = @Composable (ShellState, ShellCopy) -> Unit
typealias ShellDiagnosticsContent = @Composable (ShellState, (ShellEvent) -> Unit, () -> Unit) -> Unit

class ShellDiagnosticsSlot(
    val title: ShellText,
    val summary: ShellText,
    val content: ShellDiagnosticsContent,
)

val LocalShellCopyOverrides = staticCompositionLocalOf<Map<CopyKey, ShellText>> { emptyMap() }
val LocalShellNotice = staticCompositionLocalOf<ShellNoticeContent?> { null }
val LocalShellDiagnostics = staticCompositionLocalOf<ShellDiagnosticsSlot?> { null }

/** Host facts about what it can do; the default offers the full sample experience. */
val LocalShellCapabilities = staticCompositionLocalOf { ShellCapabilities() }

/** Optional host content at the head of Library: sign-in prompt, sync status, conflicts, paging. */
val LocalShellLibraryHeader = staticCompositionLocalOf<(@Composable () -> Unit)?> { null }

/** The same controlled presentation is mounted by Android, iOS and the JVM diagnostic host.
 * Native Material controls consume the canonical theme. Membership and the optional account flow
 * supply slots composed from the shared design-v2 primitives; their bounded admission is recorded
 * separately. The shell owns only where the slots sit, never an account decision.
 */
@Composable
fun CantosPresentation(
    state: ShellState,
    catalog: ShellCatalogView,
    dispatch: (ShellEvent) -> Unit,
    modifier: Modifier = Modifier,
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    membershipContent: @Composable () -> Unit = {},
    membershipHeader: (@Composable () -> Unit)? = null,
    membershipScreen: MembershipScreen = MembershipScreen.Intro,
    // Receives the requester that Back restores when it closes an auth intent opened from Account.
    accountContent: @Composable (loginFocus: FocusRequester) -> Unit = {},
    // Login and Register bodies; the root reads the route from its own projection.
    authContent: @Composable () -> Unit = {},
    authHeader: (@Composable () -> Unit)? = null,
    // Hosts may inject Full to observe motion; reduced motion is the default.
    motionPreference: MotionPreference = MotionPreference.Reduced,
    density: Density = Density.Comfortable,
    localeSaveUnavailable: Boolean = false,
) {
    val copy = ShellCopy(state.locale, LocalShellCopyOverrides.current)
    val notice = LocalShellNotice.current
    val back = { dispatch(ShellEvent.Back) }
    SideEffect { onThemeChanged(state.theme == ShellTheme.Dark) }
    backHandler(canGoBack(state), back)
    val entryFocus = rememberAuthEntryFocus(state.stack)
    CantosTheme(
        UiPreferences(
            theme = if (state.theme == ShellTheme.Dark) Theme.Dark else Theme.Light,
            density = density,
            reducedMotion = motionPreference == MotionPreference.Reduced,
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
                if (state.route == ShellRoute.MembershipPreview && membershipHeader != null) membershipHeader()
                else if (state.route.isAuth() && authHeader != null) authHeader()
                else ShellHeader(state, copy, dispatch, entryFocus.account)
                state.listeningFailure?.let { failure ->
                    Column(
                        Modifier.fillMaxWidth().padding(tokenDp("space.md")).testTag("listening-failure")
                            .semantics { liveRegion = LiveRegionMode.Polite },
                    ) {
                        Text(copy.text(when (failure) {
                            ShellListeningFailure.MemoryStoreUnavailable -> CopyKey.ListeningStoreFailure
                            ShellListeningFailure.ReleaseUnavailable -> CopyKey.ReleaseUnavailable
                        }))
                        TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.RetryListening) }, modifier = Modifier.testTag("listening-retry")) {
                            Text(copy.text(CopyKey.RetryListening))
                        }
                    }
                }
                val scroll = rememberRouteScroll(state.stack, membershipScreen)
                val framed = state.route == ShellRoute.MembershipPreview || state.route.isAuth()
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                    Column(
                        Modifier.widthIn(max = tokenDp("breakpoint.expanded"))
                            .fillMaxWidth().verticalScroll(scroll).padding(tokenDp(if (framed) "space.section" else "space.md"))
                            .semantics { paneTitle = routeTitle(state.route, copy) }
                            .testTag("screen"),
                        verticalArrangement = Arrangement.spacedBy(tokenDp("space.section")),
                    ) {
                        if (!framed) notice?.invoke(state, copy)
                        if (localeSaveUnavailable) Text(
                            copy.text(CopyKey.LocaleSaveFailure),
                            Modifier.testTag("locale-save-failure").semantics { liveRegion = LiveRegionMode.Polite },
                            color = MaterialTheme.colorScheme.error,
                        )
                        when (val route = state.route) {
                            ShellRoute.Discovery -> DiscoveryScreen(state, catalog, copy, dispatch)
                            ShellRoute.Search -> SearchScreen(state, catalog, copy, dispatch)
                            ShellRoute.Library -> LibraryScreen(state, catalog, copy, dispatch)
                            is ShellRoute.Work -> {
                                val work = catalog.work(route.id)
                                if (work == null) SelectionUnavailable(copy, dispatch) else WorkScreen(work, state, copy, dispatch)
                            }
                            ShellRoute.Player -> PlayerScreen(state, catalog, copy, dispatch)
                            ShellRoute.Queue -> QueueScreen(state, catalog, copy, dispatch)
                            ShellRoute.Account -> accountContent(entryFocus.login)
                            ShellRoute.Login, ShellRoute.Register -> authContent()
                            ShellRoute.MembershipPreview -> membershipContent()
                            ShellRoute.History -> HistoryScreen(state, catalog, copy, dispatch)
                            ShellRoute.Settings -> SettingsScreen(state, copy, dispatch)
                            ShellRoute.Unavailable -> SelectionUnavailable(copy, dispatch)
                        }
                    }
                }
                // Auth is a focused task: no mini-player or tab bar can cover its Back or actions.
                if (state.route != ShellRoute.Player && state.route != ShellRoute.Queue && !framed) {
                    MiniPlayer(state, catalog, copy, dispatch)
                    ShellTabs(state, copy, dispatch)
                }
            }
        }
    }
}

private fun routeTitle(route: ShellRoute, copy: ShellCopy): String = copy.text(
    when (route) {
        ShellRoute.Discovery -> CopyKey.Discover
        ShellRoute.Search -> CopyKey.Search
        ShellRoute.Library -> CopyKey.Library
        is ShellRoute.Work -> CopyKey.Work
        ShellRoute.Player -> CopyKey.Player
        ShellRoute.Queue -> CopyKey.Queue
        ShellRoute.Account -> CopyKey.Account
        ShellRoute.MembershipPreview -> CopyKey.MembershipPreview
        ShellRoute.Login -> CopyKey.SignIn
        ShellRoute.Register -> CopyKey.CreateAccount
        ShellRoute.History -> CopyKey.History
        ShellRoute.Settings -> CopyKey.Settings
        ShellRoute.Unavailable -> CopyKey.SelectionUnavailable
    },
)

private fun headerTitleKey(route: ShellRoute): CopyKey = when (route) {
    ShellRoute.Player -> CopyKey.Player
    ShellRoute.Account -> CopyKey.AccountSpace
    ShellRoute.MembershipPreview -> CopyKey.Brand
    ShellRoute.Discovery, ShellRoute.Search, ShellRoute.Library, is ShellRoute.Work, ShellRoute.Queue,
    ShellRoute.History, ShellRoute.Settings, ShellRoute.Unavailable, ShellRoute.Login, ShellRoute.Register -> CopyKey.Brand
}

@Composable
private fun ShellHeader(state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, accountFocus: FocusRequester) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = tokenDp("space.sm"), vertical = tokenDp("space.xs")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (canGoBack(state)) {
            IconButton(onClick = { dispatch(ShellEvent.Back) }, modifier = Modifier.size(tokenDp("size.control")).testTag("back")) {
                Icon(
                    if (state.route == ShellRoute.Player) Icons.Outlined.Close else Icons.AutoMirrored.Outlined.ArrowBack,
                    copy.text(if (state.route == ShellRoute.Player) CopyKey.ClosePlayer else CopyKey.Back),
                )
            }
        }
        Text(
            copy.text(headerTitleKey(state.route)),
            modifier = Modifier.weight(1f).padding(horizontal = tokenDp("space.sm"))
                .then(if (state.route == ShellRoute.Account) Modifier.semantics { heading() } else Modifier)
                .testTag("header-title"),
            style = if (state.route == ShellRoute.Player) MaterialTheme.typography.labelLarge
                else MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.primary,
        )
        if (state.route != ShellRoute.Account) {
            IconButton(onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Account)) },
                modifier = Modifier.size(tokenDp("size.control")).focusRequester(accountFocus).testTag("account")) {
                Icon(Icons.Outlined.AccountCircle, copy.text(CopyKey.Account))
            }
        }
    }
}

@Composable
private fun DiscoveryScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(CopyKey.ContinuingHeading), color = MaterialTheme.colorScheme.tertiary,
            style = MaterialTheme.typography.labelLarge)
        Text(copy.text(CopyKey.Welcome), style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        val point = continuingProgress(state).firstOrNull()
        if (point != null) ListeningPoint(point, catalog, copy, dispatch, fromHistory = false, tag = "continue")
        TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Search)) }, modifier = Modifier.testTag("search")) {
            Icon(Icons.Outlined.Search, null)
            Text(copy.text(CopyKey.Search))
        }
    }
    Text(copy.text(CopyKey.DiscoverHeading), style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
        modifier = Modifier.semantics { heading() })
    if (state.catalogMode == ShellCatalogMode.Ready && catalog.works.isNotEmpty()) {
        WorkSummary(catalog.works.first(), state, copy, dispatch, Modifier, featured = true)
        if (catalog.works.size > 1) {
            SectionHeading(CopyKey.ExploreHeading, copy)
            WorkShelf(catalog.works.drop(1), state, copy, dispatch)
        }
        ListFooter(catalog.worksPhase, copy, dispatch)
    } else {
        CatalogRecovery(if (state.catalogMode == ShellCatalogMode.Ready) ShellCatalogMode.Empty else state.catalogMode, copy, dispatch)
    }
}

@Composable
internal fun WorkShelf(works: List<ShellWork>, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
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
    work: ShellWork,
    state: ShellState,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    modifier: Modifier,
    featured: Boolean = false,
) {
    Card(
        onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Work(work.id))) },
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
                        UnitCount(work, copy, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            Text(work.description)
            if (featured) UnitCount(work, copy, color = MaterialTheme.colorScheme.onSurfaceVariant)
            SaveWorkButton(work, state, copy, dispatch, "save-card-${work.id}")
        }
    }
}

@Composable
private fun SearchScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Text(copy.text(CopyKey.Search), style = MaterialTheme.typography.headlineSmall,
        modifier = Modifier.semantics { heading() })
    OutlinedTextField(
        shape = MaterialTheme.shapes.medium,
        value = state.query, onValueChange = { dispatch(ShellEvent.Search(it)) },
        modifier = Modifier.fillMaxWidth().testTag("search-input"),
        label = { Text(copy.text(CopyKey.SearchLabel)) },
        placeholder = { Text(copy.text(CopyKey.SearchHint)) },
        leadingIcon = { Icon(Icons.Outlined.Search, null) },
        trailingIcon = {
            if (state.query.isNotEmpty()) IconButton(onClick = { dispatch(ShellEvent.Search("")) }) {
                Icon(Icons.Outlined.Close, copy.text(CopyKey.SearchClear))
            }
        },
    )
    val works = filteredWorks(state, catalog)
    // A partial list has no honest total: the count is shown only once the list is complete.
    if (catalog.worksPhase == ShellListPhase.Complete) {
        Text(copy.text(CopyKey.Results, "count" to works.size.toString()), style = MaterialTheme.typography.bodySmall)
    }
    when {
        state.catalogMode != ShellCatalogMode.Ready -> CatalogRecovery(state.catalogMode, copy, dispatch)
        works.isEmpty() -> CatalogRecovery(ShellCatalogMode.Empty, copy, dispatch)
        else -> {
            WorkShelf(works, state, copy, dispatch)
            ListFooter(catalog.worksPhase, copy, dispatch)
        }
    }
}

@Composable
internal fun SelectionUnavailable(copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Column(Modifier.testTag("selection-unavailable"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(copy.text(CopyKey.SelectionUnavailable), style = MaterialTheme.typography.titleLarge)
        Text(copy.text(CopyKey.SelectionUnavailableNote))
        Button(shape = MaterialTheme.shapes.medium, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) }, modifier = Modifier.testTag("selection-discover")) {
            Text(copy.text(CopyKey.Discover))
        }
    }
}

@Composable
private fun CatalogRecovery(mode: ShellCatalogMode, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val title = when (mode) {
        ShellCatalogMode.Ready -> CopyKey.Ready
        ShellCatalogMode.Loading -> CopyKey.Loading
        ShellCatalogMode.Empty -> CopyKey.Empty
        ShellCatalogMode.Error -> CopyKey.Error
    }
    val message = when (mode) {
        ShellCatalogMode.Ready -> CopyKey.Ready
        ShellCatalogMode.Loading -> CopyKey.LoadingNote
        ShellCatalogMode.Empty -> CopyKey.EmptyNote
        ShellCatalogMode.Error -> CopyKey.ErrorNote
    }
    Column(Modifier.semantics { liveRegion = LiveRegionMode.Polite }, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(copy.text(title), style = MaterialTheme.typography.titleLarge)
        Text(copy.text(message))
        if (mode == ShellCatalogMode.Error || mode == ShellCatalogMode.Loading) {
            Button(shape = MaterialTheme.shapes.medium, onClick = { dispatch(ShellEvent.Retry) }, modifier = Modifier.testTag("retry")) { Text(copy.text(CopyKey.Retry)) }
        } else {
            TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.Search("")); dispatch(ShellEvent.Retry); dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) }) {
                Text(copy.text(CopyKey.Discover))
            }
        }
    }
}

@Composable
private fun WorkScreen(work: ShellWork, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    WorkArtwork(work.id, Modifier.fillMaxWidth().aspectRatio(1.6f))
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(work.category, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.tertiary)
        Text(work.title, style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        Text(work.description)
        UnitCount(work, copy)
    }
    when (work.detailPhase) {
        ShellDetailPhase.Ready -> Unit
        ShellDetailPhase.Loading -> Text(copy.text(CopyKey.Loading), Modifier.testTag("work-loading")
            .semantics { liveRegion = LiveRegionMode.Polite })
        ShellDetailPhase.Failed -> CatalogRecovery(ShellCatalogMode.Error, copy, dispatch)
        ShellDetailPhase.NotFound -> {
            Column(Modifier.testTag("work-not-found").semantics { liveRegion = LiveRegionMode.Polite },
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(copy.text(CopyKey.WorkNotFound), style = MaterialTheme.typography.titleLarge)
                Text(copy.text(CopyKey.WorkNotFoundNote))
            }
        }
    }
    if (work.detailPhase != ShellDetailPhase.Ready) return
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        val firstEpisode = work.episodes.firstOrNull()
        Button(shape = MaterialTheme.shapes.medium, enabled = firstEpisode != null && firstEpisode.durationSeconds > 0,
            onClick = { firstEpisode?.let { dispatch(ShellEvent.OpenEpisode(it.id)) } }) {
            Icon(Icons.Outlined.PlayArrow, null)
            Text(copy.text(CopyKey.StartEpisode))
        }
        SaveWorkButton(work, state, copy, dispatch, "save-work")
    }
    Text(copy.text(CopyKey.SavedOnlyHere), style = MaterialTheme.typography.bodySmall)
    Text(copy.text(if (work.unitKind == ShellUnitKind.Chapter) CopyKey.Chapters else CopyKey.Episodes),
        style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
    if (work.episodes.isEmpty() && work.unitsPhase == ShellListPhase.Complete) {
        Text(copy.text(CopyKey.SelectionUnavailableNote))
    }
    work.episodes.forEach { EpisodeSelection(it, state, copy, dispatch) }
    ListFooter(work.unitsPhase, copy, dispatch, first = work.episodes.isEmpty())
}

/** The episode or chapter count, shown only when the list is complete so it is never a partial total. */
@Composable
private fun UnitCount(work: ShellWork, copy: ShellCopy, color: androidx.compose.ui.graphics.Color = androidx.compose.ui.graphics.Color.Unspecified) {
    if (work.unitsPhase != ShellListPhase.Complete) return
    Text(copy.text(if (work.unitKind == ShellUnitKind.Chapter) CopyKey.ChaptersCount else CopyKey.EpisodesCount,
        "count" to work.episodes.size.toString()), style = MaterialTheme.typography.bodySmall, color = color)
}

@Composable
internal fun EpisodeSelection(episode: ShellEpisode, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val selected = episode.id == state.selectedEpisode
    Surface(
        onClick = { dispatch(ShellEvent.OpenEpisode(episode.id)) },
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
            if (episode.numbered) Text(episode.index.toString(), style = MaterialTheme.typography.bodySmall)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(episode.title, style = MaterialTheme.typography.titleMedium,
                    maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(durationCaption(episode.durationSeconds, copy), style = MaterialTheme.typography.bodySmall)
                if (selected) Text(copy.text(CopyKey.CurrentEpisode), style = MaterialTheme.typography.bodySmall)
            }
            Icon(Icons.Outlined.PlayArrow, null)
        }
    }
    if (episode.durationSeconds <= 0) Text(copy.text(CopyKey.SelectionUnavailableNote), style = MaterialTheme.typography.bodySmall)
}

@Composable
private fun SaveWorkButton(work: ShellWork, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, tag: String) {
    val saved = work.id in state.savedWorks
    FilledTonalButton(shape = MaterialTheme.shapes.medium,
        onClick = { dispatch(ShellEvent.SetSaved(work.id, saved = !saved)) },
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
internal fun SectionHeading(key: CopyKey, copy: ShellCopy) {
    Text(copy.text(key), style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
        modifier = Modifier.semantics { heading() })
}

@Composable
private fun ShellTabs(state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val root = state.stack.firstOrNull()
    NavigationBar(containerColor = MaterialTheme.colorScheme.surface, windowInsets = androidx.compose.foundation.layout.WindowInsets(0)) {
        NavigationBarItem(selected = root == ShellRoute.Discovery || root == ShellRoute.Search, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) },
            icon = { Icon(Icons.Outlined.Home, null) }, label = { Text(copy.text(CopyKey.Discover)) }, modifier = Modifier.testTag("nav-discovery"))
        NavigationBarItem(selected = root == ShellRoute.Library || root == ShellRoute.History, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Library)) },
            icon = { Icon(Icons.AutoMirrored.Outlined.MenuBook, null) }, label = { Text(copy.text(CopyKey.Library)) }, modifier = Modifier.testTag("nav-library"))
    }
}
