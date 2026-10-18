package com.cantos.theatre.features.catalog.presentation

import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.PersonOutline
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import com.cantos.theatre.application.catalog.CatalogFailure
import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PageWalk
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.PublicCatalogState
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.application.catalog.WorkDetailState
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.unitKind
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.noto_sans_bold
import com.cantos.theatre.resources.noto_sans_regular
import com.cantos.theatre.resources.noto_serif_bold
import com.cantos.theatre.resources.noto_serif_regular
import com.cantos.ui.AccessibleName
import com.cantos.ui.BackButton
import com.cantos.ui.Badge
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.ButtonState
import com.cantos.ui.DisabledReason
import com.cantos.ui.TextField
import com.cantos.ui.CantosTheme
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.StatusTone
import com.cantos.ui.UiPreferences
import com.cantos.ui.tokenDp
import org.jetbrains.compose.resources.Font

/**
 * Production browse surface for the live public catalog: work list → work detail → units.
 * Presentation only: immutable [state] in, typed intents out. It performs no I/O, owns no
 * walk state and offers no playback: production has no media engine, so a note explains that
 * listening is not yet available and no row can be played.
 */
@Composable
fun PublicCatalogScreen(
    state: PublicCatalogState,
    onIntent: (PublicCatalogIntent) -> Unit,
    preferences: UiPreferences,
    modifier: Modifier = Modifier,
    locale: ShellLocale = ShellLocale.Vietnamese,
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    onListen: ((PublicWork, PublicUnit) -> Unit)? = null,
    player: (@Composable () -> Unit)? = null,
    libraryContent: (@Composable () -> Unit)? = null,
    accountContent: (@Composable (() -> Unit) -> Unit)? = null,
    workActions: (@Composable (PublicWork) -> Unit)? = null,
) {
    val copy = ShellCopy(locale)
    var destination by remember { mutableStateOf(CatalogDestination.Discover) }
    backHandler(state.detail != null || destination != CatalogDestination.Discover) {
        if (state.detail != null) onIntent(PublicCatalogIntent.Back) else destination = CatalogDestination.Discover
    }
    CantosTheme(preferences, uiFontFamily = catalogSans(), editorialFontFamily = catalogSerif()) {
        Surface(modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Column(Modifier.fillMaxSize().safeDrawingPadding()) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                    val detail = state.detail
                    when {
                        detail != null -> WorkDetail(detail, copy, onIntent, onListen, workActions)
                        destination == CatalogDestination.Discover -> WorkList(state, copy, onIntent) { destination = CatalogDestination.Account }
                        destination == CatalogDestination.Library && libraryContent != null -> libraryContent()
                        destination == CatalogDestination.Account && accountContent != null -> accountContent { destination = CatalogDestination.Library }
                        else -> CatalogColumn("catalog-capability", copy.text(destination.label)) {
                            item {
                                Explanation(copy.text(destination.label), copy.text(CopyKey.PersonalFeaturesUnavailable),
                                    Modifier.testTag("catalog-personal-unavailable"))
                            }
                        }
                    }
                }
                player?.invoke()
                NavigationBar(containerColor = MaterialTheme.colorScheme.surface, windowInsets = WindowInsets(0)) {
                    CatalogDestination.entries.filter { it != CatalogDestination.Account }.forEach { item ->
                        NavigationBarItem(
                            selected = destination == item,
                            onClick = { destination = item; onIntent(PublicCatalogIntent.Back) },
                            icon = { Icon(when (item) {
                                CatalogDestination.Discover -> Icons.Outlined.Home
                                CatalogDestination.Library -> Icons.AutoMirrored.Outlined.MenuBook
                                CatalogDestination.Account -> Icons.Outlined.PersonOutline
                            }, null) },
                            label = { Text(copy.text(item.label)) },
                            modifier = Modifier.testTag("catalog-nav-${item.name.lowercase()}"),
                        )
                    }
                }
            }
        }
    }
}

private enum class CatalogDestination(val label: CopyKey) {
    Discover(CopyKey.Discover), Library(CopyKey.Library), Account(CopyKey.Account),
}

@Composable
private fun catalogSans(): FontFamily = FontFamily(
    Font(Res.font.noto_sans_regular, FontWeight.Normal),
    Font(Res.font.noto_sans_bold, FontWeight.Bold),
)

@Composable
private fun catalogSerif(): FontFamily = FontFamily(
    Font(Res.font.noto_serif_regular, FontWeight.Normal),
    Font(Res.font.noto_serif_bold, FontWeight.Bold),
)

@Composable
private fun CatalogColumn(tag: String, title: String, content: LazyListScope.() -> Unit) {
    LazyColumn(
        Modifier.widthIn(max = tokenDp("breakpoint.expanded")).fillMaxSize().testTag(tag).semantics { paneTitle = title },
        contentPadding = PaddingValues(tokenDp("space.md")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        content = content,
    )
}

@Composable
private fun WorkList(
    state: PublicCatalogState,
    copy: ShellCopy,
    onIntent: (PublicCatalogIntent) -> Unit,
    onAccount: () -> Unit,
) {
    val works = state.works
    var searchExpanded by remember { mutableStateOf(false) }
    CatalogColumn("catalog-works", copy.text(CopyKey.Discover)) {
        item(key = "heading") {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        copy.text(CopyKey.DiscoverHeading),
                        Modifier.weight(1f).semantics { heading() },
                        style = MaterialTheme.typography.headlineSmall.copy(fontFamily = LocalEditorialFontFamily.current),
                    )
                    IconButton(onClick = onAccount, modifier = Modifier.size(tokenDp("size.control")).testTag("catalog-account")) {
                        Icon(Icons.Outlined.PersonOutline, contentDescription = copy.text(CopyKey.Account))
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    Button(copy.text(CopyKey.Search), { searchExpanded = !searchExpanded },
                        variant = ButtonVariant.Text, controlModifier = Modifier.testTag("catalog-search-open"))
                    if (works.received && works.phase != WalkPhase.Loading) {
                        Button(copy.text(CopyKey.Refresh), { onIntent(PublicCatalogIntent.RefreshWorks) },
                            variant = ButtonVariant.Text, controlModifier = Modifier.testTag("catalog-refresh"))
                    }
                }
            }
        }
        if (searchExpanded) item(key = "search") {
            SearchFilters(state.query, copy) { onIntent(PublicCatalogIntent.Query(it)) }
        }
        if (!state.query.isDiscovery) item(key = "query") {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(copy.text(CopyKey.CatalogQuery, "query" to state.query.text,
                    "kind" to copy.text(state.query.kind?.label() ?: CopyKey.AllKinds)),
                    Modifier.testTag("catalog-query"))
                Button(copy.text(CopyKey.SearchClear), { onIntent(PublicCatalogIntent.Query(PublicCatalogQuery.All)) },
                    variant = ButtonVariant.Text, controlModifier = Modifier.testTag("catalog-search-clear"))
            }
        }
        walk(
            works, copy,
            emptyTitle = if (state.query.isDiscovery) CopyKey.CatalogEmpty else CopyKey.Empty,
            emptyNote = if (state.query.isDiscovery) CopyKey.CatalogEmptyNote else CopyKey.EmptyNote,
            onLoadMore = { onIntent(PublicCatalogIntent.LoadMoreWorks) },
            onRetry = { onIntent(PublicCatalogIntent.RetryWorks) },
            key = { it.id.value },
        ) { work -> WorkCard(work, copy) { onIntent(PublicCatalogIntent.OpenWork(work)) } }
    }
}

@Composable
private fun SearchFilters(query: PublicCatalogQuery, copy: ShellCopy, onSearch: (PublicCatalogQuery) -> Unit) {
    var text by remember(query) { mutableStateOf(query.text) }
    var kind by remember(query) { mutableStateOf(query.kind) }
    val checked = PublicCatalogQuery.of(text, kind)
    val error = if (checked is CatalogCheck.Invalid) copy.text(CopyKey.CatalogSearchInvalid) else null
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        TextField(copy.text(CopyKey.CatalogSearchLabel), text, { text = it },
            error = error, controlModifier = Modifier.testTag("catalog-search-input"))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            (listOf<PublicWorkKind?>(null) + PublicWorkKind.entries).forEach { option ->
                Button(copy.text(option?.label() ?: CopyKey.AllKinds), { kind = option },
                    variant = if (kind == option) ButtonVariant.Tonal else ButtonVariant.Text,
                    controlModifier = Modifier.testTag("catalog-kind-${option?.name?.lowercase() ?: "all"}").semantics { selected = kind == option })
            }
        }
        Button(copy.text(CopyKey.Search), { if (checked is CatalogCheck.Valid) onSearch(checked.value) },
            state = if (error == null) ButtonState.Enabled else ButtonState.Disabled(DisabledReason.of(error)),
            controlModifier = Modifier.testTag("catalog-search-submit"))
    }
}

@Composable
private fun WorkCard(work: PublicWork, copy: ShellCopy, onOpen: () -> Unit) {
    Card(
        onClick = onOpen,
        modifier = Modifier.fillMaxWidth().testTag("catalog-work-${work.id.value}")
            .semantics { contentDescription = copy.text(CopyKey.Details, "title" to work.title) },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = MaterialTheme.shapes.large,
    ) {
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Badge(copy.text(work.kind.label()), tone = StatusTone.Info)
            Text(contentText(work.title, work.language), style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
                maxLines = 3, overflow = TextOverflow.Ellipsis)
            if (work.description.isNotBlank()) {
                Text(contentText(work.description, work.language), maxLines = 3, overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            languageLabel(work.language, copy)?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun WorkDetail(detail: WorkDetailState, copy: ShellCopy, onIntent: (PublicCatalogIntent) -> Unit,
                       onListen: ((PublicWork, PublicUnit) -> Unit)?, workActions: (@Composable (PublicWork) -> Unit)?) {
    val ready = (detail.phase as? DetailPhase.Ready)?.work
    CatalogColumn("catalog-detail", (ready ?: detail.summary)?.title ?: copy.text(CopyKey.Work)) {
        item(key = "back") {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                BackButton(AccessibleName.of(copy.text(CopyKey.Back)), { onIntent(PublicCatalogIntent.Back) },
                    controlModifier = Modifier.testTag("catalog-back"))
                if (detail.phase != DetailPhase.Loading) {
                    Button(copy.text(CopyKey.Refresh), { onIntent(PublicCatalogIntent.RefreshWork) },
                        variant = ButtonVariant.Text, controlModifier = Modifier.testTag("catalog-detail-refresh"))
                }
            }
        }
        when (val phase = detail.phase) {
            DetailPhase.Loading -> {
                detail.summary?.let { summary -> item(key = "header") { WorkHeader(summary, copy) } }
                item(key = "loading") {
                    StatusNote(copy.text(CopyKey.Loading), Modifier.testTag("catalog-loading"))
                }
            }
            is DetailPhase.Failed -> item(key = "failure") {
                if (phase.failure == CatalogFailure.NotFound) {
                    Explanation(copy.text(CopyKey.WorkNotFound), copy.text(CopyKey.WorkNotFoundNote), Modifier.testTag("catalog-failure"))
                } else {
                    FailureState(phase.failure, copy, first = true) { onIntent(PublicCatalogIntent.RetryWork) }
                }
            }
            is DetailPhase.Ready -> {
                item(key = "header") { WorkHeader(phase.work, copy); workActions?.invoke(phase.work) }
                if (onListen == null) item(key = "listening-note") {
                    Surface(
                        Modifier.fillMaxWidth().testTag("catalog-listening-note"),
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                        shape = MaterialTheme.shapes.medium,
                    ) {
                        Text(copy.text(CopyKey.ListeningNotYetAvailable), Modifier.padding(tokenDp("space.md")))
                    }
                }
                item(key = "units-heading") {
                    Text(
                        copy.text(if (phase.work.kind.unitKind == PublicUnitKind.Episode) CopyKey.Episodes else CopyKey.Chapters),
                        Modifier.semantics { heading() },
                        style = MaterialTheme.typography.titleLarge,
                    )
                }
                detail.units?.let { units ->
                    walk(
                        units, copy,
                        emptyTitle = CopyKey.UnitsEmpty, emptyNote = null,
                        onLoadMore = { onIntent(PublicCatalogIntent.LoadMoreUnits) },
                        onRetry = { onIntent(PublicCatalogIntent.RetryUnits) },
                        key = { it.id.value },
                        notFound = CopyKey.WorkNotFound to CopyKey.WorkNotFoundNote,
                    ) { unit ->
                        UnitRow(unit, phase.work.language, copy)
                        if (onListen != null) Button(copy.text(CopyKey.PlayEpisode, "title" to unit.title),
                            { onListen(phase.work, unit) }, controlModifier = Modifier.testTag("catalog-play-${unit.id.value}"))
                    }
                }
            }
        }
    }
}

@Composable
private fun WorkHeader(work: PublicWork, copy: ShellCopy) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Badge(copy.text(work.kind.label()), tone = StatusTone.Info)
        Text(contentText(work.title, work.language), Modifier.semantics { heading() }.testTag("catalog-detail-title"),
            style = MaterialTheme.typography.headlineMedium.copy(fontFamily = LocalEditorialFontFamily.current))
        languageLabel(work.language, copy)?.let {
            Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (work.description.isNotBlank()) Text(contentText(work.description, work.language), Modifier.testTag("catalog-detail-description"))
    }
}

/** Not interactive: there is no production player, so a row never offers Play. */
@Composable
private fun UnitRow(unit: PublicUnit, contentLanguage: String, copy: ShellCopy) {
    Surface(
        Modifier.fillMaxWidth().testTag("catalog-unit-${unit.id.value}"),
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.medium,
    ) {
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            Text(contentText(unit.title, contentLanguage), style = MaterialTheme.typography.titleMedium)
            if (unit.description.isNotBlank()) {
                Text(contentText(unit.description, contentLanguage), maxLines = 2, overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(durationLabel(unit.durationMs, copy), style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * Rows in server order (position ascending, then id): `position` orders, it is never shown as
 * a number. First-load states replace the list; a later failure keeps the rows above it.
 */
private fun <T> LazyListScope.walk(
    walk: PageWalk<T>,
    copy: ShellCopy,
    emptyTitle: CopyKey,
    emptyNote: CopyKey?,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    key: (T) -> String,
    notFound: Pair<CopyKey, CopyKey>? = null,
    row: @Composable (T) -> Unit,
) {
    val phase = walk.phase
    if (walk.items.isEmpty()) {
        item(key = "walk-state") {
            when {
                phase == WalkPhase.NotStarted || phase == WalkPhase.Loading ->
                    StatusNote(copy.text(CopyKey.Loading), Modifier.testTag("catalog-loading"))
                phase is WalkPhase.Failed && phase.failure == CatalogFailure.NotFound && notFound != null ->
                    Explanation(copy.text(notFound.first), copy.text(notFound.second), Modifier.testTag("catalog-failure"))
                phase is WalkPhase.Failed -> FailureState(phase.failure, copy, first = true, onRetry)
                phase == WalkPhase.More -> LoadMore(copy, onLoadMore)
                phase == WalkPhase.Complete ->
                    Explanation(copy.text(emptyTitle), emptyNote?.let(copy::text), Modifier.testTag("catalog-empty"))
            }
        }
        return
    }
    items(walk.items, key = key) { row(it) }
    item(key = "walk-footer") {
        when (phase) {
            WalkPhase.Loading -> StatusNote(copy.text(CopyKey.LoadingMore), Modifier.testTag("catalog-loading-more"))
            WalkPhase.More -> LoadMore(copy, onLoadMore)
            is WalkPhase.Failed -> FailureState(phase.failure, copy, first = false, onRetry)
            WalkPhase.Complete, WalkPhase.NotStarted -> Unit
        }
    }
}

@Composable
private fun LoadMore(copy: ShellCopy, onLoadMore: () -> Unit) {
    Button(copy.text(CopyKey.LoadMore), onLoadMore, variant = ButtonVariant.Tonal,
        controlModifier = Modifier.testTag("catalog-load-more"))
}

@Composable
private fun FailureState(failure: CatalogFailure, copy: ShellCopy, first: Boolean, onRetry: () -> Unit) {
    val (title, note) = when (failure) {
        CatalogFailure.Transport, is CatalogFailure.Unavailable ->
            if (failure is CatalogFailure.Unavailable && !failure.retryable) CopyKey.CatalogFault to CopyKey.CatalogFaultNote
            else CopyKey.CatalogUnavailable to CopyKey.CatalogUnavailableNote
        CatalogFailure.Protocol, CatalogFailure.NotFound -> CopyKey.CatalogFault to CopyKey.CatalogFaultNote
        CatalogFailure.CursorRejected -> CopyKey.CatalogChanged to CopyKey.CatalogChangedNote
    }
    // A later page keeps its rows; say so instead of replacing the list with an error screen.
    val message = if (!first && failure != CatalogFailure.CursorRejected) CopyKey.NextPageFailed else note
    Explanation(copy.text(title), copy.text(message), Modifier.testTag("catalog-failure"), error = true) {
        Button(copy.text(CopyKey.Retry), onRetry, variant = ButtonVariant.Secondary,
            controlModifier = Modifier.testTag("catalog-retry"))
    }
}

/** Announced progress line (Material text over the shared theme; no unqualified shared state component). */
@Composable
private fun StatusNote(text: String, modifier: Modifier) {
    Text(text, modifier.semantics { liveRegion = LiveRegionMode.Polite }, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** Title, optional note and optional recovery, announced politely when it appears. */
@Composable
private fun Explanation(
    title: String,
    note: String?,
    modifier: Modifier,
    error: Boolean = false,
    action: (@Composable () -> Unit)? = null,
) {
    Column(modifier.semantics { liveRegion = LiveRegionMode.Polite }, verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(title, style = MaterialTheme.typography.titleMedium,
            color = if (error) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
        note?.let { Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        action?.invoke()
    }
}

private fun PublicWorkKind.label(): CopyKey = when (this) {
    PublicWorkKind.Novel -> CopyKey.KindNovel
    PublicWorkKind.Story -> CopyKey.KindStory
    PublicWorkKind.Podcast -> CopyKey.KindPodcast
}

/** The released audio language. Known tags use the existing endonyms; others stay literal. */
internal fun languageLabel(tag: String, copy: ShellCopy): String? {
    val primary = tag.trim().substringBefore('-').lowercase()
    val name = when {
        primary.isEmpty() -> return null
        primary == "vi" -> copy.text(CopyKey.Vietnamese)
        primary == "en" -> copy.text(CopyKey.English)
        else -> tag.trim()
    }
    return copy.text(CopyKey.AudioLanguage, "language" to name)
}

/** Verified rendition duration (always positive). Under a minute shows seconds, else rounded minutes. */
internal fun durationLabel(durationMs: Long, copy: ShellCopy): String {
    if (durationMs < 60_000) {
        val seconds = ((durationMs + 999) / 1000).coerceAtLeast(1)
        return copy.text(if (seconds == 1L) CopyKey.SecondOne else CopyKey.SecondsOther, "count" to seconds.toString())
    }
    val minutes = (durationMs + 30_000) / 60_000
    return copy.text(CopyKey.Duration, "minutes" to minutes.toString())
}
