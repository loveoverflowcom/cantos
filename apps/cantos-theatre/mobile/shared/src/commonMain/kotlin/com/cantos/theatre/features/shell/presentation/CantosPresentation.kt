package com.cantos.theatre.features.shell.presentation

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.clickable
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.unit.Dp
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.errors.ErrorAction
import com.cantos.theatre.application.errors.ErrorNotice
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.LegacyCopyKey
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
import com.cantos.theatre.presentation.model.ShellSearchKind
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellUnitKind
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.canGoBack
import com.cantos.theatre.presentation.model.continuingProgress
import com.cantos.theatre.presentation.model.filteredWorks
import com.cantos.theatre.presentation.model.generated.ShellMessage
import com.cantos.theatre.presentation.model.isAccountTask
import com.cantos.ui.Button as SharedButton
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.AccessibleName
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.CantosExpression
import com.cantos.theatre.presentation.model.WorkListenPhase
import com.cantos.theatre.presentation.model.workListenProjection
import com.cantos.ui.CantosShapes
import com.cantos.ui.CantosTheme
import com.cantos.ui.Density
import com.cantos.ui.FieldState
import com.cantos.ui.Announcement
import com.cantos.ui.LoadingState
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.LogoVariant
import com.cantos.ui.MotionIntent
import com.cantos.ui.MotionPreference
import com.cantos.ui.MotionProperty
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.rememberMotionSpec
import com.cantos.ui.toAnimationSpec
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

/** Optional host-owned presentation additions; absent in an ordinary application mount. */
typealias ShellNoticeContent = @Composable (ShellState, ShellCopy) -> Unit
typealias ShellDiagnosticsContent = @Composable (ShellState, (ShellEvent) -> Unit, () -> Unit) -> Unit

class ShellDiagnosticsSlot(
    val title: String,
    val summary: String,
    val content: ShellDiagnosticsContent,
)

val LocalShellLiveCopy = staticCompositionLocalOf { false }
val LocalShellNotice = staticCompositionLocalOf<ShellNoticeContent?> { null }
val LocalShellDiagnostics = staticCompositionLocalOf<ShellDiagnosticsSlot?> { null }

/** Host facts about what it can do; the default offers the full sample experience. */
val LocalShellCapabilities = staticCompositionLocalOf { ShellCapabilities() }

/** Optional host content at the head of Library: sign-in prompt, sync status, conflicts, paging. */
val LocalShellSharedLibraryHasItems = staticCompositionLocalOf { false }
val LocalShellSharedLibrary = staticCompositionLocalOf<(@Composable () -> Unit)?> { null }
val LocalShellSharedDetail = staticCompositionLocalOf<(@Composable () -> Unit)?> { null }

val LocalShellLibraryHeader = staticCompositionLocalOf<(@Composable () -> Unit)?> { null }

/** The same controlled presentation is mounted by Android, iOS and the JVM diagnostic host.
 * Native Material controls consume the canonical theme. Membership and the optional account flow
 * supply slots composed from the shared design-v2 primitives; their bounded admission is recorded
 * separately. The shell owns only where the slots sit, never an account decision.
 */
@OptIn(ExperimentalComposeUiApi::class)
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
    // Receives the requester that Back restores when it closes an auth intent opened from Account, and
    // a slot that registers an open confirmation sheet's dismissal, which takes Back before the route does.
    accountContent: @Composable (loginFocus: FocusRequester, backOverlay: @Composable (onBack: () -> Unit) -> Unit) -> Unit = { _, _ -> },
    // Login, Register and email verification bodies; the root reads the route from its own projection.
    authContent: @Composable () -> Unit = {},
    authHeader: (@Composable () -> Unit)? = null,
    // Hosts may inject Full to observe motion; reduced motion is the default.
    motionPreference: MotionPreference = MotionPreference.Reduced,
    density: Density = Density.Comfortable,
    localeSaveUnavailable: Boolean = false,
    activeError: ErrorNotice? = null,
    onErrorAction: (ErrorAction) -> Unit = {},
    onErrorDismiss: () -> Unit = {},
    errorReturnFocus: FocusRequester? = null,
    // Appearance is resolved by the host; the controls retain the actual chosen mode.
    themeMode: ShellThemeMode = state.theme.mode(),
    accountSignedIn: Boolean = false,
    accountPresentation: com.cantos.theatre.domain.identity.AccountPresentation? = null,
) {
    var surfaceBounds by remember { mutableStateOf(Rect.Zero) }
    val copy = ShellCopy(state.locale, LocalShellLiveCopy.current)
    val notice = LocalShellNotice.current
    // An open confirmation sheet owns Back before the route stack does.
    val overlays = remember { BackOverlayRegistry() }
    val back = { if (activeError != null) onErrorDismiss() else if (!overlays.dismiss()) dispatch(ShellEvent.Back) }
    SideEffect { onThemeChanged(state.theme == ShellTheme.Dark) }
    backHandler(activeError != null || overlays.active || canGoBack(state), back)
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
        androidx.compose.runtime.CompositionLocalProvider(LocalLoadingViewport provides surfaceBounds) {
        Surface(modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Column(
                Modifier.fillMaxSize().safeDrawingPadding().imePadding()
                    .onGloballyPositioned { surfaceBounds = it.boundsInWindow() }
                    .pointerInput(overlays) {
                        awaitPointerEventScope {
                            while (true) {
                                val event = awaitPointerEvent()
                                if (event.type == PointerEventType.Press) {
                                    event.changes.firstOrNull()?.let {
                                        overlays.dismissOutside(it.position + surfaceBounds.topLeft)
                                    }
                                }
                            }
                        }
                    }.onPreviewKeyEvent {
                    if (it.key == Key.Escape && it.type == KeyEventType.KeyUp && (overlays.active || canGoBack(state))) {
                        back()
                        true
                    } else {
                        false
                    }
                },
            ) {
                if (state.route == ShellRoute.MembershipPreview && membershipHeader != null) membershipHeader()
                else if (state.route.isAccountTask() && authHeader != null) {
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                        Box(Modifier.widthIn(max = AUTH_FRAME_MAX_WIDTH).fillMaxWidth()) { authHeader() }
                    }
                }
                // Search is a fullscreen task with its own Back and input; no shared header repeats above it.
                else if (state.route != ShellRoute.Search) ShellHeader(state, copy, dispatch, entryFocus.account, accountSignedIn, accountPresentation)
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
                val framed = state.route == ShellRoute.MembershipPreview || state.route.isAccountTask()
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                    if (state.route == ShellRoute.Search) Column(
                        Modifier.widthIn(max = tokenDp("breakpoint.expanded")).fillMaxSize().padding(tokenDp("space.md"))
                            .semantics { paneTitle = routeTitle(state.route, copy) }
                            .testTag("screen"),
                        verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
                    ) {
                        ShellBanners(state, copy, notice, localeSaveUnavailable)
                        SearchScreen(state, catalog, copy, dispatch, Modifier.weight(1f).fillMaxWidth())
                    } else Column(
                        Modifier.widthIn(max = if (state.route.isAccountTask()) AUTH_FRAME_MAX_WIDTH else tokenDp("breakpoint.expanded"))
                            .fillMaxWidth().verticalScroll(scroll).padding(tokenDp(if (framed) "space.section" else "space.md"))
                            .semantics { paneTitle = routeTitle(state.route, copy) }
                            .testTag("screen"),
                        verticalArrangement = Arrangement.spacedBy(tokenDp("space.section")),
                    ) {
                        ShellBanners(state, copy, notice.takeUnless { framed }, localeSaveUnavailable)
                        when (val route = state.route) {
                            ShellRoute.Discovery -> DiscoveryScreen(state, catalog, copy, dispatch)
                            ShellRoute.Search -> Unit // Fullscreen: rendered above, outside the scrolling column.
                            ShellRoute.Library -> LibraryScreen(state, catalog, copy, dispatch)
                            is ShellRoute.SharedWork -> LocalShellSharedDetail.current?.invoke() ?: SelectionUnavailable(copy, dispatch)
                            is ShellRoute.Work -> {
                                val work = catalog.work(route.id)
                                if (work == null) SelectionUnavailable(copy, dispatch) else WorkScreen(work, state, copy, dispatch)
                            }
                            ShellRoute.Player -> PlayerScreen(state, catalog, copy, dispatch)
                            ShellRoute.Queue -> QueueScreen(state, catalog, copy, dispatch)
                            ShellRoute.Account -> accountContent(entryFocus.login) { onBack -> BackOverlay(overlays, onBack) }
                            ShellRoute.Login, ShellRoute.Register, ShellRoute.VerifyEmail -> authContent()
                            ShellRoute.MembershipPreview -> membershipContent()
                            ShellRoute.History -> HistoryScreen(state, catalog, copy, dispatch)
                            ShellRoute.Settings -> SettingsScreen(state, copy, dispatch, themeMode)
                            ShellRoute.Unavailable -> SelectionUnavailable(copy, dispatch)
                        }
                    }
                }
                // Auth is a focused task: no mini-player or tab bar can cover its Back or actions.
                // Search is fullscreen: the tab bar and mini-player step aside while playback continues.
                if (state.route != ShellRoute.Player && state.route != ShellRoute.Queue && state.route != ShellRoute.Search && !framed) {
                    MiniPlayer(state, catalog, copy, dispatch)
                    ShellTabs(state, copy, dispatch)
                }
            }
            if (activeError != null) {
                ErrorDialog(
                    notice = activeError,
                    copy = copy,
                    onAction = onErrorAction,
                    onDismiss = onErrorDismiss,
                    returnFocus = errorReturnFocus,
                )
            }
        }
        }
    }
}

@Composable
private fun ShellBanners(state: ShellState, copy: ShellCopy, notice: ShellNoticeContent?, localeSaveUnavailable: Boolean) {
    notice?.invoke(state, copy)
    if (localeSaveUnavailable) Text(
        copy.text(CopyKey.LocaleSaveFailure),
        Modifier.testTag("locale-save-failure").semantics { liveRegion = LiveRegionMode.Polite },
        color = MaterialTheme.colorScheme.error,
    )
}

// Auth frame cap from the Auth v2 `.mobile .page { max-width: 430px }` recipe; header and body share it
// so a tablet keeps a compact, centered form without changing the app-wide `breakpoint.expanded`.
private val AUTH_FRAME_MAX_WIDTH = 430.dp

private fun routeTitle(route: ShellRoute, copy: ShellCopy): String = copy.text(
    when (route) {
        ShellRoute.Discovery -> CopyKey.Discover
        ShellRoute.Search -> CopyKey.Search
        ShellRoute.Library -> CopyKey.Library
        is ShellRoute.Work -> CopyKey.Work
        is ShellRoute.SharedWork -> CopyKey.SharedTitle
        ShellRoute.Player -> CopyKey.Player
        ShellRoute.Queue -> CopyKey.Queue
        ShellRoute.Account -> CopyKey.Account
        ShellRoute.MembershipPreview -> CopyKey.MembershipPreview
        ShellRoute.Login -> CopyKey.SignIn
        ShellRoute.Register, ShellRoute.VerifyEmail -> CopyKey.Register
        ShellRoute.History -> CopyKey.History
        ShellRoute.Settings -> CopyKey.Settings
        ShellRoute.Unavailable -> CopyKey.SelectionUnavailable
    },
)

private fun headerTitleKey(route: ShellRoute): CopyKey = when (route) {
    ShellRoute.Player -> CopyKey.Player
    ShellRoute.Account -> CopyKey.Account
    ShellRoute.MembershipPreview -> CopyKey.Brand
    ShellRoute.Discovery, ShellRoute.Search, ShellRoute.Library, is ShellRoute.Work, is ShellRoute.SharedWork, ShellRoute.Queue,
    ShellRoute.History, ShellRoute.Settings, ShellRoute.Unavailable, ShellRoute.Login, ShellRoute.Register,
    ShellRoute.VerifyEmail -> CopyKey.Brand
}

@Composable
private fun ShellHeader(
    state: ShellState,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    accountFocus: FocusRequester,
    accountSignedIn: Boolean,
    accountPresentation: com.cantos.theatre.domain.identity.AccountPresentation?,
) {
    val branded = topBarShowsBrandMark(state.route)
    BoxWithConstraints(
        Modifier.fillMaxWidth().padding(horizontal = tokenDp("space.sm"), vertical = tokenDp("space.xs")),
    ) {
        val reflow = maxWidth < tokenDp("breakpoint.medium") && LocalDensity.current.fontScale > 1.3f
        val brandWidth = if (reflow || state.route == ShellRoute.Account) maxWidth
            else (maxWidth - tokenDp("size.avatarControl")).coerceAtLeast(tokenDp("size.control"))
        FlowRow(
            Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween,
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs")),
        ) {
        // Keep the same two groups mounted as text grows; only their constraints change.
        Row(Modifier.widthIn(max = brandWidth).heightIn(min = tokenDp("size.control")),
            verticalAlignment = Alignment.CenterVertically) {
        if (canGoBack(state)) {
            IconButton(onClick = { dispatch(ShellEvent.Back) }, modifier = Modifier.size(tokenDp("size.control")).testTag("back")) {
                Icon(
                    if (state.route == ShellRoute.Player) rememberCantosIconVector("close") else rememberCantosIconVector("arrow_back"),
                    copy.text(if (state.route == ShellRoute.Player) CopyKey.ClosePlayer else CopyKey.Back),
                )
            }
        }
        if (branded) {
            Logo(LogoVariant.Symbol, Modifier.size(tokenDp("size.brandMark")).testTag("header-logo"),
                tone = if (state.theme == ShellTheme.Dark) LogoTone.Dark else LogoTone.Light)
        }
        Text(
            copy.text(headerTitleKey(state.route)),
            modifier = Modifier.weight(1f).padding(start = tokenDp(if (branded) "space.inset" else "space.sm"), end = tokenDp("space.sm"))
                .then(if (state.route == ShellRoute.Account) Modifier.semantics { heading() } else Modifier)
                .testTag("header-title"),
            style = if (branded) MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight(CantosTokens.number("type.theatreHeading.weight").toInt()))
                else if (state.route == ShellRoute.Player) MaterialTheme.typography.labelLarge
                else MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.primary,
        )
        }
        if (state.route != ShellRoute.Account) Row(if (reflow) Modifier.fillMaxWidth() else Modifier,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"), Alignment.End),
            verticalAlignment = Alignment.CenterVertically) {
            TopBarAccountButton(
                label = copy.text(CopyKey.Account),
                signedIn = accountSignedIn,
                presentation = accountPresentation,
                onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Account)) },
                modifier = Modifier.focusRequester(accountFocus).testTag("account"),
            )
        }
        }
    }
}

@Composable
private fun DiscoveryScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val point = continuingProgress(state).firstOrNull()
    Surface(Modifier.fillMaxWidth().testTag("discovery-hero"),
        color = tokenColor("color.theatreHero"), contentColor = tokenColor("color.theatreOnHero"),
        shape = MaterialTheme.shapes.extraLarge) {
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            if (point != null) Text(copy.text(CopyKey.ContinuingHeading), style = MaterialTheme.typography.labelLarge)
            Text(copy.text(if (point != null) CopyKey.Welcome else CopyKey.DiscoveryWelcome), style = MaterialTheme.typography.headlineSmall.copy(
                fontWeight = FontWeight(CantosTokens.number("type.theatreHeading.weight").toInt())),
                modifier = Modifier.semantics { heading() })
            DiscoverySearchPill(state.query, copy) { dispatch(ShellEvent.Navigate(ShellRoute.Search)) }
        }
    }
    if (point != null) ListeningPoint(point, catalog, copy, dispatch, fromHistory = false, tag = "continue")
    Box(Modifier.fillMaxWidth()) {
        Text(copy.text(CopyKey.DiscoverHeading), style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
            modifier = Modifier.semantics { heading() })
        RequestEdge(catalog.request == com.cantos.theatre.presentation.model.ShellCatalogRequest.Replacement,
            copy.text(CopyKey.Loading), "discovery-refresh-edge", Modifier.align(Alignment.BottomCenter))
    }
    if (state.catalogMode == ShellCatalogMode.Ready && catalog.works.isNotEmpty()) {
        WorkShelf(catalog.works, state, copy, dispatch)
        ListFooter(if (catalog.request == com.cantos.theatre.presentation.model.ShellCatalogRequest.Replacement) ShellListPhase.Complete else catalog.worksPhase, copy, dispatch)
    } else {
        if (state.catalogMode == ShellCatalogMode.Ready && catalog.worksPhase != ShellListPhase.Complete) {
            ListFooter(catalog.worksPhase, copy, dispatch, first = true)
        } else CatalogRecovery(if (state.catalogMode == ShellCatalogMode.Ready) ShellCatalogMode.Empty else state.catalogMode, copy, dispatch, state.theme)
    }
}

/** Navigation affordance over the admitted field; only the Search route owns editable input. */
@Composable
private fun DiscoverySearchPill(query: String, copy: ShellCopy, openSearch: () -> Unit) {
    Box(Modifier.fillMaxWidth()) {
        TextField(
            label = copy.text(CopyKey.SearchLabel), value = query, onValueChange = {},
            modifier = Modifier.clearAndSetSemantics { },
            state = FieldState.ReadOnly, variant = TextFieldVariant.FloatingSearch,
            controlModifier = Modifier.focusProperties { canFocus = false },
            trailing = {
                com.cantos.ui.IconButton(
                    name = com.cantos.ui.AccessibleName.of(copy.text(CopyKey.Search)), icon = "search",
                    onPress = openSearch, variant = com.cantos.ui.ButtonVariant.Primary, circular = true,
                    controlModifier = Modifier.size(tokenDp("size.searchAction")).focusProperties { canFocus = false },
                )
            },
        )
        // The navigation target covers the read-only visual, including its text area. The
        // editable field and keyboard focus exist only after entering the existing Search route.
        Box(Modifier.matchParentSize().testTag("search").clickable(role = Role.Button, onClick = openSearch)
            .semantics { contentDescription = copy.text(CopyKey.SearchLabel) })
    }
}

@Composable
internal fun WorkShelf(works: List<ShellWork>, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val columns = if (maxWidth >= tokenDp("breakpoint.medium")) 2 else 1
        val gap = tokenDp("space.md")
        val cardWidth = (maxWidth - gap * (columns - 1)) / columns
        FlowRow(horizontalArrangement = Arrangement.spacedBy(gap), verticalArrangement = Arrangement.spacedBy(gap)) {
            works.forEach { work ->
                key(work.id) { WorkSummary(work, state, copy, dispatch, Modifier.width(cardWidth)) }
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
) {
    Card(
        onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Work(work.id))) },
        modifier = modifier.fillMaxWidth().testTag("work-${work.id}")
            .semantics { contentDescription = copy.text(ShellMessage.Details(title = work.title)) },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = MaterialTheme.shapes.large,
    ) {
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Row(verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
                WorkArtwork(work.id, Modifier.size(tokenDp("size.control") + tokenDp("space.xl")), shape = CantosShapes.thumbnail, cover = work.cover)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                    Text(work.category, color = MaterialTheme.colorScheme.tertiary, style = MaterialTheme.typography.bodySmall)
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                        Text(work.title, style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
                            modifier = Modifier.weight(1f))
                        Icon(rememberCantosIconVector("arrow_forward"), null)
                    }
                    UnitCount(work, copy, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Text(work.description, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
            SaveWorkButton(work, state, copy, dispatch, "save-card-${work.id}")
        }
    }
}

/**
 * Search v4.4 fullscreen: a leading Back, one input pill, the story-type chips and an independently
 * scrolling result list. It is the content of the existing Search route, never a dock over Discovery,
 * and every request still goes through the shared catalog controller.
 */
@Composable
private fun SearchScreen(
    state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    val focus = remember { FocusRequester() }
    // A returning listener who already has text is not greeted by the keyboard again.
    LaunchedEffect(Unit) { if (state.query.isEmpty()) focus.requestFocus() }
    var entered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { entered = true }
    val entrance by animateFloatAsState(
        if (entered) 1f else 0f, rememberMotionSpec(MotionIntent.Navigation, MotionProperty.Effects).toAnimationSpec(), label = "search-entrance")
    Column(modifier.graphicsLayer { alpha = entrance }, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Row(Modifier.fillMaxWidth().testTag("search-hero"), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalAlignment = Alignment.CenterVertically) {
            // Standard Material tonal control: the shared tonal IconButton and FilterChip are not yet admitted.
            FilledTonalIconButton(onClick = { dispatch(ShellEvent.Back) },
                modifier = Modifier.size(tokenDp("size.searchAction")).testTag("back")) {
                Icon(rememberCantosIconVector("arrow_back"), copy.text(CopyKey.Back))
            }
            TextField(
                label = copy.text(CopyKey.SearchLabel), value = state.query,
                onValueChange = { dispatch(ShellEvent.Search(it)) },
                modifier = Modifier.weight(1f),
                controlModifier = Modifier.testTag("search-input").focusRequester(focus),
                variant = TextFieldVariant.FloatingSearch,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { dispatch(ShellEvent.SubmitSearch) }),
                trailing = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        // The slot is always reserved, so the pill does not resize when text appears.
                        Box(Modifier.size(tokenDp("size.control"))) {
                            if (state.query.isNotEmpty()) IconButton(onClick = {
                                dispatch(ShellEvent.Search("")); focus.requestFocus()
                            }, modifier = Modifier.size(tokenDp("size.control")).testTag("search-clear")) {
                                Icon(rememberCantosIconVector("close"), copy.text(CopyKey.SearchClearInput))
                            }
                        }
                        com.cantos.ui.IconButton(
                            name = AccessibleName.of(copy.text(CopyKey.SearchSubmit)), icon = "search",
                            onPress = { dispatch(ShellEvent.SubmitSearch) },
                            variant = ButtonVariant.Primary, circular = true,
                            controlModifier = Modifier.size(tokenDp("size.searchAction")).testTag("search-submit"),
                        )
                    }
                },
            )
        }
        SearchKindChips(state.searchKind, copy, dispatch)
        RequestEdge(catalog.request == com.cantos.theatre.presentation.model.ShellCatalogRequest.Replacement, copy.text(CopyKey.Loading), "search-request-edge")
        SearchResults(state, catalog, copy, dispatch, Modifier.weight(1f))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SearchKindChips(selected: ShellSearchKind, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    FlowRow(
        Modifier.fillMaxWidth().selectableGroup().testTag("search-kinds")
            .semantics { contentDescription = copy.text(CopyKey.SearchFilterGroup) },
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs")),
    ) {
        ShellSearchKind.entries.forEach { kind ->
            key(kind) {
                FilterChip(
                    selected = kind == selected,
                    onClick = { dispatch(ShellEvent.SelectSearchKind(kind)) },
                    label = { Text(copy.text(kind.label())) },
                    modifier = Modifier.heightIn(min = tokenDp("size.control")).testTag("search-kind-${kind.name.lowercase()}"),
                    leadingIcon = if (kind == selected) ({ Icon(rememberCantosIconVector("check"), null) }) else null,
                )
            }
        }
    }
}

private fun ShellSearchKind.label(): CopyKey = when (this) {
    ShellSearchKind.All -> CopyKey.SearchFilterAll
    ShellSearchKind.Novel -> CopyKey.KindNovel
    ShellSearchKind.Story -> CopyKey.KindStory
    ShellSearchKind.Podcast -> CopyKey.KindPodcast
}

@Composable
private fun SearchResults(
    state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, modifier: Modifier = Modifier,
) {
    // The source applied text and kind, so the projection is read once per catalog change, never per row.
    val works = remember(state.catalogMode, state.query, catalog) { filteredWorks(state, catalog) }
    val filtered = state.query.isNotBlank() || state.searchKind != ShellSearchKind.All
    val counted = (state.catalogMode == ShellCatalogMode.Ready || state.catalogMode == ShellCatalogMode.Empty) &&
        catalog.worksPhase == ShellListPhase.Complete
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(CopyKey.SearchResultsHeading), Modifier.testTag("search-results-heading").semantics { heading() },
            style = MaterialTheme.typography.titleMedium)
        catalog.appliedSearch?.let { applied -> Text(copy.text(com.cantos.theatre.presentation.model.generated.ShellMessage.DisplayedSearch(
            query = applied.ifBlank { copy.text(CopyKey.Discover) })), Modifier.testTag("displayed-search"), style = MaterialTheme.typography.bodySmall) }
        if (counted) {
            Text(copy.text(LegacyCopyKey.Results, "count" to works.size.toString()), style = MaterialTheme.typography.bodySmall)
        }
        LazyColumn(Modifier.fillMaxWidth().weight(1f).testTag("search-results"),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            when {
                state.catalogMode != ShellCatalogMode.Ready -> item(key = "recovery") {
                    CatalogRecovery(state.catalogMode, copy, dispatch, state.theme, filtered)
                }
                works.isEmpty() && catalog.worksPhase != ShellListPhase.Complete -> item(key = "first-page") {
                    ListFooter(catalog.worksPhase, copy, dispatch, first = true)
                }
                works.isEmpty() -> item(key = "recovery") { CatalogRecovery(ShellCatalogMode.Empty, copy, dispatch, state.theme, filtered) }
                else -> {
                    items(works, key = { it.id.value }) { work -> SearchResultRow(work, copy, dispatch) }
                    item(key = "footer") {
                        ListFooter(if (catalog.request == com.cantos.theatre.presentation.model.ShellCatalogRequest.Replacement) ShellListPhase.Complete else catalog.worksPhase, copy, dispatch)
                    }
                }
            }
        }
    }
}

@Composable
private fun SearchResultRow(work: ShellWork, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Card(
        onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Work(work.id))) },
        modifier = Modifier.fillMaxWidth().testTag("work-${work.id}")
            .semantics { contentDescription = copy.text(ShellMessage.Details(title = work.title)) },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = MaterialTheme.shapes.large,
    ) {
        Row(Modifier.padding(tokenDp("space.sm")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
            WorkArtwork(work.id, Modifier.size(tokenDp("size.control") + tokenDp("space.xl")), shape = CantosShapes.thumbnail, cover = work.cover)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(work.title, style = MaterialTheme.typography.titleMedium.copy(fontFamily = LocalEditorialFontFamily.current),
                    maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(work.category, color = MaterialTheme.colorScheme.tertiary, style = MaterialTheme.typography.bodySmall)
            }
            Icon(rememberCantosIconVector("arrow_forward"), null)
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
private fun CatalogRecovery(mode: ShellCatalogMode, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, theme: ShellTheme, filtered: Boolean = false) {
    val title = when (mode) {
        ShellCatalogMode.Ready -> CopyKey.Ready
        ShellCatalogMode.Loading -> CopyKey.Loading
        ShellCatalogMode.Empty -> if (filtered) CopyKey.Empty else CopyKey.CatalogEmpty
        ShellCatalogMode.Error -> CopyKey.Error
    }
    val message = when (mode) {
        ShellCatalogMode.Ready -> CopyKey.Ready
        ShellCatalogMode.Loading -> CopyKey.LoadingNote
        ShellCatalogMode.Empty -> if (filtered) CopyKey.EmptyNote else CopyKey.CatalogEmptyNote
        ShellCatalogMode.Error -> CopyKey.ErrorNote
    }
    val empty = mode == ShellCatalogMode.Empty
    Column(Modifier.fillMaxWidth().testTag("catalog-recovery").semantics { liveRegion = LiveRegionMode.Polite },
        horizontalAlignment = if (empty) Alignment.CenterHorizontally else Alignment.Start,
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        if (mode == ShellCatalogMode.Empty) TheatreEmptyArtwork(TheatreEmptyArtworkKind.Works, theme,
            Modifier.widthIn(max = 240.dp).fillMaxWidth().aspectRatio(1.2f).align(Alignment.CenterHorizontally).testTag("catalog-empty-artwork"))
        if (mode == ShellCatalogMode.Loading) PendingRows(copy.text(title), "catalog-loading")
        else Text(copy.text(title), Modifier.fillMaxWidth(), style = MaterialTheme.typography.titleLarge,
            textAlign = if (empty) TextAlign.Center else TextAlign.Start)
        Text(copy.text(message), Modifier.fillMaxWidth(), textAlign = if (empty) TextAlign.Center else TextAlign.Start)
        if (mode == ShellCatalogMode.Error) {
            Button(shape = MaterialTheme.shapes.medium, onClick = { dispatch(ShellEvent.Retry) }, modifier = Modifier.testTag("retry")) { Text(copy.text(CopyKey.Retry)) }
        } else if (empty && filtered) {
            TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.Search("")); dispatch(ShellEvent.SelectSearchKind(ShellSearchKind.All)) }, modifier = Modifier.testTag("catalog-clear-search")) {
                Text(copy.text(CopyKey.SearchClear))
            }
        }
    }
}

@Composable
private fun WorkScreen(work: ShellWork, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    // A cover is shown whole (portrait or square) on the tonal surface; the detail never crops it.
    WorkArtwork(work.id, Modifier.fillMaxWidth().aspectRatio(1.6f), cover = work.cover, contentScale = ContentScale.Fit)
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(work.category, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.tertiary)
        Text(work.title, style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        Text(work.description)
        UnitCount(work, copy)
    }
    when (work.detailPhase) {
        ShellDetailPhase.Ready -> Unit
        ShellDetailPhase.Loading -> LoadingState(copy.text(CopyKey.Loading), Modifier.testTag("work-loading"),
            announcement = Announcement.Polite)
            ShellDetailPhase.Failed -> CatalogRecovery(ShellCatalogMode.Error, copy, dispatch, state.theme)
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
        val listen = workListenProjection(work, state.workListenRelease)
        val reason = listen.status?.let(copy::text)
        val availability = when (listen.phase) {
            WorkListenPhase.Resolving, WorkListenPhase.ListLoading -> ButtonState.Loading(DisabledReason.of(requireNotNull(reason)))
            WorkListenPhase.ListFailed, WorkListenPhase.NoTarget, WorkListenPhase.Unavailable -> ButtonState.Disabled(DisabledReason.of(requireNotNull(reason)))
            else -> ButtonState.Enabled
        }
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            CantosTheme(LocalUiPreferences.current.copy(expression = CantosExpression.Theatre)) {
                SharedButton(
                    name = AccessibleName.of(copy.text(ShellMessage.WorkListenName(action = copy.text(listen.label), title = listen.target?.title ?: work.title))),
                    onPress = { listen.event()?.let(dispatch) }, state = availability,
                    variant = ButtonVariant.ListenHero, icon = listen.icon,
                    controlModifier = Modifier.testTag("work-listen"),
                ) { Text(copy.text(listen.label)) }
            }
            if (availability == ButtonState.Enabled && reason != null) {
                Text(reason, style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.testTag("work-listen-status").semantics { liveRegion = LiveRegionMode.Polite })
            }
        }
        SaveWorkButton(work, state, copy, dispatch, "save-work")
    }
    Text(copy.text(CopyKey.SavedOnlyHere), style = MaterialTheme.typography.bodySmall)
    Text(copy.text(if (work.unitKind == ShellUnitKind.Chapter) CopyKey.Chapters else CopyKey.Episodes),
        style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
    if (work.episodes.isEmpty() && work.unitsPhase == ShellListPhase.Complete) {
        Text(copy.text(CopyKey.EpisodeNoAudio))
    }
    work.episodes.forEach { episode -> key(episode.id) { EpisodeSelection(episode, state, copy, dispatch) } }
    ListFooter(work.unitsPhase, copy, dispatch, first = work.episodes.isEmpty())
}

/** The episode or chapter count, shown only when the list is complete so it is never a partial total. */
@Composable
private fun UnitCount(work: ShellWork, copy: ShellCopy, color: androidx.compose.ui.graphics.Color = androidx.compose.ui.graphics.Color.Unspecified) {
    if (work.unitsPhase != ShellListPhase.Complete) return
    Text(copy.text(if (work.unitKind == ShellUnitKind.Chapter) LegacyCopyKey.ChaptersCount else LegacyCopyKey.EpisodesCount,
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
            contentDescription = copy.text(ShellMessage.PlayEpisode(title = episode.title))
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
            Icon(rememberCantosIconVector("play_arrow"), null)
        }
    }
    if (episode.durationSeconds <= 0) Text(copy.text(CopyKey.EpisodeNoAudio), style = MaterialTheme.typography.bodySmall)
}

@Composable
internal fun SaveWorkButton(work: ShellWork, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, tag: String) {
    val saved = work.id in state.confirmedSavedWorks
    val pending = work.id in state.pendingSavedWorks
    val failed = work.id in state.failedSavedWorks
    val desiredSaved = work.id in state.savedWorks
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        FilledTonalButton(shape = RoundedCornerShape(tokenDp("shape.pill")),
            enabled = !pending && !failed,
            colors = ButtonDefaults.filledTonalButtonColors(
                containerColor = if (saved && !pending) tokenColor("color.savedWork") else MaterialTheme.colorScheme.surfaceContainerHigh,
                contentColor = if (saved && !pending) tokenColor("color.onSavedWork") else MaterialTheme.colorScheme.onSurface,
                disabledContainerColor = if (failed && saved && !pending) tokenColor("color.savedWork") else MaterialTheme.colorScheme.surfaceContainerHigh,
                disabledContentColor = if (failed && saved && !pending) tokenColor("color.onSavedWork") else MaterialTheme.colorScheme.onSurface,
            ),
            onClick = { dispatch(ShellEvent.SetSaved(work.id, saved = !saved)) },
            modifier = Modifier.heightIn(min = tokenDp("size.control")).widthIn(min = tokenDp("size.savedWork")).testTag(tag).semantics {
                contentDescription = copy.text(if (saved) ShellMessage.UnsaveName(title = work.title) else ShellMessage.SaveName(title = work.title))
                stateDescription = copy.text(if (pending) CopyKey.SavePending else if (saved) CopyKey.SavedTab else CopyKey.Save)
            },
        ) {
            Icon(rememberCantosIconVector(if (saved) "bookmark" else "bookmark_border"), null, Modifier.size(tokenDp("size.iconCompact")))
            Spacer(Modifier.width(tokenDp("space.sm")))
            Text(copy.text(if (saved) CopyKey.Saved else CopyKey.Save))
        }
        if (pending || failed) {
            Text(copy.text(if (failed) CopyKey.SyncFailed else CopyKey.SavePending),
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.testTag("$tag-status").semantics { liveRegion = LiveRegionMode.Polite })
            if (state.savedAccount != null && (pending || work.id in state.retryableSavedWorks)) TextButton(
                onClick = { dispatch(ShellEvent.RetrySaved(work.id, state.savedAccount, desiredSaved, pending)) },
                modifier = Modifier.heightIn(min = tokenDp("size.control")).testTag("$tag-retry").semantics {
                    contentDescription = copy.text(CopyKey.Retry) + " · " + copy.text(if (desiredSaved) ShellMessage.SaveName(title = work.title) else ShellMessage.UnsaveName(title = work.title))
                },
            ) { Text(copy.text(CopyKey.Retry)) }
        }
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
            icon = { Icon(rememberCantosIconVector("home"), null) }, label = { Text(copy.text(CopyKey.Discover)) }, modifier = Modifier.testTag("nav-discovery"))
        NavigationBarItem(selected = root == ShellRoute.Library || root == ShellRoute.History, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Library)) },
            icon = { Icon(rememberCantosIconVector("menu_book"), null) }, label = { Text(copy.text(CopyKey.Library)) }, modifier = Modifier.testTag("nav-library"))
    }
}
